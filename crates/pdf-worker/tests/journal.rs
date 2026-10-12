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
    std::fs::write(&orphan, b"orphan").unwrap();
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
    std::fs::write(session.join("manifest.json.tmp"), b"interrupted temporary").unwrap();
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
