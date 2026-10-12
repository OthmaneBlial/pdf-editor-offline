#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{Error, PdfDocument};
use std::path::Path;

fn fixture() -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let font = document.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources = document.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for number in 1..=3 {
        let content = document.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 72 720 Td (Page {number}) Tj ET").into_bytes(),
        ));
        let page = document
            .add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content });
        kids.push(page.into());
    }
    document.objects.insert(pages, dictionary! { "Type" => "Pages", "Kids" => Object::Array(kids), "Count" => 3,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => resources, "Rotate" => 90 }.into());
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    document.trailer.set("Root", catalog);
    document
}

fn bytes(mut document: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn extraction_and_inherited_rotation_survive_verified_save() {
    let source = bytes(fixture());
    let mut document = PdfDocument::from_bytes(&source, None).unwrap();
    assert_eq!(document.page_count(), 3);
    assert!(document.extract_text(&[1, 3]).unwrap().contains("Page 3"));
    assert!(!document.extract_text(&[1, 3]).unwrap().contains("Page 2"));
    document.rotate(&[1, 1, 3], 90).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("rotated.pdf");
    document.save_new(&output).unwrap();
    let reopened = PdfDocument::open(&output, None).unwrap();
    assert_eq!(
        reopened
            .info()
            .unwrap()
            .pages
            .iter()
            .map(|page| page.rotation)
            .collect::<Vec<_>>(),
        [180, 90, 180]
    );
    assert!(
        reopened
            .extract_text(&[1, 2, 3])
            .unwrap()
            .contains("Page 2")
    );
    assert_eq!(
        PdfDocument::from_bytes(&source, None)
            .unwrap()
            .info()
            .unwrap()
            .pages[0]
            .rotation,
        90
    );
}

#[test]
fn no_clobber_and_failed_save_preserve_existing_files() {
    let document = PdfDocument::from_bytes(&bytes(fixture()), None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("existing.pdf");
    std::fs::write(&output, b"original data").unwrap();
    assert!(matches!(
        document.save_new(&output),
        Err(Error::OutputExists(_))
    ));
    assert_eq!(std::fs::read(&output).unwrap(), b"original data");
    assert!(
        document
            .save_new(dir.path().join("missing").join("out.pdf"))
            .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn output_symlinks_are_not_followed_or_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("link.pdf");
    std::os::unix::fs::symlink(dir.path().join("absent.pdf"), &link).unwrap();
    let document = PdfDocument::from_bytes(&bytes(fixture()), None).unwrap();
    assert!(matches!(
        document.save_new(&link),
        Err(Error::OutputExists(_))
    ));
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn bad_input_and_invalid_edits_return_errors_without_changes() {
    assert!(PdfDocument::from_bytes(b"not a PDF", None).is_err());
    assert!(PdfDocument::from_bytes(b"%PDF-1.7\nbroken", None).is_err());
    let mut document = PdfDocument::from_bytes(&bytes(fixture()), None).unwrap();
    let before = document.to_bytes().unwrap();
    assert!(document.rotate(&[0], 90).is_err());
    assert!(document.rotate(&[4], 90).is_err());
    assert!(document.rotate(&[], 90).is_err());
    assert!(document.rotate(&[1], 45).is_err());
    assert!(document.extract_text(&[0]).is_err());
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn page_tree_cycles_and_inconsistent_counts_are_refused() {
    let mut cyclic = fixture();
    let root = cyclic
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    cyclic
        .get_dictionary_mut(root)
        .unwrap()
        .set("Kids", vec![Object::Reference(root)]);
    assert!(PdfDocument::from_bytes(&bytes(cyclic), None).is_err());
    let mut bad_count = fixture();
    bad_count.get_dictionary_mut(root).unwrap().set("Count", 99);
    assert!(PdfDocument::from_bytes(&bytes(bad_count), None).is_err());
}

#[test]
fn signature_fields_block_mutation_and_saving() {
    let mut source = fixture();
    source.add_object(dictionary! { "FT" => "Sig", "Type" => "Annot", "Subtype" => "Widget" });
    let mut document = PdfDocument::from_bytes(&bytes(source), None).unwrap();
    assert!(document.info().unwrap().signature_fields > 0);
    assert!(matches!(document.rotate(&[1], 90), Err(Error::Refused(_))));
    assert!(matches!(document.to_bytes(), Err(Error::Refused(_))));
}

#[test]
fn existing_trust_lab_pages_reopen_without_losing_catalog_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1");
    for name in [
        "bookmarks.pdf",
        "forms.pdf",
        "attachment.pdf",
        "layers-transparency.pdf",
        "rotated-page.pdf",
        "mixed-fonts.pdf",
    ] {
        let source = root.join(name);
        let mut document = PdfDocument::open(&source, None).unwrap();
        document.rotate(&[1], 90).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join(name);
        document.save_new(&output).unwrap();
        let before = Document::load(source).unwrap();
        let after = Document::load(output).unwrap();
        for key in [
            b"Outlines".as_slice(),
            b"AcroForm",
            b"Names",
            b"OCProperties",
        ] {
            assert_eq!(
                before.catalog().unwrap().get(key).ok(),
                after.catalog().unwrap().get(key).ok(),
                "{name}: {key:?}"
            );
        }
        assert_eq!(before.objects.len(), after.objects.len());
    }
}
