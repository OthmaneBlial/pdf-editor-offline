#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;
#[path = "../../pdf-worker/tests/support/encryption.rs"]
mod encryption;
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
const SECRET: &str = "PRIVATE";
fn save(mut document: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
fn fixture() -> Vec<u8> {
    let mut doc = Document::load_mem(&support::fixture(2)).unwrap();
    doc.get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .remove(b"XObject");
    doc.get_object_mut((3, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("BaseFont", "Courier");
    let raw=b"% untouched high precision artwork\n0.123456789012345678 0.234567890123456789 0.3 rg 10 10 20 20 re f\nBT /F1 10 Tf 20 180 Td (prefix PRIVATE suffix) Tj ( tail) Tj ET\nBT /F1 10 Tf 20 120 Td [(split PRI) -12.123456789012345678 (VATE retained)] TJ ET\n";
    doc.get_object_mut((5, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(raw.to_vec());
    doc.get_object_mut((8, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Contents", Object::Reference((5, 0)));
    let info = doc.add_object(dictionary! {"Title"=>Object::string_literal(SECRET)});
    doc.trailer.set("Info", info);
    let metadata = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        format!("<private>{SECRET}</private>").into_bytes(),
    ));
    let action = doc.add_object(
        dictionary! {"Type"=>"Action","S"=>"JavaScript","JS"=>Object::string_literal(SECRET)},
    );
    let embedded = doc.add_object(Stream::new(
        dictionary! {"Type"=>"EmbeddedFile"},
        SECRET.as_bytes().to_vec(),
    ));
    let file=doc.add_object(dictionary!{"Type"=>"Filespec","F"=>Object::string_literal(format!("{SECRET}.txt")),"EF"=>dictionary!{"F"=>embedded}});
    let catalog = doc.get_object_mut((1, 0)).unwrap().as_dict_mut().unwrap();
    catalog.set("Metadata", metadata);
    catalog.set("OpenAction", action);
    catalog.set("Names",dictionary!{"EmbeddedFiles"=>dictionary!{"Names"=>vec![Object::string_literal(format!("{SECRET}.txt")),Object::Reference(file)]}});
    doc.add_object(Stream::new(
        dictionary! {},
        format!("orphan-{SECRET}").into_bytes(),
    ));
    save(doc)
}
#[test]
fn exact_targets_are_permanently_removed_from_text_and_targeted_carriers_with_positions_retained() {
    let source = fixture();
    let pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let original = Document::load_mem(&source).unwrap();
    assert_eq!(
        original
            .get_object((9, 0))
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Title")
            .unwrap()
            .as_str()
            .unwrap(),
        SECRET.as_bytes()
    );
    assert_eq!(
        original
            .get_object((11, 0))
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"JS")
            .unwrap()
            .as_str()
            .unwrap(),
        SECRET.as_bytes()
    );
    for id in [(10, 0), (12, 0), (14, 0)] {
        let plain = original
            .get_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content_with_limit(pdf_core::MAX_STREAM_BYTES)
            .unwrap();
        assert!(plain.windows(SECRET.len()).any(|v| v == SECRET.as_bytes()));
    }
    assert!(pdf.extract_text(&[1, 2]).unwrap().contains(SECRET));
    let (output, report) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
    assert_eq!(report.changed_text_shows, 4);
    assert_eq!(report.matched_targets, 4);
    assert_eq!(report.affected_pages, vec![1, 2]);
    assert_eq!(report.cleanup.info_links_removed, 1);
    assert_eq!(report.cleanup.xmp_links_removed, 1);
    assert_eq!(report.cleanup.embedded_file_name_trees_removed, 1);
    assert!(
        report.cleanup.action_links_removed > 0 || report.cleanup.automatic_triggers_removed > 0
    );
    let bytes = output.verified_bytes().unwrap();
    assert!(!bytes.windows(SECRET.len()).any(|v| v == SECRET.as_bytes()));
    let reopened = PdfDocument::from_bytes(&bytes, None).unwrap();
    reopened.verify_redaction(&[SECRET.into()]).unwrap();
    pdf_core::verify_redaction_bytes(&bytes, None, &[SECRET.into()]).unwrap();
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"\n% PRIVATE trailing carrier\n");
    assert!(pdf_core::verify_redaction_bytes(&trailing, None, &[SECRET.into()]).is_err());
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"\n% <50524956415445> encoded trailing carrier\n");
    assert!(pdf_core::verify_redaction_bytes(&trailing, None, &[SECRET.into()]).is_err());
    assert_eq!(reopened.info().unwrap().pages, pdf.info().unwrap().pages);
    let text = reopened.extract_text(&[1, 2]).unwrap();
    assert!(!text.contains(SECRET));
    for word in ["prefix", "suffix", "tail", "split", "retained"] {
        assert!(text.contains(word), "{text}");
    }
    let logical = Document::load_mem(&bytes).unwrap();
    assert!(!logical.trailer.has(b"Prev"));
    assert!(!logical.trailer.has(b"Info"));
    assert!(!logical.objects.contains_key(&(5, 0)));
    assert!(!logical.objects.contains_key(&(7, 0)));
    assert!(!logical.objects.contains_key(&(4, 0)));
    let page = logical.get_object((6, 0)).unwrap().as_dict().unwrap();
    let stream = logical
        .get_object(page.get(b"Contents").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    let raw = stream
        .get_plain_content_with_limit(pdf_core::MAX_STREAM_BYTES)
        .unwrap();
    assert!(
        raw.windows(b"-12.123456789012345678".len())
            .any(|v| v == b"-12.123456789012345678")
    );
    assert!(
        raw.windows(b"0.123456789012345678".len())
            .any(|v| v == b"0.123456789012345678")
    );
    assert!(raw.windows(5).any(|v| v == b"-4200"));
    assert_eq!(
        pdf.verified_bytes().unwrap(),
        PdfDocument::from_bytes(&source, None)
            .unwrap()
            .verified_bytes()
            .unwrap()
    );
}

#[test]
fn incremental_old_revisions_and_contents_arrays_leave_no_recoverable_old_streams() {
    let mut compressed = Document::load_mem(&fixture()).unwrap();
    compressed
        .get_object_mut((8, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Contents", Object::Reference((7, 0)));
    let stream = compressed
        .get_object_mut((7, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap();
    let mut plain = b"BT /F1 10 Tf 20 120 Td (public unchanged) Tj ET".to_vec();
    plain.extend(vec![b' '; 500]);
    stream.set_content(plain);
    stream.compress().unwrap();
    assert!(stream.dict.has(b"Filter"));
    stream.content.extend_from_slice(b"<50524956415445>");
    stream.dict.set("Length", stream.content.len() as i64);
    let compressed = save(compressed);
    let pdf = PdfDocument::from_bytes(&compressed, None).unwrap();
    assert!(pdf.extract_text(&[2]).unwrap().contains("public unchanged"));
    let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
    let output = output.verified_bytes().unwrap();
    assert!(!output.windows(16).any(|v| v == b"<50524956415445>"));
    let logical = Document::load_mem(&output).unwrap();
    assert!(
        logical
            .objects
            .values()
            .filter_map(|v| v.as_stream().ok())
            .all(|s| !s.dict.has(b"Filter"))
    );
    pdf_core::verify_redaction_bytes(&output, None, &[SECRET.into()]).unwrap();
    let mut streamed = Document::load_mem(&fixture()).unwrap();
    streamed.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceStream;
    let pdf = PdfDocument::from_bytes(&save(streamed), None).unwrap();
    let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
    let reopened = PdfDocument::from_bytes(&output.verified_bytes().unwrap(), None).unwrap();
    reopened.verify_redaction(&[SECRET.into()]).unwrap();
    let source = fixture();
    let original = Document::load_mem(&source).unwrap();
    let mut incremental = lopdf::IncrementalDocument::create_from(source, original);
    incremental
        .opt_clone_object_to_new_document((5, 0))
        .unwrap();
    incremental
        .new_document
        .get_object_mut((5, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(b"BT /F1 10 Tf 20 180 Td (prefix PRIVATE suffix) Tj ET".to_vec());
    let mut bytes = Vec::new();
    incremental.save_to(&mut bytes).unwrap();
    assert!(bytes.windows(5).any(|v| v == b"/Prev"));
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
    let saved = output.verified_bytes().unwrap();
    assert!(!saved.windows(SECRET.len()).any(|v| v == SECRET.as_bytes()));
    assert!(!saved.windows(5).any(|v| v == b"/Prev"));
    output.verify_redaction(&[SECRET.into()]).unwrap();
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let first = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 10 Tf 20 180 Td (prefix PRI) Tj ".to_vec(),
    ));
    let last = doc.add_object(Stream::new(
        dictionary! {},
        b"[(VATE suffix)] TJ ET".to_vec(),
    ));
    doc.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Contents",
            vec![Object::Reference(first), Object::Reference(last)],
        );
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
    assert!(output.extract_text(&[1]).unwrap().contains("suffix"));
    output.verify_redaction(&[SECRET.into()]).unwrap();
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10 reference tools"]
fn independent_readers_prove_secret_absence_and_retained_word_positions() {
    use std::process::Command;
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let version = Command::new(&qpdf).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let version = Command::new(bin.join("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
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
        let mut doc = Document::load_mem(&fixture()).unwrap();
        doc.get_object_mut((3, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("BaseFont", Object::Name(font.as_bytes().to_vec()));
        let source = save(doc);
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
        let input = temp.path().join(format!("{font}-source.pdf"));
        let result = temp.path().join(format!("{font}-redacted.pdf"));
        std::fs::write(&input, &source).unwrap();
        output.save_new(&result).unwrap();
        for path in [&input, &result] {
            assert!(
                Command::new(&qpdf)
                    .arg("--check")
                    .arg(path)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }
        let decoded = temp.path().join(format!("{font}-decoded.pdf"));
        let before_decoded = temp.path().join(format!("{font}-source-decoded.pdf"));
        assert!(
            Command::new(&qpdf)
                .args([
                    "--qdf",
                    "--object-streams=disable",
                    "--stream-data=uncompress"
                ])
                .arg(&input)
                .arg(&before_decoded)
                .output()
                .unwrap()
                .status
                .success()
        );
        let before_decoded = std::fs::read(before_decoded).unwrap();
        assert!(
            before_decoded
                .windows(SECRET.len())
                .filter(|v| *v == SECRET.as_bytes())
                .count()
                >= 5
        );
        assert!(
            Command::new(&qpdf)
                .args([
                    "--qdf",
                    "--object-streams=disable",
                    "--stream-data=uncompress"
                ])
                .arg(&result)
                .arg(&decoded)
                .output()
                .unwrap()
                .status
                .success()
        );
        let decoded = std::fs::read(decoded).unwrap();
        assert!(
            !decoded
                .windows(SECRET.len())
                .any(|v| v == SECRET.as_bytes())
        );
        let before = Command::new(bin.join("pdftotext"))
            .arg("-bbox")
            .arg(&input)
            .arg("-")
            .output()
            .unwrap();
        let after = Command::new(bin.join("pdftotext"))
            .arg("-bbox")
            .arg(&result)
            .arg("-")
            .output()
            .unwrap();
        assert!(before.status.success() && after.status.success());
        let before = String::from_utf8(before.stdout).unwrap();
        let after = String::from_utf8(after.stdout).unwrap();
        assert!(before.contains(SECRET));
        assert!(!after.contains(SECRET));
        for word in ["prefix", "suffix", "tail", "split", "retained"] {
            let suffix = format!(">{word}</word>");
            let a = before
                .lines()
                .filter(|line| line.ends_with(&suffix))
                .collect::<Vec<_>>();
            let b = after
                .lines()
                .filter(|line| line.ends_with(&suffix))
                .collect::<Vec<_>>();
            assert!(!a.is_empty(), "missing {word}: {before}");
            assert_eq!(a, b, "{font} changed {word} coordinates");
        }
        assert_eq!(std::fs::read(&input).unwrap(), source);
        if font == "Courier" {
            // Independent hand-authored blank-advance control (Courier600),
            // without calling redaction or its metric planner.
            let mut control = Document::load_mem(&source).unwrap();
            let raw=b"% untouched high precision artwork\n0.123456789012345678 0.234567890123456789 0.3 rg 10 10 20 20 re f\nBT /F1 10 Tf 20 180 Td [(prefix ) -4200 ( suffix)] TJ ( tail) Tj ET\nBT /F1 10 Tf 20 120 Td [(split ) -1800 -12.123456789012345678 -2400 ( retained)] TJ ET\n";
            control
                .get_object_mut((5, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .set_content(raw.to_vec());
            let control = save(control);
            let control_path = temp.path().join("courier-control.pdf");
            std::fs::write(&control_path, &control).unwrap();
            for page in 0..2 {
                let before = pdf_render::render_page(&source, page, 1.0).unwrap().rgba;
                let after = pdf_render::render_page(&std::fs::read(&result).unwrap(), page, 1.0)
                    .unwrap()
                    .rgba;
                let control_pixels = pdf_render::render_page(&control, page, 1.0).unwrap().rgba;
                assert_eq!(after, control_pixels);
                assert_ne!(before, after);
                let mut images = Vec::new();
                for (label, path) in [
                    ("before", &input),
                    ("after", &result),
                    ("control", &control_path),
                ] {
                    let prefix = temp.path().join(format!("{label}-{}", page + 1));
                    assert!(
                        Command::new(bin.join("pdftoppm"))
                            .args(["-r", "72", "-f"])
                            .arg((page + 1).to_string())
                            .arg("-singlefile")
                            .arg(path)
                            .arg(&prefix)
                            .output()
                            .unwrap()
                            .status
                            .success()
                    );
                    images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
                }
                assert_eq!(images[1], images[2]);
                assert_ne!(images[0], images[1]);
            }
        }
    }
}
#[test]
fn unsupported_carriers_and_state_or_unremoved_comments_fail_without_source_mutation() {
    for (key, value) in [
        ("Annots", Object::Array(Vec::new())),
        ("PieceInfo", Object::string_literal(SECRET)),
        ("Thumb", Object::Reference((4, 0))),
    ] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        doc.get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(key, value);
        let source = save(doc);
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
        assert_eq!(
            pdf.verified_bytes().unwrap(),
            PdfDocument::from_bytes(&source, None)
                .unwrap()
                .verified_bytes()
                .unwrap()
        );
    }
    for raw in [
        b"BT /F1 10 Tf 20 180 Td 1 Tc (PRIVATE) Tj ET".as_slice(),
        b"BT /F1 10 Tf 20 180 Td (PRIVATE) Tj ET /Im1 Do".as_slice(),
    ] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        doc.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(raw.to_vec());
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
    }
    let pdf = PdfDocument::from_bytes(&fixture(), None).unwrap();
    assert!(pdf.verify_redaction(&[SECRET.into()]).is_err());
    assert!(pdf.redact_text_copy(&["absent-target".into()]).is_err());
    assert!(pdf.redact_text_copy(&["".into()]).is_err());
    let request = pdf_core::TextRedactionRequest {
        targets: vec![SECRET.into()],
    };
    assert!(!format!("{request:?}").contains(SECRET));
    assert!(
        serde_json::from_str::<pdf_core::TextRedactionRequest>(
            r#"{"targets":["PRIVATE"],"path":"leak"}"#
        )
        .is_err()
    );
}

#[test]
fn proportional_fonts_overlap_and_readonly_or_cross_object_residuals_are_checked() {
    let (cipher, _) = encryption::fixture(3, "reader", false);
    let encrypted = PdfDocument::from_bytes(&cipher, Some("reader")).unwrap();
    assert!(encrypted.redact_text_copy(&[SECRET.into()]).is_err());
    assert!(pdf_core::verify_redaction_bytes(&cipher, Some("reader"), &[SECRET.into()]).is_err());
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
        let mut doc = Document::load_mem(&fixture()).unwrap();
        doc.get_object_mut((3, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("BaseFont", Object::Name(font.as_bytes().to_vec()));
        let source = save(doc);
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let (output, report) = pdf
            .redact_text_copy(&[SECRET.into(), "RIV".into()])
            .unwrap();
        assert_eq!(report.matched_targets, 8);
        output
            .verify_redaction(&[SECRET.into(), "RIV".into()])
            .unwrap();
        let text = output.extract_text(&[1]).unwrap();
        assert!(text.contains("prefix"));
        assert!(text.contains("suffix"));
        assert!(text.contains("tail"));
    }
    let mut doc = Document::load_mem(&fixture()).unwrap();
    doc.add_object(dictionary! {"FT"=>"Sig"});
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
    let mut doc = Document::load_mem(&fixture()).unwrap();
    doc.get_object_mut((1, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("MarkInfo", dictionary! {"Marked"=>true});
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
    let mut doc = Document::load_mem(&fixture()).unwrap();
    doc.get_object_mut((5,0)).unwrap().as_stream_mut().unwrap().set_content(b"BT /F1 10 Tf 20 180 Td (PRIVATE) Tj ET BT /F1 10 Tf 20 160 Td (PRI) Tj ET BT /F1 10 Tf 20 140 Td (VATE) Tj ET".to_vec());
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
}

#[test]
fn escaped_and_hex_targets_are_removed_but_opaque_font_images_forms_and_unicode_refuse() {
    for comment in [
        "% PRIVATE literal",
        "% <50524956415445> hex",
        "% (PR\\111VATE) escaped",
    ] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        doc.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(format!("{comment}\nBT /F1 10 Tf 20 180 Td (PRIVATE) Tj ET").into_bytes());
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        let (output, report) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
        assert_eq!(report.content_comments_removed, 2);
        output.verify_redaction(&[SECRET.into()]).unwrap();
    }
    for glyphs in [
        b"(PR\\111VATE)".as_slice(),
        b"<50524956415445>".as_slice(),
        b"[(PR) 0 <4956415445>]".as_slice(),
    ] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let mut raw = b"BT /F1 10 Tf 20 180 Td ".to_vec();
        raw.extend(glyphs);
        raw.extend_from_slice(if glyphs[0] == b'[' {
            b" TJ ET"
        } else {
            b" Tj ET"
        });
        doc.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(raw);
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        assert!(pdf.extract_text(&[1]).unwrap().contains(SECRET));
        let (output, _) = pdf.redact_text_copy(&[SECRET.into()]).unwrap();
        output.verify_redaction(&[SECRET.into()]).unwrap();
    }
    for key in ["ToUnicode", "FontDescriptor"] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let carrier = doc.add_object(Stream::new(dictionary! {}, SECRET.as_bytes().to_vec()));
        doc.get_object_mut((3, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(key, carrier);
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
    }
    for subtype in ["Image", "Form"] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let carrier=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>subtype,"BBox"=>vec![0.into(),0.into(),10.into(),10.into()]},SECRET.as_bytes().to_vec()));
        doc.get_object_mut((2, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("XObject", dictionary! {"Hidden"=>carrier});
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
    }
    let mut doc = Document::load_mem(&fixture()).unwrap();
    doc.get_object_mut((5, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_content(b"BT /F1 10 Tf 20 180 Td <FEFF0050005200490056004100540045> Tj ET".to_vec());
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    assert!(pdf.redact_text_copy(&[SECRET.into()]).is_err());
}
