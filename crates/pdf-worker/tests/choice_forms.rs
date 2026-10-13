use pdf_core::{FormValue, PdfDocument};
use pdf_worker::{Limits, PayloadKind, Request};
use serde_json::json;
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn request(indices: serde_json::Value) -> Request {
    Request {
        control: json!({"operation":"edit","mutation":{"mutation":"fill_text","changes":[],"choices":[{"name":"priority","indices":indices,"top_index":null}],"page":1,"font_bytes_len":FONT.len()}}),
        payload: [SOURCE, FONT].concat(),
    }
}
fn child(request: Request) -> Result<pdf_worker::Reply, pdf_worker::Error> {
    pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        request,
        Limits::default(),
    )
}
#[test]
fn actual_child_choice_has_real_pixels_and_preserves_outside_widget() {
    let reply = child(request(json!([2]))).expect("productionchild");
    assert_eq!(reply.kind, PayloadKind::Pdf);
    assert_eq!(reply.control["selected_page"], 0);
    let pdf = PdfDocument::from_bytes(&reply.payload, None).expect("savedPDF");
    assert_eq!(
        pdf.list_choice_fields().expect("choices")[0].selected_indices,
        Some(vec![2])
    );
    assert!(pdf.extract_text(&[1]).expect("text").contains("High"));
    let before = pdf_render::render_page(SOURCE, 0, 1.0).expect("before");
    let after = pdf_render::render_page(&reply.payload, 0, 1.0).expect("after");
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut changed = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (index % before.width as usize, index / before.width as usize);
            assert!(
                (72..220).contains(&x) && (190..220).contains(&y),
                "outside{x},{y}"
            );
            changed += 1;
        }
    }
    assert!(changed > 0);
}
#[test]
fn mixed_child_changes_publish_once_and_late_choice_failure_exposes_no_pdf() {
    let mut mixed = request(json!([2]));
    mixed.control["mutation"]["changes"] = json!([{"name":"full_name","value":"Ada"}]);
    mixed.control["mutation"]["buttons"] = json!([{"name":"approved","selection":"Off"}]);
    let reply = child(mixed).expect("mixedchild");
    let pdf = PdfDocument::from_bytes(&reply.payload, None).expect("PDF");
    assert!(pdf.extract_text(&[1]).expect("text").contains("Ada"));
    assert_eq!(
        pdf.list_choice_fields().expect("choices")[0].selected_indices,
        Some(vec![2])
    );
    assert_eq!(
        pdf.list_form_fields()
            .expect("forms")
            .into_iter()
            .find(|field| field.name == "full_name")
            .expect("field")
            .value,
        FormValue::Text("Ada".into())
    );
    let mut late = request(json!([99]));
    late.control["mutation"]["changes"] = json!([{"name":"full_name","value":"Ada"}]);
    late.control["mutation"]["buttons"] = json!([{"name":"approved","selection":"Off"}]);
    let failure = child(late).expect("controlledreply");
    assert_eq!(failure.kind, PayloadKind::None);
    assert!(failure.payload.is_empty());
    assert_eq!(failure.control["ok"], false);
    let original = PdfDocument::from_bytes(SOURCE, None).expect("source");
    assert!(original.extract_text(&[1]).expect("text").contains('✓'));
    assert_eq!(
        original.list_choice_fields().expect("choices")[0].selected_indices,
        Some(vec![1])
    );
}
#[test]
fn choice_controls_reject_unknown_fields_lengths_and_invalid_indices() {
    for indices in [json!([2, 1]), json!([1, 1]), json!([99]), json!([0, 2])] {
        assert!(pdf_worker::document::execute_document(request(indices)).is_err());
    }
    for key in ["path", "font_file", "extra"] {
        let mut invalid = request(json!([2]));
        invalid.control["mutation"]["choices"][0][key] = json!("unaccepted");
        assert!(pdf_worker::document::execute_document(invalid).is_err());
    }
    let mut invalid = request(json!([2]));
    invalid.control["mutation"]["font_bytes_len"] = json!(0);
    assert!(pdf_worker::document::execute_document(invalid).is_err());
}

#[test]
fn nested_forms_and_choice_reply_dtos_refuse_unknown_paths_and_values() {
    let reply = pdf_worker::document::execute_document(Request {
        control: json!({"operation":"forms"}),
        payload: SOURCE.to_vec(),
    })
    .expect("forms");
    for index in 0..4 {
        let mut control = reply.control.clone();
        match index {
            0 => control["fields"][0]["path"] = json!("unaccepted"),
            1 => control["fields"][0]["value"]["path"] = json!("unaccepted"),
            2 => control["choices"][0]["field"]["widgets"][0]["path"] = json!("unaccepted"),
            _ => control["choices"][0]["default_value"]["path"] = json!("unaccepted"),
        }
        assert!(serde_json::from_value::<pdf_worker::document::DocumentResult>(control).is_err());
    }
}
