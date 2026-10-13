use pdf_core::{ButtonChange, ButtonSelection, PdfDocument};
use pdf_worker::document::{DocumentResult, Mutation, Operation, execute_document};
use pdf_worker::{PayloadKind, Request};
use serde_json::json;

const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
fn request(operation: Operation, payload: &[u8]) -> Request {
    Request {
        control: serde_json::to_value(operation).expect("JSON"),
        payload: payload.to_vec(),
    }
}

#[test]
fn forms_are_bounded_path_free_and_nested_unknown_fields_are_rejected() {
    let reply = execute_document(request(Operation::Forms, SOURCE)).expect("forms");
    assert_eq!(reply.kind, PayloadKind::None);
    assert!(reply.payload.is_empty());
    let DocumentResult::Forms { fields } = serde_json::from_value(reply.control).expect("result")
    else {
        panic!("forms");
    };
    assert_eq!(fields.len(), 5);
    for control in [
        json!({"operation":"forms","path":"private.pdf"}),
        json!({"operation":"edit","mutation":{"mutation":"fill_buttons","page":1,"changes":[{"name":"approved","selection":"Off","path":"private.pdf"}]}}),
        json!({"operation":"edit","mutation":{"mutation":"fill_buttons","page":1,"changes":[{"name":"approved","selection":{"On":{"widget":[10,0],"path":"private.pdf"}}}]}}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: SOURCE.to_vec()
            })
            .is_err()
        );
    }
}

#[test]
fn production_child_fills_real_appearances_preserving_selection_geometry_and_source() {
    let child = std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker"));
    let discovered = pdf_worker::execute(
        child,
        &[],
        request(Operation::Forms, SOURCE),
        pdf_worker::Limits::default(),
    )
    .expect("production child discovery");
    assert_eq!(discovered.kind, PayloadKind::None);
    assert!(discovered.payload.is_empty());
    let DocumentResult::Forms { fields } =
        serde_json::from_value(discovered.control).expect("forms result")
    else {
        panic!("forms");
    };
    assert_eq!(fields.len(), 5);
    assert!(
        fields
            .iter()
            .any(|field| field.name == "approved" && field.writable_button)
    );
    let operation = Operation::Edit {
        mutation: Mutation::FillButtons {
            changes: vec![ButtonChange {
                name: "approved".into(),
                selection: ButtonSelection::Off,
            }],
            page: 1,
        },
    };
    let reply = pdf_worker::execute(
        child,
        &[],
        request(operation, SOURCE),
        pdf_worker::Limits::default(),
    )
    .expect("real child");
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Document {
        info,
        selected_page,
    } = serde_json::from_value(reply.control).expect("result")
    else {
        panic!("document");
    };
    assert_eq!(selected_page, Some(0));
    assert_eq!(
        serde_json::to_value(info).expect("info JSON"),
        serde_json::to_value(
            PdfDocument::from_bytes(SOURCE, None)
                .expect("source")
                .info()
                .expect("info")
        )
        .expect("source info JSON")
    );
    let changed = PdfDocument::from_bytes(&reply.payload, None).expect("verified PDF");
    assert!(!changed.extract_text(&[1]).expect("text").contains('✓'));
    let before = pdf_render::render_page(SOURCE, 0, 1.0).expect("source pixels");
    let after = pdf_render::render_page(&reply.payload, 0, 1.0).expect("changed pixels");
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut differences = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let x = index % before.width as usize;
            let y = index / before.width as usize;
            assert!(
                (71..=93).contains(&x) && (144..=166).contains(&y),
                "unexpected changed pixel {x},{y}"
            );
            differences += 1;
        }
    }
    assert!(differences > 0);
    assert!(
        PdfDocument::from_bytes(SOURCE, None)
            .expect("source")
            .extract_text(&[1])
            .expect("source text")
            .contains('✓')
    );
    let rejected = pdf_worker::execute(
        child,
        &[],
        request(
            Operation::Edit {
                mutation: Mutation::FillButtons {
                    changes: vec![ButtonChange {
                        name: "approved".into(),
                        selection: ButtonSelection::Off,
                    }],
                    page: 0,
                },
            },
            SOURCE,
        ),
        pdf_worker::Limits::default(),
    )
    .expect("controlled rejection");
    assert_eq!(rejected.control["ok"], false);
    assert!(rejected.payload.is_empty());
}
