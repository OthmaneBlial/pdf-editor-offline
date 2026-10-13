use super::*;
use pdf_core::{
    ContentTextFaceChange, ContentTextFont, ContentTextLayoutPolicy, ContentTextPropertyChange,
};

const FACES: [ContentTextFont; 12] = [
    ContentTextFont::Courier,
    ContentTextFont::CourierBold,
    ContentTextFont::CourierOblique,
    ContentTextFont::CourierBoldOblique,
    ContentTextFont::Helvetica,
    ContentTextFont::HelveticaBold,
    ContentTextFont::HelveticaOblique,
    ContentTextFont::HelveticaBoldOblique,
    ContentTextFont::TimesRoman,
    ContentTextFont::TimesBold,
    ContentTextFont::TimesItalic,
    ContentTextFont::TimesBoldItalic,
];
const INHERITED: &[u8] = b"0.2 0.4 0.6 rg 1 Tc 3 Tw 80 Tz 2 Ts q\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET\n10 10 20 20 re f\nBT 1 0 0 1 40 80 Tm (Inherited) Tj ET\nQ\n";
fn request(pdf: &PdfDocument, font: ContentTextFont) -> ContentTextFaceChange {
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    ContentTextFaceChange {
        identity: run.identity,
        expected: run.text.unwrap(),
        font,
        replacement: None,
        layout_policy: ContentTextLayoutPolicy::AllowWidthChange,
    }
}
fn raw(document: &Document, page: lopdf::ObjectId) -> Vec<u8> {
    let id = document
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object(id)
        .unwrap()
        .as_stream()
        .unwrap()
        .decompressed_content_with_limit(pdf_core::MAX_STREAM_BYTES)
        .unwrap()
}
fn preserve_outside(before: &[u8], after: &[u8], channels: usize) {
    assert_eq!(before.len(), after.len());
    let mut changed = 0;
    for (index, (a, b)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if a != b {
            changed += 1;
            assert!(
                (30..185).contains(&(index % 200)) && (35..70).contains(&(index / 200)),
                "outside font target {index}"
            );
        }
    }
    assert!(
        changed > 20,
        "font change must actually alter glyph painting"
    );
}

#[test]
fn twelve_faces_are_real_page_local_fonts_and_preserve_later_inherited_state() {
    let bytes = source(INHERITED, courier());
    let original = Document::load_mem(&bytes).unwrap();
    for font in FACES {
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let mut change = request(&pdf, font);
        if matches!(font, ContentTextFont::Courier) {
            change.replacement = Some("Replaced".into());
        }
        let expected = change
            .replacement
            .clone()
            .unwrap_or_else(|| change.expected.clone());
        let before = pdf.content_text_runs(1).unwrap()[0].layout.clone().unwrap();
        let predicted = pdf.content_text_runs(1).unwrap()[0]
            .face_position(&change)
            .unwrap();
        assert_eq!(predicted.baseline_start, [40., 142.]);
        pdf.change_content_text_face(1, &[change]).unwrap();
        let saved = pdf.verified_bytes().unwrap();
        let output = Document::load_mem(&saved).unwrap();
        for (id, value) in &original.objects {
            if *id == (3, 0) {
                continue;
            }
            if let Some(current) = output.objects.get(id) {
                assert_eq!(current, value, "{} sourceobject{id:?}", font.pdf_name());
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
        assert_eq!(raw(&output, (4, 0)), raw(&original, (4, 0)));
        let run = pdf.content_text_runs(1).unwrap().remove(0);
        assert_eq!(run.standard_font, Some(font));
        assert_eq!(
            serde_json::to_value(run.layout).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(run.properties_writable && run.writable);
        let before = pdf_render::render_page(&bytes, 0, 1.).unwrap();
        let after = pdf_render::render_page(&saved, 0, 1.).unwrap();
        preserve_outside(&before.rgba, &after.rgba, 4);
        assert_eq!(
            pdf_render::render_page(&bytes, 1, 1.).unwrap().rgba,
            pdf_render::render_page(&saved, 1, 1.).unwrap().rgba
        );
        assert!(pdf.extract_text(&[1]).unwrap().contains(&expected));
        assert!(pdf.extract_text(&[1]).unwrap().contains("Inherited"));
    }
}

#[test]
fn raw_precision_resource_collision_and_repeat_edits_remain_supported() {
    const PREFIX:&[u8]=b"% exact precision\n1.0000000123456789 0 0 1 0.1234567890123456 0 cm\n0.1234567890123456 0.2 0.3 rg\n10.1234567890123456 10 20 20 re f\n";
    const BODY:&[u8]=b"BT /F 20.0000000012345678 Tf 1.0000000123456789 0 0 1 40.1234567890123456 140.9876543210987654 Tm 0 Tr (\\117riginal) Tj ET";
    const TAIL: &[u8] = b"\nBT 1 0 0 1 40 80 Tm (Inherited) Tj ET\n";
    let bytes = source(&[PREFIX, BODY, TAIL].concat(), courier());
    let mut document = Document::load_mem(&bytes).unwrap();
    document
        .get_dictionary_mut((2, 0))
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Font")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("NativeContentFont1", (5, 0));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    pdf.change_content_text_face(1, &[request(&pdf, ContentTextFont::TimesItalic)])
        .unwrap();
    let output = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let edited = raw(&output, (3, 0));
    assert!(edited.starts_with(PREFIX) && edited.ends_with(TAIL));
    assert!(!edited.windows(BODY.len()).any(|part| part == BODY)); // insertion is inside the selected BT
    for token in [
        b"1.0000000123456789 0 0 1 40.1234567890123456 140.9876543210987654 Tm".as_slice(),
        b"(\\117riginal) Tj ET",
        b"/NativeContentFont2 20.0000000012345678 Tf",
        b"/F 20.0000000012345678 Tf",
    ] {
        assert!(
            edited.windows(token.len()).any(|part| part == token),
            "missing exact token {}",
            String::from_utf8_lossy(token)
        );
    }
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    pdf.replace_content_text(
        1,
        &[ContentTextChange {
            identity: run.identity,
            expected: "Original".into(),
            replacement: "Replacement".into(),
            layout_policy: ContentTextLayoutPolicy::AllowWidthChange,
        }],
    )
    .unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    pdf.update_content_text_properties(
        1,
        &[ContentTextPropertyChange {
            identity: run.identity,
            expected: "Replacement".into(),
            font_size: Some(18.),
            matrix_translation: None,
            fill_rgb: Some([1., 0., 0.]),
        }],
    )
    .unwrap();
    pdf.change_content_text_face(1, &[request(&pdf, ContentTextFont::Helvetica)])
        .unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("Replacement"));
    assert_eq!(
        pdf.content_text_runs(1).unwrap()[0].standard_font,
        Some(ContentTextFont::Helvetica)
    );
}

#[test]
fn stale_unsupported_default_advance_and_later_bad_batch_are_atomic() {
    let bytes = source(CONTENT, courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    let good = request(&pdf, ContentTextFont::Helvetica);
    let mut bad = good.clone();
    bad.layout_policy = ContentTextLayoutPolicy::PreserveWordAdvances;
    assert!(pdf.change_content_text_face(1, &[bad]).is_err());
    let second = pdf.content_text_runs(1).unwrap().remove(1);
    let mut bad = good.clone();
    bad.identity = second.identity;
    bad.expected = "Wrong".into();
    assert!(
        pdf.change_content_text_face(1, &[good.clone(), bad])
            .is_err()
    );
    let mut bad = good.clone();
    bad.identity.stream_digest[0] ^= 1;
    assert!(pdf.change_content_text_face(1, &[bad]).is_err());
    let mut bad = good.clone();
    bad.replacement = Some("日本語".into());
    assert!(pdf.change_content_text_face(1, &[bad]).is_err());
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    // PreserveWordAdvances never silently changes the advance of spaces.
    let mut empty = PdfDocument::from_bytes(
        &source(b"BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr ( ) Tj ET", courier()),
        None,
    )
    .unwrap();
    let mut spaces = request(&empty, ContentTextFont::Helvetica);
    spaces.layout_policy = Default::default();
    assert!(empty.change_content_text_face(1, &[spaces]).is_err());
    let mut override_font = courier();
    override_font.set("Widths", vec![600.into()]);
    let mut unsupported = PdfDocument::from_bytes(&source(CONTENT, override_font), None).unwrap();
    let run = unsupported.content_text_runs(1).unwrap().remove(0);
    let mut bad = good;
    bad.identity = run.identity;
    assert!(unsupported.change_content_text_face(1, &[bad]).is_err());
}

#[test]
fn standard_source_can_change_to_explicit_winansi_quotes_without_guessing_unicode() {
    let mut font = courier();
    font.remove(b"Encoding");
    let mut pdf = PdfDocument::from_bytes(&source(CONTENT, font), None).unwrap();
    let mut change = request(&pdf, ContentTextFont::TimesRoman);
    change.replacement = Some("'quote`".into());
    pdf.change_content_text_face(1, &[change]).unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert!(matches!(
        run.encoding,
        Some(pdf_core::ContentTextEncoding::WinAnsi)
    ));
    assert_eq!(run.text.as_deref(), Some("'quote`"));
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "'quote`\nA B\n");
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10.0"]
fn independent_twelve_font_faces_metrics_text_pixels_and_inherited_state() {
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
    for font in FACES {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source.pdf");
        let output = temp.path().join("changed.pdf");
        let content=b"0.2 0.4 0.6 rg q\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET\n10 10 20 20 re f\nBT 1 0 0 1 40 80 Tm (Inherited) Tj ET\nQ\n";
        let bytes = source(content, courier());
        std::fs::write(&input, &bytes).unwrap();
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let mut change = request(&pdf, font);
        change.replacement = Some("Replaced".into());
        let predicted = pdf.content_text_runs(1).unwrap()[0]
            .face_position(&change)
            .unwrap();
        pdf.change_content_text_face(1, &[change]).unwrap();
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
        let bbox = Command::new(poppler.join("pdftotext"))
            .args(["-f", "1", "-l", "1", "-bbox"])
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(bbox.status.success());
        let xml = String::from_utf8(bbox.stdout).unwrap();
        let target = proportional::word_box(&xml, "Replaced");
        assert!((target[0] - predicted.baseline_start[0]).abs() < 0.00001);
        assert!(
            (target[2] - target[0] - predicted.nominal_advance).abs() < 0.0001,
            "{} width{} vs{}",
            font.pdf_name(),
            target[2] - target[0],
            predicted.nominal_advance
        );
        let oldxml = Command::new(poppler.join("pdftotext"))
            .args(["-f", "1", "-l", "1", "-bbox"])
            .arg(&input)
            .arg("-")
            .output()
            .unwrap();
        assert_eq!(
            proportional::word_box(&xml, "Inherited"),
            proportional::word_box(&String::from_utf8(oldxml.stdout).unwrap(), "Inherited")
        );
        let a = proportional::ppm(&poppler, &input, temp.path().join("source1"), 1);
        let b = proportional::ppm(&poppler, &output, temp.path().join("output1"), 1);
        preserve_outside(&a, &b, 3);
        assert_eq!(
            proportional::ppm(&poppler, &input, temp.path().join("source2"), 2),
            proportional::ppm(&poppler, &output, temp.path().join("output2"), 2)
        );
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}
