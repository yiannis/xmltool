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

    #[arg(short, long, default_value_t = 100)]
    count: i32,
}

fn main() {
    let args = Opt::parse();

    let xml = File::open(&args.xml_path).unwrap();
    let xml = BufReader::new(xml);

    copy_plain_xml_text_for_each_item( &args.xml_path, &args.count, &args.nesting );
    //emit_write_event_for_each_read_event( xml, args.count, &args.nesting );

    process::exit(0);
}

fn root_from_nesting(nesting: &String) -> String {
    String::from(nesting.split('/').nth(1).unwrap())
}

fn parent_from_nesting(nesting: &String) -> String {
    String::from(nesting.rsplit('/').nth(0).unwrap())
}

fn emit_write_event_for_each_read_event(xml: impl BufRead, max_items: i32, nesting: &String) {
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
// * Copy XML header as is
// * Move variables into struct
// * Move functionality into struct impl
// * Expose start/end byte of tag on sax API
fn copy_plain_xml_text_for_each_item(xml_path: &std::path::PathBuf, max_items: &i32, nesting: &String) {
    let mut xml_copy = File::open(&xml_path).unwrap();
    // Also see:
    // https://rust-lang-nursery.github.io/rust-cookbook/file/read-write.html#access-a-file-randomly-using-a-memory-map

    let xml = File::open(&xml_path).unwrap();
    let xml = BufReader::new(xml);

    let root   = root_from_nesting(&nesting);
    let parent = parent_from_nesting(&nesting);

    let start = BytesStart::new(&parent);
    let end   = start.to_end().into_owned();

    let mut header = vec![0u8; 1]; // need to initialise, else compiler complains...

    let mut chunk_path = String::from("/dev/null");
    let mut chunk_file = File::open(chunk_path).unwrap();
    let mut reader = Reader::from_reader(xml);

    let items_per_chunk = max_items;
    let mut last_event_pos = 0;
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
                if str::from_utf8(e.name().as_ref()).unwrap().eq(&parent) {
                    if item_id == 0 {
                        header = vec![0u8; last_event_pos];
                        xml_copy.seek(SeekFrom::Start(0u64)).unwrap();
                        xml_copy.read_exact(&mut header[..]).unwrap();
                    }

                    if item_chunk_id == 0 {
                        chunk_path = format!("/tmp/feed.xml.{}", chunk_id);
                        println!("Chunk: {}|{} - {}", chunk_id, item_id, chunk_path);
                        chunk_file = File::create(chunk_path).unwrap();
                        chunk_file.write_all(&header[..]).unwrap();
                    }

                    let current_event_pos = reader.buffer_position();
                    //println!("<{}> at: {}-{}", parent, last_event_pos, current_event_pos);

                    reader.read_to_end_into(end.name(), &mut buf).unwrap();

                    let start_pos = last_event_pos;
                    let end_pos   = reader.buffer_position();
                    let mut buffer = vec![0u8; end_pos-start_pos];

                    xml_copy.seek(SeekFrom::Start(start_pos as u64)).unwrap();
                    xml_copy.read_exact(&mut buffer[..]).unwrap();
                    //println!("{}", str::from_utf8(&buffer[..]).unwrap());
                    chunk_file.write_all(&buffer[..]).unwrap();

                    if item_chunk_id + 1 == *items_per_chunk {
                        chunk_id += 1;
                        item_chunk_id = 0;
                        chunk_file.write_all(b"</jobs>").unwrap();
                    }
                    else {
                        item_chunk_id += 1;
                    }
                    item_id += 1;
                } else if str::from_utf8(e.name().as_ref()).unwrap().eq(&root) {
                }

                last_event_pos = reader.buffer_position();
            }

            Ok(Event::End(e)) => last_event_pos = reader.buffer_position(),

            _ => {
                last_event_pos = reader.buffer_position();
                //println!("Event at: {}", last_event_pos);
            }
        }
        buf.clear();
    }
}

//                        println!("{}", reader.read_text(end.name()).unwrap());

//                    b"item" => println!("attributes values: {:#?}",
//                                        e.attributes().map(|a| str::from_utf8(a.unwrap().key.into_inner()).unwrap())
//                                        .collect::<Vec<_>>()),

