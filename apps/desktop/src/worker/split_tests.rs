#![allow(clippy::unwrap_used)]
use super::*;
#[path = "../../../../tests/support/split_fixture.rs"]
mod support;
fn load(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf) {
    let path = temp.join("source.pdf");
    std::fs::write(&path, support::fixture(5)).unwrap();
    let mut journal = Journal::open(temp.join("journal")).unwrap();
    let mut docs = HashMap::new();
    process_with_journal(
        &mut docs,
        Command::Load {
            id: 1,
            path: path.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    (docs, journal, path)
}
fn command(revision: u64) -> Command {
    Command::Split {
        id: 1,
        revision,
        new_ids: vec![2, 3, 4],
        every: 2,
    }
}
#[test]
fn actual_child_batch_preserves_history_is_durable_and_each_copy_saves() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load(temp.path());
    let disk = std::fs::read(&path).unwrap();
    process_with_journal(
        &mut docs,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
        Some(&mut journal),
    )
    .unwrap();
    let before = docs[&1].current.bytes.clone();
    let undo = docs[&1].undo[0].bytes.clone();
    let Event::Split {
        source,
        revision,
        documents,
    } = process_with_journal(&mut docs, command(1), Some(&mut journal)).unwrap()
    else {
        panic!("split");
    };
    assert_eq!((source, revision), (1, 1));
    assert_eq!(
        documents
            .iter()
            .map(|doc| doc.status.info.page_count)
            .collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
    assert!(
        documents
            .iter()
            .all(|doc| doc.status.dirty && !doc.status.can_undo && doc.status.revision == 0)
    );
    assert_eq!(docs[&1].current.bytes, before);
    assert_eq!(docs[&1].undo[0].bytes, undo);
    assert_eq!(docs[&1].saved_revision, Some(0));
    let recovered = journal.recover().unwrap();
    assert_eq!(recovered.len(), 4);
    for (part, id) in [2, 3, 4].into_iter().enumerate() {
        let bytes = &docs[&id].current.bytes;
        let document = PdfDocument::from_bytes(bytes, None).unwrap();
        for local in 0..document.page_count() {
            let page = part * 2 + local;
            assert!(
                document
                    .extract_text(&[local as u32 + 1])
                    .unwrap()
                    .contains(&format!("PAGE {}", page + 1))
            );
            let a = pdf_render::render_page(&before, page, 1.0).unwrap();
            let b = pdf_render::render_page(bytes, local, 1.0).unwrap();
            assert_eq!((a.width, a.height, a.rgba), (b.width, b.height, b.rgba));
        }
        assert_eq!(
            recovered
                .iter()
                .find(|doc| doc.id == id.to_string())
                .unwrap()
                .current
                .bytes,
            *bytes
        );
        let output = temp.path().join(format!("part-{id}.pdf"));
        process_with_journal(
            &mut docs,
            Command::Save {
                id,
                path: output.clone(),
            },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(
            PdfDocument::open(output, None).unwrap().page_count(),
            if part == 2 { 1 } else { 2 }
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), disk);
    assert_eq!(docs[&1].current.bytes, before);
}
#[test]
fn invalid_ids_stale_child_and_manifest_failure_insert_no_partial_batch() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load(temp.path());
    let before = docs[&1].current.bytes.clone();
    for bad in [
        command(1),
        Command::Split {
            id: 1,
            revision: 0,
            new_ids: vec![2, 2, 3],
            every: 2,
        },
        Command::Split {
            id: 1,
            revision: 0,
            new_ids: vec![1, 2, 3],
            every: 2,
        },
        Command::Split {
            id: 1,
            revision: 0,
            new_ids: vec![2],
            every: 0,
        },
    ] {
        assert!(process_with_journal(&mut docs, bad, Some(&mut journal)).is_err());
    }
    document::with_test_executable(temp.path().join("missing-helper"), || {
        assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    });
    assert!(process_with_journal(&mut docs, command(0), None).is_err());
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"blocked").unwrap();
    assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, before);
    std::fs::remove_file(blocker).unwrap();
    drop(journal);
    let recovered = Journal::open(temp.path().join("journal"))
        .unwrap()
        .recover()
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].current.bytes, before);
}
#[test]
fn pending_save_and_capacity_refuse_without_changes() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load(temp.path());
    let before = docs[&1].current.bytes.clone();
    journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &before)
        .unwrap();
    assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    assert_eq!(journal.save_intents().len(), 1);
    let token = journal.save_intents()[0].token.clone();
    journal.cancel_save(&token).unwrap();
    let info = docs[&1].current.info.clone();
    for id in 2..=14 {
        docs.insert(
            id,
            WorkingDocument {
                current: document::Snapshot {
                    bytes: vec![],
                    info: info.clone(),
                    revision: 0,
                },
                undo: vec![],
                redo: vec![],
                saved_revision: None,
                next_revision: 1,
                path: None,
            },
        );
    }
    assert!(
        process_with_journal(
            &mut docs,
            Command::Split {
                id: 1,
                revision: 0,
                new_ids: vec![20, 21, 22],
                every: 2
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs.len(), 14);
    assert_eq!(docs[&1].current.bytes, before);
    assert_eq!(journal.recover().unwrap().len(), 1);
}
#[test]
#[ignore = "requires normal native executable plus pinned qpdf12.4.2 and Poppler26.10"]
fn production_native_split_saved_chunks_pass_independent_text_pixels_geometry_checks() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load(temp.path());
    let before = std::fs::read(&path).unwrap();
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
        || {
            process_with_journal(&mut docs, command(0), Some(&mut journal)).unwrap();
            for id in [2, 3, 4] {
                process_with_journal(
                    &mut docs,
                    Command::Save {
                        id,
                        path: temp.path().join(format!("part-{id}.pdf")),
                    },
                    Some(&mut journal),
                )
                .unwrap();
            }
        },
    );
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let reference = |tool: &str, path: &std::path::Path, page: usize| {
        let mut command = std::process::Command::new(bin.join(tool));
        command.args(["-f", &page.to_string(), "-l", &page.to_string()]);
        if tool == "pdftoppm" {
            command.args(["-r", "72", "-singlefile"]);
        }
        command.arg(path);
        if tool == "pdftotext" {
            command.arg("-");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    };
    let source_info = PdfDocument::open(&path, None).unwrap().info().unwrap();
    for (part, id) in [2, 3, 4].into_iter().enumerate() {
        let output = temp.path().join(format!("part-{id}.pdf"));
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let info = PdfDocument::open(&output, None).unwrap().info().unwrap();
        for local in 0..info.page_count {
            let page = part * 2 + local;
            assert_eq!(
                reference("pdftotext", &path, page + 1),
                reference("pdftotext", &output, local + 1)
            );
            assert_eq!(
                reference("pdftoppm", &path, page + 1),
                reference("pdftoppm", &output, local + 1)
            );
            let mut expected = source_info.pages[page].clone();
            expected.number = local as u32 + 1;
            assert_eq!(expected, info.pages[local]);
        }
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(journal.recover().unwrap().len(), 4);
}
#[test]
fn signed_source_and_fault_children_preserve_only_original_journal() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load(temp.path());
    let before = docs[&1].current.bytes.clone();
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
                process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err(),
                "{mode}"
            );
        });
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[&1].current.bytes, before);
        assert_eq!(journal.recover().unwrap().len(), 1);
    }
    let signed = temp.path().join("signed.pdf");
    std::fs::write(
        &signed,
        include_bytes!("../../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    process_with_journal(
        &mut docs,
        Command::Load {
            id: 5,
            path: signed,
        },
        Some(&mut journal),
    )
    .unwrap();
    let original = docs[&5].current.bytes.clone();
    assert!(
        process_with_journal(
            &mut docs,
            Command::Split {
                id: 5,
                revision: 0,
                new_ids: vec![6],
                every: 100
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[&5].current.bytes, original);
}
