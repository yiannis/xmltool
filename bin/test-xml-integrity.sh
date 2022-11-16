
XML_IN='data/four-books-simple-utf8.xml'
XML_OUT='/tmp/feed.xml'

REF_DIR='data/four-books-simple-utf8.split'

MD5_REF_0=`md5 -q $REF_DIR/0.xml`
MD5_REF_1=`md5 -q $REF_DIR/1.xml`

cargo run -- -c 2 -n /books/book -i 1 $XML_IN

MD5_OUT_0=`md5 -q $XML_OUT.0`
MD5_OUT_1=`md5 -q $XML_OUT.1`

if [[ $MD5_REF_0 == $MD5_OUT_0 ]]; then
  echo OK 0;
else
  echo FAIL 0;
fi

if [[ $MD5_REF_1 == $MD5_OUT_1 ]]; then
  echo OK 1;
else
  echo FAIL 1;
fi
