#![allow(clippy::unwrap_used)]

use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use pdf_core::{ActionKind, Error, FieldType, PdfDocument};

fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages,
        "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()], "Resources" => dictionary! {} });
    doc.objects.insert(
        pages,
        dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)] }
            .into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal(b"synthetic-test-1"),
            Object::string_literal(b"synthetic-test-2"),
        ],
    );
    doc
}

fn bytes(mut doc: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn load(doc: Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(doc), None).unwrap()
}

#[test]
fn nested_actions_metadata_attachments_forms_and_layers_are_reported_without_values() {
    let mut source = fixture();
    let page = *source.get_pages().get(&1).unwrap();
    let js = source.add_object(dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("PRIVATE_SCRIPT_SENTINEL") });
    source.catalog_mut().unwrap().set("OpenAction", js);
    source.get_dictionary_mut(page).unwrap().set("AA", dictionary! {
        "O" => dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("PRIVATE_SCRIPT_SENTINEL") }
    });
    let hidden_js = source.add_object(dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("PRIVATE_SCRIPT_SENTINEL") });
    source.add_object(Object::Array(vec![
        dictionary! { "S" => "Launch", "F" => Object::string_literal("PRIVATE_ACTION_TARGET") }
            .into(),
        dictionary! { "S" => "URI", "URI" => Object::string_literal("PRIVATE_ACTION_TARGET") }
            .into(),
        dictionary! { "S" => "GoToR", "F" => Object::string_literal("PRIVATE_ACTION_TARGET") }
            .into(),
        dictionary! { "S" => "SubmitForm", "F" => Object::string_literal("PRIVATE_ACTION_TARGET") }
            .into(),
    ]));
    let payload = source.add_object(Stream::new(
        dictionary! { "Type" => "EmbeddedFile" },
        b"PRIVATE_PAYLOAD_SENTINEL".to_vec(),
    ));
    let file = source.add_object(
        dictionary! { "Type" => "Filespec", "UF" => Object::string_literal("fixture.txt"),
        "EF" => dictionary! { "F" => payload } },
    );
    source.catalog_mut().unwrap().set("Names", dictionary! {
        "JavaScript" => dictionary! { "Names" => vec![Object::string_literal("job"), Object::Reference(js)] },
        "EmbeddedFiles" => dictionary! { "Names" => vec![Object::string_literal("fixture.txt"), Object::Reference(file)] }
    });
    let info = source
        .add_object(dictionary! { "Author" => Object::string_literal("PRIVATE_AUTHOR_SENTINEL") });
    source.trailer.set("Info", info);
    let metadata = source.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        b"PRIVATE_XMP_SENTINEL".to_vec(),
    ));
    source.catalog_mut().unwrap().set("Metadata", metadata);
    source.catalog_mut().unwrap().set("AcroForm", dictionary! {
        "XFA" => Object::string_literal("PRIVATE_XFA_SENTINEL"),
        "Fields" => vec![Object::Dictionary(dictionary! { "FT" => "Tx", "T" => Object::string_literal("PRIVATE_FIELD_SENTINEL"),
            "V" => Object::string_literal("PRIVATE_VALUE_SENTINEL") }),
            Object::Dictionary(dictionary! { "FT" => "Btn", "Ff" => 32768 }),
            Object::Dictionary(dictionary! { "FT" => "Ch", "Ff" => 131072 })]
    });
    let layer = source.add_object(
        dictionary! { "Type" => "OCG", "Name" => Object::string_literal("PRIVATE_LAYER_SENTINEL") },
    );
    source.catalog_mut().unwrap().set(
        "OCProperties",
        dictionary! { "OCGs" => vec![Object::Reference(layer)] },
    );
    let doc = load(source);
    let before = doc.to_bytes().unwrap();
    let report = doc.inspect_security().unwrap();
    assert_eq!(
        report
            .actions
            .iter()
            .filter(|finding| finding.kind == ActionKind::JavaScript)
            .count(),
        3
    );
    assert_eq!(report.actions.len(), 7);
    assert!(report.actions.iter().any(
        |finding| finding.location.object_id == Some(hidden_js) && !finding.location.reachable
    ));
    assert!(
        report
            .actions
            .iter()
            .any(|finding| finding.location.object_id == Some(js) && finding.location.reachable)
    );
    assert_eq!(report.automatic_actions.len(), 2);
    assert_eq!(report.embedded_files.len(), 1);
    assert_eq!(
        report.embedded_files[0].filename.as_deref(),
        Some("fixture.txt")
    );
    assert_eq!(report.embedded_file_streams, 1);
    assert!(report.info_dictionary_present && report.xmp_metadata_present);
    assert_eq!(report.xmp_metadata_objects.len(), 1);
    assert!(report.acroform_present && report.xfa_present);
    assert_eq!(report.field_type_counts.get(&FieldType::Text), Some(&1));
    assert_eq!(
        report.field_type_counts.get(&FieldType::RadioButton),
        Some(&1)
    );
    assert_eq!(report.field_type_counts.get(&FieldType::ComboBox), Some(&1));
    assert_eq!(report.optional_content_groups, 1);
    assert!(!report.complete_privacy_proof);
    assert!(report.revision_analysis.starts_with("unavailable"));
    assert!(!format!("{report:?}").contains("PRIVATE_"));
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert_eq!(doc.inspect_security().unwrap(), report);
    assert!(report.actions.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn indirect_action_is_counted_once_despite_multiple_references() {
    let mut source = fixture();
    let page = *source.get_pages().get(&1).unwrap();
    let action = source
        .add_object(dictionary! { "S" => "URI", "URI" => Object::string_literal("private-url") });
    let annotation =
        source.add_object(dictionary! { "Type" => "Annot", "Subtype" => "Link", "A" => action });
    source.catalog_mut().unwrap().set("OpenAction", action);
    source
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![Object::Reference(annotation)]);
    let report = load(source).inspect_security().unwrap();
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].location.object_id, Some(action));
    assert!(report.actions[0].location.reachable);
}

#[test]
fn signed_documents_remain_inspectable_without_signature_trust_claims() {
    let mut source = fixture();
    source.catalog_mut().unwrap().set(
        "AcroForm",
        dictionary! {
            "Fields" => vec![Object::Dictionary(dictionary! { "FT" => "Sig", "Type" => "Sig" })]
        },
    );
    let doc = load(source);
    let report = doc.inspect_security().unwrap();
    assert_eq!(report.signature_markers, 1);
    assert_eq!(
        report.field_type_counts.get(&FieldType::Signature),
        Some(&1)
    );
    assert!(!report.complete_privacy_proof);
    assert!(matches!(doc.to_bytes(), Err(Error::Refused(_))));
}

#[test]
fn encrypted_permissions_are_reported_from_decoded_state_only() {
    let mut source = fixture();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &source,
        owner_password: "synthetic-owner",
        user_password: "synthetic-user",
        key_length: 128,
        permissions: Permissions::PRINTABLE | Permissions::COPYABLE,
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    let input = bytes(source);
    assert!(PdfDocument::from_bytes(&input, Some("wrong")).is_err());
    let doc = PdfDocument::from_bytes(&input, Some("synthetic-user")).unwrap();
    let report = doc.inspect_security().unwrap();
    assert!(report.encrypted_input);
    let permissions = report.encryption.unwrap();
    assert_eq!(permissions.version, 2);
    assert!(permissions.print && permissions.copy);
    assert!(!permissions.modify);
    assert_eq!(permissions.fill_forms, Some(false));
    assert_eq!(permissions.assemble, Some(false));
    let plain = load(fixture()).inspect_security().unwrap();
    assert!(!plain.encrypted_input && plain.encryption.is_none());
}

#[test]
fn revision_two_permissions_do_not_interpret_reserved_later_revision_bits() {
    let mut source = fixture();
    let state = EncryptionState::try_from(EncryptionVersion::V1 {
        document: &source,
        owner_password: "synthetic-owner",
        user_password: "synthetic-user",
        permissions: Permissions::PRINTABLE | Permissions::MODIFIABLE,
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    let doc = PdfDocument::from_bytes(&bytes(source), Some("synthetic-user")).unwrap();
    let permissions = doc.inspect_security().unwrap().encryption.unwrap();
    assert_eq!(permissions.revision, 2);
    assert!(permissions.print && permissions.modify);
    assert_eq!(permissions.fill_forms, None);
    assert_eq!(permissions.assemble, None);
    assert_eq!(permissions.high_quality_print, None);
}

#[test]
fn reference_cycles_missing_targets_and_oversized_filenames_fail_safely() {
    let mut source = fixture();
    let action = source.new_object_id();
    source
        .objects
        .insert(action, dictionary! { "S" => action }.into());
    let doc = load(source);
    assert!(doc.inspect_security().is_err());
    let mut source = fixture();
    source
        .catalog_mut()
        .unwrap()
        .set("OpenAction", Object::Reference((9999, 0)));
    assert!(load(source).inspect_security().is_err());
    let mut source = fixture();
    source.add_object(
        dictionary! { "EF" => dictionary! {}, "F" => Object::string_literal(vec![b'a'; 4097]) },
    );
    assert!(matches!(
        load(source).inspect_security(),
        Err(Error::Limit(_))
    ));
}

#[test]
fn trust_lab_forms_attachments_layers_and_signed_markers_are_real_evidence() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1");
    let forms = PdfDocument::open(corpus.join("forms.pdf"), None)
        .unwrap()
        .inspect_security()
        .unwrap();
    assert!(forms.acroform_present);
    assert!(!forms.field_type_counts.is_empty());
    let attachments = PdfDocument::open(corpus.join("attachment.pdf"), None)
        .unwrap()
        .inspect_security()
        .unwrap();
    assert!(!attachments.embedded_files.is_empty());
    assert!(attachments.embedded_file_streams > 0);
    let layers = PdfDocument::open(corpus.join("layers-transparency.pdf"), None)
        .unwrap()
        .inspect_security()
        .unwrap();
    assert!(layers.optional_content_groups > 0);
    let signed = PdfDocument::open(corpus.join("signed.pdf"), None)
        .unwrap()
        .inspect_security()
        .unwrap();
    assert!(signed.signature_markers > 0);
}

#[test]
fn empty_inventory_explicitly_does_not_prove_privacy_or_revision_absence() {
    let report = load(fixture()).inspect_security().unwrap();
    assert!(report.actions.is_empty() && report.embedded_files.is_empty());
    assert_eq!(report.signature_markers, 0);
    assert!(!report.complete_privacy_proof);
    assert!(report.revision_analysis.starts_with("unavailable"));
    assert_eq!(report.scope, "all_tracked_objects_including_unreachable");
    assert!(report.reachable_objects > 0);
}

#[test]
fn aggregate_filename_and_finding_budgets_prevent_report_amplification() {
    let mut source = fixture();
    let shared_name = source.add_object(Object::string_literal(vec![b'a'; 4096]));
    for _ in 0..300 {
        source.add_object(dictionary! { "EF" => dictionary! {}, "F" => shared_name });
    }
    let doc = load(source);
    let original = doc.to_bytes().unwrap();
    assert!(matches!(
        doc.inspect_security(),
        Err(Error::Limit("inspection filenames exceed 1 MiB total"))
    ));
    assert_eq!(doc.to_bytes().unwrap(), original);

    let mut source = fixture();
    source.add_object(Object::Array(vec![
        Object::Dictionary(
            dictionary! { "S" => "JavaScript" }
        );
        20_001
    ]));
    assert!(matches!(
        load(source).inspect_security(),
        Err(Error::Limit("inspection report exceeds 20000 findings"))
    ));
}
