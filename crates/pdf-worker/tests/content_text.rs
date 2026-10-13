#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/content_fixture.rs"]
mod fixture;
use pdf_worker::document::{DocumentResult, Mutation, Operation, execute_document};
use pdf_worker::{Limits, PayloadKind, Reply, Request};
fn execute(operation: Operation, bytes: &[u8]) -> Reply {
    pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(operation).unwrap(),
            payload: bytes.to_vec(),
        },
        Limits::default(),
    )
    .unwrap()
}
fn inventory(bytes: &[u8], offset: u32) -> pdf_worker::document::ContentTextPage {
    let reply = execute(Operation::ContentText { page: 1, offset }, bytes);
    assert_eq!(reply.kind, PayloadKind::None);
    assert!(reply.payload.is_empty());
    let DocumentResult::ContentText { inventory } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("actual inventory")
    };
    inventory
}
#[test]
fn real_child_proportional_width_policy_is_explicit_strict_and_atomic() {
    let bytes = fixture::source_with_font(fixture::CONTENT, "Helvetica");
    let read = inventory(&bytes, 0);
    let mutation = |policy| Operation::Edit {
        mutation: Mutation::ReplaceContentText {
            page: 1,
            source_fingerprint: read.source_fingerprint,
            changes: vec![pdf_core::ContentTextChange {
                layout_policy: policy,
                identity: read.runs[0].identity.clone(),
                expected: "Original".into(),
                replacement: "Longer text".into(),
            }],
        },
    };
    let rejected = execute(mutation(Default::default()), &bytes);
    assert!(rejected.payload.is_empty());
    assert_eq!(rejected.control["ok"], false);
    let approved = execute(
        mutation(pdf_core::ContentTextLayoutPolicy::AllowWidthChange),
        &bytes,
    );
    assert_eq!(approved.kind, PayloadKind::Pdf);
    let pdf = pdf_core::PdfDocument::from_bytes(&approved.payload, None).unwrap();
    assert_eq!(
        pdf.extract_text(&[1])
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["Longer", "text", "A", "B"]
    );
    assert_eq!(
        pdf.extract_text(&[2])
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["Original", "A", "B"]
    );
    let mut control = serde_json::to_value(mutation(Default::default())).unwrap();
    control["mutation"]["changes"][0]["layout_policy"] = serde_json::json!("guess_or_fit");
    assert!(
        execute_document(Request {
            control,
            payload: bytes.clone()
        })
        .is_err()
    );
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap(),
        pdf_render::render_page(&approved.payload, 1, 1.0).unwrap()
    );
}
#[test]
fn production_child_replaces_original_operand_and_preserves_shared_page() {
    let bytes = fixture::source(fixture::CONTENT);
    let read = inventory(&bytes, 0);
    assert_eq!(
        read.source_fingerprint,
        pdf_worker::payload_fingerprint(&bytes)
    );
    let run = &read.runs[0];
    assert!(run.writable);
    let reply = execute(
        Operation::Edit {
            mutation: Mutation::ReplaceContentText {
                page: 1,
                source_fingerprint: read.source_fingerprint,
                changes: vec![pdf_core::ContentTextChange {
                    layout_policy: Default::default(),
                    identity: run.identity.clone(),
                    expected: "Original".into(),
                    replacement: "Replaced".into(),
                }],
            },
        },
        &bytes,
    );
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let output = pdf_core::PdfDocument::from_bytes(&reply.payload, None).unwrap();
    assert_eq!(output.extract_text(&[1]).unwrap(), "Replaced\nA B\n");
    assert_eq!(output.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap().rgba,
        pdf_render::render_page(&reply.payload, 1, 1.0)
            .unwrap()
            .rgba
    );
    let before = lopdf::Document::load_mem(&bytes).unwrap();
    let after = lopdf::Document::load_mem(&reply.payload).unwrap();
    assert_ne!(
        before
            .get_dictionary((3, 0))
            .unwrap()
            .get(b"Contents")
            .unwrap(),
        after
            .get_dictionary((3, 0))
            .unwrap()
            .get(b"Contents")
            .unwrap()
    );
    assert_eq!(
        before.get_dictionary((4, 0)).unwrap(),
        after.get_dictionary((4, 0)).unwrap()
    );
}
#[test]
fn exact_serialized_pagination_progresses_without_truncating_escaped_values() {
    let value = "(\\)[]{}?".repeat(512);
    let mut content = String::new();
    for _ in 0..300 {
        let literal = lopdf::content::Content {
            operations: vec![lopdf::content::Operation::new(
                "Tj",
                vec![lopdf::Object::string_literal(value.clone())],
            )],
        }
        .encode()
        .unwrap();
        content.push_str("BT /F 12 Tf 1 0 0 1 10 10 Tm 0 Tr ");
        content.push_str(std::str::from_utf8(&literal).unwrap());
        content.push_str(" ET\n");
    }
    let bytes = fixture::source(content.as_bytes());
    let mut offset = 0;
    let mut count = 0;
    loop {
        let page = inventory(&bytes, offset);
        assert!(page.runs.len() <= 128);
        assert!(serde_json::to_vec(&page).unwrap().len() <= 1024 * 1024);
        assert_eq!(page.total_runs, 300);
        for run in &page.runs {
            assert_eq!(run.text.as_deref(), Some(value.as_str()));
        }
        count += page.runs.len();
        if let Some(next) = page.next_offset {
            assert!(next > offset);
            offset = next;
        } else {
            break;
        }
    }
    assert_eq!(count, 300);
}
#[test]
fn forged_stale_wrong_page_and_nested_unknown_fields_return_no_pdf() {
    let bytes = fixture::source(fixture::CONTENT);
    let read = inventory(&bytes, 0);
    for scenario in 0..4 {
        let mut identity = read.runs[0].identity.clone();
        let mut hash = read.source_fingerprint;
        let mut expected = "Original".to_string();
        let page = if scenario == 2 { 2 } else { 1 };
        if scenario == 0 {
            hash[0] ^= 1;
        }
        if scenario == 1 {
            identity.stream_digest[0] ^= 1;
        }
        if scenario == 3 {
            expected = "Not same".into();
        }
        let request = Request {
            control: serde_json::to_value(Operation::Edit {
                mutation: Mutation::ReplaceContentText {
                    page,
                    source_fingerprint: hash,
                    changes: vec![pdf_core::ContentTextChange {
                        layout_policy: Default::default(),
                        identity,
                        expected,
                        replacement: "Replaced".into(),
                    }],
                },
            })
            .unwrap(),
            payload: bytes.clone(),
        };
        assert!(execute_document(request).is_err());
    }
    let mut control = serde_json::to_value(Operation::ContentText { page: 1, offset: 0 }).unwrap();
    control["path"] = serde_json::json!("not-opened.pdf");
    assert!(
        execute_document(Request {
            control,
            payload: bytes.clone()
        })
        .is_err()
    );
    let mut control = serde_json::to_value(Operation::Edit {
        mutation: Mutation::ReplaceContentText {
            page: 1,
            source_fingerprint: read.source_fingerprint,
            changes: vec![pdf_core::ContentTextChange {
                layout_policy: Default::default(),
                identity: read.runs[0].identity.clone(),
                expected: "Original".into(),
                replacement: "Replaced".into(),
            }],
        },
    })
    .unwrap();
    control["mutation"]["changes"][0]["identity"]["path"] = serde_json::json!("never-opened.pdf");
    assert!(
        execute_document(Request {
            control,
            payload: bytes.clone()
        })
        .is_err()
    );
    let bad = execute(
        Operation::ContentText {
            page: 1,
            offset: 999,
        },
        &bytes,
    );
    assert_eq!(bad.kind, PayloadKind::None);
    assert!(bad.payload.is_empty());
    assert_eq!(bad.control["ok"], false);
}
