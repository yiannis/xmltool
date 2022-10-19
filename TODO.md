# TODO

## XML Tools

### Performance

#### Use mmap
See:
https://rust-lang-nursery.github.io/rust-cookbook/file/read-write.html#access-a-file-randomly-using-a-memory-map
But not sure if that helps at all with zipped files...

#### Align I/O buffer size
Something like:
```
const BUF_SIZE: usize = 4096; // 4kb at once
let mut buf = Vec::with_capacity(BUF_SIZE);
match xmlfile.read_event(&mut buf)? {
```
See: https://usethe.computer/posts/14-xmhell.html

### Features

#### XPath support
See:
- https://github.com/shepmaster/sxd-xpath
- https://github.com/ballsteve/xrust

#### Read from .gz file

## External

### Feature requests for Quick XML
* Expose start/end byte of tag on sax API
* reader.buffer_position() returns usize. Shouldn't it return u64 to support big files?


