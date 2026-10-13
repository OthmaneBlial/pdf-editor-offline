use super::*;
use pdf_core::{
    ContentTextEmbeddedChange, ContentTextFaceChange, ContentTextFont, ContentTextLayoutPolicy,
    ContentTextPropertyChange,
};
const FONT: &[u8] = include_bytes!("../fixtures/fonts/amiri/Amiri-Regular.ttf");
fn request(pdf: &PdfDocument, value: Option<&str>) -> ContentTextEmbeddedChange {
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    ContentTextEmbeddedChange {
        identity: run.identity,
        expected: run.text.unwrap(),
        replacement: value.map(str::to_owned),
        layout_policy: ContentTextLayoutPolicy::AllowWidthChange,
    }
}
fn raw(doc: &Document, page: lopdf::ObjectId) -> Vec<u8> {
    let id = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_object(id)
        .unwrap()
        .as_stream()
        .unwrap()
        .decompressed_content_with_limit(pdf_core::MAX_STREAM_BYTES)
        .unwrap()
}
#[test]
fn genuine_ttf_unicode_show_reopens_and_repeats_value_properties_and_standard_face() {
    const PREFIX: &[u8] =
        b"% precision remains exact\n1.0000000123456789 0 0 1 0.1234567890123456 0 cm\n";
    let bytes = source(&[PREFIX, CONTENT].concat(), courier());
    let original = Document::load_mem(&bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    pdf.change_content_text_embedded_font(1, &request(&pdf, Some("Café")), FONT)
        .unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let output = Document::load_mem(&saved).unwrap();
    assert!(raw(&output, (3, 0)).starts_with(PREFIX));
    assert!(
        !raw(&output, (3, 0))
            .windows(b"(Original)".len())
            .any(|part| part == b"(Original)")
    );
    for (id, value) in &original.objects {
        if *id == (3, 0) {
            continue;
        }
        if let Some(current) = output.objects.get(id) {
            assert_eq!(current, value, "sourceobject{id:?}");
        } else {
            assert_eq!(
                value
                    .as_stream()
                    .unwrap()
                    .dict
                    .get(b"Type")
                    .unwrap()
                    .as_name()
                    .unwrap(),
                b"XRef"
            );
        }
    }
    assert_eq!(raw(&original, (4, 0)), raw(&output, (4, 0)));
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Café\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    let mut pdf = PdfDocument::from_bytes(&saved, None).unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert!(run.writable && run.properties_writable);
    assert!(run.standard_font.is_none());
    assert_eq!(
        run.embedded_font.as_ref().unwrap().postscript_name,
        "Amiri-Regular"
    );
    pdf.replace_content_text(
        1,
        &[ContentTextChange {
            identity: run.identity,
            expected: "Café".into(),
            replacement: "Résumé".into(),
            layout_policy: ContentTextLayoutPolicy::AllowWidthChange,
        }],
    )
    .unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Résumé\nA B\n");
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    pdf.update_content_text_properties(
        1,
        &[ContentTextPropertyChange {
            identity: run.identity,
            expected: "Résumé".into(),
            font_size: Some(24.),
            matrix_translation: Some([30., 150.]),
            fill_rgb: Some([1., 0., 0.]),
        }],
    )
    .unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert!(run.properties_writable);
    assert_eq!(run.layout.as_ref().unwrap().font_size, 24.);
    pdf.change_content_text_face(
        1,
        &[ContentTextFaceChange {
            identity: run.identity,
            expected: "Résumé".into(),
            replacement: Some("Back".into()),
            font: ContentTextFont::Helvetica,
            layout_policy: ContentTextLayoutPolicy::AllowWidthChange,
        }],
    )
    .unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Back\nA B\n");
    assert_eq!(
        pdf.content_text_runs(1).unwrap()[0].standard_font,
        Some(ContentTextFont::Helvetica)
    );
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.).unwrap(),
        pdf_render::render_page(&saved, 1, 1.).unwrap()
    );
    assert_ne!(
        pdf_render::render_page(&bytes, 0, 1.).unwrap().rgba,
        pdf_render::render_page(&saved, 0, 1.).unwrap().rgba
    );
}
#[test]
fn missing_coverage_stale_nonzero_spacing_positioned_shaping_and_policy_refuse_atomically() {
    let bytes = source(CONTENT, courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    for value in ["日本", "\n", "مرحبا"] {
        assert!(
            pdf.change_content_text_embedded_font(1, &request(&pdf, Some(value)), FONT)
                .is_err()
        );
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
    let mut stale = request(&pdf, None);
    stale.identity.stream_digest[0] ^= 1;
    assert!(
        pdf.change_content_text_embedded_font(1, &stale, FONT)
            .is_err()
    );
    let mut policy = request(&pdf, None);
    policy.layout_policy = Default::default();
    assert!(
        pdf.change_content_text_embedded_font(1, &policy, FONT)
            .is_err()
    );
    assert!(
        pdf.change_content_text_embedded_font(1, &request(&pdf, None), b"badfont")
            .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    let mut spaced = PdfDocument::from_bytes(
        &source(&[b"1 Tc 2 Tw\n".as_slice(), CONTENT].concat(), courier()),
        None,
    )
    .unwrap();
    let before = spaced.verified_bytes().unwrap();
    assert!(
        spaced
            .change_content_text_embedded_font(1, &request(&spaced, None), FONT)
            .is_err()
    );
    assert_eq!(spaced.verified_bytes().unwrap(), before);
}

#[test]
fn altered_font_widths_or_unicode_graph_disable_editing_without_marker_trust() {
    let bytes = source(CONTENT, courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    pdf.change_content_text_embedded_font(1, &request(&pdf, Some("Café")), FONT)
        .unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let id = pdf.content_text_runs(1).unwrap()[0]
        .embedded_font
        .as_ref()
        .unwrap()
        .font_object;
    for change in 0..3 {
        let mut doc = Document::load_mem(&saved).unwrap();
        let descendant = doc
            .get_dictionary(id)
            .unwrap()
            .get(b"DescendantFonts")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        match change {
            0 => doc.get_dictionary_mut(descendant).unwrap().set("DW", 600),
            1 => doc
                .get_dictionary_mut(id)
                .unwrap()
                .set("BaseFont", "TrustedFontMarker"),
            _ => {
                let map = doc
                    .get_dictionary(id)
                    .unwrap()
                    .get(b"ToUnicode")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                doc.get_object_mut(map)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .set_plain_content(b"badmap".to_vec());
            }
        }
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let candidate = PdfDocument::from_bytes(&bytes, None).unwrap();
        if let Ok(runs) = candidate.content_text_runs(1) {
            assert!(!runs[0].writable && !runs[0].properties_writable);
            assert!(runs[0].embedded_font.is_none());
        }
        assert_eq!(
            candidate.verified_bytes().unwrap(),
            PdfDocument::from_bytes(&bytes, None)
                .unwrap()
                .verified_bytes()
                .unwrap()
        );
    }
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10.0"]
fn independent_chosen_ttf_unicode_metrics_paint_and_unrelated_objects() {
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
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("font.pdf");
    let bytes = source(CONTENT, courier());
    std::fs::write(&input, &bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    pdf.change_content_text_embedded_font(1, &request(&pdf, Some("Café")), FONT)
        .unwrap();
    pdf.save_new(&output).unwrap();
    assert!(
        Command::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    let bbox = Command::new(poppler.join("pdftotext"))
        .args(["-f", "1", "-l", "1", "-bbox"])
        .arg(&output)
        .arg("-")
        .output()
        .unwrap();
    assert!(bbox.status.success());
    let xml = String::from_utf8(bbox.stdout).unwrap();
    let actual = proportional::word_box(&xml, "Café");
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    let position = run.replacement_position("Café").unwrap();
    assert_eq!(position.baseline_start, [40., 140.]);
    assert!((actual[0] - position.baseline_start[0]).abs() < 0.00001);
    assert!((actual[2] - actual[0] - position.nominal_advance).abs() < 0.0001);
    let original_xml = Command::new(poppler.join("pdftotext"))
        .args(["-f", "1", "-l", "1", "-bbox"])
        .arg(&input)
        .arg("-")
        .output()
        .unwrap();
    assert_eq!(
        proportional::word_box(&xml, "A"),
        proportional::word_box(&String::from_utf8(original_xml.stdout).unwrap(), "A")
    );
    for channels in [3, 4] {
        let (before, after) = if channels == 3 {
            (
                proportional::ppm(&poppler, &input, temp.path().join("before"), 1),
                proportional::ppm(&poppler, &output, temp.path().join("after"), 1),
            )
        } else {
            (
                pdf_render::render_page(&bytes, 0, 1.).unwrap().rgba,
                pdf_render::render_page(&std::fs::read(&output).unwrap(), 0, 1.)
                    .unwrap()
                    .rgba,
            )
        };
        let mut changed = 0;
        for (index, (a, b)) in before
            .chunks_exact(channels)
            .zip(after.chunks_exact(channels))
            .enumerate()
        {
            if a != b {
                changed += 1;
                assert!(
                    (30..160).contains(&(index % 200)) && (35..75).contains(&(index / 200)),
                    "outside chosenfont glyphs {index}"
                );
            }
        }
        assert!(changed > 20);
    }
    assert_eq!(
        proportional::ppm(&poppler, &input, temp.path().join("source2"), 2),
        proportional::ppm(&poppler, &output, temp.path().join("output2"), 2)
    );
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}
