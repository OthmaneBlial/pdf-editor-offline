#![allow(clippy::unwrap_used)]

use pdf_core::{DocumentInfo, PageInfo};
use pdf_worker::journal::{DocumentState, Journal, JournalError, Snapshot};
use serde_json::Value;
use std::path::Path;

fn snapshot(revision: u64, bytes: &[u8]) -> Snapshot {
    Snapshot {
        revision,
        bytes: bytes.to_vec(),
        info: DocumentInfo {
            version: "1.7".into(),
            page_count: 1,
            object_count: 4,
            encrypted: false,
            signature_fields: 0,
            pages: vec![PageInfo {
                number: 1,
                media_box: [0.0, 0.0, 200.0, 300.0],
                crop_box: [0.0, 0.0, 200.0, 300.0],
                rotation: 0,
            }],
        },
    }
}
fn state(id: &str, revision: u64, bytes: &[u8]) -> DocumentState {
    DocumentState {
        id: id.into(),
        current: snapshot(revision, bytes),
        undo: Vec::new(),
        redo: Vec::new(),
        saved_revision: None,
        next_revision: revision + 1,
        path: None,
    }
}
fn manifest(directory: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap()
}
fn rewrite(directory: &Path, value: &Value) {
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec(value).unwrap(),
    )
    .unwrap();
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options.open(path).unwrap().write_all(bytes).unwrap();
}

#[test]
fn session_discovery_locks_revision_zero_and_explicit_discard_are_bounded() {
    use pdf_worker::journal::{SessionState, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("app/recovery/sessions")).unwrap();
    let mut journal = store.create().unwrap();
    let mut initial = state("tab0", 0, b"opaque revision zero");
    initial.saved_revision = Some(0);
    journal.commit_document(&initial).unwrap();
    assert_eq!(journal.summary().bytes, initial.current.bytes.len());
    let candidates = store.discover().unwrap();
    assert_eq!(candidates.len(), 1);
    assert!(matches!(candidates[0].state, SessionState::Active));
    let session_path = candidates[0].path.clone();
    drop(journal);
    assert!(matches!(
        store.discover().unwrap()[0].state,
        SessionState::Available(_)
    ));
    let reopened = store.open_session(&session_path).unwrap();
    assert_eq!(reopened.recover().unwrap()[0].saved_revision, Some(0));
    reopened.discard().unwrap();
    assert!(!session_path.exists());
    assert!(store.discover().unwrap().is_empty());
    assert!(
        store
            .open_session(directory.path().join("unrelated"))
            .is_err()
    );
}

#[test]
fn discard_refuses_unknown_files_before_removal_and_never_touches_save_destinations() {
    use pdf_worker::journal::SessionStore;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut journal = store.create().unwrap();
    journal
        .commit_document(&state("tab", 0, b"opaque original"))
        .unwrap();
    let destination = directory.path().join("saved.pdf");
    journal
        .begin_save("tab", 0, &destination, b"published bytes")
        .unwrap();
    std::fs::write(&destination, b"published bytes").unwrap();
    let session = store.discover().unwrap()[0].path.clone();
    std::fs::write(session.join("unknown.txt"), b"not app-owned").unwrap();
    let before = std::fs::read(session.join("manifest.json")).unwrap();
    assert!(journal.discard().is_err());
    assert_eq!(
        std::fs::read(session.join("manifest.json")).unwrap(),
        before
    );
    assert!(session.join("session.lock").exists());
    std::fs::remove_file(session.join("unknown.txt")).unwrap();
    store.open_session(&session).unwrap().discard().unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"published bytes");
}

#[test]
fn batch_import_preserves_existing_tabs_intents_and_migrates_uncertain_artifacts() {
    use pdf_worker::journal::{SaveIntentRef, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut destination = store.create().unwrap();
    destination
        .commit_document(&state("existing", 0, b"existing bytes"))
        .unwrap();
    let existing_intent = destination
        .begin_save(
            "existing",
            0,
            &directory.path().join("existing.pdf"),
            b"existing save",
        )
        .unwrap();
    let mut source = store.create().unwrap();
    source
        .commit_document(&state("incoming", 0, b"incoming bytes"))
        .unwrap();
    let intent = source
        .begin_save(
            "incoming",
            0,
            &directory.path().join("incoming.pdf"),
            b"incoming save",
        )
        .unwrap();
    let artifact = source.save_artifact(&intent.token).unwrap();
    let docs = source.recover().unwrap();
    let before = destination.summary();
    assert!(
        destination
            .commit_documents_with_intents_ref(
                &[docs[0].as_ref()],
                &[SaveIntentRef {
                    intent: &intent,
                    artifact: b"mismatch"
                }]
            )
            .is_err()
    );
    assert_eq!(destination.summary(), before);
    destination
        .commit_documents_with_intents_ref(
            &[docs[0].as_ref()],
            &[SaveIntentRef {
                intent: &intent,
                artifact: &artifact,
            }],
        )
        .unwrap();
    assert_eq!(destination.summary().document_count, 2);
    assert_eq!(
        destination.save_intents(),
        &[existing_intent, intent.clone()]
    );
    assert_eq!(destination.save_artifact(&intent.token).unwrap(), artifact);
    assert_eq!(
        source.recover().unwrap()[0].current.bytes,
        b"incoming bytes"
    );
    destination.cancel_save(&intent.token).unwrap();
    assert_eq!(destination.recover().unwrap()[1].saved_revision, None);
    assert_eq!(destination.save_intents().len(), 1);
}

#[test]
fn failed_later_blob_keeps_entire_old_manifest_and_existing_documents() {
    use pdf_worker::journal::SessionStore;
    use sha2::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut journal = store.create().unwrap();
    journal
        .commit_document(&state("existing", 0, b"existing"))
        .unwrap();
    let path = store.discover().unwrap()[0].path.clone();
    let before = std::fs::read(path.join("manifest.json")).unwrap();
    let one = state("first", 0, b"first incoming");
    let two = state("second", 0, b"second incoming");
    let hash: String = Sha256::digest(&two.current.bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    std::fs::create_dir(path.join(format!("{hash}.blob"))).unwrap();
    assert!(
        journal
            .commit_documents_ref(&[one.as_ref(), two.as_ref()])
            .is_err()
    );
    assert_eq!(std::fs::read(path.join("manifest.json")).unwrap(), before);
    assert_eq!(journal.recover().unwrap().len(), 1);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, b"existing");
}

#[test]
fn journal_crash_child() {
    let Some(path) = std::env::var_os("PDF_JOURNAL_CRASH_TEST_DIRECTORY") else {
        return;
    };
    let mut journal = Journal::open(&path).unwrap();
    journal
        .commit_document(&state("crashed", 0, b"durable before abort"))
        .unwrap();
    let destination = Path::new(&path)
        .parent()
        .unwrap()
        .join("published-before-abort.pdf");
    journal
        .begin_save("crashed", 0, &destination, b"published before finish")
        .unwrap();
    std::fs::write(destination, b"published before finish").unwrap();
    std::process::abort();
}

#[test]
fn actual_process_abort_releases_session_lock_and_preserves_durable_state() {
    use pdf_worker::journal::{SessionState, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let session = store.create().unwrap();
    let path = store.discover().unwrap()[0].path.clone();
    drop(session);
    let outcome = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "journal_crash_child", "--nocapture"])
        .env("PDF_JOURNAL_CRASH_TEST_DIRECTORY", &path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!outcome.success());
    assert!(matches!(
        store.discover().unwrap()[0].state,
        SessionState::Available(_)
    ));
    let mut reopened = store.open_session(path).unwrap();
    assert_eq!(
        reopened.recover().unwrap()[0].current.bytes,
        b"durable before abort"
    );
    assert_eq!(reopened.recover().unwrap()[0].saved_revision, None);
    assert_eq!(reopened.save_intents().len(), 1);
    let intent = reopened.save_intents()[0].clone();
    assert_eq!(
        reopened.save_artifact(&intent.token).unwrap(),
        b"published before finish"
    );
    reopened.finish_save(&intent.token).unwrap();
    assert_eq!(reopened.recover().unwrap()[0].saved_revision, Some(0));
    reopened.discard().unwrap();
    assert_eq!(
        std::fs::read(intent.destination).unwrap(),
        b"published before finish"
    );
}

#[test]
fn closed_backups_and_corrupt_sessions_remain_visible_until_explicit_discard() {
    use pdf_worker::journal::{SessionState, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut journal = store.create().unwrap();
    journal
        .commit_document(&state("tab", 0, b"retained private PDF copy"))
        .unwrap();
    journal.close_document("tab").unwrap();
    assert_eq!(journal.summary().document_count, 0);
    assert!(journal.owned_disk_bytes().unwrap() > 24);
    let path = journal.directory().to_owned();
    drop(journal);
    let candidate = store.discover().unwrap().remove(0);
    assert!(matches!(candidate.state, SessionState::Available(_)));
    assert!(candidate.disk_bytes.unwrap() > 24);
    assert!(candidate.modified.is_some());
    std::fs::write(path.join("manifest.json"), b"corrupt JSON").unwrap();
    let candidate = store.discover().unwrap().remove(0);
    assert!(matches!(candidate.state, SessionState::Invalid(_)));
    assert!(candidate.disk_bytes.is_some());
    assert!(store.open_session(&path).is_err());
    store.discard_session(&path).unwrap();
    assert!(!path.exists());
}

#[test]
fn native_non_unicode_source_and_save_paths_survive_reopen_and_publication_ack() {
    use pdf_worker::journal::SessionStore;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(b"native-\xff.pdf".to_vec())
    };
    #[cfg(windows)]
    let name = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[110, 97, 116, 105, 118, 101, 45, 0xd800, 46, 112, 100, 102])
    };
    let destination = directory.path().join(name);
    let mut document = state("native", 0, b"native document");
    document.path = Some(destination.clone());
    let mut journal = store.create().unwrap();
    journal.commit_document(&document).unwrap();
    let intent = journal
        .begin_save("native", 0, &destination, b"native artifact")
        .unwrap();
    let path = journal.directory().to_owned();
    drop(journal);
    let mut reopened = store.open_session(&path).unwrap();
    assert_eq!(
        reopened.recover().unwrap()[0].path.as_ref(),
        Some(&destination)
    );
    assert_eq!(reopened.save_intents()[0].destination, destination);
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::write(&destination, b"native artifact").unwrap();
        reopened.finish_save(&intent.token).unwrap();
        assert_eq!(reopened.recover().unwrap()[0].saved_revision, Some(0));
    }
    #[cfg(target_os = "macos")]
    {
        // APFS rejects this byte sequence; the journal still retains the native path exactly.
        assert!(std::fs::write(&destination, b"native artifact").is_err());
        assert!(reopened.finish_save(&intent.token).is_err());
        assert_eq!(reopened.recover().unwrap()[0].saved_revision, None);
    }
    reopened.discard().unwrap();
    #[cfg(not(target_os = "macos"))]
    assert_eq!(std::fs::read(destination).unwrap(), b"native artifact");
}

#[cfg(unix)]
#[test]
fn descriptor_io_survives_directory_rename_and_discard_refuses_replacement() {
    use pdf_worker::journal::SessionStore;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut journal = store.create().unwrap();
    let path = journal.directory().to_owned();
    let renamed = path.with_file_name("retained-original");
    std::fs::rename(&path, &renamed).unwrap();
    std::fs::create_dir(&path).unwrap();
    journal
        .commit_document(&state("tab", 0, b"anchored bytes"))
        .unwrap();
    assert_eq!(
        journal.recover().unwrap()[0].current.bytes,
        b"anchored bytes"
    );
    assert!(renamed.join("manifest.json").exists());
    assert!(!path.join("manifest.json").exists());
    assert!(journal.discard().is_err());
    assert!(renamed.join("manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn discard_rejects_symlink_and_hardlink_before_any_file_is_removed() {
    use pdf_worker::journal::SessionStore;
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let journal = store.create().unwrap();
    let path = journal.directory().to_owned();
    let outside = directory.path().join("outside");
    write_private(&outside, b"outside private bytes");
    let candidate = path.join(format!("{}.blob", "a".repeat(64)));
    symlink(&outside, &candidate).unwrap();
    assert!(journal.discard().is_err());
    assert!(path.join("session.lock").exists());
    std::fs::remove_file(&candidate).unwrap();
    std::fs::hard_link(&outside, &candidate).unwrap();
    assert!(store.discard_session(&path).is_err());
    assert!(path.join("session.lock").exists());
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside private bytes");
}

#[test]
fn discovery_budget_refuses_excess_entries_without_deleting_sessions() {
    use pdf_worker::journal::SessionStore;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("sessions");
    let store = SessionStore::open(&root).unwrap();
    for number in 0..65u64 {
        let path = root.join(format!("session-{number:032x}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        #[cfg(windows)]
        {
            std::fs::create_dir(path).unwrap();
        }
    }
    assert!(matches!(store.discover(), Err(JournalError::Limit(_))));
    assert!(matches!(store.create(), Err(JournalError::Limit(_))));
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 65);
}

#[test]
fn concurrent_instances_create_private_parents_and_keep_distinct_active_locks() {
    use pdf_worker::journal::{SessionState, SessionStore};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("app/state/sessions");
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || SessionStore::open(root).unwrap().create().unwrap())
        })
        .collect();
    let journals: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    let store = SessionStore::open(root).unwrap();
    let candidates = store.discover().unwrap();
    assert_eq!(candidates.len(), 4);
    assert!(
        candidates
            .iter()
            .all(|candidate| matches!(candidate.state, SessionState::Active))
    );
    drop(journals);
    for candidate in store.discover().unwrap() {
        store.discard_session(candidate.path).unwrap();
    }
    assert!(store.discover().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn windows_private_acl_cleanup_and_hardlink_refusal_run_on_native_ci() {
    use pdf_worker::journal::SessionStore;
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::open(directory.path().join("sessions")).unwrap();
    let mut journal = store.create().unwrap();
    journal
        .commit_document(&state("tab", 0, b"private Windows copy"))
        .unwrap();
    let path = journal.directory().to_owned();
    let copy = directory.path().join("hardlinked-private-copy");
    std::fs::hard_link(path.join("manifest.json"), &copy).unwrap();
    assert!(journal.discard().is_err());
    assert!(path.join("session.lock").exists());
    std::fs::remove_file(&copy).unwrap();
    store.discard_session(&path).unwrap();
    assert!(!path.exists());
    // The parent temp directory's inherited ACL is deliberately not the app's protected DACL.
    assert!(SessionStore::open(directory.path()).is_err());
}

#[test]
fn durable_opaque_snapshots_history_and_saved_revision_recover_after_unlock() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    journal
        .commit_document(&state(
            "tab1",
            1,
            b"opaque initial bytes, not parsed as PDF",
        ))
        .unwrap();
    let mut edited = state("tab1", 2, b"opaque edited bytes");
    edited
        .undo
        .push(snapshot(1, b"opaque initial bytes, not parsed as PDF"));
    journal.commit_document(&edited).unwrap();
    assert!(matches!(Journal::open(&session), Err(JournalError::Locked)));
    drop(journal);
    let mut reopened = Journal::open(&session).unwrap();
    let restored = reopened.recover().unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].current.bytes, edited.current.bytes);
    assert_eq!(restored[0].undo[0].bytes, edited.undo[0].bytes);
    assert_eq!(restored[0].next_revision, 3);
    let destination = dir.path().join("saved.pdf");
    let intent = reopened
        .begin_save("tab1", 2, &destination, &edited.current.bytes)
        .unwrap();
    drop(reopened);
    let mut reopened = Journal::open(&session).unwrap();
    assert_eq!(reopened.save_intents(), std::slice::from_ref(&intent));
    assert!(reopened.finish_save(&intent.token).is_err());
    std::fs::write(&destination, b"wrong existing destination").unwrap();
    assert!(reopened.finish_save(&intent.token).is_err());
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"wrong existing destination"
    );
    std::fs::write(&destination, &edited.current.bytes).unwrap();
    reopened.finish_save(&intent.token).unwrap();
    assert!(reopened.save_intents().is_empty());
    drop(reopened);
    let recovered = Journal::open(&session).unwrap().recover().unwrap();
    assert_eq!(recovered[0].saved_revision, Some(2));
    assert_eq!(recovered[0].path.as_deref(), Some(destination.as_path()));
}

#[test]
fn corruption_and_path_traversal_are_visible_and_never_fall_back_silently() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    journal
        .commit_document(&state("tab", 1, b"first bytes"))
        .unwrap();
    journal
        .commit_document(&state("tab", 2, b"second bytes"))
        .unwrap();
    drop(journal);
    let original = std::fs::read(session.join("manifest.json")).unwrap();
    let mut value = manifest(&session);
    value["documents"][0]["current"]["hash"] = "../escape".into();
    rewrite(&session, &value);
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Invalid(_))
    ));
    std::fs::write(session.join("manifest.json"), b"{bad JSON}").unwrap();
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Json(_))
    ));
    std::fs::write(session.join("manifest.json"), &original).unwrap();
    let hash = manifest(&session)["documents"][0]["current"]["hash"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(session.join(format!("{hash}.blob")), b"tampered").unwrap();
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Invalid(_))
    ));
}

#[test]
fn missing_current_recovers_previous_manifest_and_cleans_only_unreferenced_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    journal.commit_document(&state("tab", 1, b"first")).unwrap();
    journal
        .commit_document(&state("tab", 2, b"second"))
        .unwrap();
    drop(journal);
    std::fs::remove_file(session.join("manifest.json")).unwrap();
    let orphan = session.join(format!("{}.blob", "a".repeat(64)));
    write_private(&orphan, b"orphan");
    let unrelated = session.join("user-file.txt");
    std::fs::write(&unrelated, b"keep").unwrap();
    let recovered = Journal::open(&session).unwrap();
    assert_eq!(recovered.recover().unwrap()[0].current.bytes, b"first");
    assert!(!orphan.exists());
    assert_eq!(std::fs::read(unrelated).unwrap(), b"keep");
}

#[test]
fn invalid_revision_history_inventory_and_large_declared_lengths_fail_without_commit() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    let valid = state("tab", 1, b"original");
    journal.commit_document(&valid).unwrap();
    let original = std::fs::read(session.join("manifest.json")).unwrap();
    let mut invalid = state("tab", 2, b"candidate");
    invalid.undo = (3..12)
        .map(|revision| snapshot(revision, b"history"))
        .collect();
    invalid.next_revision = 12;
    assert!(matches!(
        journal.commit_document(&invalid),
        Err(JournalError::Limit(_))
    ));
    invalid.undo.clear();
    invalid.next_revision = 2;
    assert!(matches!(
        journal.commit_document(&invalid),
        Err(JournalError::Invalid(_))
    ));
    invalid.id = "../escape".into();
    assert!(journal.commit_document(&invalid).is_err());
    assert_eq!(
        std::fs::read(session.join("manifest.json")).unwrap(),
        original
    );
    assert!(
        journal
            .begin_save("tab", 2, Path::new("output.pdf"), b"wrong revision")
            .is_err()
    );
    drop(journal);
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value["documents"][0]["current"]["length"] = (pdf_worker::MAX_PDF_BYTES + 1).into();
    rewrite(&session, &value);
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Limit(_))
    ));
    value = serde_json::from_slice(&original).unwrap();
    let duplicate = value["documents"][0].clone();
    value["documents"].as_array_mut().unwrap().push(duplicate);
    rewrite(&session, &value);
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Invalid(_))
    ));
}

#[test]
fn failed_manifest_publication_keeps_parent_state_and_requires_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    journal
        .commit_document(&state("tab", 1, b"original"))
        .unwrap();
    write_private(&session.join("manifest.json.tmp"), b"interrupted temporary");
    assert!(
        journal
            .commit_document(&state("tab", 2, b"candidate"))
            .is_err()
    );
    assert_eq!(journal.recover().unwrap()[0].current.bytes, b"original");
    assert!(journal.commit_document(&state("tab", 3, b"later")).is_err());
    drop(journal);
    let reopened = Journal::open(&session).unwrap();
    assert_eq!(reopened.recover().unwrap()[0].current.bytes, b"original");
    assert!(!session.join("manifest.json.tmp").exists());
}

#[cfg(unix)]
#[test]
fn private_permissions_and_symlink_refusal() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    journal
        .commit_document(&state("tab", 1, b"private bytes"))
        .unwrap();
    assert_eq!(
        session.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    for entry in std::fs::read_dir(&session).unwrap() {
        assert_eq!(
            entry.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(journal);
    let hash = manifest(&session)["documents"][0]["current"]["hash"]
        .as_str()
        .unwrap()
        .to_owned();
    let blob = session.join(format!("{hash}.blob"));
    std::fs::remove_file(&blob).unwrap();
    let external = dir.path().join("external");
    std::fs::write(&external, b"private bytes").unwrap();
    symlink(&external, &blob).unwrap();
    assert!(Journal::open(&session).is_err());
    assert_eq!(std::fs::read(&external).unwrap(), b"private bytes");
}

#[test]
fn save_intent_keeps_distinct_verified_artifact_bound_to_source_revision() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let output = dir.path().join("normalized.pdf");
    let mut journal = Journal::open(&session).unwrap();
    journal
        .commit_document_ref(&state("tab", 1, b"original serialized snapshot").as_ref())
        .unwrap();
    let artifact = b"verified serialization with normalized xref";
    let intent = journal.begin_save("tab", 1, &output, artifact).unwrap();
    assert_ne!(intent.source_hash, intent.artifact_hash);
    assert!(
        session
            .join(format!("{}.blob", intent.artifact_hash))
            .exists()
    );
    drop(journal);
    let mut journal = Journal::open(&session).unwrap();
    assert_eq!(
        journal.recover().unwrap()[0].current.bytes,
        b"original serialized snapshot"
    );
    std::fs::write(&output, artifact).unwrap();
    journal.finish_save(&intent.token).unwrap();
    assert_eq!(journal.recover().unwrap()[0].saved_revision, Some(1));
}

#[test]
fn inventory_history_allocation_and_manifest_size_limits_reject_before_blob_reads() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session");
    let mut journal = Journal::open(&session).unwrap();
    for index in 0..16 {
        journal
            .commit_document(&state(&format!("tab{index}"), 1, b"shared"))
            .unwrap();
    }
    assert!(matches!(
        journal.commit_document(&state("extra", 1, b"extra")),
        Err(JournalError::Limit(_))
    ));
    journal.close_document("tab15").unwrap();
    assert_eq!(journal.recover().unwrap().len(), 15);
    drop(journal);
    let original = manifest(&session);
    let mut value = original.clone();
    let mut history = value["documents"][0]["current"].clone();
    value["documents"][0]["current"]["length"] = pdf_worker::MAX_PDF_BYTES.into();
    history["length"] = pdf_worker::MAX_PDF_BYTES.into();
    history["revision"] = 2.into();
    value["documents"][0]["undo"] = serde_json::json!([history.clone(), { "revision":3, "hash":history["hash"], "length":history["length"], "info":history["info"] }]);
    value["documents"][0]["next_revision"] = 4.into();
    rewrite(&session, &value);
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Limit(_))
    ));
    rewrite(&session, &original);
    std::fs::OpenOptions::new()
        .write(true)
        .open(session.join("manifest.json"))
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        Journal::open(&session),
        Err(JournalError::Limit(_))
    ));
}

#[test]
fn typed_worker_metadata_reply_round_trip() {
    let metadata = pdf_core::DocumentMetadata {
        fields: [(pdf_core::MetadataKey::Title, "Révision 中文".into())].into(),
        info_present: true,
        xmp_present: true,
        catalog_info_present: false,
        xmp_synchronized: false,
        secure_erasure: false,
    };
    let decoded: pdf_core::DocumentMetadata =
        serde_json::from_slice(&serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(decoded, metadata);
}
