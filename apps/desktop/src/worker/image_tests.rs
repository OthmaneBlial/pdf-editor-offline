#![allow(clippy::unwrap_used)]
use super::*;
#[path = "../../../../tests/support/image_fixture.rs"]
mod image;
use super::test_pdf as support;
fn load(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf, PathBuf) {
    let input = temp.join("source.pdf");
    let image = temp.join("image.png");
    std::fs::write(&input, support::fixture(2)).unwrap();
    std::fs::write(&image, TEST_RED_PNG).unwrap();
    let mut journal = Journal::open(temp.join("journal")).unwrap();
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
    (documents, journal, input, image)
}
fn edit(revision: u64, image_file: &std::path::Path) -> Command {
    Command::Edit {
        id: 1,
        edit: Edit::ImageOverlay {
            key: FormsReadKey {
                tab: 1,
                revision,
                generation: 1,
            },
            overlay: pdf_core::ImageOverlay {
                page: 1,
                rectangle: [60.0, 30.0, 100.0, 70.0],
                rotation: 270,
            },
            image_file: image_file.to_path_buf(),
        },
    }
}
fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let (mut documents, mut journal, input, image) = load(temp.path());
    let original = documents[&1].current.bytes.clone();
    let disk = std::fs::read(&input).unwrap();
    let image_bytes = std::fs::read(&image).unwrap();
    let Event::Updated {
        status,
        selected_page,
        ..
    } = process_with_journal(&mut documents, edit(0, &image), Some(&mut journal)).unwrap()
    else {
        panic!("updated")
    };
    assert!(status.dirty && status.can_undo);
    assert_eq!(selected_page, Some(0));
    let inserted = documents[&1].current.bytes.clone();
    assert_eq!(journal.recover().unwrap()[0].current.bytes, inserted);
    let before = pdf_render::render_page(&original, 0, 1.0).unwrap();
    let after = pdf_render::render_page(&inserted, 0, 1.0).unwrap();
    let center = (250 * after.width as usize + 80) * 4;
    assert_eq!(&after.rgba[center..center + 4], &[255, 0, 0, 255]);
    assert_eq!(
        documents[&1].current.info.pages,
        PdfDocument::from_bytes(&original, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        let x = index % 200;
        let y = index / 200;
        if !(59..=101).contains(&x) || !(229..=271).contains(&y) {
            assert_eq!(a, b);
        }
    }
    process_with_journal(
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Undo,
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(documents[&1].current.bytes, original);
    process_with_journal(
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Redo,
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(documents[&1].current.bytes, inserted);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, inserted);
    let output = temp.path().join("inserted.pdf");
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
    let published = std::fs::read(&output).unwrap();
    let reopened = PdfDocument::open(&output, None).unwrap();
    assert_eq!(
        reopened.info().unwrap().pages,
        documents[&1].current.info.pages
    );
    assert_eq!(
        reopened.extract_text(&[1, 2]).unwrap(),
        PdfDocument::from_bytes(&original, None)
            .unwrap()
            .extract_text(&[1, 2])
            .unwrap()
    );
    assert_eq!(
        pdf_render::render_page(&published, 0, 1.0).unwrap().rgba,
        after.rgba
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
    assert_eq!(std::fs::read(&output).unwrap(), published);
    assert_eq!(std::fs::read(&input).unwrap(), disk);
    assert_eq!(std::fs::read(&image).unwrap(), image_bytes);
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
        let reference = |tool: &str, path: &std::path::Path| {
            let mut command = std::process::Command::new(bin.join(tool));
            command.args(["-f", "1", "-l", "1"]);
            if tool == "pdftoppm" {
                command.args(["-r", "72", "-singlefile"]);
            }
            command.arg(path);
            if tool == "pdftotext" {
                command.arg("-");
            }
            let result = command.output().unwrap();
            assert!(result.status.success());
            result.stdout
        };
        assert_eq!(
            reference("pdftotext", &input),
            reference("pdftotext", &output)
        );
        let ppm = |bytes: Vec<u8>| {
            let marker = b"\n255\n";
            let start = bytes
                .windows(marker.len())
                .position(|part| part == marker)
                .unwrap()
                + marker.len();
            (bytes[..start].to_vec(), bytes[start..].to_vec())
        };
        let (header, a) = ppm(reference("pdftoppm", &input));
        let (after_header, b) = ppm(reference("pdftoppm", &output));
        assert_eq!(header, after_header);
        let mut changed = 0;
        for (index, (a, b)) in a.chunks_exact(3).zip(b.chunks_exact(3)).enumerate() {
            let x = index % 200;
            let y = index / 200;
            if (59..=101).contains(&x) && (229..=271).contains(&y) {
                changed += usize::from(a != b);
            } else {
                assert_eq!(a, b);
            }
        }
        assert!(changed > 1000);
    }
}
#[test]
fn actual_image_child_history_journal_save_and_source_preservation() {
    exercise(false);
}
#[test]
fn stale_tab_bad_file_pending_and_failed_journal_leave_original_and_history() {
    let temp = tempfile::tempdir().unwrap();
    let (mut documents, mut journal, _, image) = load(temp.path());
    let before = documents[&1].current.bytes.clone();
    assert!(process_with_journal(&mut documents, edit(1, &image), Some(&mut journal)).is_err());
    let mut foreign = edit(0, &image);
    if let Command::Edit {
        edit: Edit::ImageOverlay { key, .. },
        ..
    } = &mut foreign
    {
        key.tab = 2;
    }
    assert!(process_with_journal(&mut documents, foreign, Some(&mut journal)).is_err());
    for contents in [b"GIF89a".as_slice(), b"\xff\xd8\xff\xd9"] {
        std::fs::write(&image, contents).unwrap();
        assert!(process_with_journal(&mut documents, edit(0, &image), Some(&mut journal)).is_err());
    }
    std::fs::write(&image, TEST_RED_PNG).unwrap();
    journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &before)
        .unwrap();
    assert!(process_with_journal(&mut documents, edit(0, &image), Some(&mut journal)).is_err());
    let token = journal.save_intents()[0].token.clone();
    journal.cancel_save(&token).unwrap();
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"blocked").unwrap();
    assert!(process_with_journal(&mut documents, edit(0, &image), Some(&mut journal)).is_err());
    assert_eq!(documents[&1].current.bytes, before);
    assert!(documents[&1].undo.is_empty());
    std::fs::remove_file(blocker).unwrap();
    drop(journal);
    let recovered = Journal::open(temp.path().join("journal"))
        .unwrap()
        .recover()
        .unwrap();
    assert_eq!(recovered[0].current.bytes, before);
}
#[test]
fn jpeg_file_and_byte_limit_are_checked_before_parent_state_commits() {
    let temp = tempfile::tempdir().unwrap();
    let (mut documents, mut journal, _, path) = load(temp.path());
    std::fs::write(&path, image::jpeg()).unwrap();
    process_with_journal(&mut documents, edit(0, &path), Some(&mut journal)).unwrap();
    let before = documents[&1].current.bytes.clone();
    std::fs::File::create(&path)
        .unwrap()
        .set_len(pdf_core::MAX_STREAM_BYTES as u64 + 1)
        .unwrap();
    assert!(process_with_journal(&mut documents, edit(1, &path), Some(&mut journal)).is_err());
    assert_eq!(documents[&1].current.bytes, before);
    assert_eq!(documents[&1].undo.len(), 1);
}
#[test]
#[ignore = "requires normal native executable plus pinned qpdf12.4.2/Poppler26.10"]
fn production_native_image_worker_save_history_and_independent_pixels() {
    let native = std::env::current_exe()
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
    assert!(native.is_file());
    document::with_test_executor(
        native,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
#[test]
fn signed_and_fault_children_never_commit_image_history_or_recovery_changes() {
    let temp = tempfile::tempdir().unwrap();
    let (mut documents, mut journal, _, image) = load(temp.path());
    let before = documents[&1].current.bytes.clone();
    let helper = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-worker-test-helper.exe"
        } else {
            "pdf-worker-test-helper"
        });
    assert!(helper.is_file());
    let limits = pdf_worker::Limits::for_test(
        pdf_worker::MEMORY_BYTES,
        30,
        std::time::Duration::from_millis(500),
    )
    .unwrap();
    for mode in [
        "abort",
        "timeout",
        "truncated",
        "checksum",
        "oversize",
        "malformed",
    ] {
        document::with_test_executor(helper.clone(), vec![mode.into()], limits, || {
            assert!(
                process_with_journal(&mut documents, edit(0, &image), Some(&mut journal)).is_err(),
                "{mode}"
            );
        });
        assert_eq!(documents[&1].current.bytes, before);
        assert!(documents[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, before);
    }
    let signed = temp.path().join("signed.pdf");
    std::fs::write(
        &signed,
        include_bytes!("../../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    process_with_journal(
        &mut documents,
        Command::Load {
            id: 2,
            path: signed,
        },
        Some(&mut journal),
    )
    .unwrap();
    let mut command = edit(0, &image);
    if let Command::Edit {
        id,
        edit: Edit::ImageOverlay { key, .. },
    } = &mut command
    {
        *id = 2;
        key.tab = 2;
    }
    let signed_before = documents[&2].current.bytes.clone();
    assert!(process_with_journal(&mut documents, command, Some(&mut journal)).is_err());
    assert_eq!(documents[&2].current.bytes, signed_before);
    assert!(documents[&2].undo.is_empty());
}
