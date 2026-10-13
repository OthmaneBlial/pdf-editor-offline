#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{ContentTextChange, PdfDocument};
use std::path::PathBuf;
use std::process::Command;
#[path = "content_edit/embedded.rs"]
mod embedded;
#[path = "content_edit/face.rs"]
mod face;
#[path = "content_edit/properties.rs"]
mod properties;
#[path = "content_edit/proportional.rs"]
mod proportional;

const CONTENT: &[u8] = b"0.2 0.4 0.6 rg 10 10 20 20 re f\nBT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET\nBT /F 12 Tf 1 0 0 1 40 110 Tm 0 Tr (A B) Tj ET\n";

fn source(content: &[u8], font: lopdf::Dictionary) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let catalog = document.new_object_id();
    let pages = document.new_object_id();
    let first = document.new_object_id();
    let second = document.new_object_id();
    let font = document.add_object(font);
    let mut stream = Stream::new(dictionary! {}, content.to_vec());
    stream.compress().unwrap();
    let stream = document.add_object(stream);
    for id in [first, second] {
        document.objects.insert(
            id,
            dictionary! {"Type"=>"Page", "Parent"=>pages,"Contents"=>stream}.into(),
        );
    }
    document.objects.insert(pages, dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>vec![first.into(),second.into()],"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}}.into());
    document.objects.insert(
        catalog,
        dictionary! {"Type"=>"Catalog","Pages"=>pages}.into(),
    );
    document.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn courier() -> lopdf::Dictionary {
    dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"}
}

fn change(pdf: &PdfDocument, index: usize, replacement: &str) -> ContentTextChange {
    let runs = pdf.content_text_runs(1).unwrap();
    assert!(runs[index].writable, "{:?}", runs[index].refusal_reason);
    ContentTextChange {
        layout_policy: Default::default(),
        identity: runs[index].identity.clone(),
        expected: runs[index].text.clone().unwrap(),
        replacement: replacement.into(),
    }
}

fn outside(before: &[u8], after: &[u8], channels: usize) {
    assert_eq!(before.len(), after.len());
    let mut changed = 0;
    for (index, (a, b)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if a != b {
            let x = index % 200;
            let y = index / 200;
            assert!(
                (35..145).contains(&x) && (35..70).contains(&y),
                "outside selected glyph envelope {x},{y}"
            );
            changed += 1;
        }
    }
    assert!(
        changed > 20,
        "replacement must really paint different glyphs"
    );
}

#[test]
fn real_operand_replacement_preserves_other_pages_resources_and_artwork() {
    let bytes = source(CONTENT, courier());
    let original = Document::load_mem(&bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let replacement = change(&pdf, 0, "Replaced");
    pdf.replace_content_text(1, std::slice::from_ref(&replacement))
        .unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let output = Document::load_mem(&saved).unwrap();
    let original_ops = lopdf::content::Content::decode(&original.get_page_content((3, 0))).unwrap();
    let edited_ops = lopdf::content::Content::decode(&output.get_page_content((3, 0))).unwrap();
    assert_eq!(original_ops.operations.len(), edited_ops.operations.len());
    for (index, (before, after)) in original_ops
        .operations
        .iter()
        .zip(&edited_ops.operations)
        .enumerate()
    {
        assert_eq!(before.operator, after.operator);
        if index != replacement.identity.operation as usize {
            assert_eq!(before.operands, after.operands);
        } else {
            assert_eq!(after.operands[0].as_str().unwrap(), b"Replaced");
        }
    }
    for (id, value) in &original.objects {
        if *id != (3, 0) && !matches!(value.type_name(), Ok(b"XRef" | b"ObjStm")) {
            assert_eq!(
                output.get_object(*id).unwrap(),
                value,
                "untouched source object{id:?}"
            );
        }
    }
    let first = pdf.extract_text(&[1]).unwrap();
    assert!(first.contains("Replaced"));
    assert!(!first.contains("Original"));
    assert!(first.contains("A B"));
    assert!(pdf.extract_text(&[2]).unwrap().contains("Original"));
    let before = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    let after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    outside(&before.rgba, &after.rgba, 4);
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap().rgba,
        pdf_render::render_page(&saved, 1, 1.0).unwrap().rgba
    );
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    assert!(
        reopened.replace_content_text(1, &[replacement]).is_err(),
        "old identity must be stale"
    );
    let second = change(&reopened, 0, "Restored");
    reopened.replace_content_text(1, &[second]).unwrap();
    assert!(reopened.extract_text(&[1]).unwrap().contains("Restored"));
}

#[test]
fn batch_errors_stale_identity_wrong_expected_and_layout_changes_roll_back() {
    let bytes = source(CONTENT, courier());
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    for value in ["Short", "Too long!", "Repl ced", "Réplaced", "Replaced\n"] {
        let request = change(&pdf, 0, value);
        assert!(pdf.replace_content_text(1, &[request]).is_err());
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
    let good = change(&pdf, 0, "Replaced");
    let mut bad = change(&pdf, 1, "C D");
    bad.expected = "X Y".into();
    assert!(pdf.replace_content_text(1, &[good.clone(), bad]).is_err());
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    assert!(
        pdf.replace_content_text(1, &[good.clone(), good.clone()])
            .is_err()
    );
    assert!(
        pdf.replace_content_text(2, std::slice::from_ref(&good))
            .is_err()
    );
    let mut forged = good;
    forged.identity.stream_digest[0] ^= 1;
    assert!(pdf.replace_content_text(1, &[forged]).is_err());
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    pdf.replace_content_text(1, &[change(&pdf, 0, "Replaced"), change(&pdf, 1, "C D")])
        .unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("C D"));
}

#[test]
fn pdf_delimiters_remain_literal_text_after_verified_save_and_reopen() {
    let mut pdf = PdfDocument::from_bytes(&source(CONTENT, courier()), None).unwrap();
    let value = "(\\)[]{}?";
    pdf.replace_content_text(1, &[change(&pdf, 0, value)])
        .unwrap();
    let reopened = PdfDocument::from_bytes(&pdf.verified_bytes().unwrap(), None).unwrap();
    assert!(reopened.extract_text(&[1]).unwrap().contains(value));
    assert_eq!(
        reopened.content_text_runs(1).unwrap()[0].text.as_deref(),
        Some(value)
    );
    assert_eq!(reopened.content_text_runs(1).unwrap().len(), 2);
}

#[test]
fn default_courier_quote_codes_are_not_presented_as_ascii_inventory() {
    let content = b"BT /F 12 Tf 1 0 0 1 40 140 Tm 0 Tr (A'B`C) Tj ET";
    let mut font = courier();
    font.remove(b"Encoding");
    let pdf = PdfDocument::from_bytes(&source(content, font), None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "A\u{2019}B\u{2018}C\n");
    let run = &pdf.content_text_runs(1).unwrap()[0];
    assert!(!run.writable);
    assert!(run.text.is_none());
    assert!(
        run.refusal_reason
            .as_deref()
            .unwrap()
            .contains("codes39/96")
    );
    let mut explicit = PdfDocument::from_bytes(&source(content, courier()), None).unwrap();
    assert_eq!(explicit.extract_text(&[1]).unwrap(), "A'B`C\n");
    explicit
        .replace_content_text(1, &[change(&explicit, 0, "`B'C!")])
        .unwrap();
    assert_eq!(explicit.extract_text(&[1]).unwrap(), "`B'C!\n");
    for named in [false, true] {
        let mut font = courier();
        if named {
            font.set("Encoding", "StandardEncoding");
        } else {
            font.remove(b"Encoding");
        }
        let mut ordinary = PdfDocument::from_bytes(&source(CONTENT, font), None).unwrap();
        ordinary
            .replace_content_text(1, &[change(&ordinary, 0, "Replaced")])
            .unwrap();
        assert!(ordinary.extract_text(&[1]).unwrap().contains("Replaced"));
        let before = ordinary.verified_bytes().unwrap();
        for replacement in ["Replac'd", "Replac`d"] {
            assert!(
                ordinary
                    .replace_content_text(1, &[change(&ordinary, 0, replacement)])
                    .is_err()
            );
            assert_eq!(ordinary.verified_bytes().unwrap(), before);
        }
    }
}

#[test]
fn supported_ascii_policy_matches_every_actual_encoding_table_slot() {
    let document = Document::new();
    for kind in [
        pdf_core::ContentTextEncoding::Standard,
        pdf_core::ContentTextEncoding::WinAnsi,
    ] {
        let mut font = courier();
        if matches!(kind, pdf_core::ContentTextEncoding::Standard) {
            font.remove(b"Encoding");
        }
        let encoding = font
            .get_font_encoding_with_limit(&document, pdf_core::MAX_STREAM_BYTES)
            .unwrap();
        for byte in 32u8..=126 {
            let ascii = char::from(byte).to_string();
            let decoded = encoding.bytes_to_string(&[byte]).unwrap();
            assert_eq!(
                kind.accepts_ascii(&ascii),
                decoded == ascii,
                "{kind:?} code{byte}: {decoded:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires pinned Poppler26.10.0"]
fn punctuation_encoding_matches_independent_poppler() {
    let temporary = tempfile::tempdir().unwrap();
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap()).join("pdftotext");
    assert!(
        String::from_utf8_lossy(&Command::new(&bin).arg("-v").output().unwrap().stderr)
            .contains("26.10.0")
    );
    for (explicit, expected) in [(false, "A\u{2019}B\u{2018}C"), (true, "A'B`C")] {
        let mut font = courier();
        if !explicit {
            font.remove(b"Encoding");
        }
        let file = temporary.path().join(if explicit {
            "winansi.pdf"
        } else {
            "default.pdf"
        });
        let bytes = source(b"BT /F 12 Tf 1 0 0 1 40 140 Tm 0 Tr (A'B`C) Tj ET", font);
        std::fs::write(&file, &bytes).unwrap();
        let output = Command::new(&bin).arg(&file).arg("-").output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            vec![expected, expected]
        );
    }
}

#[test]
fn override_fonts_marked_content_and_unsafe_text_objects_are_reported_readonly() {
    for key in [
        "Widths",
        "FirstChar",
        "LastChar",
        "FontDescriptor",
        "ToUnicode",
    ] {
        let mut font = courier();
        font.set(key, Object::Null);
        let bytes = source(CONTENT, font);
        let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        assert!(
            pdf.content_text_runs(1)
                .unwrap()
                .iter()
                .all(|run| !run.writable && run.text.is_none()),
            "{key}"
        );
    }
    for content in [
        b"/Span BMC BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj ET EMC".as_slice(),
        b"BT /F 20 Tf 1 0 0 1 40 140 Tm 7 Tr (Original) Tj ET",
        b"BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj (Original) Tj ET",
        b"BT /F 20 Tf 1 0 0 1 40 140 Tm 0 Tr (Original) Tj 1 0 Td ET",
        b"BT /F 20 Tf 1 0 0 1 40 140 Tm (Original) Tj ET",
    ] {
        let pdf = PdfDocument::from_bytes(&source(content, courier()), None).unwrap();
        assert!(
            pdf.content_text_runs(1)
                .unwrap()
                .iter()
                .all(|run| !run.writable && run.text.is_none())
        );
    }
    let pdf =
        PdfDocument::from_bytes(&source(b"BT /F 12 Tf (Original) Tj", courier()), None).unwrap();
    assert!(pdf.content_text_runs(1).is_err());
    assert!(pdf.content_text_runs(0).is_err());
}

#[test]
fn tagged_catalog_signed_encrypted_and_stream_shapes_fail_closed() {
    let input = source(CONTENT, courier());
    let ordinary = PdfDocument::from_bytes(&input, None).unwrap();
    let request = change(&ordinary, 0, "Replaced");
    for kind in ["tagged", "signed", "array", "encrypted"] {
        let mut document = Document::load_mem(&input).unwrap();
        match kind {
            "tagged" => {
                document
                    .catalog_mut()
                    .unwrap()
                    .set("MarkInfo", dictionary! {"Marked"=>true});
            }
            "signed" => {
                document.add_object(dictionary!{"FT"=>"Sig","V"=>dictionary!{"Type"=>"Sig","ByteRange"=>vec![0.into(),1.into(),2.into(),3.into()]}});
            }
            "array" => {
                let page = document.get_dictionary_mut((3, 0)).unwrap();
                let contents = page.get(b"Contents").unwrap().clone();
                page.set("Contents", vec![contents]);
            }
            "encrypted" => {
                document.trailer.set(
                    "ID",
                    vec![
                        Object::string_literal(b"synthetic-fileid".to_vec()),
                        Object::string_literal(b"synthetic-fileid".to_vec()),
                    ],
                );
                let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V1 {
                    permissions: lopdf::Permissions::all(),
                    document: &document,
                    owner_password: "owner",
                    user_password: "reader",
                })
                .unwrap();
                document.encrypt(&state).unwrap();
            }
            _ => unreachable!(),
        }
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        let mut pdf = PdfDocument::from_bytes(
            &bytes,
            if kind == "encrypted" {
                Some("reader")
            } else {
                None
            },
        )
        .unwrap();
        let before_pages = pdf.info().unwrap().pages;
        let before_text = pdf.extract_text(&[1]).unwrap();
        if kind != "array" {
            assert!(
                pdf.content_text_runs(1)
                    .unwrap()
                    .iter()
                    .all(|run| !run.writable),
                "{kind}"
            );
        } else {
            assert!(pdf.content_text_runs(1).is_err());
        }
        assert!(
            pdf.replace_content_text(1, std::slice::from_ref(&request))
                .is_err(),
            "{kind}"
        );
        assert_eq!(pdf.info().unwrap().pages, before_pages);
        assert_eq!(pdf.extract_text(&[1]).unwrap(), before_text);
        if kind != "signed" && kind != "encrypted" {
            let before = pdf.verified_bytes().unwrap();
            assert!(
                pdf.replace_content_text(1, std::slice::from_ref(&request))
                    .is_err()
            );
            assert_eq!(before, pdf.verified_bytes().unwrap());
        }
    }
}

#[test]
fn inventory_stream_operator_and_run_budgets_are_bounded() {
    for content in [
        vec![b' '; 32 * 1024 * 1024 + 1],
        b"q\n".repeat(100_001),
        b"(x) Tj\n".repeat(10_001),
    ] {
        let mut pdf = PdfDocument::from_bytes(&source(&content, courier()), None).unwrap();
        assert!(pdf.content_text_runs(1).is_err());
        assert!(pdf.replace_content_text(1, &[]).is_err());
    }
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10.0"]
fn independent_existing_operand_text_pixels_and_shared_page_gate() {
    for family in [
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.pdf");
        let output = directory.path().join("output.pdf");
        let mut font = courier();
        font.set("BaseFont", family);
        let bytes = source(CONTENT, font);
        std::fs::write(&input, &bytes).unwrap();
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        pdf.replace_content_text(1, &[change(&pdf, 0, "Replaced")])
            .unwrap();
        pdf.save_new(&output).unwrap();
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
            Command::new(qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let poppler = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
        let version = Command::new(poppler.join("pdftotext"))
            .arg("-v")
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&version.stderr).contains("pdftotext version 26.10.0"));
        let text = Command::new(poppler.join("pdftotext"))
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
            vec!["Replaced", "A", "B", "Original", "A", "B"]
        );
        let mut rasters = Vec::new();
        for (name, path) in [("before", &input), ("after", &output)] {
            for page in [1, 2] {
                let base = directory.path().join(format!("{name}-{page}"));
                let result = Command::new(poppler.join("pdftoppm"))
                    .args([
                        "-r",
                        "72",
                        "-singlefile",
                        "-f",
                        &page.to_string(),
                        "-l",
                        &page.to_string(),
                    ])
                    .arg(path)
                    .arg(&base)
                    .output()
                    .unwrap();
                assert!(result.status.success());
                let raster = std::fs::read(base.with_extension("ppm")).unwrap();
                assert!(raster.starts_with(b"P6\n200 200\n255\n"));
                rasters.push(raster[b"P6\n200 200\n255\n".len()..].to_vec());
            }
        }
        outside(&rasters[0], &rasters[2], 3);
        assert_eq!(rasters[1], rasters[3]);
        assert_eq!(std::fs::read(input).unwrap(), bytes);
    }
}
