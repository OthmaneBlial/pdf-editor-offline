#![allow(clippy::unwrap_used)]
#[path = "support/encryption.rs"]
mod encryption;
use pdf_worker::document::{DocumentResult, Operation, ReadOperation, execute_document};
use pdf_worker::{PayloadKind, Request};

fn run(operation: Operation, bytes: Vec<u8>) -> pdf_worker::Reply {
    pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(operation).unwrap(),
            payload: bytes,
        },
        pdf_worker::Limits::default(),
    )
    .unwrap()
}
fn unlock(cipher: &[u8], password: &str, read: ReadOperation) -> pdf_worker::Reply {
    let mut payload = cipher.to_vec();
    payload.extend_from_slice(password.as_bytes());
    run(
        Operation::Unlock {
            password_bytes_len: password.len() as u32,
            read,
        },
        payload,
    )
}
fn result(reply: &pdf_worker::Reply) -> DocumentResult {
    serde_json::from_value(reply.control.clone()).unwrap()
}

#[test]
fn actual_limited_child_opens_all_four_tiers_and_returns_only_read_only_view_data() {
    for revision in [2, 3, 4, 6] {
        let (cipher, plain) = encryption::fixture(revision, "synthetic-reader", false);
        let original = cipher.clone();
        let probe = run(Operation::Open, cipher.clone());
        assert!(matches!(
            result(&probe),
            DocumentResult::PasswordRequired { incorrect: false }
        ));
        assert!(probe.payload.is_empty());
        let wrong = unlock(&cipher, "wrong", ReadOperation::Info);
        assert!(matches!(
            result(&wrong),
            DocumentResult::PasswordRequired { incorrect: true }
        ));
        assert!(wrong.payload.is_empty());
        let info = unlock(&cipher, "synthetic-reader", ReadOperation::Info);
        assert!(matches!(result(&info), DocumentResult::Info { info } if info.encrypted));
        assert!(
            matches!(result(&unlock(&cipher, "synthetic-owner", ReadOperation::Info)), DocumentResult::Info { info } if info.encrypted)
        );
        let render = unlock(
            &cipher,
            "synthetic-reader",
            ReadOperation::Render {
                page: 0,
                scale: 1.0,
                logical_zoom: 1.0,
            },
        );
        assert_eq!(render.kind, PayloadKind::Rgba);
        assert_eq!(
            render.payload,
            pdf_render::render_page(&plain, 0, 1.0).unwrap().rgba
        );
        let forms = unlock(&cipher, "synthetic-reader", ReadOperation::Forms);
        assert_eq!(forms.kind, PayloadKind::None);
        assert!(forms.payload.is_empty());
        assert!(
            matches!(result(&forms), DocumentResult::Forms { fields, choices } if fields.iter().all(|field| !field.writable_button && !field.writable_text) && choices.iter().all(|field| !field.writable))
        );
        let text = unlock(&cipher, "synthetic-reader", ReadOperation::Text { page: 1 });
        assert!(
            matches!(result(&text), DocumentResult::Text { text, truncated: false } if text == pdf_core::PdfDocument::from_bytes(&plain, None).unwrap().extract_text(&[1]).unwrap())
        );
        assert!(
            unlock(&cipher, "synthetic-reader", ReadOperation::Metadata)
                .payload
                .is_empty()
        );
        assert_eq!(cipher, original);
    }
}

#[test]
fn empty_and_unicode_passwords_require_explicit_unlock_and_signed_inputs_stay_read_only() {
    for (revision, password, signed) in [
        (3, "", false),
        (6, "synthetic-é密", false),
        (4, "reader", true),
    ] {
        let (cipher, _) = encryption::fixture(revision, password, signed);
        assert!(matches!(
            result(&run(Operation::Open, cipher.clone())),
            DocumentResult::PasswordRequired { incorrect: false }
        ));
        let reply = unlock(&cipher, password, ReadOperation::Info);
        assert!(
            matches!(result(&reply), DocumentResult::Info { info } if info.encrypted && (info.signature_fields > 0) == signed)
        );
        let refused = run(Operation::PrepareSave, cipher);
        assert_eq!(refused.control["ok"], false);
        assert!(refused.payload.is_empty());
    }
}

#[test]
fn password_frames_are_bounded_strict_path_free_and_debug_redacted() {
    let (cipher, _) = encryption::fixture(3, "reader", false);
    for control in [
        serde_json::json!({"operation":"unlock","password_bytes_len":4097,"read":{"action":"info"}}),
        serde_json::json!({"operation":"unlock","password_bytes_len":100000,"read":{"action":"info"}}),
        serde_json::json!({"operation":"unlock","password_bytes_len":0,"read":{"action":"info","path":"private.pdf"}}),
        serde_json::json!({"operation":"unlock","password_bytes_len":0,"read":{"action":"prepare_save"}}),
        serde_json::json!({"operation":"unlock","password_bytes_len":0,"read":{"action":"edit","mutation":"rotate"}}),
        serde_json::json!({"operation":"unlock","password_bytes_len":0,"read":{"action":"info"},"password":"not allowed"}),
        serde_json::json!({"operation":"open","path":"private.pdf"}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: cipher.clone()
            })
            .is_err()
        );
    }
    let mut payload = cipher;
    payload.push(255);
    assert!(execute_document(Request { control: serde_json::json!({"operation":"unlock","password_bytes_len":1,"read":{"action":"info"}}), payload }).is_err());
    let request = Request {
        control: serde_json::json!({"password":"secret-in-control"}),
        payload: b"secret-in-payload".to_vec(),
    };
    assert!(!format!("{request:?}").contains("secret"));
    let reply = pdf_worker::Reply {
        control: request.control,
        payload: request.payload,
        kind: PayloadKind::None,
    };
    assert!(!format!("{reply:?}").contains("secret"));
}

#[test]
fn authenticated_note_discovery_uses_existing_eligibility_and_never_returns_writable_entries() {
    let mut source = pdf_core::PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/forms.pdf"),
        None,
    )
    .unwrap();
    source
        .add_note(
            1,
            [50.0, 50.0, 74.0, 74.0],
            "Public encrypted note Δ",
            "Fixture",
        )
        .unwrap();
    let (cipher, _) = encryption::fixture_from(&source.to_bytes().unwrap(), 4, "reader", false);
    let reply = unlock(
        &cipher,
        "reader",
        ReadOperation::Notes { page: 1, offset: 0 },
    );
    assert_eq!(reply.kind, PayloadKind::None);
    assert!(reply.payload.is_empty());
    let DocumentResult::Notes {
        entries,
        next_offset: None,
    } = result(&reply)
    else {
        panic!("bounded notes report");
    };
    assert!(
        entries
            .iter()
            .any(|entry| entry.note.contents == "Public encrypted note Δ"
                && entry.note.author == "Fixture")
    );
    assert!(
        entries
            .iter()
            .all(|entry| !entry.editable && entry.refusal_reason.is_some())
    );
}
