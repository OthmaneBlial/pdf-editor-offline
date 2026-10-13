#![allow(clippy::unwrap_used)]
//! Public synthetic two-page fixture. Both pages deliberately share one Courier content stream.
use lopdf::{Document, Stream, dictionary};
pub const CONTENT:&[u8]=b"0.2 0.4 0.6 rg 10 10 20 20 re f\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET\nBT /F 12 Tf 1 0 0 1 40 110 Tm 0 Tr (A B) Tj ET\n";
pub fn source(content: &[u8]) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let root = document.new_object_id();
    let pages = document.new_object_id();
    let first = document.new_object_id();
    let second = document.new_object_id();
    let font=document.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let mut stream = Stream::new(dictionary! {}, content.to_vec());
    stream.compress().unwrap();
    let stream = document.add_object(stream);
    for id in [first, second] {
        document.objects.insert(
            id,
            dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream}.into(),
        );
    }
    document.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![first.into(),second.into()],"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}}.into());
    document
        .objects
        .insert(root, dictionary! {"Type"=>"Catalog","Pages"=>pages}.into());
    document.trailer.set("Root", root);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
