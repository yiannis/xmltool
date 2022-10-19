use std::str;
use std::io::BufRead;
use std::io::Seek;
use std::io::Read;
use std::io::Write;
use std::io::BufReader;
use std::io::SeekFrom;
use std::fs::File;
use std::process;

use clap::Parser;

use quick_xml::events::{BytesStart, BytesEnd, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

#[derive(Parser)]
struct Opt {
    xml_path: std::path::PathBuf,

    #[arg(short, long)]
    nesting: String,

    #[arg(short, long, default_value_t = 1)]
    implementation: u8,

    #[arg(short, long, default_value_t = 100)]
    count: u32,
}

fn root_from_nesting(nesting: &String) -> String {
    String::from(nesting.split('/').nth(1).unwrap())
}

fn parent_from_nesting(nesting: &String) -> String {
    String::from(nesting.rsplit('/').nth(0).unwrap())
}

struct Counters {
    items_per_chunk: u32,
    item_chunk_id: u32,
    chunk_id: u32,
    item_id: u32,
}

impl Counters {
    fn new(items_per_chunk: u32) -> Self {
        Self {
            items_per_chunk,
            item_chunk_id: 0,
            chunk_id: 0,
            item_id: 0,
        }
    }

    fn update(&mut self) {
        if self.item_chunk_id + 1 == self.items_per_chunk {
            self.chunk_id += 1;
            self.item_chunk_id = 0;
        }
        else {
            self.item_chunk_id += 1;
        }
        self.item_id += 1;
    }
}

struct XMLSource<R> {
    reader: Reader<BufReader<R>>,
    file:   File,

    last_event_pos: usize,
    last_item_pos:  usize,
    current_pos:    usize,

    parent_name: String,

    _buf:    Vec<u8>,
}

impl XMLSource<File> {
    fn new(file_path: &std::path::PathBuf, nesting: &String) -> Self {
        let xml = File::open(&file_path).unwrap();
        let xml = BufReader::new(xml);

        Self {
            reader: Reader::from_reader(xml),
            file:   File::open(&file_path).unwrap(),

            last_event_pos: 0,
            last_item_pos:  0,
            current_pos:    0,

            parent_name: parent_from_nesting(&nesting),

            _buf: Vec::new(),
        }
    }

    fn next(&mut self) -> Event {
        self.last_event_pos = self.reader.buffer_position();
        self._buf.clear();

        let event = self.reader.read_event_into(&mut self._buf);

        self.current_pos = self.reader.buffer_position();

        match event {
            Err(e) => panic!("Error at position {}: {:?}", self.current_pos, e),
            Ok(e) => e,
        }
    }

    fn consume_item(&mut self) {
        let start = BytesStart::new(&self.parent_name);
        let end   = start.to_end().into_owned();

        self.reader.read_to_end_into(end.name(), &mut self._buf).unwrap();

        self.current_pos = self.reader.buffer_position();
    }

    fn header(&mut self) -> Vec<u8> {
        let mut header = vec![0u8; self.last_event_pos];
        self.file.seek(SeekFrom::Start(0u64)).unwrap();
        self.file.read_exact(&mut header[..]).unwrap();

        return header;
    }

    fn footer(&mut self) -> Vec<u8> {
        let bytes_total    = self.file.metadata().unwrap().len();
        let last_item_byte = self.last_item_pos as u64;
        let size           = (bytes_total-last_item_byte) as usize;

        let mut footer = vec![0u8; size];
        self.file.seek(SeekFrom::Start(last_item_byte)).unwrap();
        self.file.read_exact(&mut footer[..]).unwrap();

        return footer;
    }

    fn item(&mut self) -> Vec<u8> {
        let start_pos = self.last_event_pos;
        let end_pos   = self.current_pos;

        let mut item = vec![0u8; end_pos-start_pos];

        self.last_item_pos = end_pos;

        self.file.seek(SeekFrom::Start(start_pos as u64)).unwrap();
        self.file.read_exact(&mut item[..]).unwrap();

        return item;
    }
}

struct Chunk {
    file: File,
    path: std::path::PathBuf,
}

impl Chunk {
    fn new(id: u32) -> Self {
        let path = std::path::PathBuf::from(format!("/tmp/feed.xml.{}", id));
        let file = File::create(&path).unwrap();

        Self { file, path }
    }

    fn append(&mut self, content: &Vec<u8>) {
        self.file.write_all(&content[..]).unwrap();
    }
}

type Chunks = Vec<Chunk>;
// See: http://xion.io/post/code/rust-extension-traits.html
pub trait ChunkExtention {
    fn append_xml(self: &mut Self, content: &Vec<u8>);
    fn append_xml_at(&mut self, chunk_id: u32, content: &Vec<u8>);
    fn init_at(&mut self, chunk_id: u32);
}

impl ChunkExtention for Chunks {
    fn append_xml(&mut self, content: &Vec<u8>) {
        for f in self.iter_mut() {
            f.append(content);
        }
    }

    fn append_xml_at(&mut self, chunk_id: u32, content: &Vec<u8>) {
        self[chunk_id as usize].append(content);
    }

    fn init_at(&mut self, chunk_id: u32) {
        self.push(Chunk::new(chunk_id));
    }
}

fn item_starts(s: BytesStart, nesting: &String) -> bool {
    let parent = parent_from_nesting(&nesting);

    str::from_utf8(s.name().as_ref()).unwrap().eq(&parent)
}

fn main() {
    let args = Opt::parse();

    match args.implementation {
        1 => {
            println!("Running plain text copy version");
            copy_plain_xml_text_for_each_item( &args.xml_path, args.count, &args.nesting );
        }

        2 => {
            println!("Running node level read/write version");
            let xml = File::open(&args.xml_path).unwrap();
            let xml = BufReader::new(xml);
            emit_write_event_for_each_read_event( xml, args.count, &args.nesting );
        }

        _ => println!("ERROR: Not implemented!"),
    }

    process::exit(0);
}

fn emit_write_event_for_each_read_event(xml: impl BufRead, max_items: u32, nesting: &String) {
    let root   = root_from_nesting(&nesting);
    let parent = parent_from_nesting(&nesting);

    let mut reader = Reader::from_reader(xml);

    reader.trim_text(true);

    let mut chunk_file = String::from("/dev/null");
    let mut writer = Writer::new(File::open(chunk_file).unwrap());

    let items_per_chunk = max_items;
    let mut chunk_id = 0;
    let mut item_id = 0;
    let mut item_chunk_id = 0;
    let mut inside_item = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),

            Ok(Event::Eof) => break,

            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    p if str::from_utf8(p).unwrap().eq(&parent) => {
                        if item_chunk_id == 0 {
                            chunk_file = format!("/tmp/feed.xml.{}", chunk_id);
                            println!("Chunk: {}|{} - {}", chunk_id, item_id, chunk_file);
                            writer = Writer::new(File::create(chunk_file).unwrap());
                            writer.write_event(Event::Start(BytesStart::new(&root))).unwrap();
                        }

                        if item_chunk_id + 1 == items_per_chunk {
                            chunk_id += 1;
                            item_chunk_id = 0;
                        }
                        else {
                            item_chunk_id += 1;
                        }
                        item_id += 1;
                        inside_item = true;

                        writer.write_event(Event::Start(e)).unwrap();
                    }

                    _ => {
                        if inside_item {
                            writer.write_event(Event::Start(e)).unwrap();
                        }
                    }
                }
            }

            Ok(Event::Text(e)) => {
                if inside_item {
                    writer.write_event(Event::Text(e)).unwrap();
                }
            }

            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    p if str::from_utf8(p).unwrap().eq(&parent) => {
                        writer.write_event(Event::End(e)).unwrap();
                        if item_chunk_id%items_per_chunk == 0 {
                            writer.write_event(Event::End(BytesEnd::new(&root))).unwrap();
                        }
                        inside_item = false;
                    }

                    _ => {
                        if inside_item {
                            writer.write_event(Event::End(e)).unwrap();
                        }
                    }
                }
            }

            _ => (),
        }
        buf.clear();
    }
}

// TODO
// * Expose start/end byte of tag on sax API
// * reader.buffer_position() returns usize.
//   Shouldn't it return u64 to support big files?
// * Use mmap:
// https://rust-lang-nursery.github.io/rust-cookbook/file/read-write.html#access-a-file-randomly-using-a-memory-map
// * Potential improvement:
// const BUF_SIZE: usize = 4096; // 4kb at once
// let mut buf = Vec::with_capacity(BUF_SIZE);
// match xmlfile.read_event(&mut buf)? {
//   See: https://usethe.computer/posts/14-xmhell.html
// * XPath support:
// - https://github.com/shepmaster/sxd-xpath
// - https://github.com/ballsteve/xrust
// * Read from .gz file
fn copy_plain_xml_text_for_each_item(xml_path: &std::path::PathBuf, max_items: u32, nesting: &String) {
    let mut chunks   = Chunks::new();
    let mut xml      = XMLSource::new(xml_path, nesting);
    let mut counters = Counters::new(max_items);
    let mut header   = vec![0u8; 1];

    loop {
        match xml.next() {
            Event::Eof => {
                let mut footer = xml.footer();
                chunks.append_xml(&mut footer);
                break;
            }

            Event::Start(e) => {
                if item_starts(e, nesting) {
                    if counters.item_id == 0 {
                        header = xml.header();
                    }

                    if counters.item_chunk_id == 0 {
                        chunks.init_at(counters.chunk_id);
                        chunks.append_xml_at(counters.chunk_id, &header);
                        println!("Chunk {} created", chunks[counters.chunk_id as usize].path.display());
                    }

                    xml.consume_item();

                    let item = xml.item();
                    chunks.append_xml_at(counters.chunk_id, &item);

                    counters.update();
                }
            }

            _ => (),
        }
    }


}
