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
struct Options {
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

enum ParserState {
    Node,
    Item,
    Header,
    Footer,
}

struct XMLSource<R> {
    reader: Reader<BufReader<R>>,
    file:   File,

    current_pos: u64,
    end_pos:     u64,

    is_first_item: bool,

    parent_name: String,

    _buf: Vec<u8>,
}

impl XMLSource<File> {
    fn new(file_path: &std::path::PathBuf, nesting: &String) -> Self {
        let xml = File::open(&file_path).unwrap();
        let xml = BufReader::new(xml);

        let f = File::open(&file_path).unwrap();
        let bytes_total = f.metadata().unwrap().len();

        Self {
            reader: Reader::from_reader(xml),
            file:   f,

            current_pos: 0,
            end_pos:     bytes_total-1,

            is_first_item: true,

            parent_name: parent_from_nesting(&nesting),

            _buf: Vec::new(),
        }
    }

    fn next(&mut self) -> ParserState {
        self._buf.clear();

        let event = self.reader.read_event_into(&mut self._buf).unwrap().into_owned();

        self.current_pos = self.reader.buffer_position() as u64;

        self.event_to_item_state(event)
    }

    fn is_item_start(&self, s: BytesStart) -> bool {
        str::from_utf8(s.name().as_ref()).unwrap().eq(&self.parent_name)
    }

    fn event_to_item_state(&mut self, event: Event ) -> ParserState {
        match event {
            Event::Eof => ParserState::Footer,

            Event::Start(e) => {
                if self.is_item_start(e) {
                    if self.is_first_item {
                        self.is_first_item = false;
                        ParserState::Header
                    }
                    else {
                        ParserState::Item
                    }
                }
                else {
                    ParserState::Node
                }
            }

            _ => ParserState::Node,
        }
    }

    fn consume_item(&mut self) {
        let start = BytesStart::new(&self.parent_name);
        let end   = start.to_end().into_owned();

        self.reader.read_to_end_into(end.name(), &mut self._buf).unwrap();
        self.current_pos = self.reader.buffer_position() as u64;
    }

    fn extract(&mut self, start: u64, end: u64) -> Vec<u8> {
        let size = end - start;
        let mut buffer = vec![0u8; size as usize];

        self.file.seek(SeekFrom::Start(start)).unwrap();
        self.file.read_exact(&mut buffer[..]).unwrap();

        return buffer;
    }
}

struct XMLChunk {
    file: File,
    path: std::path::PathBuf,
}

impl XMLChunk {
    fn new(id: u32) -> Self {
        let path = std::path::PathBuf::from(format!("/tmp/feed.xml.{}", id));
        let file = File::create(&path).unwrap();

        Self { file, path }
    }

    fn append(&mut self, content: &Vec<u8>) {
        self.file.write_all(&content[..]).unwrap();
    }
}

impl Drop for XMLChunk {
    fn drop(&mut self) {
        println!("Dropping file");//self.path.into_os_string());
    }
}

struct XMLChunks {
    list:     Vec<XMLChunk>,
    counters: Counters,
}

impl XMLChunks {
    fn new(items_per_chunk: u32) -> Self {
        let list     = Vec::<XMLChunk>::new();
        let counters = Counters::new(items_per_chunk);

        Self { list, counters }
    }

    fn append_bytes_to_all(&mut self, bytes: &Vec<u8>) {
        for f in self.list.iter_mut() {
            f.append(&bytes);
        }
    }

    fn append_bytes_to_current(&mut self, bytes: &Vec<u8>) {
        let id = self.counters.chunk_id as usize;

        self.list[id].append(&bytes);
    }

    fn new_chunk(&mut self, header: &Vec<u8>) {
        self.list.push(XMLChunk::new(self.counters.chunk_id));
        self.append_bytes_to_current(header);

        let id = self.counters.chunk_id as usize;
        println!(
            "XMLChunk {} created",
            self.list[id].path.display()
        );
    }
}

struct XMLCopySplitter<R> {
    xml_source: XMLSource<R>,
    xml_chunks: XMLChunks,

    xml_header: Vec<u8>,

    start_node_source_pos: u64,
    last_node_source_pos:  u64,
}

impl XMLCopySplitter<File> {
    fn new(
        xml_path: &std::path::PathBuf,
        max_items: u32,
        nesting: &String
    ) -> Self {
        Self {
            xml_source: XMLSource::new(xml_path, nesting),
            xml_chunks: XMLChunks::new(max_items),

            xml_header: Vec::new(),

            start_node_source_pos: 0,
            last_node_source_pos:  0,
        }
    }

    fn run(&mut self) {
        loop {
            match self.xml_source.next() {
                ParserState::Node   => self.handle_node(),
                ParserState::Item   => self.handle_item(),
                ParserState::Header => {
                    self.handle_header();
                    self.handle_item();
                }
                ParserState::Footer => {
                    self.handle_footer();
                    break;
                }
            }
        }
    }

    fn handle_item(&mut self) {
        self.xml_source.consume_item();

        if self.xml_chunks.counters.item_chunk_id == 0 {
            self.xml_chunks.new_chunk(&self.xml_header);
        }

        let next_node_source_pos: u64 = self.xml_source.current_pos;
        let item = self.xml_source.extract(
            self.start_node_source_pos,
            next_node_source_pos,
        );
        self.xml_chunks.append_bytes_to_current(&item);

        self.start_node_source_pos = next_node_source_pos;

        self.xml_chunks.counters.update();
    }

    fn handle_header(&mut self) {
        self.xml_header = self.xml_source.extract(
            0,
            self.last_node_source_pos
        );
        self.start_node_source_pos = self.last_node_source_pos;
    }

    fn handle_footer(&mut self) {
        let footer = self.xml_source.extract(
            self.start_node_source_pos,
            self.xml_source.end_pos + 1,
        );
        self.xml_chunks.append_bytes_to_all(&footer);
    }

    fn handle_node(&mut self) {
        self.last_node_source_pos = self.xml_source.current_pos;
    }
}

fn main() {
    let args = Options::parse();

    match args.implementation {
        1 => {
            println!("Running byte copy version");

            XMLCopySplitter::new(
                &args.xml_path,
                args.count,
                &args.nesting
            ).run();
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
