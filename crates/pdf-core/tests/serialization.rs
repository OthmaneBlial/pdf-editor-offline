#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{DocumentInfo, DocumentMetadata, Error, MetadataKey, PdfDocument};

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

#[test]
fn verified_bytes_reopen_preserves_geometry_objects_and_worker_reply_types() {
    let mut source = fixture();
    let orphan =
        source.add_object(dictionary! { "Custom" => Object::string_literal("retain orphan") });
    let mut bytes = Vec::new();
    source.save_to(&mut bytes).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes, None).unwrap();
    document.rotate(&[1], 90).unwrap();
    document
        .update_metadata(&[(MetadataKey::Title, Some("Révision 中文"))])
        .unwrap();
    let verified = document.verified_bytes().unwrap();
    let reopened = PdfDocument::from_bytes(&verified, None).unwrap();
    assert_eq!(
        reopened.info().unwrap().pages,
        document.info().unwrap().pages
    );
    assert_eq!(
        Document::load_mem(&verified)
            .unwrap()
            .get_object(orphan)
            .unwrap(),
        source.get_object(orphan).unwrap()
    );
    let info = document.info().unwrap();
    let decoded: DocumentInfo =
        serde_json::from_slice(&serde_json::to_vec(&info).unwrap()).unwrap();
    assert_eq!(decoded.pages, info.pages);
    let metadata = document.metadata().unwrap();
    let decoded: DocumentMetadata =
        serde_json::from_slice(&serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(decoded, metadata);
    assert_eq!(document.metadata().unwrap(), reopened.metadata().unwrap());
}

#[test]
fn verified_bytes_refuses_signed_input_without_mutation() {
    let mut source = fixture();
    source.add_object(dictionary! { "FT" => "Sig" });
    let mut bytes = Vec::new();
    source.save_to(&mut bytes).unwrap();
    let document = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(matches!(document.verified_bytes(), Err(Error::Refused(_))));
    assert_eq!(document.info().unwrap().signature_fields, 1);
}
