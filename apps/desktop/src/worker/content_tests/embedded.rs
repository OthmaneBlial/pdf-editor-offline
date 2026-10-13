use super::*;
const FONT: &[u8] = include_bytes!("../../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const VALUE: &str = "AV Café ΩЖ";
fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let output = temp.path().join("new.pdf");
    let font = temp.path().join("font.ttf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
    std::fs::write(&font, FONT).unwrap();
    let mut journal = Journal::open(temp.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    process_with_journal(
        &mut documents,
        Command::Load {
            id: 1,
            path: input.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let key = ContentTextKey {
        tab: 1,
        revision: 0,
        page: 1,
        offset: 0,
        generation: 1,
    };
    let Event::ContentText { inventory, .. } = process_with_journal(
        &mut documents,
        Command::ContentText(key),
        Some(&mut journal),
    )
    .unwrap() else {
        panic!("inventory")
    };
    let change = pdf_core::ContentTextEmbeddedChange {
        identity: inventory.runs[0].identity.clone(),
        expected: "Original".into(),
        replacement: Some(VALUE.into()),
        layout_policy: pdf_core::ContentTextLayoutPolicy::AllowWidthChange,
    };
    let command = |key, hash, change, font_file| Command::Edit {
        id: 1,
        edit: Edit::ContentTextEmbeddedFont {
            key,
            source_fingerprint: hash,
            change,
            font_file,
        },
    };
    for case in 0..7 {
        let mut k = key;
        let mut hash = inventory.source_fingerprint;
        let mut c = change.clone();
        let mut f = font.clone();
        match case {
            0 => k.tab = 2,
            1 => k.revision = 1,
            2 => hash[0] ^= 1,
            3 => c.expected = "Wrong".into(),
            4 => c.layout_policy = Default::default(),
            5 => f = temp.path().join("missing.ttf"),
            _ => c.replacement = Some("😺".into()),
        };
        assert!(
            process_with_journal(&mut documents, command(k, hash, c, f), Some(&mut journal))
                .is_err()
        );
        assert_eq!(documents[&1].current.bytes, bytes);
        assert!(documents[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, bytes);
    }
    assert!(
        documents
            .get_mut(&1)
            .unwrap()
            .edit_durable(
                Edit::ContentTextEmbeddedFont {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change: change.clone(),
                    font_file: font.clone()
                },
                HISTORY_BYTES,
                "1",
                |_| Err("forced durable failure".into())
            )
            .is_err()
    );
    assert_eq!(documents[&1].current.bytes, bytes);
    process_with_journal(
        &mut documents,
        command(key, inventory.source_fingerprint, change, font.clone()),
        Some(&mut journal),
    )
    .unwrap();
    let changed = documents[&1].current.bytes.clone();
    assert_eq!(documents[&1].undo.len(), 1);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, changed);
    for (edit, expected) in [(Edit::Undo, &bytes), (Edit::Redo, &changed)] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(&documents[&1].current.bytes, expected);
    }
    process_with_journal(
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let saved = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), format!("{VALUE}\nA B\n"));
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(
        pdf.info().unwrap().pages,
        pdf_core::PdfDocument::from_bytes(&bytes, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
    assert!(pdf.content_text_runs(1).unwrap()[0].properties_writable);
    assert!(
        process_with_journal(
            &mut documents,
            Command::Save {
                id: 1,
                path: output.clone()
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    assert_eq!(std::fs::read(&font).unwrap(), FONT);
    if reference {
        let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
        let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
        assert!(
            std::process::Command::new(qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let text = std::process::Command::new(bin.join("pdftotext"))
            .arg("-raw")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert_eq!(
            String::from_utf8(text.stdout).unwrap(),
            format!("{VALUE}\nA B\x0cOriginal\nA B\x0c")
        );
        let raster = |path: &PathBuf, page: &str| {
            let result = std::process::Command::new(bin.join("pdftoppm"))
                .args(["-singlefile", "-r", "72", "-f", page])
                .arg(path)
                .output()
                .unwrap();
            assert!(result.status.success());
            result
                .stdout
                .strip_prefix(b"P6\n200 200\n255\n")
                .unwrap()
                .to_vec()
        };
        assert_eq!(raster(&input, "2"), raster(&output, "2"));
        let before = raster(&input, "1");
        let after = raster(&output, "1");
        let mut changed = 0;
        for (i, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                assert!(
                    (39..=190).contains(&(i % 200)) && (35..=65).contains(&(i / 200)),
                    "outside actual text ink {},{}",
                    i % 200,
                    i / 200
                );
                changed += 1;
            }
        }
        assert!(changed > 20);
    }
}
#[test]
fn embedded_child_report_rejects_inconsistent_ids_names_and_unbounded_metrics() {
    let bytes = fixture::source(fixture::CONTENT);
    let mut pdf = pdf_core::PdfDocument::from_bytes(&bytes, None).unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    pdf.change_content_text_embedded_font(
        1,
        &pdf_core::ContentTextEmbeddedChange {
            identity: run.identity,
            expected: "Original".into(),
            replacement: Some(VALUE.into()),
            layout_policy: pdf_core::ContentTextLayoutPolicy::AllowWidthChange,
        },
        FONT,
    )
    .unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert!(!document::embedded_report_invalid(&run));
    let mut longest = run.clone();
    longest.embedded_font.as_mut().unwrap().postscript_name = "F".repeat(128);
    assert!(!document::embedded_report_invalid(&longest));
    for case in 0..8 {
        let mut altered = run.clone();
        let font = altered.embedded_font.as_mut().unwrap();
        match case {
            0 => font.font_object.0 = 0,
            1 => font.postscript_name = "bad\nname".into(),
            2 => font.widths.push(-1.0),
            3 => font.advances.push(f64::NAN),
            4 => font.widths = vec![0.0; 65_536],
            5 => font.advances = vec![0.0; 131_070],
            6 => altered.standard_font = Some(pdf_core::ContentTextFont::Courier),
            _ => font.postscript_name = "F".repeat(129),
        };
        assert!(document::embedded_report_invalid(&altered), "case{case}");
    }
}
#[test]
fn chosen_font_real_child_keeps_history_source_identity_and_durable_atomic_errors() {
    exercise(false);
}
#[test]
#[ignore = "requires fresh normal native executable and pinned qpdf12.4.2/Poppler26.10"]
fn production_native_chosen_font_unicode_kerning_and_saved_output() {
    let qpdf = std::process::Command::new(std::env::var_os("PDF_QPDF_BIN").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&qpdf.stdout).contains("12.4.2"));
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let poppler = std::process::Command::new(bin.join("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&poppler.stderr).contains("26.10.0"));
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-editor-offline-native.exe"
        } else {
            "pdf-editor-offline-native"
        });
    document::with_test_executor(
        binary,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
