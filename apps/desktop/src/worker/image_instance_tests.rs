#![allow(clippy::unwrap_used)]
use super::*;
fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let image = temp.path().join("replacement.png");
    let source = test_pdf::fixture(2);
    std::fs::write(&input, &source).unwrap();
    std::fs::write(&image, TEST_BLUE_PNG).unwrap();
    let mut journal = Journal::open(temp.path().join("journal")).unwrap();
    let mut docs = HashMap::new();
    process_with_journal(
        &mut docs,
        Command::Load {
            id: 1,
            path: input.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let old = docs[&1].current.bytes.clone();
    let key = FormsReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    let Event::ImageInstances { instances, .. } = process_with_journal(
        &mut docs,
        Command::ImageInstances { key, page: 1 },
        Some(&mut journal),
    )
    .unwrap() else {
        panic!("inventory")
    };
    assert_eq!(instances.len(), 1);
    let edit = |key, selection, image_file| Command::Edit {
        id: 1,
        edit: Edit::ReplaceImage {
            key,
            selection,
            image_file,
        },
    };
    for bad_key in [
        FormsReadKey { tab: 2, ..key },
        FormsReadKey { revision: 1, ..key },
    ] {
        assert!(
            process_with_journal(
                &mut docs,
                edit(bad_key, instances[0].selection.clone(), image.clone()),
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(docs[&1].current.bytes, old);
    }
    let mut stale = instances[0].selection.clone();
    stale.source_hash = "0".repeat(64);
    assert!(
        process_with_journal(
            &mut docs,
            edit(key, stale, image.clone()),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs[&1].current.bytes, old);
    let unsupported = temp.path().join("unsupported.gif");
    std::fs::write(&unsupported, b"GIF89a").unwrap();
    assert!(
        process_with_journal(
            &mut docs,
            edit(key, instances[0].selection.clone(), unsupported),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs[&1].current.bytes, old);
    let Event::Updated {
        status,
        selected_page,
        ..
    } = process_with_journal(
        &mut docs,
        edit(key, instances[0].selection.clone(), image.clone()),
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("update")
    };
    assert!(status.dirty && status.can_undo);
    assert_eq!(selected_page, Some(0));
    assert_eq!(docs[&1].undo.len(), 1);
    let replaced = docs[&1].current.bytes.clone();
    assert_eq!(journal.recover().unwrap()[0].current.bytes, replaced);
    let raster = |bytes: &[u8], page| pdf_render::render_page(bytes, page, 1.0).unwrap().rgba;
    assert_eq!(raster(&old, 1), raster(&replaced, 1));
    let before = raster(&old, 0);
    let after = raster(&replaced, 0);
    let center = (265 * 200 + 45) * 4;
    assert_eq!(&after[center..center + 4], &[0, 0, 255, 255]);
    for (i, (a, b)) in before
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            assert!(
                (30..60).contains(&(i % 200)) && (250..280).contains(&(i / 200)),
                "outside expected image raster footprint at ({}, {}) before={a:?} after={b:?}",
                i % 200,
                i / 200
            );
        }
    }
    for (edit, bytes) in [(Edit::Undo, old.clone()), (Edit::Redo, replaced.clone())] {
        process_with_journal(&mut docs, Command::Edit { id: 1, edit }, Some(&mut journal)).unwrap();
        assert_eq!(docs[&1].current.bytes, bytes);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, bytes);
    }
    let output = temp.path().join("replaced.pdf");
    process_with_journal(
        &mut docs,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert!(!docs[&1].status().unwrap().dirty);
    let saved = std::fs::read(&output).unwrap();
    let original = PdfDocument::from_bytes(&old, None).unwrap();
    let final_pdf = PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(
        original.info().unwrap().pages,
        final_pdf.info().unwrap().pages
    );
    assert_eq!(
        original.extract_text(&[1, 2]).unwrap(),
        final_pdf.extract_text(&[1, 2]).unwrap()
    );
    assert_eq!(raster(&saved, 0), after);
    assert!(
        process_with_journal(
            &mut docs,
            Command::Save {
                id: 1,
                path: output.clone()
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), source);
    assert_eq!(std::fs::read(&image).unwrap(), TEST_BLUE_PNG);
    if reference {
        assert!(
            std::process::Command::new(std::env::var_os("PDF_QPDF_BIN").unwrap())
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
        let text = |path: &std::path::Path| {
            let r = std::process::Command::new(bin.join("pdftotext"))
                .arg("-raw")
                .arg(path)
                .arg("-")
                .output()
                .unwrap();
            assert!(r.status.success());
            r.stdout
        };
        assert_eq!(text(&input), text(&output));
        let raster = |path: &std::path::Path, page: &str| {
            let r = std::process::Command::new(bin.join("pdftoppm"))
                .args(["-f", page, "-r", "72", "-singlefile"])
                .arg(path)
                .output()
                .unwrap();
            assert!(r.status.success());
            r.stdout
        };
        assert_eq!(raster(&input, "2"), raster(&output, "2"));
        let control_path = temp.path().join("control-no-selected-image.pdf");
        let mut control_doc = lopdf::Document::load_mem(&source).unwrap();
        let raw = control_doc
            .get_object((5, 0))
            .unwrap()
            .as_stream()
            .unwrap()
            .content
            .clone();
        let raw = std::str::from_utf8(&raw).unwrap().replace("/Im1 Do", "");
        control_doc
            .get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(raw.into_bytes());
        let mut control_bytes = vec![];
        control_doc.save_to(&mut control_bytes).unwrap();
        std::fs::write(&control_path, control_bytes).unwrap();
        let before = raster(&input, "1");
        let after = raster(&output, "1");
        let control = raster(&control_path, "1");
        let control = control.strip_prefix(b"P6\n200 300\n255\n").unwrap();
        let before = before.strip_prefix(b"P6\n200 300\n255\n").unwrap();
        let after = after.strip_prefix(b"P6\n200 300\n255\n").unwrap();
        let mut changed = 0;
        for (i, ((a, b), unpainted)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .zip(control.chunks_exact(3))
            .enumerate()
        {
            if a == unpainted {
                assert_eq!(
                    a,
                    b,
                    "changed outside independently observed original-image footprint at ({},{})",
                    i % 200,
                    i / 200
                );
            } else {
                assert_eq!(a, [200, 40, 80]);
                assert_eq!(b, [0, 0, 255]);
                changed += 1;
            }
        }
        assert!(changed > 500);
    }
}
#[test]
fn real_image_replacement_child_inventory_stale_failure_history_journal_and_save() {
    exercise(false);
}
#[test]
#[ignore = "requires freshly built native binary, pinned qpdf and Poppler"]
fn production_native_image_replacement_child_and_independent_readers() {
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("pdf-editor-offline-native");
    document::with_test_executor(
        binary,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
