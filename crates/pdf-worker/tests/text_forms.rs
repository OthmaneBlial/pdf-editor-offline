use pdf_core::{FormValue, PdfDocument};
use pdf_worker::{Limits, PayloadKind, Request};
use serde_json::json;

const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn request(length: usize) -> Request {
    Request {
        control: json!({"operation":"edit","mutation":{"mutation":"fill_text",
            "changes":[{"name":"full_name","value":"Ada Unicode é"}],"page":1,"font_bytes_len":length}}),
        payload: [PDF, FONT].concat(),
    }
}

#[test]
fn production_child_fills_text_and_verifies_real_appearance() {
    let mut mixed = request(FONT.len());
    mixed.control["mutation"]["buttons"] = json!([{"name":"approved","selection":"Off"}]);
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        mixed,
        Limits::default(),
    )
    .expect("actual bounded child");
    assert_eq!(reply.kind, PayloadKind::Pdf);
    assert_eq!(reply.control["selected_page"], 0);
    let doc = PdfDocument::from_bytes(&reply.payload, None).expect("verified PDF");
    assert_eq!(doc.page_count(), 1);
    assert!(
        doc.extract_text(&[1])
            .expect("appearance text")
            .contains("Ada Unicode é")
    );
    let field = doc
        .list_form_fields()
        .expect("fields")
        .into_iter()
        .find(|f| f.name == "full_name")
        .expect("field");
    assert_eq!(field.value, FormValue::Text("Ada Unicode é".into()));
    let button = doc
        .list_form_fields()
        .expect("mixed fields")
        .into_iter()
        .find(|f| f.name == "approved")
        .expect("button");
    assert_eq!(
        button.widgets[0].appearance_state.as_deref(),
        Some(b"Off".as_slice())
    );
}

#[test]
fn font_suffix_and_nested_control_refuse_tampering_before_exposure() {
    let mut failure = request(8);
    failure.payload = [PDF, b"bad font"].concat();
    failure.control["mutation"]["buttons"] = json!([{"name":"approved","selection":"Off"}]);
    assert!(pdf_worker::document::execute_document(failure).is_err());
    assert!(
        PdfDocument::from_bytes(PDF, None)
            .expect("original source")
            .extract_text(&[1])
            .expect("source text")
            .contains('✓')
    );
    for length in [
        0,
        16 * 1024 * 1024 + 1,
        PDF.len() + FONT.len(),
        FONT.len() + 1,
    ] {
        assert!(pdf_worker::document::execute_document(request(length)).is_err());
    }
    let mut invalid = request(FONT.len());
    invalid.control["mutation"]["font_file"] = json!("never accepted");
    assert!(pdf_worker::document::execute_document(invalid).is_err());
    let mut invalid = request(FONT.len());
    invalid.control["mutation"]["changes"][0]["path"] = json!("never accepted");
    assert!(pdf_worker::document::execute_document(invalid).is_err());
    let mut invalid = request(FONT.len());
    invalid.payload = vec![0; pdf_worker::MAX_PDF_BYTES + 1];
    assert!(pdf_worker::document::execute_document(invalid).is_err());
}
