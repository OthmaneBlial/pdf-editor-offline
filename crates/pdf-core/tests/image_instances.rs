#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{ImageOverlay, PdfDocument};
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn bytes(mut doc: Document) -> Vec<u8> {
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn donor() -> PdfDocument {
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    doc.get_object_mut((4, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = vec![0, 0, 255];
    PdfDocument::from_bytes(&bytes(doc), None).unwrap()
}
fn fixture() -> Vec<u8> {
    let mut doc = Document::load_mem(&support::fixture(2)).unwrap();
    let a = doc.add_object(Stream::new(
        dictionary! {},
        b"\nq 2 0 0 2 0 0 cm q ".to_vec(),
    ));
    let b = doc.add_object(Stream::new(
        dictionary! {},
        b"20 0 0 10 50 70 cm /Im1 Do Q Q\n".to_vec(),
    ));
    doc.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Contents",
            vec![Object::Reference((5, 0)), a.into(), b.into()],
        );
    bytes(doc)
}
#[test]
fn arrays_cross_stream_state_shared_images_and_pages_replace_only_selected_do() {
    let source = fixture();
    let original = Document::load_mem(&source).unwrap();
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let before = pdf.info().unwrap();
    let text = pdf.extract_text(&[1, 2]).unwrap();
    let images = pdf.list_image_instances(1).unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].selection.image_id, images[1].selection.image_id);
    assert_eq!(images[1].matrix, [40.0, 0.0, 0.0, 20.0, 100.0, 140.0]);
    assert!(images.iter().all(|i| i.replaceable));
    let renderer = |data: &[u8], page| pdf_render::render_page(data, page, 1.0).unwrap().rgba;
    let page1 = renderer(&source, 0);
    let page2 = renderer(&source, 1);
    pdf.replace_image_instance(&images[1].selection, &donor())
        .unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let actual = Document::load_mem(&saved).unwrap();
    assert_eq!(pdf.info().unwrap().pages, before.pages);
    assert_eq!(pdf.extract_text(&[1, 2]).unwrap(), text);
    for (id, value) in &original.objects {
        if *id != (6, 0) {
            assert_eq!(
                actual.get_object(*id).unwrap(),
                value,
                "old object {id:?} changed"
            );
        }
    }
    assert_eq!(renderer(&saved, 1), page2);
    let after = renderer(&saved, 0);
    let mut changed = 0;
    for (i, (a, b)) in page1.chunks_exact(4).zip(after.chunks_exact(4)).enumerate() {
        if a != b {
            assert!(
                (100..140).contains(&(i % 200)) && (140..160).contains(&(i / 200)),
                "outside expected image raster footprint at ({}, {}) before={a:?} after={b:?}",
                i % 200,
                i / 200
            );
            changed += 1;
        }
    }
    assert!(changed > 500);
    let new = pdf.list_image_instances(1).unwrap();
    assert_eq!(new[0].selection.image_id, images[0].selection.image_id);
    assert_ne!(new[1].selection.image_id, images[1].selection.image_id);
    assert_eq!(new[1].matrix, images[1].matrix);
}
#[test]
fn overlay_wrapped_contents_are_discovered_and_replaced_and_stale_identity_refuses() {
    let mut pdf = PdfDocument::from_bytes(&fixture(), None).unwrap();
    pdf.add_image_overlay(
        &ImageOverlay {
            page: 1,
            rectangle: [80.0, 30.0, 120.0, 70.0],
            rotation: 90,
        },
        &donor(),
    )
    .unwrap();
    let images = pdf.list_image_instances(1).unwrap();
    assert_eq!(images.len(), 3);
    assert_eq!(images[2].matrix, [0.0, 40.0, -40.0, 0.0, 120.0, 30.0]);
    let old = pdf.verified_bytes().unwrap();
    for n in 0..4 {
        let mut selection = images[2].selection.clone();
        match n {
            0 => selection.source_hash = "0".repeat(64),
            1 => selection.operator = images[0].selection.operator,
            2 => selection.image_id = (999, 0),
            _ => selection.resource_name = b"Im1".to_vec(),
        };
        assert!(pdf.replace_image_instance(&selection, &donor()).is_err());
        assert_eq!(pdf.verified_bytes().unwrap(), old);
    }
    pdf.replace_image_instance(&images[2].selection, &donor())
        .unwrap();
    let current = pdf.verified_bytes().unwrap();
    assert!(
        pdf.replace_image_instance(&images[2].selection, &donor())
            .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), current);
}
#[test]
fn unsafe_source_graphics_forms_carriers_and_tags_refuse_without_mutation() {
    for content in [
        b"Q /Im1 Do".as_slice(),
        b"q /Im1 Do",
        b"1 0 cm /Im1 Do",
        b"BT /Im1 Do ET",
        b"/OC /Layer BDC /Im1 Do EMC",
    ] {
        let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
        doc.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .content = content.to_vec();
        doc.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Length", content.len() as i64);
        let raw = bytes(doc);
        let pdf = PdfDocument::from_bytes(&raw, None).unwrap();
        assert!(pdf.list_image_instances(1).is_err());
    }
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    doc.get_object_mut((4, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("Subtype", "Form");
    assert!(
        PdfDocument::from_bytes(&bytes(doc), None)
            .unwrap()
            .list_image_instances(1)
            .is_err()
    );
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    doc.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("StructParents", 0);
    let mut pdf = PdfDocument::from_bytes(&bytes(doc), None).unwrap();
    let image = pdf.list_image_instances(1).unwrap().remove(0);
    assert!(!image.replaceable);
    let old = pdf.verified_bytes().unwrap();
    assert!(
        pdf.replace_image_instance(&image.selection, &donor())
            .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), old);
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_reader_replacement_preserves_text_geometry_and_shared_instance_pixels() {
    let source = fixture();
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let image = pdf.list_image_instances(1).unwrap().remove(1);
    pdf.replace_image_instance(&image.selection, &donor())
        .unwrap();
    let output = pdf.verified_bytes().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let result = temp.path().join("replacement.pdf");
    let control_path = temp.path().join("control-no-selected-image.pdf");
    let mut control_doc = Document::load_mem(&source).unwrap();
    let id = control_doc
        .get_object((6, 0))
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    control_doc
        .get_object_mut(id.as_reference().unwrap())
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(b"20 0 0 10 50 70 cm Q Q\n".to_vec());
    std::fs::write(&control_path, bytes(control_doc)).unwrap();
    std::fs::write(&input, &source).unwrap();
    std::fs::write(&result, &output).unwrap();
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    assert!(
        std::process::Command::new(qpdf)
            .arg("--check")
            .arg(&result)
            .output()
            .unwrap()
            .status
            .success()
    );
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let text = |path: &std::path::Path| {
        let out = std::process::Command::new(bin.join("pdftotext"))
            .arg("-raw")
            .arg(path)
            .arg("-")
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
    };
    assert_eq!(text(&input), text(&result));
    let raster = |path: &std::path::Path, page: &str| {
        let out = std::process::Command::new(bin.join("pdftoppm"))
            .args(["-f", page, "-singlefile", "-r", "72"])
            .arg(path)
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
    };
    assert_eq!(raster(&input, "2"), raster(&result, "2"));
    let before = raster(&input, "1");
    let after = raster(&result, "1");
    let control = raster(&control_path, "1");
    let control = control.strip_prefix(b"P6\n200 300\n255\n").unwrap();
    let before = before.strip_prefix(b"P6\n200 300\n255\n").unwrap();
    let after = after.strip_prefix(b"P6\n200 300\n255\n").unwrap();
    let mut changed = 0;
    for (i, ((a, b), unpainted)) in before
        .chunks_exact(3)
        .zip(after.chunks_exact(3))
        .zip(control.chunks_exact(3))
        .enumerate()
    {
        if a == unpainted {
            assert_eq!(
                a,
                b,
                "changed outside independently observed original-image footprint at ({},{})",
                i % 200,
                i / 200
            );
        } else {
            assert_eq!(a, [200, 40, 80]);
            assert_eq!(b, [0, 0, 255]);
            changed += 1;
        }
    }
    assert!(changed > 500);
    assert_eq!(std::fs::read(&input).unwrap(), source);
}

#[test]
fn content_cycles_operator_limits_and_readonly_signature_markers_are_controlled() {
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    let id = doc.new_object_id();
    doc.objects
        .insert(id, Object::Array(vec![Object::Reference(id)]));
    doc.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Contents", id);
    let pdf = PdfDocument::from_bytes(&bytes(doc), None).unwrap();
    assert!(matches!(
        pdf.list_image_instances(1),
        Err(pdf_core::Error::Invalid(_))
    ));
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    let stream = doc.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
    stream.set_content(b"n\n".repeat(100_001));
    let pdf = PdfDocument::from_bytes(&bytes(doc), None).unwrap();
    assert!(matches!(
        pdf.list_image_instances(1),
        Err(pdf_core::Error::Limit(_))
    ));
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    doc.add_object(dictionary! {"Type"=>"Sig"});
    let mut pdf = PdfDocument::from_bytes(&bytes(doc), None).unwrap();
    let image = pdf.list_image_instances(1).unwrap().remove(0);
    assert!(!image.replaceable);
    assert!(matches!(
        pdf.replace_image_instance(&image.selection, &donor()),
        Err(pdf_core::Error::Refused(_))
    ));
}

#[test]
fn image_aliases_and_shared_content_streams_keep_other_page_and_original_graph() {
    let mut doc = Document::load_mem(&support::fixture(2)).unwrap();
    let image_alias = doc.add_object(Object::Reference((4, 0)));
    let content_alias = doc.add_object(Object::Reference((5, 0)));
    doc.get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Im1", image_alias);
    for page in [(6, 0), (8, 0)] {
        doc.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Contents", content_alias);
    }
    let source = bytes(doc);
    let original = Document::load_mem(&source).unwrap();
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let image = pdf.list_image_instances(1).unwrap().remove(0);
    assert_eq!(image.selection.image_id, (4, 0));
    let old_second = pdf_render::render_page(&source, 1, 1.0).unwrap().rgba;
    pdf.replace_image_instance(&image.selection, &donor())
        .unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let actual = Document::load_mem(&saved).unwrap();
    for (id, value) in &original.objects {
        if *id != (6, 0) {
            assert_eq!(actual.objects.get(id).unwrap(), value);
        }
    }
    assert_eq!(
        pdf_render::render_page(&saved, 1, 1.0).unwrap().rgba,
        old_second
    );
    assert_eq!(
        pdf.list_image_instances(2).unwrap()[0].selection.image_id,
        (4, 0)
    );
}

#[test]
fn replacement_splices_only_name_and_retains_every_untouched_decoded_byte() {
    let raw=b"% original comment q Do\n0.1234567890123456789 0.2 0.3 rg\nBT /F1 20.1234567890123456789 Tf 30.0000000123456789 120 Td (PAGE \\(Do\\) \\053) Tj ET\n/Span << /Nested [true null << /Key (cm % literal) >>] /Hex <444F> >> DP\nq 30.1234567890123456789 0 0 30.9876543210987654321 30.000000123456789 20 cm /Im#31 % keep this comment\nDo Q\n";
    let mut doc = Document::load_mem(&support::fixture(1)).unwrap();
    doc.get_object_mut((5, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(raw.to_vec());
    let source = bytes(doc);
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let instance = pdf.list_image_instances(1).unwrap().remove(0);
    assert_eq!(
        instance.matrix[0],
        "30.1234567890123456789".parse::<f64>().unwrap()
    );
    assert_eq!(
        instance.matrix[3],
        "30.9876543210987654321".parse::<f64>().unwrap()
    );
    pdf.replace_image_instance(&instance.selection, &donor())
        .unwrap();
    let output = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let page = output.get_object((6, 0)).unwrap().as_dict().unwrap();
    let stream = output
        .get_object(page.get(b"Contents").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    let actual = stream
        .decompressed_content_with_limit(pdf_core::MAX_STREAM_BYTES)
        .unwrap();
    let at = raw.windows(6).position(|v| v == b"/Im#31").unwrap();
    let mut expected = raw[..at].to_vec();
    expected.extend_from_slice(b"/NativeImageReplacement");
    expected.extend_from_slice(&raw[at + 6..]);
    assert_eq!(actual, expected);
    assert_eq!(
        output
            .get_object((5, 0))
            .unwrap()
            .as_stream()
            .unwrap()
            .content,
        raw
    );
}
