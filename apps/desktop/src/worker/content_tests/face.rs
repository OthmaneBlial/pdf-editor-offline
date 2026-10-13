use super::*;

fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("new.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
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
    let change = pdf_core::ContentTextFaceChange {
        identity: inventory.runs[0].identity.clone(),
        expected: "Original".into(),
        font: pdf_core::ContentTextFont::TimesBold,
        replacement: None,
        layout_policy: pdf_core::ContentTextLayoutPolicy::AllowWidthChange,
    };
    let command = |key, hash, change| Command::Edit {
        id: 1,
        edit: Edit::ContentTextFace {
            key,
            source_fingerprint: hash,
            change,
        },
    };
    for case in 0..5 {
        let mut target = key;
        let mut hash = inventory.source_fingerprint;
        let mut altered = change.clone();
        match case {
            0 => target.tab = 2,
            1 => target.revision = 99,
            2 => target.page = 0,
            3 => hash[0] ^= 1,
            _ => altered.expected = "Wrong".into(),
        };
        assert!(
            process_with_journal(
                &mut documents,
                command(target, hash, altered),
                Some(&mut journal)
            )
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
                Edit::ContentTextFace {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change: change.clone()
                },
                HISTORY_BYTES,
                "1",
                |_| Err("forced journal failure".into())
            )
            .is_err()
    );
    assert_eq!(documents[&1].current.bytes, bytes);
    assert!(documents[&1].undo.is_empty());
    let intent = journal
        .begin_save("1", 0, &temp.path().join("uncertain.pdf"), &bytes)
        .unwrap();
    assert!(
        process_with_journal(
            &mut documents,
            command(key, inventory.source_fingerprint, change.clone()),
            Some(&mut journal)
        )
        .is_err()
    );
    journal.cancel_save(&intent.token).unwrap();
    let Event::Updated {
        status,
        selected_page,
        ..
    } = process_with_journal(
        &mut documents,
        command(key, inventory.source_fingerprint, change.clone()),
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("updated")
    };
    assert!(status.dirty && status.can_undo);
    assert_eq!(selected_page, Some(0));
    let edited = documents[&1].current.bytes.clone();
    assert_eq!(documents[&1].undo.len(), 1);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, edited);
    assert!(
        process_with_journal(
            &mut documents,
            command(key, inventory.source_fingerprint, change),
            Some(&mut journal)
        )
        .is_err()
    );
    for (edit, expected) in [(Edit::Undo, &bytes), (Edit::Redo, &edited)] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(&documents[&1].current.bytes, expected);
        assert_eq!(&journal.recover().unwrap()[0].current.bytes, expected);
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
    let pdf = PdfDocument::from_bytes(&saved, None).unwrap();
    let run = pdf.content_text_runs(1).unwrap().remove(0);
    assert_eq!(
        run.standard_font,
        Some(pdf_core::ContentTextFont::TimesBold)
    );
    assert!(run.writable && run.properties_writable);
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Original\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_ne!(
        pdf_render::render_page(&bytes, 0, 1.).unwrap().rgba,
        pdf_render::render_page(&saved, 0, 1.).unwrap().rgba
    );
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.).unwrap(),
        pdf_render::render_page(&saved, 1, 1.).unwrap()
    );
    assert_eq!(std::fs::read(input).unwrap(), bytes);
    assert_eq!(journal.recover().unwrap()[0].saved_revision, Some(1));
    if reference {
        let qpdf = std::path::PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap());
        let poppler = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
                .starts_with(b"qpdf version 12.4.2")
        );
        assert!(
            String::from_utf8_lossy(
                &std::process::Command::new(poppler.join("pdftotext"))
                    .arg("-v")
                    .output()
                    .unwrap()
                    .stderr
            )
            .contains("26.10.0")
        );
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let text = std::process::Command::new(poppler.join("pdftotext"))
            .args(["-f", "1", "-l", "1", "-raw"])
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let original_text = std::process::Command::new(poppler.join("pdftotext"))
            .args(["-f", "1", "-l", "1", "-raw"])
            .arg(temp.path().join("source.pdf"))
            .arg("-")
            .output()
            .unwrap();
        assert!(original_text.status.success());
        assert_eq!(original_text.stdout, b"Original\nA B\x0c");
        assert_eq!(
            text.stdout, original_text.stdout,
            "font-only edit must preserve exact independent-reader output"
        );
        let mut rasters = Vec::new();
        for (index, path) in [temp.path().join("source.pdf"), output].iter().enumerate() {
            let base = temp.path().join(format!("raster{index}"));
            let result = std::process::Command::new(poppler.join("pdftoppm"))
                .args(["-r", "72", "-f", "1", "-l", "1", "-singlefile"])
                .arg(path)
                .arg(&base)
                .output()
                .unwrap();
            assert!(result.status.success());
            let raster = std::fs::read(base.with_extension("ppm")).unwrap();
            let header = b"P6\n200 200\n255\n";
            assert!(raster.starts_with(header));
            rasters.push(raster[header.len()..].to_vec());
        }
        let mut changed = 0;
        for (index, (old, new)) in rasters[0]
            .chunks_exact(3)
            .zip(rasters[1].chunks_exact(3))
            .enumerate()
        {
            if old != new {
                changed += 1;
                assert!(
                    (30..185).contains(&(index % 200)) && (35..70).contains(&(index / 200)),
                    "outside fonttarget{index}"
                );
            }
        }
        assert!(changed > 20);
    }
}
#[test]
fn font_face_actor_durable_history_and_failure_guards() {
    exercise(false);
}
#[test]
#[ignore = "requires freshly built native binary and pinned qpdf12.4.2/Poppler26.10"]
fn production_native_font_face_transaction_is_durable_and_source_bound() {
    let current = std::env::current_exe().unwrap();
    let native = current
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-editor-offline-native.exe"
        } else {
            "pdf-editor-offline-native"
        });
    document::with_test_executor(
        native,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
