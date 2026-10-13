#![allow(clippy::unwrap_used)]
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Operation, execute_document},
};
#[path = "../../../tests/support/redaction_fixture.rs"]
mod fixture;
fn request(operation: Operation, bytes: &[u8]) -> Request {
    Request {
        control: serde_json::to_value(operation).unwrap(),
        payload: bytes.to_vec(),
    }
}
fn targets() -> pdf_core::TextRedactionRequest {
    pdf_core::TextRedactionRequest {
        targets: vec![fixture::SECRET.into()],
    }
}
#[test]
fn production_child_returns_verifiable_redacted_copy_with_path_free_bounded_request() {
    let source = fixture::fixture();
    let operation = Operation::RedactText { request: targets() };
    assert!(!format!("{operation:?}").contains(fixture::SECRET));
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request(operation, &source),
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Redacted { info, report } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("redaction report")
    };
    assert_eq!(report.changed_text_shows, 2);
    assert_eq!(
        info.pages,
        pdf_core::PdfDocument::from_bytes(&source, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
    pdf_core::verify_redaction_bytes(&reply.payload, None, &targets().targets).unwrap();
    let verified = execute_document(request(
        Operation::VerifyRedaction { request: targets() },
        &reply.payload,
    ))
    .unwrap();
    assert_eq!(verified.kind, PayloadKind::None);
    assert!(matches!(
        serde_json::from_value::<DocumentResult>(verified.control).unwrap(),
        DocumentResult::RedactionVerified {}
    ));
    assert!(
        execute_document(request(
            Operation::VerifyRedaction { request: targets() },
            &source
        ))
        .is_err()
    );
    for control in [
        serde_json::json!({"operation":"redact_text","request":{"targets":["PRIVATE"],"path":"bad"}}),
        serde_json::json!({"operation":"redact_text","request":{"targets":["PRIVATE"]},"path":"bad"}),
        serde_json::json!({"operation":"verify_redaction","request":{"targets":[]}}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: source.clone()
            })
            .is_err()
        );
    }
    let signed = include_bytes!("../../../tests/fixtures/pdfs/signed.pdf");
    assert!(
        serde_json::from_value::<DocumentResult>(
            serde_json::json!({"result":"redaction_verified","path":"bad"})
        )
        .is_err()
    );
    assert!(
        execute_document(request(
            Operation::RedactText { request: targets() },
            signed
        ))
        .is_err()
    );
}
