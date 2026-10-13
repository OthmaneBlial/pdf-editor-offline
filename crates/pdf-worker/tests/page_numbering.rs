#![allow(clippy::unwrap_used)]
use pdf_worker::document::{Mutation, Operation};
use pdf_worker::{Limits, PayloadKind, Request};
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn request() -> Request {
    Request {
        control: serde_json::to_value(Operation::Edit {
            mutation: Mutation::PageNumbers {
                numbering: pdf_core::PageNumbering {
                    pages: vec![1],
                    start: 2026,
                    position: pdf_core::NumberPosition::Footer,
                    margin: 18.0,
                    height: 24.0,
                    size: 12.0,
                    color: [0.0; 3],
                    alignment: pdf_core::TextAlignment::Center,
                },
                page: 1,
                font_bytes_len: FONT.len() as u32,
            },
        })
        .unwrap(),
        payload: [PDF, FONT].concat(),
    }
}
#[test]
fn real_child_numbering_preserves_forms_and_geometry() {
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    assert_eq!(reply.control["selected_page"], 0);
    let pdf = pdf_core::PdfDocument::from_bytes(&reply.payload, None).unwrap();
    let old = pdf_core::PdfDocument::from_bytes(PDF, None).unwrap();
    assert_eq!(pdf.info().unwrap().pages, old.info().unwrap().pages);
    assert_eq!(
        pdf.list_form_fields().unwrap(),
        old.list_form_fields().unwrap()
    );
    assert!(pdf.extract_text(&[1]).unwrap().contains("2026"));
}
#[test]
fn malformed_numbering_font_selection_and_paths_are_controlled_errors() {
    for length in [0, 16 * 1024 * 1024 + 1, PDF.len() + FONT.len()] {
        let mut r = request();
        r.control["mutation"]["font_bytes_len"] = length.into();
        assert!(pdf_worker::document::execute_document(r).is_err());
    }
    for key in ["path", "font_file"] {
        let mut r = request();
        r.control["mutation"]["numbering"][key] = "private".into();
        assert!(pdf_worker::document::execute_document(r).is_err());
    }
    let mut r = request();
    r.control["mutation"]["numbering"]["pages"] = serde_json::json!([1, 1]);
    assert!(pdf_worker::document::execute_document(r).is_err());
}
