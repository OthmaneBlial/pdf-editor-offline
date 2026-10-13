#![allow(clippy::unwrap_used)]
use super::*;
#[path = "../../../../tests/support/content_fixture.rs"]
pub(crate) mod fixture;

#[test]
#[ignore = "requires freshly built native binary and pinned qpdf12.4.2/Poppler26.10"]
fn production_native_proportional_width_change_is_durable_and_source_bound() {
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
        || {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source.pdf");
            let output = temp.path().join("new.pdf");
            let bytes = fixture::source_with_font(fixture::CONTENT, "Helvetica");
            std::fs::write(&source, &bytes).unwrap();
            let mut journal = Journal::open(temp.path().join("journal")).unwrap();
            let mut documents = HashMap::new();
            process_with_journal(
                &mut documents,
                Command::Load {
                    id: 1,
                    path: source.clone(),
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
            let command = |policy| Command::Edit {
                id: 1,
                edit: Edit::ContentText {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change: pdf_core::ContentTextChange {
                        layout_policy: policy,
                        identity: inventory.runs[0].identity.clone(),
                        expected: "Original".into(),
                        replacement: "Longer text".into(),
                    },
                },
            };
            assert!(
                process_with_journal(
                    &mut documents,
                    command(Default::default()),
                    Some(&mut journal)
                )
                .is_err()
            );
            assert_eq!(documents[&1].current.bytes, bytes);
            assert!(documents[&1].undo.is_empty());
            let Event::Updated { status, .. } = process_with_journal(
                &mut documents,
                command(pdf_core::ContentTextLayoutPolicy::AllowWidthChange),
                Some(&mut journal),
            )
            .unwrap() else {
                panic!("updated")
            };
            assert!(status.can_undo && status.dirty);
            assert_eq!(documents[&1].undo.len(), 1);
            assert_eq!(
                journal.recover().unwrap()[0].current.bytes,
                documents[&1].current.bytes
            );
            assert!(
                process_with_journal(
                    &mut documents,
                    command(pdf_core::ContentTextLayoutPolicy::AllowWidthChange),
                    Some(&mut journal)
                )
                .is_err()
            );
            let edited = documents[&1].current.bytes.clone();
            process_with_journal(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Undo,
                },
                Some(&mut journal),
            )
            .unwrap();
            assert_eq!(documents[&1].current.bytes, bytes);
            process_with_journal(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Redo,
                },
                Some(&mut journal),
            )
            .unwrap();
            assert_eq!(documents[&1].current.bytes, edited);
            process_with_journal(
                &mut documents,
                Command::Save {
                    id: 1,
                    path: output.clone(),
                },
                Some(&mut journal),
            )
            .unwrap();
            let qpdf = std::path::PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap());
            let poppler =
                std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
            assert!(
                std::process::Command::new(&qpdf)
                    .arg("--version")
                    .output()
                    .unwrap()
                    .stdout
                    .starts_with(b"qpdf version 12.4.2")
            );
            assert!(
                std::process::Command::new(qpdf)
                    .arg("--check")
                    .arg(&output)
                    .status()
                    .unwrap()
                    .success()
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
            // Pinned Poppler infers `AB` from this source's narrow12pt
            // Helvetica space in default mode. Raw mode preserves literal
            // `A B`. Assert both source baselines and both exact edits.
            for raw in [false, true] {
                for (path, expected) in [
                    (
                        &source,
                        if raw {
                            vec!["Original", "A", "B", "Original", "A", "B"]
                        } else {
                            vec!["Original", "AB", "Original", "AB"]
                        },
                    ),
                    (
                        &output,
                        if raw {
                            vec!["Longer", "text", "A", "B", "Original", "A", "B"]
                        } else {
                            vec!["Longer", "text", "AB", "Original", "AB"]
                        },
                    ),
                ] {
                    let mut command = std::process::Command::new(poppler.join("pdftotext"));
                    if raw {
                        command.arg("-raw");
                    }
                    let text = command.arg(path).arg("-").output().unwrap();
                    assert!(text.status.success());
                    assert_eq!(
                        String::from_utf8(text.stdout)
                            .unwrap()
                            .split_whitespace()
                            .collect::<Vec<_>>(),
                        expected
                    );
                }
            }
            assert_eq!(
                pdf_render::render_page(&bytes, 1, 1.0).unwrap(),
                pdf_render::render_page(&edited, 1, 1.0).unwrap()
            );
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
        },
    );
}

#[test]
fn failed_child_preserves_content_snapshot_history_journal_and_retry() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&source, &bytes).unwrap();
    let mut journal = Journal::open(temp.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    process_with_journal(
        &mut documents,
        Command::Load {
            id: 1,
            path: source.clone(),
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
    let command = || Command::Edit {
        id: 1,
        edit: Edit::ContentText {
            key,
            source_fingerprint: inventory.source_fingerprint,
            change: pdf_core::ContentTextChange {
                layout_policy: Default::default(),
                identity: inventory.runs[0].identity.clone(),
                expected: "Original".into(),
                replacement: "Replaced".into(),
            },
        },
    };
    let current = std::env::current_exe().unwrap();
    let helper = current
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-worker-test-helper.exe"
        } else {
            "pdf-worker-test-helper"
        });
    assert!(helper.is_file(), "build test-helper first");
    for mode in ["abort", "timeout", "malformed"] {
        let limits = pdf_worker::Limits::for_test(
            pdf_worker::MEMORY_BYTES,
            pdf_worker::CPU_SECONDS,
            std::time::Duration::from_millis(500),
        )
        .unwrap();
        document::with_test_executor(helper.clone(), vec![mode.into()], limits, || {
            assert!(process_with_journal(&mut documents, command(), Some(&mut journal)).is_err());
        });
        assert_eq!(documents[&1].current.bytes, bytes);
        assert!(documents[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, bytes);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }
    assert!(matches!(
        process_with_journal(&mut documents, command(), Some(&mut journal)).unwrap(),
        Event::Updated { .. }
    ));
    assert_eq!(documents[&1].undo.len(), 1);
}

fn exercise(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("saved.pdf");
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
    let change = pdf_core::ContentTextChange {
        layout_policy: Default::default(),
        identity: inventory.runs[0].identity.clone(),
        expected: "Original".into(),
        replacement: "Replaced".into(),
    };
    let command = |key, hash, change| Command::Edit {
        id: 1,
        edit: Edit::ContentText {
            key,
            source_fingerprint: hash,
            change,
        },
    };
    for index in 0..4 {
        let mut altered = change.clone();
        let mut hash = inventory.source_fingerprint;
        let mut target = key;
        match index {
            0 => target.revision = 99,
            1 => target.tab = 2,
            2 => hash[0] ^= 1,
            _ => altered.replacement = "Too long text".into(),
        }
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
                Edit::ContentText {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change: change.clone()
                },
                HISTORY_BYTES,
                "1",
                |_| Err("forced durable failure".into())
            )
            .is_err()
    );
    assert_eq!(documents[&1].current.bytes, bytes);
    assert!(documents[&1].undo.is_empty());
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
    assert!(
        process_with_journal(
            &mut documents,
            command(key, inventory.source_fingerprint, change),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(documents[&1].current.bytes, edited);
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
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Replaced\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap().rgba,
        pdf_render::render_page(&saved, 1, 1.0).unwrap().rgba
    );
    assert_eq!(journal.recover().unwrap()[0].saved_revision, Some(1));
    assert!(!documents[&1].status().unwrap().dirty);
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
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    if reference {
        let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
        assert!(
            String::from_utf8_lossy(
                &std::process::Command::new(&qpdf)
                    .arg("--version")
                    .output()
                    .unwrap()
                    .stdout
            )
            .contains("12.4.2")
        );
        assert!(
            std::process::Command::new(qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let poppler = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
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
        let text = std::process::Command::new(poppler.join("pdftotext"))
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert_eq!(
            String::from_utf8(text.stdout)
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            vec!["Replaced", "A", "B", "Original", "A", "B"]
        );
        let pixels = |path: &std::path::Path, page: u32| {
            let result = std::process::Command::new(poppler.join("pdftoppm"))
                .args([
                    "-singlefile",
                    "-r",
                    "72",
                    "-f",
                    &page.to_string(),
                    "-l",
                    &page.to_string(),
                ])
                .arg(path)
                .output()
                .unwrap();
            assert!(result.status.success());
            result
                .stdout
                .strip_prefix(b"P6\n200 200\n255\n")
                .unwrap()
                .to_vec()
        };
        let before = pixels(&input, 1);
        let after = pixels(&output, 1);
        let mut changes = 0;
        for (index, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                let x = index % 200;
                let y = index / 200;
                assert!(
                    (35..145).contains(&x) && (35..70).contains(&y),
                    "outside selected text {x},{y}"
                );
                changes += 1;
            }
        }
        assert!(changes > 20);
        assert_eq!(pixels(&input, 2), pixels(&output, 2));
        let before = lopdf::Document::load_mem(&bytes).unwrap();
        let after = lopdf::Document::load_mem(&saved).unwrap();
        assert_eq!(
            before.get_dictionary((4, 0)).unwrap(),
            after.get_dictionary((4, 0)).unwrap()
        );
        assert_ne!(
            before.get_page_content((3, 0)),
            after.get_page_content((3, 0))
        );
        assert_eq!(
            before.get_page_content((4, 0)),
            after.get_page_content((4, 0))
        );
    }
}
#[test]
fn content_edit_real_child_durable_rollback_history_and_save() {
    exercise(false);
}
#[test]
fn content_edit_pending_save_refuses_without_history_change() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("source.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&path, &bytes).unwrap();
    let mut documents = HashMap::new();
    let mut journal = Journal::open(temporary.path().join("journal")).unwrap();
    process_with_journal(
        &mut documents,
        Command::Load { id: 1, path },
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
    let inventory = documents[&1].content_text(1, 0).unwrap();
    journal
        .begin_save("1", 0, &temporary.path().join("uncertain.pdf"), &bytes)
        .unwrap();
    assert!(
        process_with_journal(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::ContentText {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change: pdf_core::ContentTextChange {
                        layout_policy: Default::default(),
                        identity: inventory.runs[0].identity.clone(),
                        expected: "Original".into(),
                        replacement: "Replaced".into()
                    }
                }
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(documents[&1].current.bytes, bytes);
    assert!(documents[&1].undo.is_empty());
    assert_eq!(journal.save_intents().len(), 1);
}
#[test]
#[ignore = "requires freshly built native binary and pinned qpdf/Poppler"]
fn content_edit_production_native_child_independent_reader_pixels() {
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
    assert!(native.is_file());
    document::with_test_executor(
        native,
        vec![std::ffi::OsString::from("--pdf-worker")],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
