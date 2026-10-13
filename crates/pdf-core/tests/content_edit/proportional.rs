use super::*;
use pdf_core::ContentTextLayoutPolicy;

const FAMILIES: [&str; 8] = [
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
];
const PREFIX:&[u8]=b"0.2 0.4 0.6 rg 10 10 20 20 re f\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (1234 tail) Tj ET\nBT /F 12 Tf 1 0 0 1 40 100 Tm 0 Tr (Next) Tj ET\n";
fn font(family: &str) -> lopdf::Dictionary {
    dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>family,"Encoding"=>"WinAnsiEncoding"}
}
#[test]
fn proportional_exact_word_advance_preserves_original_policy_and_font_objects() {
    for family in FAMILIES {
        let bytes = source(PREFIX, font(family));
        let original = Document::load_mem(&bytes).unwrap();
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let runs = pdf.content_text_runs(1).unwrap();
        assert_eq!(runs[0].standard_font.unwrap().pdf_name(), family);
        assert!(runs[0].validate_replacement("9876 tail").is_ok());
        assert!(runs[0].validate_replacement("WWWW tail").is_err());
        assert!(runs[0].validate_replacement("1234 longer tail").is_err());
        let before = pdf.to_bytes().unwrap();
        assert!(
            pdf.replace_content_text(1, &[change(&pdf, 0, "WWWW tail")])
                .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
        pdf.replace_content_text(1, &[change(&pdf, 0, "9876 tail")])
            .unwrap();
        let after = Document::load_mem(&pdf.to_bytes().unwrap()).unwrap();
        for (id, object) in &original.objects {
            if *id != (3, 0) {
                if let Some(current) = after.objects.get(id) {
                    assert_eq!(*current, *object, "{family} object{id:?}");
                } else {
                    // Saving regenerates transport XRef streams; no logical
                    // source font/resource/content object may disappear.
                    assert_eq!(
                        object
                            .as_stream()
                            .unwrap()
                            .dict
                            .get(b"Type")
                            .unwrap()
                            .as_name()
                            .unwrap(),
                        b"XRef"
                    );
                    let transports = after
                        .objects
                        .iter()
                        .filter_map(|(id, object)| {
                            object
                                .as_stream()
                                .ok()
                                .filter(|stream| {
                                    stream.dict.get(b"Type").and_then(Object::as_name).ok()
                                        == Some(b"XRef")
                                })
                                .map(|_| *id)
                        })
                        .collect::<Vec<_>>();
                    eprintln!(
                        "{family}: regenerated transport source XRef {id:?}, candidate XRef {transports:?}; every other source object except selected page remains exact"
                    );
                }
            }
        }
        assert_eq!(
            pdf.extract_text(&[1])
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["9876", "tail", "Next"]
        );
        assert_eq!(
            pdf.extract_text(&[2])
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["1234", "tail", "Next"]
        );
    }
}
#[test]
fn exact_metrics_allow_real_variable_counts_only_with_explicit_zero_character_spacing() {
    for (family, old, new) in [
        ("Helvetica", "O tail", "tti tail"),
        ("Times-Roman", "M tail", "fii tail"),
    ] {
        for zero in [false, true] {
            let content = format!(
                "BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr {} ({old}) Tj ET",
                if zero { "0 Tc" } else { "" }
            );
            let mut pdf =
                PdfDocument::from_bytes(&source(content.as_bytes(), font(family)), None).unwrap();
            let run = &pdf.content_text_runs(1).unwrap()[0];
            assert_eq!(run.validate_replacement(new).is_ok(), zero);
            if zero {
                let original = run.replacement_position(old).unwrap();
                assert_eq!(
                    original.nominal_advance,
                    run.replacement_position(new).unwrap().nominal_advance
                );
                pdf.replace_content_text(1, &[change(&pdf, 0, new)])
                    .unwrap();
                pdf.replace_content_text(1, &[change(&pdf, 0, old)])
                    .unwrap();
                assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), old);
            }
        }
    }
}
#[test]
fn explicit_width_change_tracks_inherited_spacing_transforms_and_graphics_restore() {
    let content=b"2 Tc 4 Tw 80 Tz 3 Ts q 1 0.2 0.1 1 5 7 cm BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr 1 Tc 5 Tw 120 Tz 4 Ts (Original tail) Tj ET BT /F 12 Tf 1 0 0 1 40 100 Tm 0 Tr (Next) Tj ET Q BT /F 12 Tf 1 0 0 1 40 80 Tm 0 Tr (Outer) Tj ET";
    let bytes = source(content, font("Helvetica"));
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let runs = pdf.content_text_runs(1).unwrap();
    let first = runs[0].layout.as_ref().unwrap();
    assert_eq!(
        [
            first.character_spacing,
            first.word_spacing,
            first.horizontal_scale,
            first.rise
        ],
        [1.0, 5.0, 120.0, 4.0]
    );
    assert_eq!(runs[1].layout.as_ref().unwrap().character_spacing, 1.0);
    let outer = runs[2].layout.as_ref().unwrap();
    assert_eq!(
        [
            outer.character_spacing,
            outer.word_spacing,
            outer.horizontal_scale,
            outer.rise
        ],
        [2.0, 4.0, 80.0, 3.0]
    );
    assert_eq!(outer.current_transform, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let replacement = "A considerably longer value";
    let old = runs[0].replacement_position("Original tail").unwrap();
    // Column-vector CTM * Tm maps (0, rise4) through the
    // translated/skewed source matrix to (59.4,159), not Tm * CTM.
    assert!((old.baseline_start[0] - 59.4).abs() < 0.0001);
    assert!((old.baseline_start[1] - 159.0).abs() < 0.0001);
    assert_eq!(
        runs[2]
            .replacement_position("Outer")
            .unwrap()
            .baseline_start,
        [40.0, 83.0]
    );
    let new = runs[0].replacement_position(replacement).unwrap();
    assert_eq!(old.baseline_start, new.baseline_start);
    assert!(new.nominal_advance > old.nominal_advance);
    let mut request = change(&pdf, 0, replacement);
    request.layout_policy = ContentTextLayoutPolicy::AllowWidthChange;
    pdf.replace_content_text(1, &[request]).unwrap();
    let current = pdf.content_text_runs(1).unwrap();
    for index in [1, 2] {
        assert_eq!(
            serde_json::to_value(&current[index].layout).unwrap(),
            serde_json::to_value(&runs[index].layout).unwrap()
        );
        assert_eq!(current[index].text, runs[index].text);
    }
    assert_eq!(
        Document::load_mem(&pdf.to_bytes().unwrap())
            .unwrap()
            .get_dictionary((4, 0))
            .unwrap(),
        Document::load_mem(&bytes)
            .unwrap()
            .get_dictionary((4, 0))
            .unwrap()
    );
}
#[test]
fn malformed_state_and_unbounded_width_changes_refuse_atomically() {
    for content in [
        "Q BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET",
        "q BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET",
        "(bad) Tc BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET",
        "1 2 cm BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET",
        "BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET (dependent invalid show) Tj",
    ] {
        let pdf =
            PdfDocument::from_bytes(&source(content.as_bytes(), font("Helvetica")), None).unwrap();
        assert!(pdf.content_text_runs(1).is_err());
    }
    let mut pdf = PdfDocument::from_bytes(
        &source(
            b"BT /F 1000 Tf 1 0 0 1 40 140 Tm 0 Tr (A) Tj ET",
            font("Helvetica"),
        ),
        None,
    )
    .unwrap();
    let before = pdf.to_bytes().unwrap();
    let mut request = change(&pdf, 0, &"W".repeat(2000));
    request.layout_policy = ContentTextLayoutPolicy::AllowWidthChange;
    assert!(pdf.replace_content_text(1, &[request]).is_err());
    assert_eq!(pdf.to_bytes().unwrap(), before);
    let run = &pdf.content_text_runs(1).unwrap()[0];
    assert!(
        run.validate_replacement_with_policy("東京", ContentTextLayoutPolicy::AllowWidthChange)
            .is_err()
    );
    // A bounded final endpoint is insufficient: earlier glyph origins can
    // exceed the budget and return through negative character spacing.
    let pdf = PdfDocument::from_bytes(
        &source(
            b"1000000 Tw -500000 Tc BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr ( ) Tj ET",
            font("Helvetica"),
        ),
        None,
    )
    .unwrap();
    let run = &pdf.content_text_runs(1).unwrap()[0];
    assert!(run.replacement_position("aaaa    ").is_err());
    assert!(
        run.validate_replacement_with_policy(
            &"a".repeat(65537),
            ContentTextLayoutPolicy::AllowWidthChange
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10.0"]
fn independent_proportional_variable_width_text_anchors_pixels_and_shared_pages() {
    let qpdf = PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap());
    let poppler = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        Command::new(&qpdf)
            .arg("--version")
            .output()
            .unwrap()
            .stdout
            .starts_with(b"qpdf version 12.4.2")
    );
    assert!(
        String::from_utf8_lossy(
            &Command::new(poppler.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    for family in FAMILIES {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source.pdf");
        let output = temp.path().join("new.pdf");
        let content = [b"1 Tc 3 Tw 80 Tz 2 Ts\n".as_slice(), PREFIX].concat();
        let bytes = source(&content, font(family));
        std::fs::write(&input, &bytes).unwrap();
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let original_run = pdf.content_text_runs(1).unwrap().remove(0);
        let predicted_tail_delta = original_run
            .replacement_position("Much longer ")
            .unwrap()
            .baseline_end[0]
            - original_run
                .replacement_position("1234 ")
                .unwrap()
                .baseline_end[0];
        let mut request = change(&pdf, 0, "Much longer tail");
        request.layout_policy = ContentTextLayoutPolicy::AllowWidthChange;
        pdf.replace_content_text(1, &[request]).unwrap();
        pdf.save_new(&output).unwrap();
        let check = Command::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        let text = Command::new(poppler.join("pdftotext"))
            .args(["-f", "1", "-l", "1"])
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert_eq!(
            String::from_utf8(text.stdout)
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["Much", "longer", "tail", "Next"]
        );
        let mut boxes = Vec::new();
        let mut rasters = Vec::new();
        for (index, path) in [&input, &output].into_iter().enumerate() {
            let bbox = Command::new(poppler.join("pdftotext"))
                .args(["-f", "1", "-l", "1", "-bbox"])
                .arg(path)
                .arg("-")
                .output()
                .unwrap();
            assert!(bbox.status.success());
            boxes.push(String::from_utf8(bbox.stdout).unwrap());
            for page in [1, 2] {
                rasters.push(ppm(
                    &poppler,
                    path,
                    temp.path().join(format!("{index}-{page}")),
                    page,
                ));
            }
        }
        assert_eq!(word_box(&boxes[0], "Next"), word_box(&boxes[1], "Next"));
        let before = word_box(&boxes[0], "1234");
        let after = word_box(&boxes[1], "Much");
        assert!((before[0] - after[0]).abs() < 0.001 && (before[1] - after[1]).abs() < 0.001);
        let old_tail = word_box(&boxes[0], "tail");
        let new_tail = word_box(&boxes[1], "tail");
        assert!(new_tail[0] > old_tail[0] + 10.0);
        assert!(
            ((new_tail[0] - old_tail[0]) - predicted_tail_delta).abs() < 0.001,
            "{family} inherited Tc/Tw/Tz tail delta differs from nominal prediction"
        );
        let mut changed = 0;
        for (index, (old, new)) in rasters[0]
            .chunks_exact(3)
            .zip(rasters[2].chunks_exact(3))
            .enumerate()
        {
            if old != new {
                changed += 1;
                let (x, y) = (index % 200, index / 200);
                assert!(
                    (30..200).contains(&x) && (30..77).contains(&y),
                    "{family} outside verified union envelope {x},{y}"
                );
            }
        }
        assert!(changed > 20);
        assert_eq!(rasters[1], rasters[3]);
        let native_before = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
        let native_after = pdf_render::render_page(&pdf.to_bytes().unwrap(), 0, 1.0).unwrap();
        let mut native_changed = 0;
        for (index, (old, new)) in native_before
            .rgba
            .chunks_exact(4)
            .zip(native_after.rgba.chunks_exact(4))
            .enumerate()
        {
            if old != new {
                native_changed += 1;
                assert!((30..200).contains(&(index % 200)) && (30..77).contains(&(index / 200)));
            }
        }
        assert!(native_changed > 20);
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}
fn word_box(xml: &str, text: &str) -> [f64; 4] {
    let line = xml
        .lines()
        .find(|line| line.ends_with(&format!(">{text}</word>")))
        .unwrap();
    ["xMin", "yMin", "xMax", "yMax"].map(|key| {
        line.split(&format!("{key}=\""))
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    })
}
fn ppm(poppler: &std::path::Path, path: &std::path::Path, base: PathBuf, page: u32) -> Vec<u8> {
    let result = Command::new(poppler.join("pdftoppm"))
        .args([
            "-r",
            "72",
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
        ])
        .arg(path)
        .arg(&base)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(base.with_extension("ppm")).unwrap();
    let header = b"P6\n200 200\n255\n";
    assert!(bytes.starts_with(header));
    bytes[header.len()..].to_vec()
}
