#![allow(clippy::unwrap_used)]

use super::*;
use crate::state::MetadataReadKey;
use crate::worker::{Command, Edit, process_with_journal};
use lopdf::{Document, Stream, dictionary};
use pdf_worker::journal::DocumentState;

fn fixture(directory: &Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 10 20 Td (Recovery fixture) Tj ET".to_vec(),
    ));
    let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(&path).unwrap();
    path
}
fn act(
    recovery: &mut Recovery,
    documents: &mut HashMap<u64, WorkingDocument>,
    command: Command,
) -> Result<Event, String> {
    process_with_journal(documents, command, Some(recovery.journal()?))
}
fn session_id(recovery: &mut Recovery) -> String {
    recovery
        .journal()
        .unwrap()
        .directory()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn durable_events_follow_reopenable_load_metadata_history_save_and_close() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), "source.pdf");
    let original = std::fs::read(&source).unwrap();
    let mut recovery = Recovery::at(&dir.path().join("sessions")).unwrap();
    let mut documents = HashMap::new();
    act(
        &mut recovery,
        &mut documents,
        Command::Load {
            id: 1,
            path: source.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        recovery.journal().unwrap().recover().unwrap()[0].saved_revision,
        Some(0)
    );
    let key = MetadataReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Metadata {
                key,
                changes: vec![(pdf_core::MetadataKey::Title, Some("Reprise 中文".into()))],
            },
        },
    )
    .unwrap();
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Undo,
        },
    )
    .unwrap();
    assert_eq!(
        recovery.journal().unwrap().recover().unwrap()[0]
            .current
            .revision,
        0
    );
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Redo,
        },
    )
    .unwrap();
    assert_eq!(
        recovery.journal().unwrap().recover().unwrap()[0].undo.len(),
        1
    );
    let output = dir.path().join("copy.pdf");
    act(
        &mut recovery,
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        recovery.journal().unwrap().recover().unwrap()[0].saved_revision,
        Some(1)
    );
    assert_eq!(
        pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .metadata()
            .unwrap()
            .fields[&pdf_core::MetadataKey::Title],
        "Reprise 中文"
    );
    assert_eq!(std::fs::read(source).unwrap(), original);
    act(&mut recovery, &mut documents, Command::Close(1)).unwrap();
    assert!(documents.is_empty());
    assert!(recovery.journal().unwrap().recover().unwrap().is_empty());
    assert!(recovery.journal().unwrap().owned_disk_bytes().unwrap() > 0);
    let own = recovery.journal().unwrap().directory().to_owned();
    recovery.finish(&documents);
    assert!(!own.exists());
}

#[test]
fn journal_failure_preserves_parent_for_edit_undo_redo_close_load_and_images() {
    for mode in ["edit", "undo", "redo", "close", "load", "images"] {
        let dir = tempfile::tempdir().unwrap();
        let source = fixture(dir.path(), "source.pdf");
        let mut recovery = Recovery::at(&dir.path().join("sessions")).unwrap();
        let mut documents = HashMap::new();
        act(
            &mut recovery,
            &mut documents,
            Command::Load {
                id: 1,
                path: source.clone(),
            },
        )
        .unwrap();
        act(
            &mut recovery,
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::Rotate(1, 90),
            },
        )
        .unwrap();
        if mode == "redo" {
            act(
                &mut recovery,
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Undo,
                },
            )
            .unwrap();
        }
        let bytes = documents[&1].current.bytes.clone();
        let revision = documents[&1].current.revision;
        let undo = documents[&1].undo.len();
        let redo = documents[&1].redo.len();
        let journal_path = recovery.journal().unwrap().directory().to_owned();
        std::fs::write(journal_path.join("manifest.json.tmp"), b"block publication").unwrap();
        let command = match mode {
            "edit" => Command::Edit {
                id: 1,
                edit: Edit::Rotate(1, 90),
            },
            "undo" => Command::Edit {
                id: 1,
                edit: Edit::Undo,
            },
            "redo" => Command::Edit {
                id: 1,
                edit: Edit::Redo,
            },
            "close" => Command::Close(1),
            "load" => Command::Load {
                id: 2,
                path: source,
            },
            _ => {
                let image = dir.path().join("image.png");
                std::fs::write(&image, crate::worker::TEST_RED_PNG).unwrap();
                Command::Images {
                    id: 2,
                    paths: vec![image],
                }
            }
        };
        assert!(
            act(&mut recovery, &mut documents, command).is_err(),
            "{mode}"
        );
        assert_eq!(documents.len(), 1, "{mode}");
        assert_eq!(documents[&1].current.bytes, bytes, "{mode}");
        assert_eq!(documents[&1].current.revision, revision, "{mode}");
        assert_eq!(documents[&1].undo.len(), undo, "{mode}");
        assert_eq!(documents[&1].redo.len(), redo, "{mode}");
        drop(recovery);
        std::fs::remove_file(journal_path.join("manifest.json.tmp")).unwrap();
        let reopened = Journal::open(&journal_path).unwrap().recover().unwrap();
        assert_eq!(reopened.len(), 1);
        assert_eq!(reopened[0].current.bytes, bytes);
    }
}

#[test]
fn crashed_published_save_recovers_beside_existing_tabs_remaps_and_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), "source.pdf");
    let sessions = dir.path().join("sessions");
    let mut prior = Recovery::at(&sessions).unwrap();
    let old_id = session_id(&mut prior);
    let mut old_documents = HashMap::new();
    act(
        &mut prior,
        &mut old_documents,
        Command::Load {
            id: 1,
            path: source.clone(),
        },
    )
    .unwrap();
    act(
        &mut prior,
        &mut old_documents,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
    )
    .unwrap();
    let artifact = old_documents[&1].prepare_save().unwrap();
    let output = dir.path().join("unacknowledged.pdf");
    prior
        .journal()
        .unwrap()
        .begin_save("1", 1, &output, &artifact)
        .unwrap();
    document::publish(&artifact, &output).unwrap();
    drop(prior); // Simulate process exit before the saved revision acknowledgement.
    let mut recovery = Recovery::at(&sessions).unwrap();
    let mut documents = HashMap::new();
    act(
        &mut recovery,
        &mut documents,
        Command::Load {
            id: 7,
            path: source,
        },
    )
    .unwrap();
    let existing = documents[&7].current.bytes.clone();
    let Event::RecoveryRestored {
        documents: returned,
    } = recovery.restore(&old_id, &mut documents).unwrap()
    else {
        panic!("restored event");
    };
    assert_eq!(returned[0].id, 8);
    assert_eq!(returned[0].pending_save.as_deref(), Some(output.as_path()));
    assert!(returned[0].status.dirty && returned[0].status.can_undo);
    assert_eq!(documents[&7].current.bytes, existing);
    assert!(!sessions.join(old_id).exists());
    assert_eq!(recovery.journal().unwrap().recover().unwrap().len(), 2);
    assert!(
        act(
            &mut recovery,
            &mut documents,
            Command::Edit {
                id: 8,
                edit: Edit::Undo
            }
        )
        .is_err()
    );
    assert!(act(&mut recovery, &mut documents, Command::Close(8)).is_err());
    act(
        &mut recovery,
        &mut documents,
        Command::ResolveSave { id: 8 },
    )
    .unwrap();
    assert_eq!(std::fs::read(output).unwrap(), artifact);
    assert!(!documents[&8].status().unwrap().dirty);
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 8,
            edit: Edit::Undo,
        },
    )
    .unwrap();
    assert_eq!(documents[&8].current.info.pages[0].rotation, 0);
    assert!(documents[&8].status().unwrap().dirty);
}

#[test]
fn resolve_missing_or_mismatched_copy_and_explicit_cancel_preserve_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), "source.pdf");
    let mut recovery = Recovery::at(&dir.path().join("sessions")).unwrap();
    let mut documents = HashMap::new();
    act(
        &mut recovery,
        &mut documents,
        Command::Load {
            id: 1,
            path: source,
        },
    )
    .unwrap();
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
    )
    .unwrap();
    let current = documents[&1].current.bytes.clone();
    let artifact = documents[&1].prepare_save().unwrap();
    let output = dir.path().join("pending.pdf");
    recovery
        .journal()
        .unwrap()
        .begin_save("1", 1, &output, &artifact)
        .unwrap();
    std::fs::write(&output, b"unrelated existing file").unwrap();
    assert!(matches!(
        act(
            &mut recovery,
            &mut documents,
            Command::ResolveSave { id: 1 }
        )
        .unwrap(),
        Event::SavePending { .. }
    ));
    assert_eq!(recovery.journal().unwrap().save_intents().len(), 1);
    assert_eq!(std::fs::read(&output).unwrap(), b"unrelated existing file");
    act(&mut recovery, &mut documents, Command::CancelSave { id: 1 }).unwrap();
    assert!(documents[&1].status().unwrap().dirty);
    assert_eq!(documents[&1].current.bytes, current);
    assert_eq!(std::fs::read(&output).unwrap(), b"unrelated existing file");
    let missing = dir.path().join("missing-copy.pdf");
    recovery
        .journal()
        .unwrap()
        .begin_save("1", 1, &missing, &artifact)
        .unwrap();
    act(
        &mut recovery,
        &mut documents,
        Command::ResolveSave { id: 1 },
    )
    .unwrap();
    assert_eq!(std::fs::read(missing).unwrap(), artifact);
    assert!(!documents[&1].status().unwrap().dirty);
    let another = dir.path().join("already-exists.pdf");
    std::fs::write(&another, b"retain").unwrap();
    assert!(
        act(
            &mut recovery,
            &mut documents,
            Command::Save {
                id: 1,
                path: another
            }
        )
        .is_err()
    );
    assert!(recovery.journal().unwrap().save_intents().is_empty());
}

#[test]
fn failed_acknowledgement_of_published_copy_survives_reopen_and_explicit_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), "source.pdf");
    let sessions = dir.path().join("sessions");
    let mut recovery = Recovery::at(&sessions).unwrap();
    let old_id = session_id(&mut recovery);
    let session_path = recovery.journal().unwrap().directory().to_owned();
    let mut documents = HashMap::new();
    act(
        &mut recovery,
        &mut documents,
        Command::Load {
            id: 1,
            path: source,
        },
    )
    .unwrap();
    act(
        &mut recovery,
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
    )
    .unwrap();
    let committed = documents[&1].current.bytes.clone();
    let artifact = documents[&1].prepare_save().unwrap();
    let output = dir.path().join("published.pdf");
    recovery
        .journal()
        .unwrap()
        .begin_save("1", 1, &output, &artifact)
        .unwrap();
    document::publish(&artifact, &output).unwrap();
    std::fs::write(
        session_path.join("manifest.json.tmp"),
        b"fail save acknowledgement",
    )
    .unwrap();
    assert!(matches!(
        act(
            &mut recovery,
            &mut documents,
            Command::ResolveSave { id: 1 }
        )
        .unwrap(),
        Event::SavePending { .. }
    ));
    assert_eq!(documents[&1].current.bytes, committed);
    assert_eq!(documents[&1].saved_revision, Some(0));
    assert_eq!(documents[&1].undo.len(), 1);
    assert_eq!(recovery.journal().unwrap().save_intents().len(), 1);
    assert_eq!(std::fs::read(&output).unwrap(), artifact);
    drop(recovery);
    std::fs::remove_file(session_path.join("manifest.json.tmp")).unwrap();
    let mut recovery = Recovery::at(&sessions).unwrap();
    let mut restored = HashMap::new();
    recovery.restore(&old_id, &mut restored).unwrap();
    assert!(restored[&1].status().unwrap().dirty);
    assert_eq!(recovery.journal().unwrap().save_intents().len(), 1);
    act(&mut recovery, &mut restored, Command::ResolveSave { id: 1 }).unwrap();
    assert!(!restored[&1].status().unwrap().dirty);
    assert!(recovery.journal().unwrap().save_intents().is_empty());
    assert_eq!(std::fs::read(output).unwrap(), artifact);
}

#[test]
fn failed_restore_keeps_source_and_existing_tabs_and_never_partially_commits() {
    for malformed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), "source.pdf");
        let sessions = dir.path().join("sessions");
        let store = SessionStore::open(&sessions).unwrap();
        let mut source = store.create().unwrap();
        let source_id = source
            .directory()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let working = WorkingDocument::load(&path).unwrap();
        let mut state = DocumentState {
            id: "1".into(),
            current: pdf_worker::journal::Snapshot {
                revision: 0,
                bytes: working.current.bytes,
                info: working.current.info,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: Some(0),
            next_revision: 1,
            path: Some(path.clone()),
        };
        if malformed {
            state.current.bytes = b"%PDF-invalid recovery data".to_vec();
        }
        source.commit_document(&state).unwrap();
        drop(source);
        let mut recovery = Recovery::at(&sessions).unwrap();
        let mut documents = HashMap::new();
        act(&mut recovery, &mut documents, Command::Load { id: 9, path }).unwrap();
        let existing = documents[&9].current.bytes.clone();
        if !malformed {
            std::fs::write(
                recovery
                    .journal()
                    .unwrap()
                    .directory()
                    .join("manifest.json.tmp"),
                b"block destination commit",
            )
            .unwrap();
        }
        assert!(recovery.restore(&source_id, &mut documents).is_err());
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[&9].current.bytes, existing);
        assert!(sessions.join(&source_id).is_dir());
        assert_eq!(
            store
                .open_session(sessions.join(source_id))
                .unwrap()
                .recover()
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn discovery_shows_busy_empty_private_backups_and_rejects_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), "source.pdf");
    let sessions = dir.path().join("sessions");
    let mut prior = Recovery::at(&sessions).unwrap();
    let prior_id = session_id(&mut prior);
    let mut documents = HashMap::new();
    act(&mut prior, &mut documents, Command::Load { id: 1, path }).unwrap();
    act(&mut prior, &mut documents, Command::Close(1)).unwrap();
    let mut current = Recovery::at(&sessions).unwrap();
    let Event::RecoveryListed { sessions: listed } = current.list().unwrap() else {
        panic!("list");
    };
    assert!(
        listed
            .iter()
            .find(|entry| entry.id == prior_id)
            .unwrap()
            .busy
    );
    assert!(current.restore("../escape", &mut HashMap::new()).is_err());
    assert!(current.discard(&prior_id).is_err());
    drop(prior);
    #[cfg(unix)]
    {
        std::fs::File::open(sessions.join(&prior_id))
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60)),
            )
            .unwrap();
    }
    let Event::RecoveryListed { sessions: listed } = current.list().unwrap() else {
        panic!("list");
    };
    let retained = listed.iter().find(|entry| entry.id == prior_id).unwrap();
    assert_eq!(retained.documents, 0);
    assert!(retained.bytes > 0 && !retained.busy);
    #[cfg(unix)]
    assert!(
        retained
            .error
            .as_ref()
            .unwrap()
            .contains("Older than seven days")
    );
    current.discard(&prior_id).unwrap();
    assert!(!sessions.join(prior_id).exists());
}
