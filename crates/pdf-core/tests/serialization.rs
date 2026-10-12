#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;

fn fixture() -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let content = document.add_object(Stream::new(dictionary! {}, b"0 0 20 20 re f".to_vec()));
    let page = document.add_object(
        dictionary! { "Type" => "Page", "Parent" => pages, "Resources" => dictionary! {},
        "MediaBox" => vec![0.into(),0.into(),200.into(),300.into()], "Contents" => content },
    );
    document.objects.insert(
        pages,
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1 }
            .into(),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    document.trailer.set("Root", catalog);
    document
}

#[test]
fn crop_geometry_is_part_of_verified_save_information() {
    let mut source = fixture();
    let mut bytes = Vec::new();
    source.save_to(&mut bytes).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert_eq!(
        document.info().unwrap().pages[0].crop_box,
        [0.0, 0.0, 200.0, 300.0]
    );
    document.crop(&[1], [10.0, 20.0, 180.0, 280.0]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    document.save_new(dir.path().join("cropped.pdf")).unwrap();
    assert_eq!(
        PdfDocument::open(dir.path().join("cropped.pdf"), None)
            .unwrap()
            .info()
            .unwrap()
            .pages[0]
            .crop_box,
        [10.0, 20.0, 180.0, 280.0]
    );
}

#[test]
fn expanded_object_streams_survive_verified_new_copy_saving() {
    let mut source = fixture();
    let mut bytes = Vec::new();
    source.save_modern(&mut bytes).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes, None).unwrap();
    document.rotate(&[1], 90).unwrap();
    let dir = tempfile::tempdir().unwrap();
    document.save_new(dir.path().join("modern.pdf")).unwrap();
    assert_eq!(
        PdfDocument::open(dir.path().join("modern.pdf"), None)
            .unwrap()
            .info()
            .unwrap()
            .pages[0]
            .rotation,
        90
    );
}
