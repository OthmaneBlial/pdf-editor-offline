#![allow(clippy::unwrap_used)]

use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use pdf_core::{Error, PdfDocument};

fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let stream = doc.add_object(Stream::new(
        dictionary! {},
        b"q 1 0 0 rg 20 30 20 20 re f Q".to_vec(),
    ));
    let first = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream});
    let second = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream});
    doc.objects.insert(pages,dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>vec![first.into(),second.into()],"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary! {}}.into());
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal("note-edit-first!"),
            Object::string_literal("note-edit-next!!"),
        ],
    );
    doc
}
fn note() -> Object {
    dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![50.into(),60.into(),74.into(),84.into()],"Contents"=>Object::string_literal("OLD"),"T"=>Object::string_literal("Original"),"F"=>2,"Name"=>"Note"}.into()
}
fn bytes(mut doc: Document) -> Vec<u8> {
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn load(doc: Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(doc), None).unwrap()
}
fn with_note() -> (Document, lopdf::ObjectId) {
    let mut doc = fixture();
    let id = doc.add_object(note());
    let first = doc.get_pages()[&1];
    doc.get_dictionary_mut(first)
        .unwrap()
        .set("Annots", vec![id.into()]);
    (doc, id)
}

#[test]
fn edited_unicode_round_trips_and_keeps_appearance_rectangle_flags_and_original_object() {
    let mut document = load(fixture());
    let original = document
        .add_note(1, [50., 60., 74., 84.], "OLD", "Original")
        .unwrap();
    let before = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    let old_id = original.object_id.unwrap();
    let edited = document
        .edit_note(1, 1, Some("café 中文 😀"), Some("Élodie"))
        .unwrap();
    assert_ne!(edited.object_id, original.object_id);
    assert_eq!(edited.annotation_index, 1);
    assert_eq!(edited.contents, "café 中文 😀");
    assert_eq!(edited.author, "Élodie");
    assert_eq!(edited.rectangle, original.rectangle);
    assert_eq!(edited.flags, original.flags);
    let after = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(after.objects.get(&old_id), before.objects.get(&old_id));
    let edited_dict = after.get_dictionary(edited.object_id.unwrap()).unwrap();
    let original_dict = before.get_dictionary(old_id).unwrap();
    for key in [b"AP".as_slice(), b"Rect", b"F", b"Name", b"Open"] {
        assert_eq!(
            edited_dict.get(key).unwrap(),
            original_dict.get(key).unwrap()
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("edited.pdf");
    document.save_new(&output).unwrap();
    assert_eq!(
        PdfDocument::open(&output, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap(),
        vec![edited]
    );
}

#[test]
fn direct_and_indirect_notes_shared_arrays_and_duplicate_occurrences_only_change_selected_slot() {
    for direct in [false, true] {
        let mut doc = fixture();
        let pages = doc.get_pages();
        let target = if direct {
            note()
        } else {
            doc.add_object(note()).into()
        };
        let link=doc.add_object(dictionary! {"Subtype"=>"Link","Rect"=>vec![10.into(),10.into(),20.into(),20.into()],"Dest"=>vec![pages[&2].into(),"Fit".into()]});
        let widget = dictionary! {"Subtype"=>"Widget","Rect"=>vec![10.into(),10.into(),20.into(),20.into()],"FT"=>"Tx"};
        let shared = doc.add_object(Object::Array(vec![
            link.into(),
            widget.into(),
            target.clone(),
            target,
        ]));
        for page in pages.values() {
            doc.get_dictionary_mut(*page).unwrap().set("Annots", shared);
        }
        let before = bytes(doc.clone());
        let mut document = PdfDocument::from_bytes(&before, None).unwrap();
        let old = document.list_notes(&[1, 2]).unwrap();
        assert_eq!(old[0].annotation_index, 3);
        assert_eq!(old[1].annotation_index, 4);
        document.edit_note(1, 3, Some("NEW"), None).unwrap();
        let notes = document.list_notes(&[1, 2]).unwrap();
        assert_eq!(notes[0].contents, "NEW");
        assert_eq!(notes[1].contents, "OLD");
        assert_eq!(notes[2].contents, "OLD");
        assert_eq!(notes[3].contents, "OLD");
        let after = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(after.objects.get(&shared), doc.objects.get(&shared));
        assert_eq!(after.objects.get(&pages[&2]), doc.objects.get(&pages[&2]));
        assert_eq!(after.objects.get(&link), doc.objects.get(&link));
        let removed = document.remove_note(1, 3).unwrap();
        assert_eq!(removed.contents, "NEW");
        let notes = document.list_notes(&[1, 2]).unwrap();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].annotation_index, 3);
        assert!(notes.iter().all(|note| note.contents == "OLD"));
        assert_eq!(bytes(doc), before);
    }
}

#[test]
fn removal_keeps_old_values_and_other_pages_without_secure_erasure() {
    let (mut doc, id) = with_note();
    let pages = doc.get_pages();
    doc.get_dictionary_mut(pages[&2])
        .unwrap()
        .set("Annots", vec![id.into()]);
    let mut document = load(doc.clone());
    document.remove_note(1, 1).unwrap();
    assert!(document.list_notes(&[1]).unwrap().is_empty());
    assert_eq!(document.list_notes(&[2]).unwrap()[0].contents, "OLD");
    let after = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(after.objects.get(&id), doc.objects.get(&id));
    assert_eq!(after.objects.get(&pages[&2]), doc.objects.get(&pages[&2]));
}

#[test]
fn sequential_edits_after_reopen_ignore_harmless_orphan_annotation_containers() {
    for first_edit in [false, true] {
        let (mut doc, first) = with_note();
        let second = doc.add_object(note());
        let page = doc.get_pages()[&1];
        let old_array = doc.add_object(Object::Array(vec![first.into(), second.into()]));
        doc.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", old_array);
        let mut document = load(doc.clone());
        if first_edit {
            document.edit_note(1, 1, Some("FIRST"), None).unwrap();
        } else {
            document.remove_note(1, 1).unwrap();
        }
        let intermediate = document.to_bytes().unwrap();
        let index = if first_edit { 2 } else { 1 };
        for second_edit in [false, true] {
            let mut document = PdfDocument::from_bytes(&intermediate, None).unwrap();
            if second_edit {
                document.edit_note(1, index, Some("SECOND"), None).unwrap();
                assert_eq!(
                    document.list_notes(&[1]).unwrap().last().unwrap().contents,
                    "SECOND"
                );
            } else {
                document.remove_note(1, index).unwrap();
                assert_eq!(
                    document.list_notes(&[1]).unwrap().len(),
                    usize::from(first_edit)
                );
            }
            let output = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
            for id in [first, second, old_array] {
                assert_eq!(output.objects.get(&id), doc.objects.get(&id));
            }
        }
    }
}

#[test]
fn an_external_live_link_to_a_retained_old_container_still_refuses_second_edit() {
    let (mut doc, first) = with_note();
    let second = doc.add_object(note());
    let page = doc.get_pages()[&1];
    let array = doc.add_object(Object::Array(vec![first.into(), second.into()]));
    doc.get_dictionary_mut(page).unwrap().set("Annots", array);
    let mut document = load(doc);
    document.remove_note(1, 1).unwrap();
    let mut intermediate = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    let alias = intermediate.add_object(Object::Reference(array));
    let root = intermediate
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    intermediate
        .get_dictionary_mut(root)
        .unwrap()
        .set("ExternalOldContainerLink", alias);
    let mut document = load(intermediate);
    let before = document.to_bytes().unwrap();
    assert!(matches!(
        document.edit_note(1, 1, Some("SECOND"), None),
        Err(Error::Refused(_))
    ));
    assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn linked_locked_named_tagged_and_custom_targets_are_refused_without_partial_edits() {
    for key in [
        "Popup",
        "IRT",
        "RT",
        "Parent",
        "StructParent",
        "NM",
        "RC",
        "A",
        "AA",
        "Custom",
    ] {
        let (mut doc, id) = with_note();
        doc.get_dictionary_mut(id).unwrap().set(key, Object::Null);
        let mut document = load(doc);
        let before = document.to_bytes().unwrap();
        assert!(
            matches!(
                document.edit_note(1, 1, Some("NEW"), None),
                Err(Error::Refused(_))
            ),
            "{key}"
        );
        assert!(
            matches!(document.remove_note(1, 1), Err(Error::Refused(_))),
            "{key}"
        );
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    for flag in [64, 128, 512] {
        let (mut doc, id) = with_note();
        doc.get_dictionary_mut(id).unwrap().set("F", flag);
        let mut document = load(doc);
        let before = document.to_bytes().unwrap();
        assert!(matches!(
            document.edit_note(1, 1, Some("NEW"), None),
            Err(Error::Refused(_))
        ));
        assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
        assert_eq!(document.to_bytes().unwrap(), before);
    }
}

#[test]
fn incoming_reply_external_and_alias_chain_references_cannot_bypass_the_guard() {
    for alias in [false, true] {
        let (mut doc, id) = with_note();
        let target = if alias {
            let first = doc.add_object(Object::Reference(id));
            doc.add_object(Object::Reference(first))
        } else {
            id
        };
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(root)
            .unwrap()
            .set("ExternalNoteLink", target);
        let mut document = load(doc);
        let before = document.to_bytes().unwrap();
        assert!(matches!(
            document.edit_note(1, 1, Some("NEW"), None),
            Err(Error::Refused(_))
        ));
        assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    let (mut doc, id) = with_note();
    let page = doc.get_pages()[&1];
    doc.get_dictionary_mut(page).unwrap().set("Annots",vec![id.into(),dictionary! {"Subtype"=>"Text","Rect"=>vec![50.into(),60.into(),74.into(),84.into()],"Contents"=>Object::string_literal("REPLY"),"IRT"=>id}.into()]);
    let mut document = load(doc);
    let before = document.to_bytes().unwrap();
    assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn page_occurrences_through_alias_chains_are_supported_and_new_owner_is_selected_page() {
    let (mut doc, id) = with_note();
    let pages = doc.get_pages();
    doc.get_dictionary_mut(id).unwrap().set("P", pages[&1]);
    let first_alias = doc.add_object(Object::Reference(id));
    let second_alias = doc.add_object(Object::Reference(first_alias));
    let array = doc.add_object(Object::Array(vec![second_alias.into()]));
    let array_alias = doc.add_object(Object::Reference(array));
    for page in pages.values() {
        doc.get_dictionary_mut(*page)
            .unwrap()
            .set("Annots", array_alias);
    }
    let mut document = load(doc.clone());
    let edited = document.edit_note(2, 1, None, Some("New author")).unwrap();
    assert_eq!(document.list_notes(&[1]).unwrap()[0].author, "Original");
    assert_eq!(document.list_notes(&[2]).unwrap()[0].author, "New author");
    let output = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(
        output
            .get_dictionary(edited.object_id.unwrap())
            .unwrap()
            .get(b"P")
            .unwrap()
            .as_reference()
            .unwrap(),
        pages[&2]
    );
    for original in [id, first_alias, second_alias, array, array_alias, pages[&1]] {
        assert_eq!(output.objects.get(&original), doc.objects.get(&original));
    }
}

#[test]
fn external_references_to_shared_annotation_container_aliases_are_refused() {
    for direct in [false, true] {
        let (mut doc, id) = with_note();
        let pages = doc.get_pages();
        let entry = if direct { note() } else { id.into() };
        let array = doc.add_object(Object::Array(vec![entry]));
        let alias = doc.add_object(Object::Reference(array));
        for page in pages.values() {
            doc.get_dictionary_mut(*page).unwrap().set("Annots", alias);
        }
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(root)
            .unwrap()
            .set("ExternalContainerLink", alias);
        let mut document = load(doc);
        let before = document.to_bytes().unwrap();
        assert!(matches!(
            document.edit_note(1, 1, Some("NEW"), None),
            Err(Error::Refused(_))
        ));
        assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
        assert_eq!(document.to_bytes().unwrap(), before);
    }
}

#[test]
fn validation_limits_non_text_and_bad_index_are_transactional() {
    let (doc, _) = with_note();
    let mut document = load(doc);
    let before = document.to_bytes().unwrap();
    assert!(document.edit_note(1, 1, None, None).is_err());
    assert!(document.edit_note(1, 1, Some("NUL\0value"), None).is_err());
    assert!(
        document
            .edit_note(1, 1, Some(&"x".repeat(65537)), None)
            .is_err()
    );
    assert!(
        document
            .edit_note(1, 1, None, Some(&"x".repeat(1025)))
            .is_err()
    );
    for (page, index) in [(0, 1), (3, 1), (1, 0), (1, 2)] {
        assert!(document.edit_note(page, index, Some("NEW"), None).is_err());
        assert!(document.remove_note(page, index).is_err());
    }
    assert_eq!(document.to_bytes().unwrap(), before);
    let (mut doc, id) = with_note();
    doc.get_dictionary_mut(id).unwrap().set("Subtype", "Link");
    let mut document = load(doc);
    let before = document.to_bytes().unwrap();
    assert!(matches!(
        document.edit_note(1, 1, Some("NEW"), None),
        Err(Error::Refused(_))
    ));
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn direct_payload_and_numeric_array_copy_growth_is_refused_before_mutation() {
    for object in [
        Object::string_literal("x".repeat(1024 * 1024)),
        Object::Array(vec![Object::Integer(0); 100001]),
    ] {
        let (mut doc, _) = with_note();
        let page = doc.get_pages()[&1];
        let target = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .clone();
        doc.get_dictionary_mut(page).unwrap().set(
            "Annots",
            vec![
                target,
                dictionary! {"Subtype"=>"Link","LargeCustom"=>object}.into(),
            ],
        );
        let mut document = load(doc);
        let before = document.to_bytes().unwrap();
        assert!(matches!(
            document.edit_note(1, 1, Some("NEW"), None),
            Err(Error::Limit(_))
        ));
        assert!(matches!(document.remove_note(1, 1), Err(Error::Limit(_))));
        assert_eq!(document.to_bytes().unwrap(), before);
    }
}

#[test]
fn signed_and_decrypted_notes_stay_read_only() {
    let (mut doc, _) = with_note();
    doc.add_object(dictionary! {"FT"=>"Sig"});
    let mut document = load(doc);
    assert_eq!(document.list_notes(&[1]).unwrap()[0].contents, "OLD");
    assert!(matches!(
        document.edit_note(1, 1, Some("NEW"), None),
        Err(Error::Refused(_))
    ));
    assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
    let (mut doc, _) = with_note();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &doc,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes(doc), Some("reader")).unwrap();
    assert_eq!(document.list_notes(&[1]).unwrap()[0].contents, "OLD");
    assert!(matches!(
        document.edit_note(1, 1, Some("NEW"), None),
        Err(Error::Refused(_))
    ));
    assert!(matches!(document.remove_note(1, 1), Err(Error::Refused(_))));
}
