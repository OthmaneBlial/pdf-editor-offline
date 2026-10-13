#![allow(clippy::expect_used)]

use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use pdf_core::{Error, MetadataKey, PdfDocument};

fn fixture(kind: u8, bytes: Vec<u8>) -> PdfDocument {
    let mut source = Document::load_mem(include_bytes!(
        "../../../examples/sample_pdfs/demo-basic.pdf"
    ))
    .expect("synthetic demo");
    let value = Object::String(bytes, StringFormat::Literal);
    match kind {
        0 => {
            let info = source.add_object(dictionary! {"Title" => value});
            source.trailer.set("Info", info);
        }
        1 => {
            let note = source.add_object(dictionary! {
                "Type" => "Annot", "Subtype" => "Text", "Contents" => value,
                "Rect" => vec![72.into(), 72.into(), 96.into(), 96.into()],
            });
            let page = *source.get_pages().get(&1).expect("page");
            source
                .get_dictionary_mut(page)
                .expect("page dictionary")
                .set("Annots", vec![Object::Reference(note)]);
        }
        2 => {
            let payload = source.add_object(Stream::new(
                dictionary! {"Type" => "EmbeddedFile"},
                b"public synthetic attachment".to_vec(),
            ));
            let file = source.add_object(dictionary! {
                "Type" => "Filespec", "F" => value, "EF" => dictionary! {"F" => payload},
            });
            source.catalog_mut().expect("catalog").set("Names", dictionary! {
                "EmbeddedFiles" => dictionary! {"Names" => vec![Object::string_literal("entry"), Object::Reference(file)]},
            });
        }
        _ => unreachable!("known test reader"),
    }
    let mut encoded = Vec::new();
    source.save_to(&mut encoded).expect("synthetic PDF");
    PdfDocument::from_bytes(&encoded, None).expect("strict document loading")
}

#[test]
fn metadata_notes_and_attachment_reports_refuse_dropped_pdfdoc_characters() {
    for slot in [0, 0x7f, 0x9f, 0xad] {
        let bytes = vec![b'A', slot, b'B'];
        assert!(matches!(
            fixture(0, bytes.clone()).metadata(),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            fixture(1, bytes.clone()).list_notes(&[1]),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            fixture(2, bytes).inspect_security(),
            Err(Error::Refused(_))
        ));
    }
}

#[test]
fn all_three_reports_preserve_valid_unicode_and_refuse_malformed_unicode() {
    let text = "café 中文 😀";
    for prefix in [b"\xfe\xff".as_slice(), b"\xef\xbb\xbf"] {
        let mut bytes = prefix.to_vec();
        if prefix == b"\xfe\xff" {
            bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
        } else {
            bytes.extend(text.as_bytes());
        }
        assert_eq!(
            fixture(0, bytes.clone())
                .metadata()
                .expect("metadata")
                .fields[&MetadataKey::Title],
            text
        );
        assert_eq!(
            fixture(1, bytes.clone()).list_notes(&[1]).expect("notes")[0].contents,
            text
        );
        assert_eq!(
            fixture(2, bytes)
                .inspect_security()
                .expect("inspection")
                .embedded_files[0]
                .filename
                .as_deref(),
            Some(text)
        );
    }
    let invalid = b"\xfe\xff\xd8\0".to_vec();
    assert!(fixture(0, invalid.clone()).metadata().is_err());
    assert!(fixture(1, invalid.clone()).list_notes(&[1]).is_err());
    assert!(fixture(2, invalid).inspect_security().is_err());
}
