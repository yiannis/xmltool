## Usage

```
xmltool --nesting /books/book --count 5000 library.xml
```

### With the provided WayUP feed

```
cargo build --release
time target/release/xmltool -c 1000 -n /source/jobs/job -i 2 wayup.xml
```
### Try the Perl implementation
```
time perl split.pl wayup.xml /source/jobs/job
```

## Notes

### Quick XML
* After each `reader.read_event_*()`
  * the `_buf` contains the content of the currently parsed element, eg:
    * book isbn="123"
    * /book
  * the file buffer position is at the first char of the next node, eg:
    * `<` for an element
