#![allow(clippy::unwrap_used)]
use pdf_worker::{Limits, PayloadKind, Request};
use serde_json::json;
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
fn request() -> Request {
    Request {
        control: json!({"operation":"edit","mutation":{"mutation":"vector_overlay","overlay":{"page":1,"shape":{"shape":"polyline","points":[[350,400],[400,450],[450,410],[550,490]]},"stroke":{"color":[0.6,0.1,0.0],"width":3},"fill":null,"opacity":0.7}}}),
        payload: PDF.to_vec(),
    }
}
#[test]
fn actual_vector_child_returns_verified_pdf_with_source_values_preserved() {
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
    let original = pdf_core::PdfDocument::from_bytes(PDF, None).unwrap();
    assert_eq!(pdf.info().unwrap().pages, original.info().unwrap().pages);
    assert_eq!(
        pdf.list_form_fields().unwrap(),
        original.list_form_fields().unwrap()
    );
    assert_eq!(
        pdf.extract_text(&[1]).unwrap(),
        original.extract_text(&[1]).unwrap()
    );
    assert_ne!(reply.payload, PDF);
}
#[test]
fn vector_worker_refuses_paths_nested_unknowns_and_resource_bombs() {
    for location in [0, 1, 2] {
        let mut req = request();
        let object = match location {
            0 => &mut req.control["mutation"],
            1 => &mut req.control["mutation"]["overlay"]["shape"],
            _ => &mut req.control["mutation"]["overlay"]["stroke"],
        };
        object["path"] = json!("private");
        assert!(pdf_worker::document::execute_document(req).is_err());
    }
    let mut req = request();
    req.control["mutation"]["overlay"]["shape"]["points"] = json!(vec![[400.0, 400.0]; 4097]);
    assert!(pdf_worker::document::execute_document(req).is_err());
    let mut req = request();
    req.control["mutation"]["overlay"]["opacity"] = json!(2);
    assert!(pdf_worker::document::execute_document(req).is_err());
}
