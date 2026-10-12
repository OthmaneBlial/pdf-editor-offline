#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;

fn normalized(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn document(appearance: Vec<u8>, flags: i64, states: bool) -> Vec<u8> {
    let mut document = Document::with_version("1.7");
    let font = document.add_object(
        dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
    );
    let resources = dictionary! { "Font" => dictionary! {"F1" => font}};
    let visible = document.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "Resources" => resources.clone()},
        appearance,
    ));
    let unused = document.add_object(Stream::new(
        dictionary! { "Resources" => resources.clone()},
        b"BT /F1 12 Tf (UNSELECTED) Tj ET".to_vec(),
    ));
    let normal = if states {
        Object::Dictionary(dictionary! { "Yes" => visible, "Off" => unused })
    } else {
        Object::Reference(visible)
    };
    let annotation = document.add_object(dictionary! { "Type" => "Annot", "Subtype" => "Widget", "F" => flags, "AS" => "Yes", "V" => Object::string_literal("NOT APPEARANCE"), "AP" => dictionary! {"N" => normal}});
    let empty = document.add_object(Stream::new(dictionary! {}, Vec::new()));
    let pages = document.new_object_id();
    let page = document.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()], "Resources" => resources, "Contents" => empty, "Annots" => vec![annotation.into()]});
    document.objects.insert(
        pages,
        dictionary! {"Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1}.into(),
    );
    let catalog = document.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages});
    document.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn visible_widget_appearance_not_field_value_is_extracted() {
    let bytes = document(b"BT /F1 12 Tf (VISIBLE) Tj ET".to_vec(), 0, true);
    let parsed = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert_eq!(normalized(&parsed.extract_text(&[1]).unwrap()), "VISIBLE");
    assert_eq!(
        bytes,
        document(b"BT /F1 12 Tf (VISIBLE) Tj ET".to_vec(), 0, true)
    );
}

#[test]
fn invisible_hidden_and_no_view_appearances_are_excluded() {
    for flags in [1, 2, 32] {
        let bytes = document(b"BT /F1 12 Tf (HIDDEN) Tj ET".to_vec(), flags, false);
        let parsed = PdfDocument::from_bytes(&bytes, None).unwrap();
        assert_eq!(normalized(&parsed.extract_text(&[1]).unwrap()), "");
    }
}

#[test]
fn oversized_appearance_refused() {
    let bytes = document(vec![b' '; pdf_core::MAX_STREAM_BYTES + 1], 0, false);
    let mut compressed = Document::load_mem(&bytes).unwrap();
    for object in compressed.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            stream.compress().unwrap();
        }
    }
    let mut bytes = Vec::new();
    compressed.save_to(&mut bytes).unwrap();
    assert!(bytes.len() < 100_000);
    let parsed = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(parsed.extract_text(&[1]).is_err());
}

#[test]
fn recursive_invoked_appearance_is_refused() {
    let bytes = document(b"VISIBLE".to_vec(), 0, false);
    let mut cyclic = Document::load_mem(&bytes).unwrap();
    let id = *cyclic
        .objects
        .iter()
        .find(|(_, object)| {
            object
                .as_stream()
                .is_ok_and(|stream| stream.content == b"VISIBLE")
        })
        .unwrap()
        .0;
    let stream = cyclic
        .objects
        .get_mut(&id)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.dict.set(
        "Resources",
        dictionary! { "XObject" => dictionary! { "Loop" => id }},
    );
    stream.set_content(b"/Loop Do".to_vec());
    let mut bytes = Vec::new();
    cyclic.save_to(&mut bytes).unwrap();
    let parsed = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(parsed.extract_text(&[1]).is_err());
}

#[test]
fn trust_lab_forms_and_signature_appearances_are_read_without_modification() {
    for (name, expected) in [
        (
            "forms",
            "Synthetic form controls Synthetic value ✓ Normal 2026-08-24",
        ),
        ("signed", "Cryptographically signed synthetic fixture SIGN"),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../trust_lab/corpus/v1/{name}.pdf"));
        let before = std::fs::read(&path).unwrap();
        let parsed = PdfDocument::open(&path, None).unwrap();
        assert_eq!(normalized(&parsed.extract_text(&[1]).unwrap()), expected);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
