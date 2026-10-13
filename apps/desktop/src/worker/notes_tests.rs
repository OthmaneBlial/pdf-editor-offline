#![allow(clippy::unwrap_used)]
use super::*;
use pdf_worker::document::NoteChange;
const SOURCE: &[u8] = include_bytes!("../../../../tests/fixtures/pdfs/bookmarks.pdf");
fn load(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf) {
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
fn key(revision: u64) -> NotesKey {
    NotesKey {
        tab: 1,
        revision,
        page: 1,
        offset: 0,
        generation: 1,
    }
}
fn add() -> NoteChange {
    NoteChange::Add {
        page: 1,
        rectangle: [30.0, 30.0, 54.0, 54.0],
        contents: "Review 中文 😀".into(),
        author: "Reviewer".into(),
    }
}
fn edit(change: NoteChange, revision: u64) -> Command {
    Command::Edit {
        id: 1,
        edit: Edit::Note {
            key: key(revision),
            change,
        },
    }
}
fn listed(
    docs: &mut HashMap<u64, WorkingDocument>,
    journal: &mut Journal,
    revision: u64,
) -> Vec<pdf_worker::document::NoteEntry> {
    let Event::Notes { entries, .. } =
        process_with_journal(docs, Command::Notes(key(revision)), Some(journal)).unwrap()
    else {
        panic!("notes");
    };
    entries
}
#[test]
fn actual_notes_roundtrip_history_journal_and_verified_save_preserve_source_and_appearance() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load(temp.path());
    assert!(listed(&mut docs, &mut journal, 0).is_empty());
    process_with_journal(&mut docs, edit(add(), 0), Some(&mut journal)).unwrap();
    let note = listed(&mut docs, &mut journal, 1).remove(0);
    assert!(note.editable);
    assert_eq!(note.note.contents, "Review 中文 😀");
    let before = docs[&1].current.bytes.clone();
    process_with_journal(
        &mut docs,
        edit(
            NoteChange::Edit {
                page: 1,
                annotation_index: note.note.annotation_index,
                expected_id: note.note.object_id,
                contents: Some("Updated العربية".into()),
                author: None,
            },
            1,
        ),
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(
        pdf_render::render_page(&before, 0, 1.0).unwrap().rgba,
        pdf_render::render_page(&docs[&1].current.bytes, 0, 1.0)
            .unwrap()
            .rgba
    );
    assert_eq!(
        listed(&mut docs, &mut journal, 2)[0].note.contents,
        "Updated العربية"
    );
    let updated = docs[&1].current.bytes.clone();
    process_with_journal(
        &mut docs,
        Command::Edit {
            id: 1,
            edit: Edit::Undo,
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(
        listed(&mut docs, &mut journal, 1)[0].note.contents,
        "Review 中文 😀"
    );
    process_with_journal(
        &mut docs,
        Command::Edit {
            id: 1,
            edit: Edit::Redo,
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(docs[&1].current.bytes, updated);
    let note = listed(&mut docs, &mut journal, 2).remove(0).note;
    process_with_journal(
        &mut docs,
        edit(
            NoteChange::Remove {
                page: 1,
                annotation_index: note.annotation_index,
                expected_id: note.object_id,
            },
            2,
        ),
        Some(&mut journal),
    )
    .unwrap();
    assert!(listed(&mut docs, &mut journal, 3).is_empty());
    assert_eq!(
        pdf_core::PdfDocument::from_bytes(SOURCE, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .trim(),
        pdf_core::PdfDocument::from_bytes(&docs[&1].current.bytes, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .trim()
    );
    assert_eq!(
        journal.recover().unwrap()[0].current.bytes,
        docs[&1].current.bytes
    );
    let output = temp.path().join("saved.pdf");
    process_with_journal(
        &mut docs,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert!(
        pdf_core::PdfDocument::open(output, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()
            .is_empty()
    );
    assert_eq!(std::fs::read(path).unwrap(), SOURCE);
}
#[test]
fn stale_identity_wrong_page_limits_pending_save_and_journal_failures_preserve_state() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _) = load(temp.path());
    let before = docs[&1].current.bytes.clone();
    assert!(process_with_journal(&mut docs, edit(add(), 9), Some(&mut journal)).is_err());
    let mut zero = key(0);
    zero.page = 0;
    assert!(
        process_with_journal(
            &mut docs,
            Command::Edit {
                id: 1,
                edit: Edit::Note {
                    key: zero,
                    change: NoteChange::Add {
                        page: 0,
                        rectangle: [30.0, 30.0, 54.0, 54.0],
                        contents: "x".into(),
                        author: String::new()
                    }
                }
            },
            Some(&mut journal)
        )
        .is_err()
    );
    let mut wrong = key(0);
    wrong.tab = 2;
    assert!(
        process_with_journal(
            &mut docs,
            Command::Edit {
                id: 1,
                edit: Edit::Note {
                    key: wrong,
                    change: add()
                }
            },
            Some(&mut journal)
        )
        .is_err()
    );
    let bad = NoteChange::Add {
        page: 2,
        rectangle: [30.0, 30.0, 54.0, 54.0],
        contents: "x".into(),
        author: "y".into(),
    };
    assert!(process_with_journal(&mut docs, edit(bad, 0), Some(&mut journal)).is_err());
    let oversized = NoteChange::Add {
        page: 1,
        rectangle: [30.0, 30.0, 54.0, 54.0],
        contents: "x".repeat(pdf_core::MAX_NOTE_CONTENT_BYTES + 1),
        author: String::new(),
    };
    assert!(process_with_journal(&mut docs, edit(oversized, 0), Some(&mut journal)).is_err());
    journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &before)
        .unwrap();
    assert!(process_with_journal(&mut docs, edit(add(), 0), Some(&mut journal)).is_err());
    let token = journal.save_intents()[0].token.clone();
    journal.cancel_save(&token).unwrap();
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"block").unwrap();
    assert!(process_with_journal(&mut docs, edit(add(), 0), Some(&mut journal)).is_err());
    std::fs::remove_file(blocker).unwrap();
    assert_eq!(docs[&1].current.bytes, before);
    assert!(docs[&1].undo.is_empty());
    assert_eq!(journal.recover().unwrap()[0].current.bytes, before);
    let directory = journal.directory().to_path_buf();
    drop(journal);
    let mut journal = Journal::open(directory).unwrap();
    process_with_journal(&mut docs, edit(add(), 0), Some(&mut journal)).unwrap();
    let before = docs[&1].current.bytes.clone();
    assert!(
        process_with_journal(
            &mut docs,
            edit(
                NoteChange::Remove {
                    page: 1,
                    annotation_index: 1,
                    expected_id: Some((999, 0))
                },
                1
            ),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs[&1].current.bytes, before);
}

#[test]
#[ignore = "build normal native executable and pinned qpdf/Poppler for independent notes gate"]
fn production_native_notes_preserve_page_content_pixels_metadata_and_verified_new_copy() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path) = load(temp.path());
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
    let added = temp.path().join("added.pdf");
    let edited = temp.path().join("edited.pdf");
    let removed = temp.path().join("removed.pdf");
    document::with_test_executor(
        native,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || {
            process_with_journal(&mut docs, edit(add(), 0), Some(&mut journal)).unwrap();
            let bytes = docs[&1].current.bytes.clone();
            std::fs::write(&added, &bytes).unwrap();
            let note = listed(&mut docs, &mut journal, 1).remove(0).note;
            process_with_journal(
                &mut docs,
                edit(
                    NoteChange::Edit {
                        page: 1,
                        annotation_index: note.annotation_index,
                        expected_id: note.object_id,
                        contents: Some("Updated العربية 😀".into()),
                        author: Some("中文 reviewer".into()),
                    },
                    1,
                ),
                Some(&mut journal),
            )
            .unwrap();
            process_with_journal(
                &mut docs,
                Command::Save {
                    id: 1,
                    path: edited.clone(),
                },
                Some(&mut journal),
            )
            .unwrap();
            let note = listed(&mut docs, &mut journal, 2).remove(0).note;
            assert_eq!(note.contents, "Updated العربية 😀");
            assert_eq!(note.author, "中文 reviewer");
            process_with_journal(
                &mut docs,
                edit(
                    NoteChange::Remove {
                        page: 1,
                        annotation_index: note.annotation_index,
                        expected_id: note.object_id,
                    },
                    2,
                ),
                Some(&mut journal),
            )
            .unwrap();
            process_with_journal(
                &mut docs,
                Command::Save {
                    id: 1,
                    path: removed.clone(),
                },
                Some(&mut journal),
            )
            .unwrap();
        },
    );
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let output = std::process::Command::new(&qpdf)
        .arg("--json")
        .arg(&edited)
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let note = pdf_core::PdfDocument::open(&edited, None)
        .unwrap()
        .list_notes(&[1])
        .unwrap()
        .remove(0);
    let id = note.object_id.unwrap();
    let object = &json["qpdf"][1][format!("obj:{} {} R", id.0, id.1)]["value"];
    assert_eq!(object["/Contents"], "u:Updated العربية 😀");
    assert_eq!(object["/T"], "u:中文 reviewer");
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let source = lopdf::Document::load_mem(SOURCE).unwrap();
    let source_content = source
        .get_page_content_with_limit(source.get_pages()[&1], pdf_core::MAX_STREAM_BYTES)
        .unwrap();
    for target in [&added, &edited, &removed] {
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--check")
                .arg(target)
                .output()
                .unwrap()
                .status
                .success()
        );
        let doc = lopdf::Document::load(target).unwrap();
        assert_eq!(
            doc.get_page_content_with_limit(doc.get_pages()[&1], pdf_core::MAX_STREAM_BYTES)
                .unwrap(),
            source_content
        );
        assert_eq!(
            pdf_core::PdfDocument::open(target, None)
                .unwrap()
                .info()
                .unwrap()
                .pages,
            pdf_core::PdfDocument::from_bytes(SOURCE, None)
                .unwrap()
                .info()
                .unwrap()
                .pages
        );
    }
    let run = |tool: &str, path: &std::path::Path| {
        let mut cmd = std::process::Command::new(bin.join(tool));
        cmd.args(["-f", "1", "-l", "1"]);
        if tool == "pdftoppm" {
            cmd.args(["-r", "72", "-singlefile"]);
        }
        cmd.arg(path);
        if tool == "pdftotext" {
            cmd.arg("-");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    for target in [&added, &edited, &removed] {
        assert_eq!(run("pdftotext", &path), run("pdftotext", target));
    }
    let a = run("pdftoppm", &added);
    let b = run("pdftoppm", &edited);
    assert_eq!(a, b);
    assert_eq!(run("pdftoppm", &path), run("pdftoppm", &removed));
    assert_ne!(run("pdftoppm", &path), a);
    assert_eq!(std::fs::read(path).unwrap(), SOURCE);
    assert!(
        pdf_core::PdfDocument::open(removed, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()
            .is_empty()
    );
}
