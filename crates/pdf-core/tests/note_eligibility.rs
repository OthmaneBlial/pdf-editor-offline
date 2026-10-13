#![allow(clippy::unwrap_used)]
use pdf_core::{Note, PdfDocument};
#[test]
fn read_only_eligibility_matches_real_updates_and_never_mutates_source() {
    let mut doc = PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/bookmarks.pdf"),
        None,
    )
    .unwrap();
    let note = doc
        .add_note(1, [30.0, 30.0, 54.0, 54.0], "Comment", "Reviewer")
        .unwrap();
    let before = doc.to_bytes().unwrap();
    doc.note_editability(1, note.annotation_index).unwrap();
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc.note_editability(1, 0).is_err());
    assert!(doc.note_editability(2, note.annotation_index).is_err());
    let mut alias = lopdf::Document::load_mem(&before).unwrap();
    alias
        .catalog_mut()
        .unwrap()
        .set("ExternalNote", note.object_id.unwrap());
    let mut bytes = vec![];
    alias.save_to(&mut bytes).unwrap();
    let mut alias = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(alias.note_editability(1, note.annotation_index).is_err());
    assert!(
        alias
            .edit_note(1, note.annotation_index, Some("changed"), None)
            .is_err()
    );
    assert_eq!(alias.list_notes(&[1]).unwrap()[0].contents, "Comment");
    doc.edit_note(1, note.annotation_index, Some("changed"), None)
        .unwrap();
    doc.note_editability(1, note.annotation_index).unwrap();
    doc.remove_note(1, note.annotation_index).unwrap();
    assert!(doc.list_notes(&[1]).unwrap().is_empty());
}
#[test]
fn note_reply_codec_rejects_unknown_fields_and_roundtrips_identity() {
    let note = Note {
        page: 1,
        annotation_index: 2,
        object_id: Some((7, 0)),
        rectangle: [1.0, 2.0, 3.0, 4.0],
        contents: "中文 😀".into(),
        author: "Reviewer".into(),
        flags: 4,
    };
    let mut json = serde_json::to_value(&note).unwrap();
    assert_eq!(serde_json::from_value::<Note>(json.clone()).unwrap(), note);
    json["path"] = serde_json::json!("private.pdf");
    assert!(serde_json::from_value::<Note>(json).is_err());
}
