#![allow(clippy::unwrap_used)]
use pdf_worker::{Limits, PayloadKind, Request};
use serde_json::json;
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn request(length: usize) -> Request {
    Request {
        control: json!({"operation":"edit","mutation":{"mutation":"text_overlay","font_bytes_len":length,"overlay":{"page":1,"rectangle":[350,400,550,500],"value":"Overlay é\nSecond line ΩЖ","size":12,"color":[0.1,0.3,0.2],"alignment":"left"}}}),
        payload: [PDF, FONT].concat(),
    }
}
#[test]
fn real_child_overlay_returns_verified_page_and_preserves_form_values() {
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request(FONT.len()),
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
    assert!(
        pdf.extract_text(&[1])
            .unwrap()
            .contains("Overlay é\nSecond line ΩЖ")
    );
}
#[test]
fn overlay_suffix_paths_geometry_and_coverage_are_rejected() {
    for length in [
        0,
        16 * 1024 * 1024 + 1,
        PDF.len() + FONT.len(),
        FONT.len() + 1,
    ] {
        assert!(pdf_worker::document::execute_document(request(length)).is_err());
    }
    for key in ["path", "font_file"] {
        let mut req = request(FONT.len());
        req.control["mutation"]["overlay"][key] = "private".into();
        assert!(pdf_worker::document::execute_document(req).is_err());
    }
    let mut req = request(FONT.len());
    req.control["mutation"]["overlay"]["rectangle"] = json!([0, 0, 99999, 99999]);
    assert!(pdf_worker::document::execute_document(req).is_err());
    let mut req = request(FONT.len());
    req.control["mutation"]["overlay"]["value"] = "中文".into();
    assert!(pdf_worker::document::execute_document(req).is_err());
    assert!(
        pdf_core::PdfDocument::from_bytes(PDF, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains('✓')
    );
}
