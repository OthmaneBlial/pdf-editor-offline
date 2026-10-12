#![allow(clippy::unwrap_used)]

use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, StringFormat,
    dictionary,
};
use pdf_core::{Error, MAX_METADATA_VALUE_BYTES, MetadataKey, PdfDocument};

fn fixture() -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let page=document.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()]});
    document.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let xmp = document.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        b"<metadata><title>Independent XMP title</title></metadata>".to_vec(),
    ));
    let info=document.add_object(dictionary! {"Title"=>Object::String(b"caf\xe9".to_vec(),StringFormat::Literal),"Author"=>Object::string_literal("Original"),"CreationDate"=>Object::string_literal("D:20261012030000Z"),"Custom"=>dictionary! {"Untouched"=>vec![Object::Integer(7),Object::Boolean(true)]}});
    let root=document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"Metadata"=>xmp,"Info"=>dictionary! {"Producer"=>Object::string_literal("nonstandard producer")},"AliasToOriginalInfo"=>info});
    document.trailer.set("Root", root);
    document.trailer.set("Info", info);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("metadata-first!!"),
            Object::string_literal("metadata-next!!!"),
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

#[test]
fn unicode_update_and_removal_preserve_xmp_custom_values_dates_and_aliases() {
    let source = fixture();
    let original = bytes(source.clone());
    let old_info = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let old_root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let mut document = PdfDocument::from_bytes(&original, None).unwrap();
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Title],
        "café"
    );
    let text = "Rapport – café 中文 😀";
    let result = document
        .update_metadata(&[
            (MetadataKey::Title, Some(text)),
            (MetadataKey::Author, None),
            (MetadataKey::Keywords, Some("")),
        ])
        .unwrap();
    assert_eq!(result.fields[&MetadataKey::Title], text);
    assert!(!result.fields.contains_key(&MetadataKey::Author));
    assert_eq!(result.fields[&MetadataKey::Keywords], "");
    assert!(result.xmp_present && result.catalog_info_present);
    assert!(!result.xmp_synchronized && !result.secure_erasure);
    let output = document.to_bytes().unwrap();
    let reopened = Document::load_mem(&output).unwrap();
    assert_eq!(
        reopened.objects.get(&old_info),
        source.objects.get(&old_info)
    );
    assert_eq!(
        reopened.objects.get(&old_root),
        source.objects.get(&old_root)
    );
    let new_info = reopened
        .get_dictionary(
            reopened
                .trailer
                .get(b"Info")
                .unwrap()
                .as_reference()
                .unwrap(),
        )
        .unwrap();
    let previous = source.get_dictionary(old_info).unwrap();
    for key in [b"CreationDate".as_slice(), b"Custom"] {
        assert_eq!(new_info.get(key).unwrap(), previous.get(key).unwrap());
    }
    assert!(
        new_info
            .get(b"Title")
            .unwrap()
            .as_str()
            .unwrap()
            .starts_with(b"\xFE\xFF")
    );
    assert_eq!(
        PdfDocument::from_bytes(&output, None)
            .unwrap()
            .metadata()
            .unwrap(),
        result
    );
    assert_eq!(bytes(source), original);
}

#[test]
fn missing_and_direct_info_are_supported_without_mutating_other_dictionary() {
    let mut source = fixture();
    source.trailer.remove(b"Info");
    let mut document = load(source);
    let before = document.to_bytes().unwrap();
    assert!(!document.metadata().unwrap().info_present);
    document
        .update_metadata(&[(MetadataKey::Title, None)])
        .unwrap();
    assert_eq!(document.to_bytes().unwrap(), before);
    document
        .update_metadata(&[(MetadataKey::Producer, Some("Rust"))])
        .unwrap();
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Producer],
        "Rust"
    );
    let mut source = fixture();
    source.trailer.set(
        "Info",
        dictionary! {"Title"=>Object::string_literal("direct"),"Custom"=>Object::Integer(99)},
    );
    let mut document = load(source);
    document
        .update_metadata(&[(MetadataKey::Title, Some("new"))])
        .unwrap();
    let output = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(
        output
            .get_dictionary(output.trailer.get(b"Info").unwrap().as_reference().unwrap())
            .unwrap()
            .get(b"Custom")
            .unwrap()
            .as_i64()
            .unwrap(),
        99
    );
}

#[test]
fn invalid_changes_do_not_partially_apply_and_exact_value_limit_roundtrips() {
    let mut document = load(fixture());
    let before = document.to_bytes().unwrap();
    for changes in [
        vec![],
        vec![
            (MetadataKey::Title, Some("first")),
            (MetadataKey::Title, Some("second")),
        ],
        vec![(MetadataKey::Title, Some("NUL\0value"))],
    ] {
        assert!(document.update_metadata(&changes).is_err());
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    let oversized = "x".repeat(MAX_METADATA_VALUE_BYTES + 1);
    assert!(matches!(
        document.update_metadata(&[
            (MetadataKey::Title, Some("would change")),
            (MetadataKey::Author, Some(&oversized))
        ]),
        Err(Error::Limit(_))
    ));
    assert_eq!(document.to_bytes().unwrap(), before);
    let exact = "é".repeat(MAX_METADATA_VALUE_BYTES / 2);
    assert_eq!(
        document
            .update_metadata(&[(MetadataKey::Title, Some(&exact))])
            .unwrap()
            .fields[&MetadataKey::Title],
        exact
    );
    assert!(MetadataKey::parse("creation-date").is_err());
}

#[test]
fn malformed_encoding_reference_cycles_and_non_strings_refuse_transactionally() {
    for value in [
        Object::String(vec![0xfe, 0xff, 0x00], StringFormat::Hexadecimal),
        Object::String(vec![0xfe, 0xff, 0xd8, 0x00], StringFormat::Hexadecimal),
        Object::String(vec![0xef, 0xbb, 0xbf, 0xff], StringFormat::Hexadecimal),
        Object::Integer(4),
        Object::Reference((999, 0)),
    ] {
        let mut source = fixture();
        let id = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
        source.get_dictionary_mut(id).unwrap().set("Title", value);
        let mut document = load(source);
        let before = document.to_bytes().unwrap();
        assert!(document.metadata().is_err());
        assert!(
            document
                .update_metadata(&[(MetadataKey::Author, Some("new"))])
                .is_err()
        );
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    let mut source = fixture();
    let id = source.add_object(Object::Null);
    source.objects.insert(id, Object::Reference(id));
    source.trailer.set("Info", id);
    let mut document = load(source);
    let before = document.to_bytes().unwrap();
    assert!(matches!(document.metadata(), Err(Error::Invalid(_))));
    assert!(
        document
            .update_metadata(&[(MetadataKey::Title, Some("new"))])
            .is_err()
    );
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn info_entry_and_read_value_limits_are_explicit() {
    let mut source = fixture();
    let id = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    for index in 0..10001 {
        source
            .get_dictionary_mut(id)
            .unwrap()
            .set(format!("Custom{index}"), Object::Null);
    }
    assert!(matches!(load(source).metadata(), Err(Error::Limit(_))));
    let mut source = fixture();
    let id = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    source.get_dictionary_mut(id).unwrap().set(
        "Title",
        Object::string_literal("x".repeat(MAX_METADATA_VALUE_BYTES + 1)),
    );
    assert!(matches!(load(source).metadata(), Err(Error::Limit(_))));
}

#[test]
fn all_six_keys_and_utf8_bom_are_read_without_exposing_the_bom() {
    let mut source = fixture();
    let id = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let mut encoded = b"\xEF\xBB\xBF".to_vec();
    encoded.extend_from_slice("valid café 😀".as_bytes());
    source
        .get_dictionary_mut(id)
        .unwrap()
        .set("Title", Object::String(encoded, StringFormat::Hexadecimal));
    let mut document = load(source);
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Title],
        "valid café 😀"
    );
    let keys = [
        MetadataKey::Title,
        MetadataKey::Author,
        MetadataKey::Subject,
        MetadataKey::Keywords,
        MetadataKey::Creator,
        MetadataKey::Producer,
    ];
    let changes = keys
        .iter()
        .map(|key| (*key, Some("café 中文 😀")))
        .collect::<Vec<_>>();
    let report = document.update_metadata(&changes).unwrap();
    for key in keys {
        assert_eq!(report.fields[&key], "café 中文 😀");
    }
    assert_eq!(
        load(Document::load_mem(&document.to_bytes().unwrap()).unwrap())
            .metadata()
            .unwrap(),
        report
    );
}

#[test]
fn large_custom_direct_metadata_refuses_amplification_but_indirect_payload_stays_shared() {
    let mut source = fixture();
    let info = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    source.get_dictionary_mut(info).unwrap().set(
        "LargeCustom",
        Object::string_literal("x".repeat(1024 * 1024)),
    );
    let mut document = load(source);
    let before = document.to_bytes().unwrap();
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Author],
        "Original"
    );
    assert!(matches!(
        document.update_metadata(&[(MetadataKey::Title, Some("new"))]),
        Err(Error::Limit(_))
    ));
    assert_eq!(document.to_bytes().unwrap(), before);
    let mut source = fixture();
    let info = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let payload = source.add_object(Object::string_literal("x".repeat(2 * 1024 * 1024)));
    source
        .get_dictionary_mut(info)
        .unwrap()
        .set("LargeCustom", payload);
    let mut document = load(source.clone());
    document
        .update_metadata(&[(MetadataKey::Title, Some("new"))])
        .unwrap();
    let output = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    let new_info = output
        .get_dictionary(output.trailer.get(b"Info").unwrap().as_reference().unwrap())
        .unwrap();
    assert_eq!(
        new_info
            .get(b"LargeCustom")
            .unwrap()
            .as_reference()
            .unwrap(),
        payload
    );
    assert_eq!(output.objects.get(&payload), source.objects.get(&payload));
    let logical = output
        .objects
        .values()
        .filter(|object| {
            !object.as_stream().ok().is_some_and(|stream| {
                stream.dict.has_type(b"XRef") || stream.dict.has_type(b"ObjStm")
            })
        })
        .count();
    assert_eq!(logical, source.objects.len() + 1);
}

#[test]
fn numeric_arrays_are_bounded_before_the_pending_scan_grows() {
    let mut source = fixture();
    let info = source.trailer.get(b"Info").unwrap().as_reference().unwrap();
    source
        .get_dictionary_mut(info)
        .unwrap()
        .set("Numbers", Object::Array(vec![Object::Integer(0); 10001]));
    let mut document = load(source);
    let before = document.to_bytes().unwrap();
    assert!(matches!(
        document.update_metadata(&[(MetadataKey::Title, Some("new"))]),
        Err(Error::Limit(_))
    ));
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn signed_and_decrypted_metadata_are_read_only() {
    let mut source = fixture();
    source.add_object(dictionary! {"FT"=>"Sig"});
    let mut document = load(source);
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Author],
        "Original"
    );
    assert!(matches!(
        document.update_metadata(&[(MetadataKey::Title, Some("new"))]),
        Err(Error::Refused(_))
    ));
    let mut source = fixture();
    let encryption = EncryptionVersion::V2 {
        document: &source,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: Permissions::all(),
    };
    let state = EncryptionState::try_from(encryption).unwrap();
    source.encrypt(&state).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes(source), Some("reader")).unwrap();
    assert_eq!(
        document.metadata().unwrap().fields[&MetadataKey::Author],
        "Original"
    );
    assert!(matches!(
        document.update_metadata(&[(MetadataKey::Title, None)]),
        Err(Error::Refused(_))
    ));
}
