#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{PdfDocument, VectorOverlay, VectorShape, VectorStroke};
use std::process::Command;
fn source(content: &[u8]) -> Vec<u8> {
    let mut d = Document::with_version("1.3");
    let root = d.new_object_id();
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let stream = d.add_object(Stream::new(dictionary! {}, content.to_vec()));
    d.objects.insert(
        page,
        dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream}.into(),
    );
    d.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"Old"=>font}}}.into());
    d.objects
        .insert(root, dictionary! {"Type"=>"Catalog","Pages"=>pages}.into());
    d.trailer.set("Root", root);
    let mut bytes = Vec::new();
    d.save_to(&mut bytes).unwrap();
    bytes
}
const ART:&[u8]=b"0.2 0.4 0.6 rg 1 0 0 1 17 9 cm 0 0 100 100 re W n 0 0 90 90 re f BT /Old 12 Tf 20 30 Td (Original) Tj ET\n";
fn request(shape: VectorShape) -> VectorOverlay {
    VectorOverlay {
        page: 1,
        shape,
        stroke: Some(VectorStroke {
            color: [0.8, 0.1, 0.0],
            width: 4.0,
        }),
        fill: None,
        opacity: 0.5,
    }
}
fn requests() -> Vec<VectorOverlay> {
    let mut rectangle = request(VectorShape::Rectangle {
        rectangle: [130.0, 130.0, 270.0, 250.0],
    });
    rectangle.fill = Some([0.0, 0.4, 0.6]);
    vec![
        rectangle,
        request(VectorShape::Ellipse {
            rectangle: [130.0, 130.0, 270.0, 250.0],
        }),
        request(VectorShape::Line {
            from: [130.0, 130.0],
            to: [270.0, 250.0],
        }),
        request(VectorShape::Polyline {
            points: vec![
                [130.0, 130.0],
                [170.0, 230.0],
                [210.0, 150.0],
                [270.0, 250.0],
            ],
        }),
    ]
}
fn pixels(before: &[u8], after: &[u8], channels: usize) {
    let mut changed = 0;
    assert_eq!(before.len(), after.len());
    for (i, (a, b)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if a != b {
            assert!(
                (127..273).contains(&(i % 300)) && (47..173).contains(&(i / 300)),
                "changed outside paint bounds at{i}"
            );
            changed += 1;
        }
    }
    assert!(changed > 40);
}
#[test]
fn all_shapes_paint_only_their_region_preserve_objects_text_geometry_and_repeat() {
    let input = source(ART);
    let old = Document::load_mem(&input).unwrap();
    let before = pdf_render::render_page(&input, 0, 1.0).unwrap();
    for req in requests() {
        let mut p = PdfDocument::from_bytes(&input, None).unwrap();
        let info = p.info().unwrap();
        p.add_vector_overlay(&req).unwrap();
        let bytes = p.verified_bytes().unwrap();
        let saved = Document::load_mem(&bytes).unwrap();
        assert_eq!(saved.version, "1.4");
        for (id, o) in &old.objects {
            if *id != (3, 0)
                && !matches!(o,Object::Stream(s)if s.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|n|n==b"XRef"))
            {
                assert_eq!(saved.get_object(*id).unwrap(), o);
            }
        }
        assert_eq!(p.info().unwrap().pages, info.pages);
        assert!(p.extract_text(&[1]).unwrap().contains("Original"));
        let after = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
        pixels(&before.rgba, &after.rgba, 4);
        if matches!(req.shape, VectorShape::Rectangle { .. }) {
            let center = &after.rgba[(100 * 300 + 200) * 4..][..3];
            for (actual, expected) in center.iter().zip([128u8, 179, 204]) {
                assert!(actual.abs_diff(expected) <= 3, "paint alpha: {center:?}");
            }
        }
        let mut again = PdfDocument::from_bytes(&bytes, None).unwrap();
        again.add_vector_overlay(&req).unwrap();
        again.verified_bytes().unwrap();
    }
}
#[test]
fn pending_source_clipping_is_consumed_before_restoring_overlay_state() {
    let input = source(&[ART, b"0 0 5 5 re W\n"].concat());
    let before = pdf_render::render_page(&input, 0, 1.0).unwrap();
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    pdf.add_vector_overlay(&requests()[0]).unwrap();
    let after = pdf_render::render_page(&pdf.verified_bytes().unwrap(), 0, 1.0).unwrap();
    pixels(&before.rgba, &after.rgba, 4);
}
#[test]
fn vector_placement_honors_crop_origins_and_all_quarter_turns() {
    for rotation in [0, 90, 180, 270] {
        let mut d = Document::load_mem(&source(ART)).unwrap();
        let page = d.get_object_mut((3, 0)).unwrap().as_dict_mut().unwrap();
        page.set(
            "CropBox",
            vec![50.into(), 70.into(), 280.into(), 270.into()],
        );
        page.set("Rotate", rotation);
        let mut input = Vec::new();
        d.save_to(&mut input).unwrap();
        let mut p = PdfDocument::from_bytes(&input, None).unwrap();
        let info = p.info().unwrap();
        p.add_vector_overlay(&requests()[0]).unwrap();
        assert_eq!(p.info().unwrap().pages, info.pages);
        let before = pdf_render::render_page(&input, 0, 1.0).unwrap();
        let after = pdf_render::render_page(&p.verified_bytes().unwrap(), 0, 1.0).unwrap();
        assert_eq!((before.width, before.height), (after.width, after.height));
        let map = |x: f64, y: f64| match rotation {
            0 => [x - 50.0, 270.0 - y],
            90 => [y - 70.0, x - 50.0],
            180 => [280.0 - x, y - 70.0],
            _ => [270.0 - y, 280.0 - x],
        };
        let a = map(127.0, 127.0);
        let b = map(273.0, 253.0);
        let mut changed = 0;
        for (i, (old, new)) in before
            .rgba
            .chunks_exact(4)
            .zip(after.rgba.chunks_exact(4))
            .enumerate()
        {
            if old != new {
                let x = (i % after.width as usize) as f64;
                let y = (i / after.width as usize) as f64;
                assert!(
                    x >= a[0].min(b[0])
                        && x < a[0].max(b[0])
                        && y >= a[1].min(b[1])
                        && y < a[1].max(b[1]),
                    "rotation{rotation}: changed{x},{y}"
                );
                changed += 1;
            }
        }
        assert!(changed > 40);
        let mut outside = requests()[0].clone();
        outside.shape = VectorShape::Line {
            from: [50.0, 100.0],
            to: [80.0, 100.0],
        };
        let snapshot = p.verified_bytes().unwrap();
        assert!(p.add_vector_overlay(&outside).is_err());
        assert_eq!(p.verified_bytes().unwrap(), snapshot);
    }
}
#[test]
fn invalid_geometry_paint_paths_and_source_fail_atomically() {
    let input = source(ART);
    let mut p = PdfDocument::from_bytes(&input, None).unwrap();
    let before = p.verified_bytes().unwrap();
    let mut invalid = requests();
    let mut x = request(VectorShape::Line {
        from: [0.0, 20.0],
        to: [30.0, 20.0],
    });
    invalid.clear();
    invalid.push(x.clone());
    x.opacity = f64::NAN;
    invalid.push(x.clone());
    x.opacity = 0.0;
    invalid.push(x.clone());
    x.opacity = 1.0;
    x.stroke.as_mut().unwrap().width = -1.0;
    invalid.push(x);
    invalid.push(request(VectorShape::Polyline {
        points: vec![[20.0, 20.0]; 4097],
    }));
    invalid.push(request(VectorShape::Line {
        from: [20.0, 20.0],
        to: [20.0, 20.0],
    }));
    let mut filled = request(VectorShape::Line {
        from: [20.0, 20.0],
        to: [30.0, 30.0],
    });
    filled.fill = Some([0.0, 0.0, 0.0]);
    invalid.push(filled);
    let mut collapsed = request(VectorShape::Rectangle {
        rectangle: [50.0, 50.0, 50.000_000_1, 70.0],
    });
    collapsed.stroke = None;
    collapsed.fill = Some([0.0, 0.0, 0.0]);
    invalid.push(collapsed);
    for req in invalid {
        assert!(p.add_vector_overlay(&req).is_err());
        assert_eq!(p.verified_bytes().unwrap(), before);
    }
    for content in [b"Q\n".as_slice(), b"BT\n", b"/Tx BMC\n"] {
        let mut p = PdfDocument::from_bytes(&source(content), None).unwrap();
        let before = p.verified_bytes().unwrap();
        assert!(p.add_vector_overlay(&requests()[0]).is_err());
        assert_eq!(p.verified_bytes().unwrap(), before);
    }
}
#[test]
fn strict_nested_schema_and_total_operator_limit() {
    let req = serde_json::to_value(&requests()[0]).unwrap();
    let mut bad = req.clone();
    bad["shape"]["path"] = serde_json::json!("private");
    assert!(serde_json::from_value::<VectorOverlay>(bad).is_err());
    let mut bad = req;
    bad["stroke"]["secret"] = serde_json::json!(1);
    assert!(serde_json::from_value::<VectorOverlay>(bad).is_err());
    let mut p = PdfDocument::from_bytes(&source(&b"n\n".repeat(99_995)), None).unwrap();
    let before = p.verified_bytes().unwrap();
    assert!(p.add_vector_overlay(&requests()[0]).is_err());
    assert_eq!(p.verified_bytes().unwrap(), before);
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_vector_pixels_text_and_original_objects() {
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    std::fs::write(&input, source(ART)).unwrap();
    assert!(
        String::from_utf8_lossy(
            &Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .contains("12.4.2")
    );
    assert!(
        String::from_utf8_lossy(
            &Command::new(bin.join("pdftoppm"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let raster = |path: &std::path::Path| {
        let out = Command::new(bin.join("pdftoppm"))
            .args(["-singlefile", "-r", "72"])
            .arg(path)
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
            .strip_prefix(b"P6\n300 300\n255\n")
            .unwrap()
            .to_vec()
    };
    let before = raster(&input);
    let json = |path: &std::path::Path| {
        let out = Command::new(&qpdf)
            .arg("--json")
            .arg(path)
            .output()
            .unwrap();
        assert!(out.status.success());
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
    };
    let old = json(&input);
    for (i, req) in requests().iter().enumerate() {
        let mut p = PdfDocument::open(&input, None).unwrap();
        p.add_vector_overlay(req).unwrap();
        let output = temp.path().join(format!("shape{i}.pdf"));
        p.save_new(&output).unwrap();
        assert!(
            Command::new(&qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let rendered = raster(&output);
        pixels(&before, &rendered, 3);
        if i == 0 {
            let center = &rendered[(100 * 300 + 200) * 3..][..3];
            for (actual, expected) in center.iter().zip([128u8, 179, 204]) {
                assert!(
                    actual.abs_diff(expected) <= 3,
                    "independent paint alpha: {center:?}"
                );
            }
        }
        let new = json(&output);
        for (id, o) in old["qpdf"][1].as_object().unwrap() {
            if id != "obj:3 0 R" && id != "trailer" && o["stream"]["dict"]["/Type"] != "/XRef" {
                assert_eq!(&new["qpdf"][1][id], o);
            }
        }
        let text = Command::new(bin.join("pdftotext"))
            .arg("-raw")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert_eq!(String::from_utf8(text.stdout).unwrap(), "Original\u{c}");
    }
    assert_eq!(std::fs::read(input).unwrap(), source(ART));
}
