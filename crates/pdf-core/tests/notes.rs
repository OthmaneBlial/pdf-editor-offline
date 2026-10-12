#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use pdf_core::{Error, MAX_NOTE_CONTENT_BYTES, PdfDocument};

fn fixture() -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let stream = document.add_object(Stream::new(
        dictionary! {},
        b"q 1 0 0 rg 10 20 20 20 re f Q".to_vec(),
    ));
    let first = document
        .add_object(dictionary! {"Type" => "Page", "Parent" => pages, "Contents" => stream});
    let second = document
        .add_object(dictionary! {"Type" => "Page", "Parent" => pages, "Contents" => stream});
    document.objects.insert(
        pages,
        dictionary! {
            "Type" => "Pages", "Count" => 2, "Kids" => vec![first.into(), second.into()],
            "MediaBox" => vec![10.into(), 20.into(), 210.into(), 320.into()],
            "CropBox" => vec![20.into(), 30.into(), 200.into(), 300.into()],
            "Rotate" => 90, "Resources" => dictionary! {},
        }
        .into(),
    );
    let catalog = document.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages});
    document.trailer.set("Root", catalog);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("synthetic-first!"),
            Object::string_literal("synthetic-next!!"),
        ],
    );
    document
}

fn bytes(mut document: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn load(document: Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(document), None).unwrap()
}

const RECT: [f32; 4] = [50.0, 60.0, 74.0, 84.0];

#[test]
fn unicode_notes_round_trip_with_real_page_reference_and_self_contained_appearance() {
    let mut document = load(fixture());
    let info = document.info().unwrap();
    let contents = "ASCII café 中文 العربية 😀\nSecond line";
    let note = document.add_note(1, RECT, contents, "Élodie 作者").unwrap();
    assert_eq!(
        document.list_notes(&[1, 1, 2]).unwrap(),
        std::slice::from_ref(&note)
    );
    assert_eq!(document.info().unwrap().pages, info.pages);
    assert_eq!(document.info().unwrap().object_count, info.object_count + 2);
    assert_eq!(document.extract_text(&[1]).unwrap().trim(), "");
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("note.pdf");
    document.save_new(&output).unwrap();
    assert_eq!(
        PdfDocument::open(&output, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap(),
        std::slice::from_ref(&note)
    );
    let raw = Document::load(&output).unwrap();
    let annot = raw.get_dictionary(note.object_id.unwrap()).unwrap();
    assert_eq!(
        annot.get(b"P").unwrap().as_reference().unwrap(),
        raw.get_pages()[&1]
    );
    for key in [b"Contents".as_slice(), b"T"] {
        assert!(
            annot
                .get(key)
                .unwrap()
                .as_str()
                .unwrap()
                .starts_with(b"\xFE\xFF")
        );
    }
    assert_eq!(annot.get(b"F").unwrap().as_i64().unwrap(), 4);
    let appearance = annot
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let appearance = raw.get_object(appearance).unwrap().as_stream().unwrap();
    assert_eq!(
        appearance.dict.get(b"Subtype").unwrap().as_name().unwrap(),
        b"Form"
    );
    assert!(
        appearance
            .dict
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .is_empty()
    );
    assert!(appearance.content.windows(2).any(|chunk| chunk == b"rg"));
}

#[test]
fn shared_and_direct_arrays_keep_old_notes_links_widgets_and_other_pages_unchanged() {
    for shared in [false, true] {
        let mut source = fixture();
        let ids = source.get_pages();
        let link = source.add_object(dictionary! {"Subtype" => "Link", "Rect" => vec![30.into(), 40.into(), 40.into(), 50.into()], "Dest" => vec![ids[&2].into(), "Fit".into()]});
        let widget = dictionary! {"Subtype" => "Widget", "Rect" => vec![30.into(), 40.into(), 40.into(), 50.into()], "FT" => "Tx"};
        let old_note = dictionary! {"Subtype" => "Text", "Rect" => vec![30.into(), 40.into(), 40.into(), 50.into()], "Contents" => Object::string_literal("OLD"), "F" => 2};
        let entries = vec![link.into(), widget.into(), old_note.into()];
        let value = if shared {
            Object::Reference(source.add_object(Object::Array(entries.clone())))
        } else {
            Object::Array(entries.clone())
        };
        for id in ids.values() {
            source
                .get_dictionary_mut(*id)
                .unwrap()
                .set("Annots", value.clone());
        }
        // Annots is not inherited. This irrelevant ancestor marker must not be modified.
        let parent = source
            .catalog()
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        source
            .get_dictionary_mut(parent)
            .unwrap()
            .set("Annots", value.clone());
        let mut document = load(source);
        let before = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
        document.add_note(1, RECT, "NEW", "AUTHOR").unwrap();
        let after = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(
            after.get_dictionary(ids[&2]).unwrap(),
            before.get_dictionary(ids[&2]).unwrap()
        );
        assert_eq!(
            after.get_dictionary(parent).unwrap(),
            before.get_dictionary(parent).unwrap()
        );
        if let Object::Reference(array) = value {
            assert_eq!(
                after.get_object(array).unwrap(),
                before.get_object(array).unwrap()
            );
        }
        let added = after
            .get_dictionary(ids[&1])
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(&added[..3], entries);
        assert_eq!(
            document
                .list_notes(&[2, 1])
                .unwrap()
                .iter()
                .map(|note| (note.page, note.contents.as_str(), note.flags))
                .collect::<Vec<_>>(),
            [(2, "OLD", 2), (1, "OLD", 2), (1, "NEW", 4)]
        );
    }
}

#[test]
fn invalid_geometry_oversized_text_and_invalid_pages_are_atomic() {
    let mut document = load(fixture());
    let original = document.to_bytes().unwrap();
    for (page, rect, text) in [
        (0, RECT, "x"),
        (3, RECT, "x"),
        (1, [f32::NAN, 60.0, 74.0, 84.0], "x"),
        (1, [50.0, 60.0, f32::INFINITY, 84.0], "x"),
        (1, [74.0, 60.0, 50.0, 84.0], "x"),
        (1, [10.0, 20.0, 34.0, 44.0], "x"),
    ] {
        assert!(document.add_note(page, rect, text, "").is_err());
        assert_eq!(document.to_bytes().unwrap(), original);
    }
    assert!(matches!(
        document.add_note(1, RECT, &"x".repeat(MAX_NOTE_CONTENT_BYTES + 1), ""),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        document.add_note(1, RECT, "", &"x".repeat(1025)),
        Err(Error::Limit(_))
    ));
    assert_eq!(document.to_bytes().unwrap(), original);
}

#[test]
fn malformed_arrays_references_strings_and_rectangles_fail_explicitly() {
    for malformed in [Object::Integer(7), Object::Reference((999, 0))] {
        let mut source = fixture();
        let page = source.get_pages()[&1];
        source
            .get_dictionary_mut(page)
            .unwrap()
            .set("Annots", malformed);
        let mut document = load(source);
        assert!(document.list_notes(&[1]).is_err());
        let before = document.to_bytes().unwrap();
        assert!(document.add_note(1, RECT, "x", "").is_err());
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    for value in [
        Object::String(vec![0xFE, 0xFF, 0], StringFormat::Hexadecimal),
        Object::String(vec![0xFE, 0xFF, 0xD8, 0], StringFormat::Hexadecimal),
        Object::String(vec![0xEF, 0xBB, 0xBF, 0xFF], StringFormat::Hexadecimal),
        Object::Integer(1),
        Object::string_literal("x".repeat(MAX_NOTE_CONTENT_BYTES * 2 + 3)),
    ] {
        let mut source = fixture();
        let page = source.get_pages()[&1];
        source.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Dictionary(dictionary! {"Subtype"=>"Text", "Rect" => vec![50.into(),60.into(),74.into(),84.into()], "Contents" => value})]);
        assert!(load(source).list_notes(&[1]).is_err());
    }
    let mut source = fixture();
    let cycle = source.new_object_id();
    source.objects.insert(cycle, Object::Reference(cycle));
    let page = source.get_pages()[&1];
    source
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", cycle);
    let mut document = load(source);
    assert!(
        document
            .list_notes(&[1])
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
    assert!(
        document
            .add_note(1, RECT, "x", "")
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
}

#[test]
fn notes_are_not_inherited_and_malformed_note_fields_are_not_silently_omitted() {
    let mut source = fixture();
    let parent = source
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    source.get_dictionary_mut(parent).unwrap().set("Annots", 7);
    let mut document = load(source);
    assert!(document.list_notes(&[1, 2]).unwrap().is_empty());
    document.add_note(1, RECT, "NEW", "").unwrap();
    assert!(document.list_notes(&[2]).unwrap().is_empty());
    for (key, value) in [
        (
            b"Rect".as_slice(),
            Object::Array(vec![0.into(), 1.into(), 2.into()]),
        ),
        (
            b"Rect",
            Object::Array(vec![
                0.into(),
                1.into(),
                Object::Name(b"invalid-number".to_vec()),
                3.into(),
            ]),
        ),
        (b"F", Object::Integer(-1)),
        (b"F", Object::string_literal("invalid")),
        (b"T", Object::Integer(4)),
    ] {
        let mut source = fixture();
        let page = source.get_pages()[&1];
        let mut note =
            dictionary! {"Subtype"=>"Text", "Rect"=>vec![50.into(),60.into(),74.into(),84.into()]};
        note.set(key, value);
        source
            .get_dictionary_mut(page)
            .unwrap()
            .set("Annots", vec![Object::Dictionary(note)]);
        assert!(load(source).list_notes(&[1]).is_err());
    }
}

#[test]
fn legacy_pdfdoc_utf8_and_utf16_notes_decode_and_structural_operations_preserve_notes() {
    let mut source = fixture();
    let page = source.get_pages()[&1];
    let strings = [
        Object::String(vec![0x80], StringFormat::Literal),
        Object::String(b"\xEF\xBB\xBFUTF8".to_vec(), StringFormat::Literal),
        lopdf::text_string("中文"),
    ];
    source.get_dictionary_mut(page).unwrap().set("Annots", strings.into_iter().map(|value| Object::Dictionary(dictionary! {"Subtype"=>"Text", "Rect" => vec![50.into(),60.into(),74.into(),84.into()], "Contents"=>value})).collect::<Vec<_>>());
    let mut document = load(source);
    let notes = document.list_notes(&[1]).unwrap();
    assert_eq!(
        notes
            .iter()
            .map(|note| note.contents.as_str())
            .collect::<Vec<_>>(),
        ["•", "UTF8", "中文"]
    );
    document.reverse().unwrap();
    assert_eq!(
        document
            .list_notes(&[2])
            .unwrap()
            .iter()
            .map(|note| &note.contents)
            .collect::<Vec<_>>(),
        notes.iter().map(|note| &note.contents).collect::<Vec<_>>()
    );
    document.rotate(&[2], 90).unwrap();
    document.crop(&[2], [30.0, 40.0, 180.0, 280.0]).unwrap();
    document.compress().unwrap();
    let reopened = PdfDocument::from_bytes(&document.to_bytes().unwrap(), None).unwrap();
    assert_eq!(
        reopened.list_notes(&[2]).unwrap(),
        document.list_notes(&[2]).unwrap()
    );
    assert!(matches!(
        document.extract_pages(&[2]),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        document.delete_pages(&[1]),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        load(fixture()).append(&document),
        Err(Error::Refused(_))
    ));
}

#[test]
fn signed_and_encrypted_inputs_allow_listing_but_refuse_addition() {
    let signed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pdfs/signed.pdf");
    let mut document = PdfDocument::open(signed, None).unwrap();
    document.list_notes(&[1]).unwrap();
    assert!(matches!(
        document.add_note(1, RECT, "x", ""),
        Err(Error::Refused(_))
    ));
    let mut source = fixture();
    let page = source.get_pages()[&1];
    source.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Dictionary(dictionary! {"Subtype"=>"Text", "Rect"=>vec![50.into(),60.into(),74.into(),84.into()], "Contents"=>lopdf::text_string("Encrypted café 中文")})]);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &source,
        owner_password: "owner",
        user_password: "user",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes(source), Some("user")).unwrap();
    assert_eq!(
        document.list_notes(&[1]).unwrap()[0].contents,
        "Encrypted café 中文"
    );
    assert!(matches!(
        document.add_note(1, RECT, "x", ""),
        Err(Error::Refused(_))
    ));
}
