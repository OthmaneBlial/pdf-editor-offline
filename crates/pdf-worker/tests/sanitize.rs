#![allow(clippy::unwrap_used)]
use pdf_core::PdfDocument;
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Operation, execute_document},
};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/attachment.pdf");
#[test]
fn production_child_returns_verified_targeted_copy_without_source_or_paths() {
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(Operation::Sanitize).unwrap(),
            payload: SOURCE.to_vec(),
        },
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Sanitized { info, report } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("sanitized result");
    };
    assert!(report.full_rewrite);
    assert!(report.embedded_file_name_trees_removed > 0);
    assert!(!report.limitations.is_empty());
    let clean = PdfDocument::from_bytes(&reply.payload, None).unwrap();
    assert_eq!(clean.info().unwrap().pages, info.pages);
    assert_eq!(
        clean.info().unwrap().pages,
        PdfDocument::from_bytes(SOURCE, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
    assert!(clean.inspect_security().unwrap().embedded_files.is_empty());
    assert!(
        execute_document(Request {
            control: serde_json::json!({"operation":"sanitize","path":"source.pdf"}),
            payload: SOURCE.to_vec()
        })
        .is_err()
    );
}
#[test]
fn signed_input_and_unknown_report_fields_refuse() {
    assert!(
        execute_document(Request {
            control: serde_json::to_value(Operation::Sanitize).unwrap(),
            payload: include_bytes!("../../../tests/fixtures/pdfs/signed.pdf").to_vec()
        })
        .is_err()
    );
    let reply = execute_document(Request {
        control: serde_json::to_value(Operation::Sanitize).unwrap(),
        payload: SOURCE.to_vec(),
    })
    .unwrap();
    let mut value = reply.control;
    value["report"]["private_path"] = serde_json::json!("payload.pdf");
    assert!(serde_json::from_value::<DocumentResult>(value).is_err());
}
