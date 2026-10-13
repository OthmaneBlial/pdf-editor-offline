#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/split_fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../tests/support/image_removal_fixture.rs"]
mod fixture;
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Operation, execute_document},
};
fn request(operation: Operation, payload: &[u8]) -> Request {
    Request {
        control: serde_json::to_value(operation).unwrap(),
        payload: payload.to_vec(),
    }
}
#[test]
fn actual_child_removes_images_and_verifies_exact_output_in_path_free_frames() {
    let source = base::fixture(2);
    let selections = fixture::selections(&source);
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request(
            Operation::RedactImages {
                selections: selections.clone(),
            },
            &source,
        ),
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::ImagesRedacted { report, .. } =
        serde_json::from_value(reply.control).unwrap()
    else {
        panic!("image report")
    };
    assert_eq!(report.removed_draws.len(), 2);
    let verify = |output: &[u8]| {
        let mut frame = source.clone();
        frame.extend_from_slice(output);
        execute_document(request(
            Operation::VerifyImageRedaction {
                source_bytes_len: source.len(),
                selections: selections.clone(),
            },
            &frame,
        ))
    };
    let verification = verify(&reply.payload).unwrap();
    assert_eq!(verification.kind, PayloadKind::None);
    assert!(matches!(
        serde_json::from_value::<DocumentResult>(verification.control).unwrap(),
        DocumentResult::ImagesRedactionVerified {}
    ));
    assert!(verify(&source).is_err());
    let mut extra = reply.payload.clone();
    extra.extend_from_slice(b"\n% retained extra bytes\n");
    assert!(verify(&extra).is_err());
    for len in [0, source.len() * 2 + 1] {
        assert!(
            execute_document(request(
                Operation::VerifyImageRedaction {
                    source_bytes_len: len,
                    selections: selections.clone()
                },
                &source
            ))
            .is_err()
        );
    }
    for control in [
        serde_json::json!({"operation":"redact_images","selections":selections,"path":"forbidden"}),
        serde_json::json!({"operation":"images_redaction_verified","path":"forbidden"}),
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
    assert!(execute_document(request(Operation::RedactImages { selections }, signed)).is_err());
}
