use std::str;
use std::io::BufRead;
use std::io::BufReader;
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

    emit_write_event_for_each_read_event( xml, args.count, &args.nesting );

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

//                        println!("{}", reader.read_text(end.name()).unwrap());

//                    b"item" => println!("attributes values: {:#?}",
//                                        e.attributes().map(|a| str::from_utf8(a.unwrap().key.into_inner()).unwrap())
//                                        .collect::<Vec<_>>()),

