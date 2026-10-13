#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
use pdf_worker::{
    PayloadKind, Request,
    document::{DocumentResult, Operation, execute_document},
};
#[test]
fn production_path_free_split_frame_is_contiguous_bounded_and_independently_reopenable() {
    let source = support::fixture(5);
    let reply = pdf_worker::execute(
        std::path::Path::new(env!("CARGO_BIN_EXE_pdf-editor-offline-worker")),
        &[],
        Request {
            control: serde_json::to_value(Operation::Split { every: 2 }).unwrap(),
            payload: source.clone(),
        },
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Split { mut parts } = serde_json::from_value(reply.control).unwrap() else {
        panic!("split");
    };
    assert_eq!(parts.len(), 3);
    let info = pdf_core::PdfDocument::from_bytes(&source, None)
        .unwrap()
        .info()
        .unwrap();
    assert_eq!(
        pdf_worker::document::split_slices(&parts, &reply.payload, &info, 2)
            .unwrap()
            .len(),
        3
    );
    parts[0].length += 1;
    assert!(pdf_worker::document::split_slices(&parts, &reply.payload, &info, 2).is_err());
    parts[0].length -= 1;
    parts[0].last_page += 1;
    assert!(pdf_worker::document::split_slices(&parts, &reply.payload, &info, 2).is_err());
    parts[0].last_page -= 1;
    parts[0].info.pages[0].rotation = 90;
    assert!(pdf_worker::document::split_slices(&parts, &reply.payload, &info, 2).is_err());
    parts[0].info.pages[0].rotation = 0;
    let mut extra = reply.payload.clone();
    extra.push(0);
    assert!(pdf_worker::document::split_slices(&parts, &extra, &info, 2).is_err());
    let mut offset = 0;
    let mut next = 1;
    for part in parts {
        assert_eq!(part.first_page, next);
        assert!(part.length > 0);
        let end = offset + part.length;
        assert!(end <= reply.payload.len());
        let doc = pdf_core::PdfDocument::from_bytes(&reply.payload[offset..end], None).unwrap();
        assert_eq!(doc.info().unwrap().pages, part.info.pages);
        assert_eq!(
            doc.page_count(),
            (part.last_page - part.first_page + 1) as usize
        );
        assert!(
            doc.extract_text(&[1])
                .unwrap()
                .contains(&format!("PAGE {}", part.first_page))
        );
        next = part.last_page + 1;
        offset = end;
    }
    assert_eq!(offset, reply.payload.len());
    assert_eq!(next, 6);
    for control in [
        serde_json::json!({"operation":"split","every":2,"output_dir":"private"}),
        serde_json::json!({"operation":"split","every":0}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: source.clone()
            })
            .is_err()
        );
    }
}
