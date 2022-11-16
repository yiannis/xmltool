# TODO

## XML Tools

### Code
 - [ ] Integration tests with several types of XML files
 - [ ] Move structs, etc to external .rs file
 - [ ] Update all code after linter
 - [ ] Add unit tests
 - [ ] Benchmarks using comtrya

### Features

#### XML Split command
- [ ] Add output path option
- [X] Preserve whitespace formatting (almost?)
- [ ] Read from .gz file
- [ ] Convert to utf-8

#### CSV to XML command
- [ ] Add this new command!

#### Display unique values command

#### XPath support
* Modules:
 - https://github.com/shepmaster/sxd-xpath
 - https://github.com/ballsteve/xrust
* TODO:
 - Match parent element with full nesting string

### BUGS

#### Too many open files
```
XMLChunk /tmp/feed.xml.250 created
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Os { code: 24, kind: Uncategorized, message: "Too many open files" }', src/main.rs:176:40
```
### Performance

#### Small improvements

* XMLSource::extract() should reuse the same buffer

#### Use mmap
See:
https://rust-lang-nursery.github.io/rust-cookbook/file/read-write.html#access-a-file-randomly-using-a-memory-map
But not sure if that helps at all with zipped files...

This could have an extra improvement:
Do not copy the items one by one,
but only the XML chunks.

#### Align I/O buffer size
Something like:
```
const BUF_SIZE: usize = 4096; // 4kb at once
let mut buf = Vec::with_capacity(BUF_SIZE);
match xmlfile.read_event(&mut buf)? {
```
See: https://usethe.computer/posts/14-xmhell.html

#### Double pass
* On the first pass, parse the XML and create a map of the file positions we want to split.
* On the second pass, open the file and copy the contents.

## External

### Feature requests for Quick XML
* Expose start/end byte of XML tag on sax API
* An extra WhiteSpace `pub enum Event<'a> {` member. (Need it to keep the formatting)


