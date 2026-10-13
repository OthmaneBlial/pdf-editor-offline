#![allow(clippy::unwrap_used)]
use super::*;
const PDF: &[u8] = include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf");
fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    std::fs::write(&input, PDF).unwrap();
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
    let request = pdf_core::VectorOverlay {
        page: 1,
        shape: pdf_core::VectorShape::MultiPolyline {
            strokes: vec![
                vec![[350.0, 400.0], [400.0, 480.0], [450.0, 410.0]],
                vec![[500.0, 420.0], [550.0, 500.0]],
                vec![[480.0, 470.0]],
            ],
        },
        stroke: Some(pdf_core::VectorStroke {
            color: [0.8, 0.1, 0.0],
            width: 4.0,
        }),
        fill: None,
        opacity: 0.5,
    };
    let command = |key, overlay| Command::Edit {
        id: 1,
        edit: Edit::VectorOverlay { key, overlay },
    };
    for key in [
        FormsReadKey { tab: 2, ..key },
        FormsReadKey {
            revision: 10,
            ..key
        },
    ] {
        assert!(
            process_with_journal(
                &mut documents,
                command(key, request.clone()),
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(documents[&1].current.bytes, old);
    }
    let mut invalid = request.clone();
    invalid.opacity = 2.0;
    assert!(
        process_with_journal(&mut documents, command(key, invalid), Some(&mut journal)).is_err()
    );
    assert_eq!(documents[&1].current.bytes, old);
    process_with_journal(
        &mut documents,
        command(key, request.clone()),
        Some(&mut journal),
    )
    .unwrap();
    let changed = documents[&1].current.bytes.clone();
    assert_ne!(changed, old);
    assert_eq!(documents[&1].undo.len(), 1);
    assert!(documents[&1].status().unwrap().dirty);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, changed);
    assert!(
        process_with_journal(&mut documents, command(key, request), Some(&mut journal)).is_err()
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
    let output = temp.path().join("visual-signature.pdf");
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
    let original = pdf_core::PdfDocument::from_bytes(PDF, None).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(pdf.info().unwrap().pages, original.info().unwrap().pages);
    assert_eq!(
        pdf.list_form_fields().unwrap(),
        original.list_form_fields().unwrap()
    );
    assert_eq!(
        pdf.extract_text(&[1]).unwrap(),
        original.extract_text(&[1]).unwrap()
    );
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
        let raster = |path: &PathBuf| {
            let out = std::process::Command::new(bin.join("pdftoppm"))
                .args(["-singlefile", "-r", "72"])
                .arg(path)
                .output()
                .unwrap();
            assert!(out.status.success());
            out.stdout
                .strip_prefix(b"P6\n595 842\n255\n")
                .unwrap()
                .to_vec()
        };
        let before = raster(&input);
        let after = raster(&output);
        let mut changed = 0;
        for (i, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                assert!((347..553).contains(&(i % 595)) && (339..445).contains(&(i / 595)));
                changed += 1;
            }
        }
        assert!(changed > 40);
        // Pen lifts do not invent a line from (450,410) to (500,420).
        let connector = (427 * 595 + 475) * 3;
        assert_eq!(
            &before[connector..connector + 3],
            &after[connector..connector + 3]
        );
        let dot = (372 * 595 + 480) * 3;
        assert_ne!(&before[dot..dot + 3], &after[dot..dot + 3]);
        let extract = |path: &PathBuf| {
            let out = std::process::Command::new(bin.join("pdftotext"))
                .arg("-raw")
                .arg(path)
                .arg("-")
                .output()
                .unwrap();
            assert!(out.status.success());
            out.stdout
        };
        assert_eq!(extract(&input), extract(&output));
    }
}
#[test]
fn real_visual_signature_child_stale_history_journal_and_verified_save() {
    exercise(false);
}
#[test]
#[ignore = "requires freshly built normal native executable, qpdf12.4.2 and Poppler26.10"]
fn production_native_visual_signature_worker_and_independent_saved_pixels() {
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("pdf-editor-offline-native");
    assert!(binary.is_file());
    document::with_test_executor(
        binary,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
