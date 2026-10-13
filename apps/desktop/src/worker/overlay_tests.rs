#![allow(clippy::unwrap_used)]
use super::*;
const PDF: &[u8] = include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf");
fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let font = temp.path().join("font.ttf");
    std::fs::write(&input, PDF).unwrap();
    std::fs::write(
        &font,
        include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
    )
    .unwrap();
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
    let old = documents[&1].current.bytes.clone();
    let key = FormsReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    let value = "Overlay é\nSecond line ΩЖ";
    let request = pdf_core::TextOverlay {
        page: 1,
        rectangle: [350.0, 400.0, 550.0, 500.0],
        value: value.into(),
        size: 12.0,
        color: [0.1, 0.3, 0.2],
        alignment: pdf_core::TextAlignment::Center,
    };
    let command = |key, font_file, overlay| Command::Edit {
        id: 1,
        edit: Edit::TextOverlay {
            key,
            overlay,
            font_file,
        },
    };
    let bad = temp.path().join("bad.ttf");
    std::fs::write(&bad, b"bad font").unwrap();
    let mut overflow = request.clone();
    overflow.value = "A\n".repeat(100);
    for invalid in [
        command(key, bad, request.clone()),
        command(
            FormsReadKey {
                revision: 99,
                ..key
            },
            font.clone(),
            request.clone(),
        ),
        command(
            FormsReadKey { tab: 2, ..key },
            font.clone(),
            request.clone(),
        ),
        command(key, font.clone(), overflow),
    ] {
        assert!(process_with_journal(&mut documents, invalid, Some(&mut journal)).is_err());
        assert_eq!(documents[&1].current.bytes, old);
        assert!(documents[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, old);
    }
    assert!(
        documents
            .get_mut(&1)
            .unwrap()
            .edit_durable(
                Edit::TextOverlay {
                    key,
                    overlay: request.clone(),
                    font_file: font.clone()
                },
                HISTORY_BYTES,
                "1",
                |_| Err("forced journal failure".into())
            )
            .is_err()
    );
    assert_eq!(documents[&1].current.bytes, old);
    assert!(documents[&1].undo.is_empty());
    let Event::Updated {
        status,
        selected_page,
        ..
    } = process_with_journal(
        &mut documents,
        command(key, font.clone(), request.clone()),
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("updated");
    };
    assert!(status.dirty && status.can_undo);
    assert_eq!(selected_page, Some(0));
    assert_eq!(documents[&1].undo.len(), 1);
    let changed = documents[&1].current.bytes.clone();
    let original = pdf_core::PdfDocument::from_bytes(&old, None).unwrap();
    let edited = pdf_core::PdfDocument::from_bytes(&changed, None).unwrap();
    assert_eq!(edited.info().unwrap().pages, original.info().unwrap().pages);
    assert_eq!(
        edited.list_form_fields().unwrap(),
        original.list_form_fields().unwrap()
    );
    assert!(edited.extract_text(&[1]).unwrap().contains(value));
    assert!(
        process_with_journal(
            &mut documents,
            command(key, font, request),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(documents[&1].current.bytes, changed);
    for (edit, expected) in [(Edit::Undo, old.clone()), (Edit::Redo, changed.clone())] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(documents[&1].current.bytes, expected);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, expected);
    }
    let output = temp.path().join("saved-overlay.pdf");
    process_with_journal(
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert!(!documents[&1].status().unwrap().dirty);
    let saved = std::fs::read(&output).unwrap();
    assert!(
        pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains(value)
    );
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
    assert_eq!(std::fs::read(&input).unwrap(), PDF);
    if reference {
        let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
        let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
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
        assert!(String::from_utf8(text.stdout).unwrap().contains(value));
        let pixels = |path: &PathBuf| {
            let result = std::process::Command::new(bin.join("pdftoppm"))
                .args(["-singlefile", "-r", "72"])
                .arg(path)
                .output()
                .unwrap();
            assert!(result.status.success());
            result
                .stdout
                .strip_prefix(b"P6\n595 842\n255\n")
                .unwrap()
                .to_vec()
        };
        let before = pixels(&input);
        let after = pixels(&output);
        let mut count = 0;
        for (index, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                let x = index % 595;
                let y = index / 595;
                assert!(
                    (350..550).contains(&x) && (342..442).contains(&y),
                    "outside overlay{x},{y}"
                );
                count += 1;
            }
        }
        assert!(count > 20);
    }
}
#[test]
fn real_overlay_child_durable_failure_history_stale_and_save_are_atomic() {
    exercise(false);
}
#[test]
#[ignore = "build normal native executable, pinned qpdf12.4.2/Poppler26.10 required"]
fn production_native_overlay_worker_save_and_independent_pixels() {
    let exe = std::env::current_exe().unwrap();
    let binary = exe
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-editor-offline-native.exe"
        } else {
            "pdf-editor-offline-native"
        });
    assert!(binary.is_file());
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    assert!(
        String::from_utf8_lossy(
            &std::process::Command::new(qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .contains("12.4.2")
    );
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        String::from_utf8_lossy(
            &std::process::Command::new(bin.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    document::with_test_executor(
        binary,
        vec![std::ffi::OsString::from("--pdf-worker")],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
