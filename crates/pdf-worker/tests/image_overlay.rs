#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/image_fixture.rs"]
mod image;
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Mutation, Operation, execute_document},
};
fn placement() -> pdf_core::ImageOverlay {
    pdf_core::ImageOverlay {
        page: 1,
        rectangle: [60.0, 30.0, 100.0, 70.0],
        rotation: 90,
    }
}
#[test]
fn actual_path_free_child_inserts_image_verifies_geometry_and_source_text() {
    let source = support::fixture(2);
    let image = image::jpeg();
    let length = image.len() as u32;
    let mut payload = source.clone();
    payload.extend(&image);
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(Operation::Edit {
                mutation: Mutation::ImageOverlay {
                    overlay: placement(),
                    image_bytes_len: length,
                },
            })
            .unwrap(),
            payload,
        },
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Document {
        info,
        selected_page,
    } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("document")
    };
    assert_eq!(selected_page, Some(0));
    let before = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
    let after = pdf_core::PdfDocument::from_bytes(&reply.payload, None).unwrap();
    assert_eq!(info.pages, before.info().unwrap().pages);
    assert_eq!(
        after.extract_text(&[1, 2]).unwrap(),
        before.extract_text(&[1, 2]).unwrap()
    );
    assert_ne!(reply.payload, source);
    let rendered = pdf_render::render_page(&reply.payload, 0, 1.0).unwrap();
    let offset = (250 * rendered.width as usize + 80) * 4;
    assert_eq!(&rendered.rgba[offset..offset + 4], &[128, 128, 128, 255]);
}
#[test]
fn invalid_lengths_images_and_extra_path_fields_refuse_without_output() {
    for length in [0, 1, u32::MAX] {
        assert!(
            execute_document(Request {
                control: serde_json::to_value(Operation::Edit {
                    mutation: Mutation::ImageOverlay {
                        overlay: placement(),
                        image_bytes_len: length
                    }
                })
                .unwrap(),
                payload: support::fixture(2)
            })
            .is_err()
        );
    }
    let mut control = serde_json::to_value(Operation::Edit {
        mutation: Mutation::ImageOverlay {
            overlay: placement(),
            image_bytes_len: 10,
        },
    })
    .unwrap();
    control["mutation"]["path"] = serde_json::json!("/private.pdf");
    assert!(
        execute_document(Request {
            control,
            payload: support::fixture(2)
        })
        .is_err()
    );
}
