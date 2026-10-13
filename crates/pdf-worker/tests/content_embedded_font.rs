#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/content_fixture.rs"]
mod fixture;
use pdf_worker::document::{DocumentResult, Mutation, Operation, execute_document};
use pdf_worker::{Limits, PayloadKind, Request};
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn request(pdf: &[u8], font: &[u8], ack: bool) -> Request {
    let run = pdf_core::PdfDocument::from_bytes(pdf, None)
        .unwrap()
        .content_text_runs(1)
        .unwrap()
        .remove(0);
    Request {
        control: serde_json::to_value(Operation::Edit {
            mutation: Mutation::ContentTextEmbeddedFont {
                page: 1,
                source_fingerprint: pdf_worker::payload_fingerprint(pdf),
                font_bytes_len: font.len() as u32,
                change: pdf_core::ContentTextEmbeddedChange {
                    identity: run.identity,
                    expected: "Original".into(),
                    replacement: Some("AV Café ΩЖ".into()),
                    layout_policy: if ack {
                        pdf_core::ContentTextLayoutPolicy::AllowWidthChange
                    } else {
                        Default::default()
                    },
                },
            },
        })
        .unwrap(),
        payload: [pdf, font].concat(),
    }
}
#[test]
fn real_chosen_font_child_returns_unicode_positioned_tj_and_preserves_source() {
    let source = fixture::source(fixture::CONTENT);
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request(&source, FONT, true),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Document {
        selected_page,
        info,
    } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("document")
    };
    assert_eq!(selected_page, Some(0));
    assert_eq!(info.page_count, 2);
    let pdf = pdf_core::PdfDocument::from_bytes(&reply.payload, None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "AV Café ΩЖ\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    let raw = lopdf::Document::load_mem(&reply.payload).unwrap();
    let operations =
        lopdf::content::Content::decode(&raw.get_page_content(raw.get_pages()[&1])).unwrap();
    assert!(
        operations.operations.iter().any(|op| op.operator == "TJ"),
        "actual positioned kerning must be emitted"
    );
    assert!(run.writable && run.properties_writable);
    assert_eq!(run.embedded_font.unwrap().postscript_name, "IBMPlexSans");
    assert_eq!(
        pdf.info().unwrap().pages,
        pdf_core::PdfDocument::from_bytes(&source, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
}
#[test]
fn chosen_font_payload_identity_ack_and_strict_schema_errors_are_controlled() {
    let source = fixture::source(fixture::CONTENT);
    assert!(execute_document(request(&source, FONT, false)).is_err());
    assert!(execute_document(request(&source, b"not a font", true)).is_err());
    for variant in 0..5 {
        let mut r = request(&source, FONT, true);
        let control = &mut r.control["mutation"];
        match variant {
            0 => control["font_bytes_len"] = 0.into(),
            1 => control["font_bytes_len"] = (16 * 1024 * 1024 + 1).into(),
            2 => {
                let byte = control["source_fingerprint"][0].as_u64().unwrap();
                control["source_fingerprint"][0] = (byte ^ 1).into();
            }
            3 => control["path"] = "not-authorized.ttf".into(),
            _ => control["change"]["path"] = "not-authorized.ttf".into(),
        }
        assert!(execute_document(r).is_err(), "variant{variant}");
    }
    let mut r = request(&source, FONT, true);
    r.payload.truncate(2);
    assert!(execute_document(r).is_err());
    assert_eq!(
        pdf_core::PdfDocument::from_bytes(&source, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap(),
        "Original\nA B\n"
    );
}
