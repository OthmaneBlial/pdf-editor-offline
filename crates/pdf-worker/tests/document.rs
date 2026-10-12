use pdf_core::{MetadataKey, PdfDocument};
use pdf_worker::document::{DocumentResult, MetadataChange, Mutation, Operation, execute_document};
use pdf_worker::{PayloadKind, Reply, Request};
use serde_json::json;

fn fixture(signed: bool) -> Vec<u8> {
    fixture_with_text(signed, "Page One")
}

fn fixture_with_text(signed: bool, first_text: &str) -> Vec<u8> {
    let stream = |text: &str| format!("<< /Length {} >>\nstream\n{text}\nendstream", text.len());
    let mut objects=vec![
        "<< /Type /Catalog /Pages 2 0 R /Metadata 8 0 R /Info 9 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [4 0 R 6 0 R] /Count 2 >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 80] /Resources << /Font << /F1 3 0 R >> >> /Contents 5 0 R >>".to_owned(),
        stream(&format!("1 0 0 rg 0 0 100 80 re f BT /F1 12 Tf 10 20 Td ({first_text}) Tj ET")),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 120 90] /Resources << /Font << /F1 3 0 R >> >> /Contents 7 0 R >>".to_owned(),
        stream("0 0 1 rg 0 0 120 90 re f BT /F1 12 Tf 10 20 Td (Page Two) Tj ET"),
        "<< /Type /Metadata /Subtype /XML /Length 27 >>\nstream\n<x:xmpmeta>old</x:xmpmeta>\nendstream".to_owned(),
        "<< /Title (Original) /Author (Ada) /CreationDate (D:20261012) /Custom (Keep this) >>".to_owned(),
    ];
    // Compute stream length exactly; aliases, XMP and custom fields exercise preservation.
    let xmp = "<x:xmpmeta>old</x:xmpmeta>";
    objects[7] = format!(
        "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n{xmp}\nendstream",
        xmp.len()
    );
    if signed {
        objects.push("<< /FT /Sig /Type /Annot /Subtype /Widget >>".into());
    }
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 9 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

fn run(operation: Operation, bytes: &[u8]) -> Reply {
    execute_document(Request {
        control: serde_json::to_value(operation).expect("operation JSON"),
        payload: bytes.to_vec(),
    })
    .expect("document operation")
}

fn result(reply: &Reply) -> DocumentResult {
    serde_json::from_value(reply.control.clone()).expect("typed result")
}

fn edit(bytes: &[u8], mutation: Mutation) -> Reply {
    run(Operation::Edit { mutation }, bytes)
}

#[test]
fn info_text_metadata_and_prepare_save_are_real_typed_operations() {
    let bytes = fixture(false);
    let info = run(Operation::Info, &bytes);
    assert_eq!(info.kind, PayloadKind::None);
    assert!(info.payload.is_empty());
    let DocumentResult::Info { info } = result(&info) else {
        panic!("info result");
    };
    assert_eq!(info.page_count, 2);
    assert_eq!(info.pages[1].media_box, [0.0, 0.0, 120.0, 90.0]);
    let DocumentResult::Text { text, truncated } =
        result(&run(Operation::Text { page: 2 }, &bytes))
    else {
        panic!("text result");
    };
    assert!(text.contains("Page Two") && !truncated);
    let DocumentResult::Metadata { metadata } = result(&run(Operation::Metadata, &bytes)) else {
        panic!("metadata result");
    };
    assert_eq!(metadata.fields[&MetadataKey::Author], "Ada");
    assert!(
        metadata.xmp_present
            && metadata.catalog_info_present
            && !metadata.xmp_synchronized
            && !metadata.secure_erasure
    );
    let prepared = run(Operation::PrepareSave, &bytes);
    assert_eq!(prepared.kind, PayloadKind::Pdf);
    let reopened = PdfDocument::from_bytes(&prepared.payload, None).expect("verified bytes");
    let DocumentResult::Document {
        info,
        selected_page,
    } = result(&prepared)
    else {
        panic!("document result");
    };
    assert_eq!(
        serde_json::to_value(info).expect("reply info"),
        serde_json::to_value(reopened.info().expect("reopened info")).expect("reopened info JSON")
    );
    assert_eq!(selected_page, None);
    assert!(
        reopened
            .extract_text(&[1])
            .expect("text")
            .contains("Page One")
    );
}

#[test]
fn edit_operations_return_verified_data_and_correct_page_selection() {
    let bytes = fixture(false);
    let moved = edit(
        &bytes,
        Mutation::Move {
            page: 2,
            earlier: true,
        },
    );
    assert!(
        PdfDocument::from_bytes(&moved.payload, None)
            .expect("moved")
            .extract_text(&[1])
            .expect("text")
            .contains("Page Two")
    );
    assert!(matches!(
        result(&moved),
        DocumentResult::Document {
            selected_page: Some(0),
            ..
        }
    ));
    let reversed = edit(&moved.payload, Mutation::Reverse { page: 1 });
    assert!(matches!(
        result(&reversed),
        DocumentResult::Document {
            selected_page: Some(1),
            ..
        }
    ));
    assert!(
        PdfDocument::from_bytes(&reversed.payload, None)
            .expect("reverse")
            .extract_text(&[1])
            .expect("text")
            .contains("Page One")
    );
    let blank = edit(
        &reversed.payload,
        Mutation::InsertBlank {
            position: 2,
            width: 200.0,
            height: 300.0,
        },
    );
    let document = PdfDocument::from_bytes(&blank.payload, None).expect("blank");
    assert_eq!(document.page_count(), 3);
    assert!(
        document
            .extract_text(&[2])
            .expect("blank text")
            .trim()
            .is_empty()
    );
    assert!(matches!(
        result(&blank),
        DocumentResult::Document {
            selected_page: Some(1),
            ..
        }
    ));
    let rotated = edit(
        &blank.payload,
        Mutation::Rotate {
            page: 1,
            degrees: 90,
        },
    );
    let cropped = edit(
        &rotated.payload,
        Mutation::Crop {
            page: 1,
            rectangle: [10.0, 10.0, 90.0, 70.0],
        },
    );
    let info = PdfDocument::from_bytes(&cropped.payload, None)
        .expect("crop")
        .info()
        .expect("info");
    assert_eq!(info.pages[0].rotation, 90);
    assert_eq!(info.pages[0].crop_box, [10.0, 10.0, 90.0, 70.0]);
    let rendered = run(
        Operation::Render {
            page: 0,
            scale: 2.0,
            logical_zoom: 1.0,
        },
        &cropped.payload,
    );
    let DocumentResult::Render { raster } = result(&rendered) else {
        panic!("raster result");
    };
    assert_eq!([raster.width, raster.height], [120, 160]);
    assert_eq!(rendered.kind, PayloadKind::Rgba);
    assert_eq!(rendered.payload.len(), 120 * 160 * 4);
    assert!(
        rendered
            .payload
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 0, 0, 255])
    );
    assert_eq!(
        PdfDocument::from_bytes(&bytes, None)
            .expect("original")
            .page_count(),
        2
    );
}

#[test]
fn metadata_edits_preserve_unicode_original_aliases_xmp_dates_and_custom_values() {
    let bytes = fixture(false);
    let changed = edit(
        &bytes,
        Mutation::Metadata {
            changes: vec![
                MetadataChange {
                    key: MetadataKey::Title,
                    value: Some("Résumé 東京 ✓".into()),
                },
                MetadataChange {
                    key: MetadataKey::Author,
                    value: None,
                },
            ],
        },
    );
    let metadata = PdfDocument::from_bytes(&changed.payload, None)
        .expect("changed")
        .metadata()
        .expect("metadata");
    assert_eq!(metadata.fields[&MetadataKey::Title], "Résumé 東京 ✓");
    assert!(!metadata.fields.contains_key(&MetadataKey::Author));
    for retained in [
        "Original",
        "CreationDate",
        "D:20261012",
        "Custom",
        "Keep this",
        "<x:xmpmeta>old</x:xmpmeta>",
    ] {
        assert!(
            changed
                .payload
                .windows(retained.len())
                .any(|window| window == retained.as_bytes()),
            "retains {retained}"
        );
    }
    assert_eq!(
        PdfDocument::from_bytes(&bytes, None)
            .expect("original")
            .metadata()
            .expect("metadata")
            .fields[&MetadataKey::Title],
        "Original"
    );
}

#[test]
fn signed_inputs_remain_readable_but_all_pdf_rewrites_are_refused() {
    let bytes = fixture(true);
    let DocumentResult::Info { info } = result(&run(Operation::Info, &bytes)) else {
        panic!("info");
    };
    assert!(info.signature_fields > 0);
    run(Operation::Metadata, &bytes);
    run(Operation::Text { page: 1 }, &bytes);
    run(
        Operation::Render {
            page: 0,
            scale: 1.0,
            logical_zoom: 1.0,
        },
        &bytes,
    );
    for operation in [
        Operation::PrepareSave,
        Operation::Edit {
            mutation: Mutation::Rotate {
                page: 1,
                degrees: 90,
            },
        },
        Operation::Edit {
            mutation: Mutation::Metadata {
                changes: vec![MetadataChange {
                    key: MetadataKey::Title,
                    value: Some("Blocked".into()),
                }],
            },
        },
    ] {
        assert!(
            execute_document(Request {
                control: serde_json::to_value(operation).expect("control"),
                payload: bytes.clone()
            })
            .is_err()
        );
    }
}

#[test]
fn malformed_control_paths_invalid_pages_and_resource_bombs_are_controlled_errors() {
    let bytes = fixture(false);
    for control in [
        json!({"operation":"info","path":"/etc/passwd"}),
        json!({"operation":"edit","mutation":{"mutation":"rotate","page":1,"degrees":90,"path":"ignored"}}),
        json!({"operation":"launch","command":"anything"}),
        json!({"operation":"text","page":0}),
        json!({"operation":"render","page":9,"scale":1.0,"logical_zoom":1.0}),
        json!({"operation":"render","page":0,"scale":1e308,"logical_zoom":1.0}),
        json!({"operation":"render","page":0,"scale":-1.0,"logical_zoom":1.0}),
        json!({"operation":"render","page":0,"scale":1000000.0,"logical_zoom":1.0}),
        json!({"operation":"render","page":0,"scale":1.0,"logical_zoom":0.0}),
        json!({"operation":"edit","mutation":{"mutation":"crop","page":1,"rectangle":[0,0,1000,90]}}),
        json!({"operation":"edit","mutation":{"mutation":"metadata","changes":[{"key":"title","value":"bad\0value"}]}}),
    ] {
        assert!(
            execute_document(Request {
                control: control.clone(),
                payload: bytes.clone()
            })
            .is_err(),
            "rejects {control}"
        );
    }
    assert!(
        execute_document(Request {
            control: json!({"operation":"info"}),
            payload: b"broken PDF".to_vec()
        })
        .is_err()
    );
}

const RED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0,
    0, 0, 54, 136, 73, 214, 0, 0, 0, 16, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 68, 12,
    40, 20, 0, 68, 208, 5, 251, 164, 207, 222, 128, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn images_use_exact_bounded_slices_and_reply_with_a_real_verified_pdf() {
    let mut payload = RED_PNG.to_vec();
    payload.extend_from_slice(RED_PNG);
    let converted = run(
        Operation::Images {
            lengths: vec![RED_PNG.len(), RED_PNG.len()],
        },
        &payload,
    );
    assert_eq!(converted.kind, PayloadKind::Pdf);
    let document = PdfDocument::from_bytes(&converted.payload, None).expect("image PDF");
    assert_eq!(document.page_count(), 2);
    assert_eq!(
        document.info().expect("geometry").pages[1].media_box,
        [0.0, 0.0, 2.0, 3.0]
    );
    assert!(matches!(
        result(&converted),
        DocumentResult::Document {
            selected_page: Some(0),
            ..
        }
    ));
    let rendered = run(
        Operation::Render {
            page: 0,
            scale: 1.0,
            logical_zoom: 1.0,
        },
        &converted.payload,
    );
    assert!(
        rendered
            .payload
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 0, 0, 255])
    );
    for lengths in [
        vec![],
        vec![0],
        vec![usize::MAX],
        vec![pdf_core::MAX_STREAM_BYTES + 1],
        vec![RED_PNG.len() - 1],
        vec![RED_PNG.len() + 1],
        vec![RED_PNG.len(); pdf_core::MAX_PAGES + 1],
    ] {
        assert!(
            execute_document(Request {
                control: json!({"operation":"images","lengths":lengths}),
                payload: RED_PNG.to_vec()
            })
            .is_err()
        );
    }
}

#[test]
fn actual_page_text_reply_is_bounded_before_json_transport() {
    let bytes = fixture_with_text(
        false,
        &"X".repeat(pdf_worker::document::MAX_TEXT_BYTES + 128),
    );
    let response = run(Operation::Text { page: 1 }, &bytes);
    let DocumentResult::Text { text, truncated } = result(&response) else {
        panic!("text reply");
    };
    assert!(truncated);
    assert_eq!(text.len(), pdf_worker::document::MAX_TEXT_BYTES);
    assert!(text.is_char_boundary(text.len()));
    assert!(response.payload.is_empty());
}

#[test]
fn production_child_roundtrips_real_operations_and_retains_input_after_rejection() {
    let helper = std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker"));
    let bytes = fixture(false);
    let request = |operation: Operation, payload: Vec<u8>| Request {
        control: serde_json::to_value(operation).expect("control"),
        payload,
    };
    let changed = pdf_worker::execute(
        helper,
        &[],
        request(
            Operation::Edit {
                mutation: Mutation::Rotate {
                    page: 1,
                    degrees: 90,
                },
            },
            bytes.clone(),
        ),
        pdf_worker::Limits::default(),
    )
    .expect("isolated edit");
    let DocumentResult::Document { info, .. } = result(&changed) else {
        panic!("document reply");
    };
    assert_eq!(info.pages[0].rotation, 90);
    let rejected = pdf_worker::execute(
        helper,
        &[],
        Request {
            control: json!({"operation":"info","path":"/not/allowed"}),
            payload: bytes.clone(),
        },
        pdf_worker::Limits::default(),
    )
    .expect("controlled operation error");
    assert_eq!(rejected.control["ok"], false);
    assert!(rejected.payload.is_empty());
    let original = pdf_worker::execute(
        helper,
        &[],
        request(Operation::Info, bytes),
        pdf_worker::Limits::default(),
    )
    .expect("next isolated operation");
    let DocumentResult::Info { info } = result(&original) else {
        panic!("info reply");
    };
    assert_eq!(info.pages[0].rotation, 0);
}
