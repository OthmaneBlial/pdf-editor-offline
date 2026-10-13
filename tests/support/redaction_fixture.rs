//! Synthetic target-bearing page and hidden carriers; no redaction writer used.
use lopdf::{Document, Object, Stream, dictionary};
pub const SECRET: &str = "PRIVATE";
pub fn fixture() -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    doc.objects.insert(
        (1, 0),
        dictionary! {"Type"=>"Catalog","Pages"=>Object::Reference((2,0))}.into(),
    );
    doc.objects.insert((2,0),dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference((6,0)),Object::Reference((8,0))],"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>Object::Reference((3,0))}}}.into());
    doc.objects.insert(
        (3, 0),
        dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"}.into(),
    );
    doc.objects.insert(
        (4, 0),
        Stream::new(dictionary! {}, b"obsolete-PRIVATE".to_vec()).into(),
    );
    doc.objects.insert(
        (5, 0),
        Stream::new(
            dictionary! {},
            b"BT /F1 10 Tf 20 180 Td (prefix PRIVATE suffix) Tj ET".to_vec(),
        )
        .into(),
    );
    doc.objects.insert(
        (7, 0),
        Stream::new(dictionary! {}, b"unused public stream".to_vec()).into(),
    );
    for (id, rotation) in [(6, 0), (8, 90)] {
        doc.objects.insert((id,0),dictionary!{"Type"=>"Page","Parent"=>Object::Reference((2,0)),"Contents"=>Object::Reference((5,0)),"Rotate"=>rotation}.into());
    }
    doc.max_id = 8;
    doc.trailer.set("Root", Object::Reference((1, 0)));
    let info = doc.add_object(dictionary! {"Title"=>Object::string_literal(SECRET)});
    doc.trailer.set("Info", info);
    let xmp = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        format!("<private>{SECRET}</private>").into_bytes(),
    ));
    let js = doc.add_object(
        dictionary! {"Type"=>"Action","S"=>"JavaScript","JS"=>Object::string_literal(SECRET)},
    );
    let file = doc.add_object(Stream::new(
        dictionary! {"Type"=>"EmbeddedFile"},
        SECRET.as_bytes().to_vec(),
    ));
    let spec=doc.add_object(dictionary!{"Type"=>"Filespec","F"=>Object::string_literal(format!("{SECRET}.txt")),"EF"=>dictionary!{"F"=>file}});
    let root = doc.get_object_mut((1, 0)).unwrap().as_dict_mut().unwrap();
    root.set("Metadata", xmp);
    root.set("OpenAction", js);
    root.set("Names",dictionary!{"EmbeddedFiles"=>dictionary!{"Names"=>vec![Object::string_literal(format!("{SECRET}.txt")),Object::Reference(spec)]}});
    doc.add_object(Stream::new(
        dictionary! {},
        format!("orphan-{SECRET}").into_bytes(),
    ));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
