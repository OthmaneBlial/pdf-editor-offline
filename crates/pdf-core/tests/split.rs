#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{Error, PdfDocument};
fn fixture(count: usize) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let root = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = vec![];
    for page in 1..=count {
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 20 Tf 30 120 Td (PAGE {page}) Tj ET").into_bytes(),
        ));
        let id=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>root,"Contents"=>content,"Rotate"=>((page-1)%4*90) as i64});
        kids.push(Object::Reference(id));
    }
    doc.objects.insert(root,dictionary!{"Type"=>"Pages","Count"=>count as i64,"Kids"=>kids,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}.into());
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>root});
    doc.trailer.set("Root", catalog);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn verified_chunks_preserve_order_geometry_source_and_keep_unreachable_data_honestly() {
    let bytes = fixture(5);
    let doc = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = doc.to_bytes().unwrap();
    let source = doc.info().unwrap();
    let parts = doc.split_every(2).unwrap();
    assert!(
        parts[0]
            .bytes
            .windows(b"PAGE 5".len())
            .any(|value| value == b"PAGE 5"),
        "unreachable other-page content remains; split is not erasure"
    );
    assert_eq!(
        parts
            .iter()
            .map(|part| (part.first_page, part.last_page))
            .collect::<Vec<_>>(),
        vec![(1, 2), (3, 4), (5, 5)]
    );
    for part in parts {
        let chunk = PdfDocument::from_bytes(&part.bytes, None).unwrap();
        assert_eq!(
            chunk.page_count(),
            (part.last_page - part.first_page + 1) as usize
        );
        for (index, page) in chunk.info().unwrap().pages.iter().enumerate() {
            let mut expected = source.pages[part.first_page as usize - 1 + index].clone();
            expected.number = index as u32 + 1;
            assert_eq!(*page, expected);
            assert!(
                chunk
                    .extract_text(&[index as u32 + 1])
                    .unwrap()
                    .contains(&format!("PAGE {}", part.first_page + index as u32))
            );
        }
    }
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(matches!(doc.split_every(0), Err(Error::Pages(_))));
}
#[test]
fn exclusive_directory_and_advanced_refusals_never_replace_or_create_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let doc = PdfDocument::from_bytes(&fixture(3), None).unwrap();
    let output = temp.path().join("new");
    let parts = doc.split_new_directory(2, &output).unwrap();
    assert_eq!(parts.len(), 2);
    assert!(output.join("part-0001.pdf").is_file());
    let first = std::fs::read(&parts[0].file).unwrap();
    assert!(matches!(
        doc.split_new_directory(2, &output),
        Err(Error::OutputExists(_))
    ));
    assert_eq!(std::fs::read(&parts[0].file).unwrap(), first);
    let advanced = PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/bookmarks.pdf"),
        None,
    )
    .unwrap();
    let refused = temp.path().join("refused");
    assert!(advanced.split_new_directory(1, &refused).is_err());
    assert!(!refused.exists());
    let huge = PdfDocument::from_bytes(&fixture(1025), None).unwrap();
    assert!(matches!(huge.split_every(1), Err(Error::Limit(_))));
}
#[test]
fn aggregate_work_and_payload_are_refused_before_copies() {
    let mut raw = Document::load_mem(&fixture(2)).unwrap();
    raw.add_object(Object::Array(vec![Object::Integer(1); 1_000_000]));
    let mut bytes = vec![];
    raw.save_to(&mut bytes).unwrap();
    let doc = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(matches!(doc.split_every(1), Err(Error::Limit(_))));
    let mut raw = Document::load_mem(&fixture(2)).unwrap();
    raw.add_object(Object::string_literal(vec![b'x'; 65 * 1024 * 1024]));
    let mut bytes = vec![];
    raw.save_to(&mut bytes).unwrap();
    let doc = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(matches!(doc.split_every(1), Err(Error::Limit(_))));
}
