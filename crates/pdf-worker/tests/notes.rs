#![allow(clippy::unwrap_used)]
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Mutation, NoteChange, Operation, execute_document},
};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/bookmarks.pdf");
fn child(operation: Operation, payload: Vec<u8>) -> pdf_worker::Reply {
    pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(operation).unwrap(),
            payload,
        },
        pdf_worker::Limits::default(),
    )
    .unwrap()
}
#[test]
fn production_note_child_discovers_edits_removes_and_preserves_appearance_and_source() {
    let add = child(
        Operation::Edit {
            mutation: Mutation::Note {
                change: NoteChange::Add {
                    page: 1,
                    rectangle: [30.0, 30.0, 54.0, 54.0],
                    contents: "Review 中文 😀".into(),
                    author: "Reviewer".into(),
                },
            },
        },
        SOURCE.to_vec(),
    );
    assert_eq!(add.kind, PayloadKind::Pdf);
    let listed = child(Operation::Notes { page: 1, offset: 0 }, add.payload.clone());
    let DocumentResult::Notes {
        entries,
        next_offset,
    } = serde_json::from_value(listed.control).unwrap()
    else {
        panic!("notes");
    };
    assert_eq!(entries.len(), 1);
    assert!(entries[0].editable);
    assert!(next_offset.is_none());
    let note = &entries[0].note;
    let before = pdf_render::render_page(&add.payload, 0, 1.0).unwrap();
    let edited = child(
        Operation::Edit {
            mutation: Mutation::Note {
                change: NoteChange::Edit {
                    page: 1,
                    annotation_index: note.annotation_index,
                    expected_id: note.object_id,
                    contents: Some("Updated العربية".into()),
                    author: None,
                },
            },
        },
        add.payload.clone(),
    );
    assert_eq!(
        before.rgba,
        pdf_render::render_page(&edited.payload, 0, 1.0)
            .unwrap()
            .rgba
    );
    let doc = pdf_core::PdfDocument::from_bytes(&edited.payload, None).unwrap();
    let note = doc.list_notes(&[1]).unwrap().remove(0);
    assert_eq!(note.contents, "Updated العربية");
    assert_eq!(
        doc.extract_text(&[1]).unwrap().trim(),
        pdf_core::PdfDocument::from_bytes(SOURCE, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .trim()
    );
    let removed = child(
        Operation::Edit {
            mutation: Mutation::Note {
                change: NoteChange::Remove {
                    page: 1,
                    annotation_index: note.annotation_index,
                    expected_id: note.object_id,
                },
            },
        },
        edited.payload,
    );
    assert!(
        pdf_core::PdfDocument::from_bytes(&removed.payload, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        SOURCE,
        include_bytes!("../../../tests/fixtures/pdfs/bookmarks.pdf")
    );
    assert!(execute_document(Request{control:serde_json::json!({"operation":"edit","mutation":{"mutation":"note","change":{"change":"remove","page":1,"annotation_index":note.annotation_index,"expected_id":[999,0]}}}),payload:add.payload}).is_err());
}
#[test]
fn paths_bad_offsets_limits_and_signed_mutations_refuse() {
    for control in [
        serde_json::json!({"operation":"notes","page":1,"offset":0,"path":"private.pdf"}),
        serde_json::json!({"operation":"notes","page":1,"offset":u32::MAX}),
        serde_json::json!({"operation":"edit","mutation":{"mutation":"note","change":{"change":"add","page":1,"rectangle":[1,2,3,4],"contents":"a","author":"b","path":"private.pdf"}}}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: SOURCE.to_vec()
            })
            .is_err()
        );
    }
    assert!(
        execute_document(Request {
            control: serde_json::to_value(Operation::Edit {
                mutation: Mutation::Note {
                    change: NoteChange::Add {
                        page: 1,
                        rectangle: [30.0, 30.0, 54.0, 54.0],
                        contents: "x".into(),
                        author: "y".into()
                    }
                }
            })
            .unwrap(),
            payload: include_bytes!("../../../tests/fixtures/pdfs/signed.pdf").to_vec()
        })
        .is_err()
    );
}
