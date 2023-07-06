use warnings;
use strict;

use XML::LibXML::Reader;

my %type_name = (
    XML_READER_TYPE_ELEMENT()                => 'ELEMENT',
    XML_READER_TYPE_ATTRIBUTE()              => 'ATTRIBUTE',
    XML_READER_TYPE_TEXT()                   => 'TEXT',
    XML_READER_TYPE_CDATA()                  => 'CDATA',
    XML_READER_TYPE_ENTITY_REFERENCE()       => 'ENTITY_REFERENCE',
    XML_READER_TYPE_ENTITY()                 => 'ENTITY',
    XML_READER_TYPE_PROCESSING_INSTRUCTION() => 'PROCESSING_INSTRUCTION',
    XML_READER_TYPE_COMMENT()                => 'COMMENT',
    XML_READER_TYPE_DOCUMENT()               => 'DOCUMENT',
    XML_READER_TYPE_DOCUMENT_TYPE()          => 'DOCUMENT_TYPE',
    XML_READER_TYPE_DOCUMENT_FRAGMENT()      => 'DOCUMENT_FRAGMENT',
    XML_READER_TYPE_NOTATION()               => 'NOTATION',
    XML_READER_TYPE_WHITESPACE()             => 'WHITESPACE',
    XML_READER_TYPE_SIGNIFICANT_WHITESPACE() => 'SIGNIFICANT_WHITESPACE',
    XML_READER_TYPE_END_ELEMENT()            => 'END_ELEMENT',
);

my $filepath = shift or die;
my $nesting  = shift or die; 

my $OUT_ROOT = '/tmp/analyser-split.';
my $fd_out;
my $chunk_id = 0;
my $item_chunk_id = 0;
my $items_per_chunk = 1000;

main();

##########################
sub build_pattern {
    return XML::LibXML::Pattern->new( $nesting ); # XPath
}

sub build_reader {
    my $reader = XML::LibXML::Reader->new( location => $filepath )
        or die "cannot read file '$filepath': $!\n";

    return $reader;
}

sub main {
    my $reader  = build_reader();
    my $pattern = build_pattern();

    while ( my $ret = $reader->nextPatternMatch( $pattern ) ) {
        die 'Error parsing feed' if $ret == -1;

        next if $type_name{ $reader->nodeType } eq 'END_ELEMENT';

        my $xml = $reader->readOuterXml();
        dump_split( $xml );
    }
}

sub dump_split {
    my $xml = shift;

    if ($item_chunk_id == 0) {
        if ($fd_out) {
            my $footer = footer_from_nesting();
            print $fd_out $footer;

            close $fd_out;
        }

        my $out_path = sprintf("%s%03d", $OUT_ROOT, $chunk_id);
        open( my $fd, '>', $out_path ) or die "$!";
        binmode $fd, ':encoding(UTF-8)';
        $fd_out = $fd;

        my $header = header_from_nesting();
        print $fd_out $header;
    }

    if (($item_chunk_id + 1) == $items_per_chunk) {
        $chunk_id++;
        $item_chunk_id = 0;
    }
    else {
        $item_chunk_id++;
    }

    print $fd_out $xml;
}

sub header_from_nesting {
    my @els = grep {$_} split(m{/}, $nesting);
    pop @els;

    join("\n", map {"<$_>"} @els);
}

sub footer_from_nesting {
    my @els = reverse grep {$_} split(m{/}, $nesting);
    shift @els;

    join("\n", map {"</$_>"} @els);
}
