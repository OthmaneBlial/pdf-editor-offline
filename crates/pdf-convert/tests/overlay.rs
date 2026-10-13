#![allow(clippy::unwrap_used)]
use pdf_core::{ImageOverlay, PdfDocument};
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn png() -> Vec<u8> {
    let mut output = vec![];
    {
        let mut encoder = png::Encoder::new(&mut output, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 0, 0, 255, 0])
            .unwrap();
    }
    output
}
fn jpeg() -> Vec<u8> {
    let mut output = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 95)
        .encode(
            &[255, 0, 0, 0, 0, 255],
            2,
            1,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    output
}
fn overlay(rotation: u16) -> ImageOverlay {
    ImageOverlay {
        page: 1,
        rectangle: [60.0, 30.0, 100.0, 70.0],
        rotation,
    }
}
#[test]
fn actual_png_alpha_and_jpeg_insert_are_verified_preserve_source_and_visible_outside_rectangle() {
    let source = support::fixture(2);
    let baseline = pdf_render::render_page(&source, 0, 1.0).unwrap();
    for image in [png(), jpeg()] {
        for rotation in [0, 90, 180, 270] {
            let mut document = PdfDocument::from_bytes(&source, None).unwrap();
            let report =
                pdf_convert::add_image_overlay(&mut document, &overlay(rotation), &image).unwrap();
            assert_eq!((report.width, report.height), (2, 1));
            let bytes = document.verified_bytes().unwrap();
            let rendered = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
            assert_eq!(
                (baseline.width, baseline.height),
                (rendered.width, rendered.height)
            );
            let mut changed = 0;
            for (index, (a, b)) in baseline
                .rgba
                .chunks_exact(4)
                .zip(rendered.rgba.chunks_exact(4))
                .enumerate()
            {
                let x = index % baseline.width as usize;
                let y = index / baseline.width as usize;
                if (59..=101).contains(&x) && (229..=271).contains(&y) {
                    changed += usize::from(a != b);
                } else {
                    assert_eq!(a, b, "outside overlay at{x},{y}");
                }
            }
            assert!(changed > 500);
            if report.format == "png" {
                let (opaque, transparent) = match rotation {
                    0 => ((70, 250), (90, 250)),
                    90 => ((80, 260), (80, 240)),
                    180 => ((90, 250), (70, 250)),
                    _ => ((80, 240), (80, 260)),
                };
                let pixel =
                    |point: (usize, usize)| (point.1 * baseline.width as usize + point.0) * 4;
                let opaque = pixel(opaque);
                let transparent = pixel(transparent);
                assert_eq!(&rendered.rgba[opaque..opaque + 4], &[255, 0, 0, 255]);
                assert_eq!(
                    &rendered.rgba[transparent..transparent + 4],
                    &baseline.rgba[transparent..transparent + 4]
                );
            }
            assert_eq!(
                document.extract_text(&[1, 2]).unwrap(),
                PdfDocument::from_bytes(&source, None)
                    .unwrap()
                    .extract_text(&[1, 2])
                    .unwrap()
            );
            assert_eq!(
                pdf_render::render_page(&source, 1, 1.0).unwrap().rgba,
                pdf_render::render_page(&bytes, 1, 1.0).unwrap().rgba
            );
            let parsed = lopdf::Document::load_mem(&bytes).unwrap();
            let found = parsed
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .find(|stream| {
                    stream
                        .dict
                        .get(b"Width")
                        .is_ok_and(|value| value.as_i64().is_ok_and(|n| n == 2))
                        && stream.dict.get(b"ColorSpace").is_ok_and(|value| {
                            value.as_name().is_ok_and(|name| name == b"DeviceRGB")
                        })
                })
                .unwrap();
            if report.format == "jpeg" {
                assert_eq!(found.content, image);
                assert_eq!(
                    found.dict.get(b"Filter").unwrap().as_name().unwrap(),
                    b"DCTDecode"
                );
            } else {
                assert!(found.dict.has(b"SMask"));
            }
        }
    }
}
#[test]
fn unsupported_and_bad_images_leave_source_identical() {
    let mut document = PdfDocument::from_bytes(&support::fixture(1), None).unwrap();
    let before = document.verified_bytes().unwrap();
    for bad in [
        b"GIF89a".as_slice(),
        b"\xff\xd8\xff\xd9",
        b"\x89PNG\r\n\x1a\n",
    ] {
        assert!(pdf_convert::add_image_overlay(&mut document, &overlay(0), bad).is_err());
        assert_eq!(document.verified_bytes().unwrap(), before);
    }
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_png_jpeg_rotation_and_alpha_preserve_original_page_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    std::fs::write(&source, support::fixture(2)).unwrap();
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let reference = |tool: &str, path: &std::path::Path| {
        let mut command = std::process::Command::new(bin.join(tool));
        command.args(["-f", "1", "-l", "1"]);
        if tool == "pdftoppm" {
            command.args(["-r", "72", "-singlefile"]);
        }
        command.arg(path);
        if tool == "pdftotext" {
            command.arg("-");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    };
    let before = reference("pdftoppm", &source);
    let split = |ppm: Vec<u8>| {
        let marker = b"\n255\n";
        let start = ppm
            .windows(marker.len())
            .position(|part| part == marker)
            .unwrap()
            + marker.len();
        (ppm[..start].to_vec(), ppm[start..].to_vec())
    };
    let (header, pixels) = split(before);
    for (index, image) in [png(), jpeg()].into_iter().enumerate() {
        for rotation in [0, 90, 180, 270] {
            let mut document = PdfDocument::open(&source, None).unwrap();
            pdf_convert::add_image_overlay(&mut document, &overlay(rotation), &image).unwrap();
            let output = temp.path().join(format!("{index}-{rotation}.pdf"));
            document.save_new(&output).unwrap();
            assert!(
                std::process::Command::new(&qpdf)
                    .arg("--check")
                    .arg(&output)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            assert_eq!(
                reference("pdftotext", &source),
                reference("pdftotext", &output)
            );
            let (after_header, after) = split(reference("pdftoppm", &output));
            assert_eq!(header, after_header);
            let mut changed = 0;
            for (index, (a, b)) in pixels
                .chunks_exact(3)
                .zip(after.chunks_exact(3))
                .enumerate()
            {
                let x = index % 200;
                let y = index / 200;
                if (59..=101).contains(&x) && (229..=271).contains(&y) {
                    changed += usize::from(a != b);
                } else {
                    assert_eq!(a, b);
                }
            }
            assert!(changed > 500);
        }
    }
    assert_eq!(std::fs::read(source).unwrap(), support::fixture(2));
}
#[test]
fn validated_png_srgb_profile_is_imported_without_foreign_resource_references() {
    let mut bytes = vec![];
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[100, 150, 200])
            .unwrap();
    }
    let mut document = PdfDocument::from_bytes(&support::fixture(1), None).unwrap();
    let report = pdf_convert::add_image_overlay(&mut document, &overlay(0), &bytes).unwrap();
    assert!(report.srgb_profile);
    let bytes = document.verified_bytes().unwrap();
    assert!(pdf_render::render_page(&bytes, 0, 1.0).is_ok());
    let parsed = lopdf::Document::load_mem(&bytes).unwrap();
    let image=parsed.objects.values().filter_map(|object|object.as_stream().ok()).find(|stream|matches!(stream.dict.get(b"ColorSpace"),Ok(lopdf::Object::Array(values)) if values.first().is_some_and(|value|value.as_name().is_ok_and(|name|name==b"ICCBased")))).unwrap();
    let space = image.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
    let profile = parsed
        .get_object(space[1].as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(profile.dict.get(b"N").unwrap().as_i64().unwrap(), 3);
    assert_eq!(&profile.content[36..40], b"acsp");
}
#[test]
fn insertion_on_rotated_page_uses_pdf_coordinates_and_preserves_other_artwork() {
    let source = support::fixture(2);
    let before = pdf_render::render_page(&source, 1, 1.0).unwrap();
    let mut document = PdfDocument::from_bytes(&source, None).unwrap();
    let mut placement = overlay(90);
    placement.page = 2;
    pdf_convert::add_image_overlay(&mut document, &placement, &png()).unwrap();
    let after = pdf_render::render_page(&document.verified_bytes().unwrap(), 1, 1.0).unwrap();
    assert_eq!((before.width, before.height), (300, 200));
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut changed = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        let x = index % 300;
        let y = index / 300;
        if (29..=71).contains(&x) && (59..=101).contains(&y) {
            changed += usize::from(a != b);
        } else {
            assert_eq!(a, b, "rotated page outside rectangle at{x},{y}");
        }
    }
    assert!(changed > 500);
}
