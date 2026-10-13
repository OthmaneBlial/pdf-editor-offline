#![allow(clippy::unwrap_used)]
use super::*;
const SOURCE: &[u8] = include_bytes!("../../../../tests/fixtures/pdfs/attachment.pdf");
fn load_source(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf) {
    let path = temp.join("source.pdf");
    std::fs::write(&path, SOURCE).unwrap();
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
    Command::Sanitize {
        id: 1,
        new_id: 2,
        revision,
    }
}
#[test]
fn actual_child_copy_is_journaled_unsaved_source_preserved_and_save_reopens() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load_source(temp.path());
    process_with_journal(
        &mut docs,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
        Some(&mut journal),
    )
    .unwrap();
    let original = docs[&1].current.bytes.clone();
    let undo = docs[&1].undo[0].bytes.clone();
    let event = process_with_journal(&mut docs, command(1), Some(&mut journal)).unwrap();
    let Event::Sanitized {
        id, status, report, ..
    } = event
    else {
        panic!("sanitized");
    };
    assert_eq!(id, 2);
    assert!(status.dirty && !status.can_undo);
    assert!(report.full_rewrite && report.embedded_file_name_trees_removed > 0);
    assert_eq!(docs[&1].current.bytes, original);
    assert_eq!(docs[&1].undo.len(), 1);
    assert_eq!(docs[&1].undo[0].bytes, undo);
    assert_eq!(docs[&1].current.revision, 1);
    assert_eq!(docs[&1].saved_revision, Some(0));
    assert!(docs[&2].path.is_none());
    assert!(docs[&2].saved_revision.is_none());
    let retained = journal.recover().unwrap();
    assert_eq!(retained.len(), 2);
    let snapshot = retained.iter().find(|doc| doc.id == "2").unwrap();
    assert_eq!(snapshot.current.bytes, docs[&2].current.bytes);
    assert!(snapshot.saved_revision.is_none());
    let output = temp.path().join("published.pdf");
    process_with_journal(
        &mut docs,
        Command::Save {
            id: 2,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let clean = PdfDocument::open(&output, None).unwrap();
    assert!(clean.inspect_security().unwrap().embedded_files.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), SOURCE);
    assert_eq!(docs[&1].current.bytes, original);
    assert!(!docs[&2].status().unwrap().dirty);
}
#[test]
fn stale_collision_child_and_journal_failures_preserve_original_and_history() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load_source(temp.path());
    let original = docs[&1].current.bytes.clone();
    for command in [
        command(1),
        Command::Sanitize {
            id: 1,
            new_id: 1,
            revision: 0,
        },
        Command::Sanitize {
            id: 1,
            new_id: 0,
            revision: 0,
        },
    ] {
        assert!(process_with_journal(&mut docs, command, Some(&mut journal)).is_err());
    }
    document::with_test_executable(temp.path().join("missing-helper"), || {
        assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    });
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"block publication").unwrap();
    assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, original);
    assert!(docs[&1].undo.is_empty());
    std::fs::remove_file(blocker).unwrap();
    drop(journal);
    let recovered = Journal::open(temp.path().join("journal"))
        .unwrap()
        .recover()
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].current.bytes, original);
}
#[test]
fn pending_save_and_readonly_sources_do_not_create_copies() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load_source(temp.path());
    let original = docs[&1].current.bytes.clone();
    let artifact = PdfDocument::from_bytes(&original, None)
        .unwrap()
        .verified_bytes()
        .unwrap();
    journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &artifact)
        .unwrap();
    assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    assert_eq!(journal.save_intents().len(), 1);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, original);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("signed.pdf");
    std::fs::write(
        &path,
        include_bytes!("../../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    let mut docs = HashMap::new();
    let mut journal = Journal::open(temp.path().join("journal")).unwrap();
    process_with_journal(&mut docs, Command::Load { id: 1, path }, Some(&mut journal)).unwrap();
    assert!(process_with_journal(&mut docs, command(0), Some(&mut journal)).is_err());
    assert_eq!(docs.len(), 1);
}

#[test]
fn full_tab_budget_refuses_before_creating_another_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load_source(temp.path());
    let original = docs[&1].current.bytes.clone();
    let info = docs[&1].current.info.clone();
    // Opaque placeholders exercise the count guard; no placeholder is parsed.
    for id in 2..=MAX_TABS as u64 {
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
            Command::Sanitize {
                id: 1,
                new_id: 20,
                revision: 0
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs.len(), MAX_TABS);
    assert_eq!(docs[&1].current.bytes, original);
    assert_eq!(journal.recover().unwrap().len(), 1);
}

#[test]
#[ignore = "build pdf-worker --features test-helper before this real transport fault gate"]
fn crashed_timed_out_and_truncated_children_cannot_commit_a_copy() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load_source(temp.path());
    let original = docs[&1].current.bytes.clone();
    let current = std::env::current_exe().unwrap();
    let helper = current
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
            )
        });
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[&1].current.bytes, original);
        assert_eq!(docs[&1].current.revision, 0);
        assert!(docs[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, original);
        assert_eq!(std::fs::read(&path).unwrap(), SOURCE);
    }
}

#[test]
#[ignore = "build the normal native executable before this production self-host gate"]
fn normal_native_executable_self_hosts_verified_sanitization() {
    let current = std::env::current_exe().unwrap();
    let native = current
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
    let reply = pdf_worker::execute(
        &native,
        &["--pdf-worker".into()],
        pdf_worker::Request {
            control: serde_json::to_value(Operation::Sanitize).unwrap(),
            payload: SOURCE.to_vec(),
        },
        pdf_worker::Limits::default(),
    )
    .unwrap();
    assert_eq!(reply.kind, PayloadKind::Pdf);
    let DocumentResult::Sanitized { report, .. } = serde_json::from_value(reply.control).unwrap()
    else {
        panic!("sanitized");
    };
    assert!(report.full_rewrite);
    assert!(report.embedded_file_name_trees_removed > 0);
    let clean = PdfDocument::from_bytes(&reply.payload, None).unwrap();
    assert_eq!(
        clean.info().unwrap().pages,
        PdfDocument::from_bytes(SOURCE, None)
            .unwrap()
            .info()
            .unwrap()
            .pages
    );
    assert!(clean.inspect_security().unwrap().embedded_files.is_empty());
}
