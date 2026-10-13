#![allow(clippy::unwrap_used)]
use lopdf::Document;
use pdf_core::PdfDocument;
#[path = "../../../tests/support/redaction_fixture.rs"]
mod fixture;

fn cases() -> [&'static [u8]; 6] {
    [
        b"BT /F1 10 Tf 2 Tc 3 Tw 80 Tz 4 Ts 20 180 Td (prefix PRIVATE suffix) Tj ET BT 20 120 Td (before PRIVATE after) Tj ET",
        b"BT /F1 10 Tf 1 Tc 2 Tw 75 Tz 3 Ts 20 180 Td (prefix PRIVATE suffix) Tj ET q BT /F1 12 Tf -0.5 Tc -2 Tw 120 Tz -4 Ts 20 140 Td [(before PRI) -12.123456789012345678 (VATE after)] TJ ET Q BT 20 80 Td (third PRIVATE final) Tj ET",
        b"BT /F1 10 Tf 12 TL 1 Tc 2 Tw 90 Tz 20 180 Td (prefix PRIVATE suffix) Tj (before PRIVATE after) ' 3 0.5 (third PRIVATE final) \" ET",
        b"BT /F1 10 Tf 3 Tr 20 180 Td (prefix PRIVATE suffix) Tj ET BT 20 120 Td (before PRIVATE after) Tj ET",
        b"BT /F1 10.123456789012345 Tf 0.123456789012345678 Tc 1.2345678901234567 Tw 81.234567890123456 Tz 0.123456789012345 Ts 20 180 Td 0 Tr (prefix PRIVATE suffix) Tj ET",
        b"/F1 10 Tf 12 TL BT 20 180 Td 0 -12 TD (prefix PRIVATE suffix) Tj T* (before PRIVATE after) Tj ET",
    ]
}
fn controls() -> [&'static [u8]; 6] {
    // Independent Courier600 controls; values are hand-derived from PDF text
    // advances, not obtained from the redactor or its standard-metric helper.
    [
        b"BT /F1 10 Tf 2 Tc 3 Tw 80 Tz 4 Ts 20 180 Td [(prefix ) -5600 ( suffix)] TJ ET BT 20 120 Td [(before ) -5600 ( after)] TJ ET",
        b"BT /F1 10 Tf 1 Tc 2 Tw 75 Tz 3 Ts 20 180 Td [(prefix ) -4900 ( suffix)] TJ ET q BT /F1 12 Tf -0.5 Tc -2 Tw 120 Tz -4 Ts 20 140 Td [(before ) -1675 -12.123456789012345678 -2233.333333333333 ( after)] TJ ET Q BT 20 80 Td [(third ) -4900 ( final)] TJ ET",
        b"BT /F1 10 Tf 12 TL 1 Tc 2 Tw 90 Tz 20 180 Td [(prefix ) -4900 ( suffix)] TJ T* [(before ) -4900 ( after)] TJ 3 Tw 0.5 Tc T* [(third ) -4550 ( final)] TJ ET",
        b"BT /F1 10 Tf 3 Tr 20 180 Td [(prefix ) -4200 ( suffix)] TJ ET BT 20 120 Td [(before ) -4200 ( after)] TJ ET",
        b"BT /F1 10.123456789012345 Tf 0.123456789012345678 Tc 1.2345678901234567 Tw 81.234567890123456 Tz 0.123456789012345 Ts 20 180 Td 0 Tr [(prefix ) -4285.365852899613330344008669767919450243 ( suffix)] TJ ET",
        b"/F1 10 Tf 12 TL BT 20 180 Td 0 -12 TD [(prefix ) -4200 ( suffix)] TJ T* [(before ) -4200 ( after)] TJ ET",
    ]
}
fn source(program: &[u8]) -> Vec<u8> {
    let mut doc = Document::load_mem(&fixture::fixture()).unwrap();
    doc.get_object_mut((5, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(program.to_vec());
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn wide_source(program: &[u8]) -> Vec<u8> {
    let mut doc = Document::load_mem(&source(program)).unwrap();
    doc.get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "MediaBox",
            vec![lopdf::Object::Integer(0), 0.into(), 400.into(), 300.into()],
        );
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn actual_text_state_inheritance_spacing_rise_invisible_text_and_quotes_are_removed() {
    for program in cases() {
        let source = source(program);
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        assert!(pdf.extract_text(&[1, 2]).unwrap().contains(fixture::SECRET));
        let (copy, report) = pdf.redact_text_copy(&[fixture::SECRET.into()]).unwrap();
        assert!(report.changed_text_shows >= 2);
        assert_eq!(copy.info().unwrap().pages, pdf.info().unwrap().pages);
        let bytes = copy.verified_bytes().unwrap();
        pdf_core::verify_redaction_bytes(&bytes, None, &[fixture::SECRET.into()]).unwrap();
        assert!(
            !bytes
                .windows(fixture::SECRET.len())
                .any(|s| s == fixture::SECRET.as_bytes())
        );
        let text = copy.extract_text(&[1, 2]).unwrap();
        assert!(text.contains("prefix") && text.contains("suffix"), "{text}");
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
}
#[test]
fn clipping_unknown_state_bad_quote_operands_and_invalid_layout_refuse_atomically() {
    for program in [
        b"BT /F1 10 Tf 4 Tr 20 180 Td (PRIVATE) Tj ET".as_slice(),
        b"BT /F1 10 Tf 7 Tr 20 180 Td (PRIVATE) Tj ET".as_slice(),
        b"BT /F1 10 Tf 0 Tz 20 180 Td (PRIVATE) Tj ET".as_slice(),
        b"BT /F1 0 Tf 20 180 Td (PRIVATE) Tj ET".as_slice(),
        b"BT /F1 10 Tf 20 180 Td (PRIVATE) (extra) ' ET".as_slice(),
        b"BT /F1 10 Tf 20 180 Td (bad) 1 (PRIVATE) \" ET".as_slice(),
        b"BT /F1 10 Tf 20 180 Td 1 (PRIVATE) \" ET".as_slice(),
        b"BT /F1 10 Tf 20 180 Td (PRIVATE) Tj q Q ET".as_slice(),
    ] {
        let bytes = source(program);
        let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        assert!(pdf.redact_text_copy(&[fixture::SECRET.into()]).is_err());
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
}
#[test]
fn removed_space_advance_and_all_unmodified_precision_tokens_are_retained() {
    for (word, expected) in [("3", "-11500"), ("-2", "-11000")] {
        let bytes = source(
            format!(
                "BT /F1 10 Tf 2 Tc {word} Tw 20 180 Td (prefix PRIVATE suffix) Tj ( tail) Tj ET"
            )
            .as_bytes(),
        );
        let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let (copy, _) = pdf.redact_text_copy(&["PRIVATE suffix".into()]).unwrap();
        let output = copy.verified_bytes().unwrap();
        let doc = Document::load_mem(&output).unwrap();
        let content = doc.get_page_content(doc.get_pages()[&1]);
        assert!(String::from_utf8(content).unwrap().contains(expected));
        let text = copy.extract_text(&[1, 2]).unwrap();
        assert!(text.contains("prefix") && text.contains("tail"));
        assert!(!text.contains("suffix"));
    }
    let bytes = source(cases()[4]);
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let (copy, _) = pdf.redact_text_copy(&[fixture::SECRET.into()]).unwrap();
    let output = copy.verified_bytes().unwrap();
    for token in [
        "10.123456789012345 Tf",
        "0.123456789012345678 Tc",
        "1.2345678901234567 Tw",
        "81.234567890123456 Tz",
        "0.123456789012345 Ts",
    ] {
        assert!(
            output
                .windows(token.len())
                .any(|bytes| bytes == token.as_bytes())
        );
    }
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_removed_spaces_keep_following_glyph_positions_and_pixels() {
    use std::process::Command;
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
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
            &Command::new(bin.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let temp = tempfile::tempdir().unwrap();
    // Keep the following word in the page at both rotations and retain readable
    // 10-point word grouping. A smaller font made this reader split the letters.
    for (word, adjustment) in [("3", "11500"), ("-2", "11000")] {
        let bytes = wide_source(
            format!(
                "BT /F1 10 Tf 2 Tc {word} Tw 20 180 Td (prefix PRIVATE suffix) Tj ( tail) Tj ET"
            )
            .as_bytes(),
        );
        let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let (copy, _) = pdf.redact_text_copy(&["PRIVATE suffix".into()]).unwrap();
        let output = copy.verified_bytes().unwrap();
        let control = wide_source(
            format!(
                "BT /F1 10 Tf 2 Tc {word} Tw 20 180 Td [(prefix ) -{adjustment}] TJ ( tail) Tj ET"
            )
            .as_bytes(),
        );
        let mut boxes = Vec::new();
        let mut files = Vec::new();
        for (label, data) in [
            ("before", &bytes),
            ("after", &output),
            ("control", &control),
        ] {
            let path = temp.path().join(format!("{word}-{label}.pdf"));
            std::fs::write(&path, data).unwrap();
            assert!(
                Command::new(&qpdf)
                    .arg("--check")
                    .arg(&path)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            let text = Command::new(bin.join("pdftotext"))
                .arg("-bbox")
                .arg(&path)
                .arg("-")
                .output()
                .unwrap();
            assert!(text.status.success());
            let text = String::from_utf8(text.stdout).unwrap();
            let mut records = text
                .lines()
                .filter(|line| line.contains(">prefix</word>") || line.contains(">tail</word>"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            records.sort();
            boxes.push(records);
            files.push(path);
        }
        assert_eq!(boxes[0].len(), 4, "word-space {word}: {boxes:?}");
        assert_eq!(boxes[0], boxes[1]);
        assert_eq!(boxes[0], boxes[2]);
        for page in 0..2 {
            assert!(
                pdf_render::render_page(&output, page, 1.0).unwrap().rgba
                    == pdf_render::render_page(&control, page, 1.0).unwrap().rgba
            );
            let mut images = Vec::new();
            for (index, path) in files.iter().enumerate() {
                let prefix = temp.path().join(format!("{word}-{page}-{index}"));
                assert!(
                    Command::new(bin.join("pdftoppm"))
                        .args([
                            "-r",
                            "72",
                            "-f",
                            &(page + 1).to_string(),
                            "-l",
                            &(page + 1).to_string(),
                            "-singlefile"
                        ])
                        .arg(path)
                        .arg(&prefix)
                        .output()
                        .unwrap()
                        .status
                        .success()
                );
                images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
            }
            assert!(images[1] == images[2]);
            assert!(images[0] != images[1]);
        }
    }
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_text_state_advances_and_invisible_text_are_proved_with_readers() {
    use std::process::Command;
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
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
            &Command::new(bin.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let temp = tempfile::tempdir().unwrap();
    for font in [
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
        "Times-Roman",
        "Times-Bold",
        "Times-Italic",
        "Times-BoldItalic",
    ] {
        for (index, program) in cases().into_iter().enumerate() {
            let mut doc = Document::load_mem(&source(program)).unwrap();
            doc.get_object_mut((3, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("BaseFont", font);
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();
            let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
            let (copy, _) = pdf.redact_text_copy(&[fixture::SECRET.into()]).unwrap();
            let output = copy.verified_bytes().unwrap();
            let before = temp.path().join(format!("{font}-{index}-before.pdf"));
            let after = temp.path().join(format!("{font}-{index}-after.pdf"));
            std::fs::write(&before, &bytes).unwrap();
            std::fs::write(&after, &output).unwrap();
            let mut boxes = Vec::new();
            for path in [&before, &after] {
                assert!(
                    Command::new(&qpdf)
                        .arg("--check")
                        .arg(path)
                        .output()
                        .unwrap()
                        .status
                        .success()
                );
                let qdf = path.with_extension("qdf");
                assert!(
                    Command::new(&qpdf)
                        .args(["--qdf", "--object-streams=disable"])
                        .arg(path)
                        .arg(&qdf)
                        .output()
                        .unwrap()
                        .status
                        .success()
                );
                let decoded = std::fs::read(qdf).unwrap();
                assert_eq!(
                    decoded
                        .windows(fixture::SECRET.len())
                        .any(|v| v == fixture::SECRET.as_bytes()),
                    path == &before
                );
                let text = Command::new(bin.join("pdftotext"))
                    .arg("-bbox")
                    .arg(path)
                    .arg("-")
                    .output()
                    .unwrap();
                assert!(text.status.success());
                let text = String::from_utf8(text.stdout).unwrap();
                assert_eq!(
                    text.contains(fixture::SECRET),
                    path == &before,
                    "{font}-{index}"
                );
                boxes.push(
                    text.lines()
                        .filter(|line| {
                            ["prefix", "suffix", "before", "after", "third", "final"]
                                .iter()
                                .any(|word| line.contains(&format!(">{word}</word>")))
                        })
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                );
            }
            assert!(!boxes[0].is_empty());
            // Empty gaps can change Poppler's column grouping. Compare every
            // complete word/coordinate record exactly, retaining multiplicity.
            boxes[0].sort();
            boxes[1].sort();
            assert_eq!(boxes[0], boxes[1], "{font}-{index}");
            if font == "Courier" {
                let control = source(controls()[index]);
                let control_path = temp.path().join(format!("{index}-control.pdf"));
                std::fs::write(&control_path, &control).unwrap();
                for page in 0..2 {
                    let before_pixels = pdf_render::render_page(&bytes, page, 1.0).unwrap().rgba;
                    let after_pixels = pdf_render::render_page(&output, page, 1.0).unwrap().rgba;
                    let control_pixels = pdf_render::render_page(&control, page, 1.0).unwrap().rgba;
                    assert!(
                        after_pixels == control_pixels,
                        "native control case{index} page{page}"
                    );
                    assert_eq!(
                        before_pixels == after_pixels,
                        index == 3,
                        "native visible/invisible case{index}"
                    );
                    let mut images = Vec::new();
                    for (label, path) in [
                        ("before", &before),
                        ("after", &after),
                        ("control", &control_path),
                    ] {
                        let prefix = temp.path().join(format!("{index}-{page}-{label}"));
                        assert!(
                            Command::new(bin.join("pdftoppm"))
                                .args([
                                    "-r",
                                    "72",
                                    "-f",
                                    &(page + 1).to_string(),
                                    "-l",
                                    &(page + 1).to_string(),
                                    "-singlefile"
                                ])
                                .arg(path)
                                .arg(&prefix)
                                .output()
                                .unwrap()
                                .status
                                .success()
                        );
                        images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
                    }
                    assert!(
                        images[1] == images[2],
                        "Poppler control case{index} page{page}"
                    );
                    assert_eq!(
                        images[0] == images[1],
                        index == 3,
                        "Poppler visible/invisible case{index}"
                    );
                }
            }
            assert_eq!(std::fs::read(&before).unwrap(), bytes);
        }
    }
}
