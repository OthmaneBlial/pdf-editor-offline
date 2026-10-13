use super::*;
use pdf_core::ContentTextPropertyChange;

const INHERITED: &[u8] = b"0.2 0.4 0.6 rg 1 Tc 3 Tw 80 Tz 2 Ts q\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET\n10 10 20 20 re f\nBT 1 0 0 1 40 80 Tm (Inherited) Tj ET\nQ\n";
fn property(pdf: &PdfDocument) -> ContentTextPropertyChange {
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    ContentTextPropertyChange {
        identity: run.identity,
        expected: "Original".into(),
        font_size: Some(30.0),
        matrix_translation: Some([20.0, 155.0]),
        fill_rgb: Some([1.0, 0.0, 0.0]),
    }
}
fn raw_stream(document: &Document, page: lopdf::ObjectId) -> Vec<u8> {
    // get_page_content appends an artificial newline after each stream. Byte
    // preservation must compare the actual bounded decoded stream, not that adapter.
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
fn check_pixels(before: &[u8], after: &[u8], channels: usize) {
    assert_eq!(before.len(), after.len());
    let mut changed = 0;
    let mut red = 0;
    for (index, (old, new)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if old != new {
            changed += 1;
            let (x, y) = (index % 200, index / 200);
            assert!(
                (15..200).contains(&x) && (15..70).contains(&y),
                "changed later inherited text/artwork pixel {x},{y}: {old:?}→{new:?}"
            );
            if new[0] > 200 && new[1] < 50 && new[2] < 50 {
                red += 1;
            }
        }
    }
    assert!(
        changed > 100 && red > 30,
        "real moved/resized/red glyph pixels required"
    );
}

#[test]
fn scoped_properties_preserve_inherited_text_state_artwork_shared_stream_and_original_operand() {
    let bytes = source(INHERITED, courier());
    let original = Document::load_mem(&bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let change = property(&pdf);
    let predicted = pdf.content_text_runs(1).unwrap()[0]
        .property_position(&change)
        .unwrap();
    assert_eq!(predicted.baseline_start, [20.0, 157.0]);
    pdf.update_content_text_properties(1, &[change]).unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let output = Document::load_mem(&saved).unwrap();
    for (id, value) in &original.objects {
        if *id == (3, 0) {
            continue;
        }
        if let Some(current) = output.objects.get(id) {
            assert_eq!(current, value, "original logical object{id:?}");
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
    let content = lopdf::content::Content::decode(&output.get_page_content((3, 0))).unwrap();
    let shows = content
        .operations
        .iter()
        .filter(|op| op.operator == "Tj")
        .collect::<Vec<_>>();
    assert_eq!(shows[0].operands[0].as_str().unwrap(), b"Original");
    assert_eq!(shows[1].operands[0].as_str().unwrap(), b"Inherited");
    let before = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    let after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    check_pixels(&before.rgba, &after.rgba, 4);
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap().rgba,
        pdf_render::render_page(&saved, 1, 1.0).unwrap().rgba
    );
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert!(
        run.writable,
        "reopened generated run: {:?}",
        run.refusal_reason
    );
    assert_eq!(run.layout.as_ref().unwrap().font_size, 30.0);
    assert_eq!(run.layout.as_ref().unwrap().text_matrix[4..], [20.0, 155.0]);
    let repeat = ContentTextPropertyChange {
        identity: run.identity,
        expected: "Original".into(),
        font_size: Some(18.0),
        matrix_translation: None,
        fill_rgb: None,
    };
    pdf.update_content_text_properties(1, &[repeat]).unwrap();
    let repeat = pdf.verified_bytes().unwrap();
    check_pixels(
        &before.rgba,
        &pdf_render::render_page(&repeat, 0, 1.0).unwrap().rgba,
        4,
    );
}

#[test]
fn properties_refuse_stale_nonfinite_profile_complex_state_and_publish_nothing() {
    let bytes = source(INHERITED, courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let mut nonfinite = property(&pdf);
    nonfinite.font_size = Some(f32::INFINITY);
    assert!(
        serde_json::to_value(&nonfinite).is_err(),
        "non-finite optional size must not serialize as an absent property alongside RGB"
    );
    for mutate in [0, 1, 2, 3, 4, 5] {
        let mut change = property(&pdf);
        match mutate {
            0 => change.font_size = Some(0.0),
            1 => change.font_size = Some(f32::NAN),
            2 => change.fill_rgb = Some([1.1, 0.0, 0.0]),
            3 => change.matrix_translation = Some([f32::INFINITY, 0.0]),
            4 => change.expected = "Different".into(),
            _ => change.identity.stream_digest[0] ^= 1,
        }
        assert!(pdf.update_content_text_properties(1, &[change]).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    for prefix in [b"/GS gs\n".as_slice(), b"/DeviceRGB cs\n", b"0 0 0 1 k\n"] {
        let source = source(&[prefix, INHERITED].concat(), courier());
        let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(
            pdf.update_content_text_properties(1, &[property(&pdf)])
                .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut raw = Document::load_mem(&bytes).unwrap();
    raw.catalog_mut()
        .unwrap()
        .set("OutputIntents", vec![Object::Null]);
    let mut bytes = Vec::new();
    raw.save_to(&mut bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(
        pdf.update_content_text_properties(1, &[property(&pdf)])
            .is_err()
    );
}

#[test]
fn property_batch_rolls_back_late_target_failure_and_preserves_original_affine_basis() {
    let bytes = source(
        &[b"0.5 0.2 0.1 0.6 10 15 cm\n".as_slice(), CONTENT].concat(),
        courier(),
    );
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let runs = pdf.content_text_runs(1).unwrap();
    let first = property(&pdf);
    let second = ContentTextPropertyChange {
        identity: runs[1].identity.clone(),
        expected: "stale".into(),
        font_size: Some(10.0),
        matrix_translation: None,
        fill_rgb: None,
    };
    assert!(
        pdf.update_content_text_properties(1, &[first.clone(), second.clone()])
            .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
    let mut second = second;
    second.expected = "A B".into();
    let predicted = runs[0].property_position(&first).unwrap();
    assert_eq!(
        predicted.baseline_start,
        [
            20.0 * f64::from(0.5f32) + 155.0 * f64::from(0.1f32) + 10.0,
            20.0 * f64::from(0.2f32) + 155.0 * f64::from(0.6f32) + 15.0
        ]
    );
    pdf.update_content_text_properties(1, &[first, second])
        .unwrap();
    let updated = pdf.content_text_runs(1).unwrap();
    assert_eq!(
        updated[0].layout.as_ref().unwrap().text_matrix[..4],
        runs[0].layout.as_ref().unwrap().text_matrix[..4]
    );
    assert_eq!(
        updated[0].layout.as_ref().unwrap().current_transform,
        runs[0].layout.as_ref().unwrap().current_transform
    );
    assert_eq!(
        updated[0]
            .replacement_position("Original")
            .unwrap()
            .baseline_start,
        predicted.baseline_start
    );
    assert_eq!(updated[1].layout.as_ref().unwrap().font_size, 10.0);
    assert!(updated.iter().all(|run| run.properties_writable));
}

#[test]
fn raw_precision_comments_strings_and_untouched_program_survive_properties_and_replacement() {
    const PREFIX:&[u8]=b"% exact untouched precision sentinel\n1.0000000123456789 0.00000000123456789 0 1 0.1234567890123456 0.9876543210987654 cm\n0.1234567890123456 0.2345678901234567 0.3456789012345678 rg\n10.1234567890123456 10.9876543210987654 20.1234567890123456 20.9876543210987654 re f\n";
    const BODY:&[u8]=b"BT /F 20.0000000012345678 Tf 1.0000000123456789 0.00000000123456789 0.00000000987654321 1.000000023456789 40.1234567890123456 140.9876543210987654 Tm 0 Tr (\\117riginal) Tj ET";
    const TAIL:&[u8]=b"\n% exact later artwork and inherited text\n10.9876543210987654 50.1234567890123456 20.9876543210987654 10.1234567890123456 re f\nBT 1 0 0 1 40.1234567890123456 80.9876543210987654 Tm (Inherited) Tj ET\n";
    let bytes = source(&[PREFIX, BODY, TAIL].concat(), courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    pdf.update_content_text_properties(1, &[property(&pdf)])
        .unwrap();
    let output = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let edited = raw_stream(&output, (3, 0));
    const EXACT_CTM: &[u8] =
        b"1.0000000123456789 0.00000000123456789 0 1 0.1234567890123456 0.9876543210987654 cm";
    assert!(
        edited
            .windows(EXACT_CTM.len())
            .any(|part| part == EXACT_CTM),
        "untouched high-precision CTM was reserialized: {}",
        String::from_utf8_lossy(&edited[..edited.len().min(PREFIX.len())])
    );
    assert!(
        edited.starts_with(PREFIX),
        "untouched numeric operands/comments must remain byte-exact"
    );
    assert!(
        edited.ends_with(TAIL),
        "later numeric operands/comments must remain byte-exact"
    );
    assert!(
        edited
            .windows(b"(\\117riginal) Tj ET".len())
            .any(|part| part == b"(\\117riginal) Tj ET")
    );
    assert!(edited.windows(b"1.0000000123456789 0.00000000123456789 0.00000000987654321 1.000000023456789 20 155 Tm".len()).any(|part|part==b"1.0000000123456789 0.00000000123456789 0.00000000987654321 1.000000023456789 20 155 Tm"),"new translation must retain exact original matrix basis operands");
    assert!(
        edited
            .windows(b"/F 20.0000000012345678 Tf".len())
            .filter(|part| *part == b"/F 20.0000000012345678 Tf")
            .count()
            >= 2,
        "original font-size setter must replay without precision loss"
    );
    // Options are intent deltas: untouched selected Tf/Tm numbers must never be
    // rewritten from the f32 layout displayed by the parent.
    const EXACT_TM:&[u8]=b"1.0000000123456789 0.00000000123456789 0.00000000987654321 1.000000023456789 40.1234567890123456 140.9876543210987654 Tm";
    for size in [None, Some(24.0)] {
        let mut only = PdfDocument::from_bytes(&bytes, None).unwrap();
        let run = only.content_text_runs(1).unwrap().remove(0);
        let change = ContentTextPropertyChange {
            identity: run.identity,
            expected: "Original".into(),
            font_size: size,
            matrix_translation: None,
            fill_rgb: size.is_none().then_some([1.0, 0.0, 0.0]),
        };
        only.update_content_text_properties(1, &[change]).unwrap();
        let output = Document::load_mem(&only.verified_bytes().unwrap()).unwrap();
        let raw = raw_stream(&output, (3, 0));
        assert!(raw.starts_with(PREFIX) && raw.ends_with(TAIL));
        assert_eq!(
            raw.windows(EXACT_TM.len())
                .filter(|part| *part == EXACT_TM)
                .count(),
            1,
            "color-only/size-only must retain exact selected matrix once"
        );
        assert_eq!(
            raw.windows(b"/F 20.0000000012345678 Tf".len())
                .filter(|part| *part == b"/F 20.0000000012345678 Tf")
                .count(),
            2,
            "color-only/size-only must retain exact original font-size and replay it"
        );
        let decoded = lopdf::content::Content::decode(&raw).unwrap();
        assert_eq!(
            decoded
                .operations
                .iter()
                .filter(|op| op.operator == "Tm")
                .count(),
            2,
            "no invented matrix update for an untouched property"
        );
        assert_eq!(
            only.content_text_runs(1).unwrap()[0]
                .layout
                .as_ref()
                .unwrap()
                .font_size,
            size.map(f64::from).unwrap_or(20.0)
        );
    }
    let mut replacement = PdfDocument::from_bytes(&bytes, None).unwrap();
    replacement
        .replace_content_text(1, &[change(&replacement, 0, "Replaced")])
        .unwrap();
    let output = Document::load_mem(&replacement.verified_bytes().unwrap()).unwrap();
    let edited = raw_stream(&output, (3, 0));
    let expected = [
        PREFIX,
        BODY.split(|byte| *byte == b'(').next().unwrap(),
        b"(Replaced) Tj ET",
        TAIL,
    ]
    .concat();
    assert_eq!(
        edited, expected,
        "replacement may alter only the selected string-operand bytes"
    );
    assert_eq!(raw_stream(&output, (4, 0)), [PREFIX, BODY, TAIL].concat());
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10.0"]
fn independent_property_baseline_size_color_and_later_inheritance() {
    let poppler = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let qpdf = PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap());
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
    for family in [
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
        let bytes = source(
            INHERITED,
            dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>family,"Encoding"=>"WinAnsiEncoding"},
        );
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source.pdf");
        let output = temp.path().join("changed.pdf");
        std::fs::write(&input, &bytes).unwrap();
        pdf.update_content_text_properties(1, &[property(&pdf)])
            .unwrap();
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
                rasters.push(proportional::ppm(
                    &poppler,
                    path,
                    temp.path().join(format!("{index}-{page}")),
                    page,
                ));
            }
        }
        let old = proportional::word_box(&boxes[0], "Original");
        let new = proportional::word_box(&boxes[1], "Original");
        assert!((old[0] - 40.0).abs() < 0.001 && (new[0] - 20.0).abs() < 0.001);
        assert!(new[3] - new[1] > old[3] - old[1] + 5.0);
        assert_eq!(
            proportional::word_box(&boxes[0], "Inherited"),
            proportional::word_box(&boxes[1], "Inherited"),
            "{family}: later BT inherits exact original size/spacing/rise"
        );
        check_pixels(&rasters[0], &rasters[2], 3);
        assert_eq!(rasters[1], rasters[3]);
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}
