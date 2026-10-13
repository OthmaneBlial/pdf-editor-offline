#![allow(clippy::unwrap_used)]
use super::document::{OpenResult, WorkingDocument};
use super::*;
use pdf_worker::document::ReadOperation;
use std::path::Path;
#[path = "../../../../crates/pdf-worker/tests/support/encryption.rs"]
pub(crate) mod encryption;

fn tree(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = vec![];
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            files.extend(tree(&entry.path()));
        } else {
            files.push((entry.path(), std::fs::read(entry.path()).unwrap()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn password_retry_keeps_ciphertext_and_never_writes_a_recovery_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let (cipher, plain) = encryption::fixture(4, "synthetic-reader", false);
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &cipher).unwrap();
    let mut journal = pdf_worker::journal::Journal::open(temp.path().join("journal")).unwrap();
    let before = tree(journal.directory());
    let mut docs = HashMap::new();
    let Event::PasswordRequired {
        fingerprint,
        incorrect: false,
        ..
    } = process_with_journal(
        &mut docs,
        Command::Load {
            id: 1,
            path: path.clone(),
        },
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("password prompt");
    };
    assert!(docs.is_empty());
    for password in ["wrong", ""] {
        assert!(matches!(
            process_with_journal(
                &mut docs,
                Command::Unlock {
                    id: 1,
                    path: path.clone(),
                    fingerprint,
                    password: password.into()
                },
                Some(&mut journal)
            )
            .unwrap(),
            Event::PasswordRequired {
                incorrect: true,
                ..
            }
        ));
        assert!(docs.is_empty());
    }
    let Event::Unlocked { status, .. } = process_with_journal(
        &mut docs,
        Command::Unlock {
            id: 1,
            path: path.clone(),
            fingerprint,
            password: "synthetic-reader".into(),
        },
        Some(&mut journal),
    )
    .unwrap() else {
        panic!("real unlock");
    };
    assert!(status.info.encrypted && !status.dirty && !status.can_undo && !status.can_redo);
    assert_eq!(docs[&1].current.bytes, cipher);
    assert!(docs[&1].session_only());
    let read = docs[&1].read(ReadOperation::Text { page: 1 }).unwrap();
    assert!(
        matches!(read.0, DocumentResult::Text { text, .. } if text == PdfDocument::from_bytes(&plain, None).unwrap().extract_text(&[1]).unwrap())
    );
    assert!(docs.get_mut(&1).unwrap().edit(Edit::Rotate(1, 90)).is_err());
    assert!(docs[&1].prepare_save().is_err());
    let output = temp.path().join("refused.pdf");
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
    assert!(!output.exists());
    assert_eq!(journal.summary().document_count, 0);
    assert_eq!(tree(journal.directory()), before);
    assert_eq!(std::fs::read(path).unwrap(), cipher);
    process_with_journal(&mut docs, Command::Close(1), Some(&mut journal)).unwrap();
    assert!(docs.is_empty());
    assert_eq!(tree(journal.directory()), before);
}

#[test]
fn encrypted_native_note_reports_are_bounded_read_only_and_edit_refusal_preserves_ciphertext() {
    let mut source = PdfDocument::from_bytes(
        include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
        None,
    )
    .unwrap();
    source
        .add_note(
            1,
            [50.0, 50.0, 74.0, 74.0],
            "Encrypted native note Ω",
            "Fixture",
        )
        .unwrap();
    let (cipher, _) = encryption::fixture_from(&source.to_bytes().unwrap(), 6, "reader", false);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &cipher).unwrap();
    let OpenResult::Open(mut document) = WorkingDocument::unlock(
        &path,
        pdf_worker::payload_fingerprint(&cipher),
        "reader".into(),
    )
    .unwrap() else {
        panic!("authenticated");
    };
    let (entries, _) = document.notes(1, 0).unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.note.contents == "Encrypted native note Ω")
    );
    assert!(
        entries
            .iter()
            .all(|entry| !entry.editable && entry.refusal_reason.is_some())
    );
    assert!(document.notes(0, 0).is_err());
    assert!(document.notes(1, 100000).is_err());
    assert!(document.edit(Edit::Rotate(1, 90)).is_err());
    assert_eq!(document.current.bytes, cipher);
    assert_eq!(std::fs::read(path).unwrap(), cipher);
}

#[test]
fn encrypted_opening_rejects_stale_source_ids_and_capacity_without_replacing_tabs() {
    let temp = tempfile::tempdir().unwrap();
    let (cipher, _) = encryption::fixture(3, "reader", false);
    let path = temp.path().join("encrypted.pdf");
    std::fs::write(&path, &cipher).unwrap();
    let fingerprint = pdf_worker::payload_fingerprint(&cipher);
    let mut docs = HashMap::new();
    std::fs::write(&path, encryption::fixture(3, "replacement", false).0).unwrap();
    assert!(WorkingDocument::unlock(&path, fingerprint, "reader".into()).is_err());
    assert!(docs.is_empty());
    std::fs::write(&path, &cipher).unwrap();
    for id in [0, u64::MAX] {
        assert!(
            process(
                &mut docs,
                Command::Unlock {
                    id,
                    path: path.clone(),
                    fingerprint,
                    password: "reader".into()
                }
            )
            .is_err()
        );
    }
    let OpenResult::Open(document) =
        WorkingDocument::unlock(&path, fingerprint, "reader".into()).unwrap()
    else {
        panic!("unlocked");
    };
    docs.insert(1, document);
    assert!(
        process(
            &mut docs,
            Command::Unlock {
                id: 1,
                path: path.clone(),
                fingerprint,
                password: "reader".into()
            }
        )
        .is_err()
    );
    assert_eq!(docs[&1].current.bytes, cipher);
    for id in 2..=MAX_TABS as u64 {
        let OpenResult::Open(document) =
            WorkingDocument::unlock(&path, fingerprint, "reader".into()).unwrap()
        else {
            panic!("unlocked");
        };
        docs.insert(id, document);
    }
    assert!(
        process(
            &mut docs,
            Command::Unlock {
                id: 99,
                path,
                fingerprint,
                password: "reader".into()
            }
        )
        .is_err()
    );
    assert_eq!(docs.len(), MAX_TABS);
}

#[test]
fn session_only_opening_works_without_recovery_while_ordinary_load_stays_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let (cipher, plain) = encryption::fixture(3, "reader", false);
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &cipher).unwrap();
    let ordinary = temp.path().join("ordinary.pdf");
    std::fs::write(&ordinary, plain).unwrap();
    let mut docs = HashMap::new();
    let mut recovery = Err("simulated unavailable recovery".into());
    let Event::PasswordRequired { fingerprint, .. } = process_durable(
        &mut docs,
        &mut recovery,
        Command::Load {
            id: 1,
            path: path.clone(),
        },
    )
    .unwrap() else {
        panic!("challenge");
    };
    process_durable(
        &mut docs,
        &mut recovery,
        Command::Unlock {
            id: 1,
            path,
            fingerprint,
            password: "reader".into(),
        },
    )
    .unwrap();
    assert!(docs[&1].session_only());
    assert!(
        process_durable(
            &mut docs,
            &mut recovery,
            Command::Load {
                id: 2,
                path: ordinary
            }
        )
        .is_err()
    );
    assert_eq!(docs.len(), 1);
    process_durable(&mut docs, &mut recovery, Command::Close(1)).unwrap();
    assert!(docs.is_empty());
}

#[test]
fn timeout_or_crashed_child_leaves_encrypted_source_closed_and_retry_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    let (cipher, _) = encryption::fixture(3, "reader", false);
    let path = temp.path().join("encrypted.pdf");
    std::fs::write(&path, &cipher).unwrap();
    let fingerprint = pdf_worker::payload_fingerprint(&cipher);
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
    assert!(
        helper.is_file(),
        "build pdf-worker --features test-helper first"
    );
    for mode in ["abort", "timeout", "malformed"] {
        let limits = pdf_worker::Limits::for_test(
            pdf_worker::MEMORY_BYTES,
            pdf_worker::CPU_SECONDS,
            std::time::Duration::from_millis(500),
        )
        .unwrap();
        document::with_test_executor(helper.clone(), vec![mode.into()], limits, || {
            let error = match WorkingDocument::unlock(&path, fingerprint, "reader".into()) {
                Err(error) => error,
                _ => panic!("fault must refuse"),
            };
            assert!(!error.contains("reader"));
        });
        assert_eq!(std::fs::read(&path).unwrap(), cipher);
    }
    assert!(matches!(
        WorkingDocument::unlock(&path, fingerprint, "reader".into()).unwrap(),
        OpenResult::Open(_)
    ));
    assert!(WorkingDocument::unlock(&path, fingerprint, "x".repeat(4097)).is_err());
}

#[test]
#[ignore = "requires freshly built native binary and pinned qpdf12.4.2/Poppler26.10"]
fn production_native_child_matches_independent_decrypt_geometry_and_text_without_password_arguments()
 {
    use std::io::Write;
    use std::process::{Command as Process, Stdio};
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
    let qpdf = PathBuf::from(std::env::var_os("PDF_QPDF_BIN").expect("pinned qpdf"));
    let poppler = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("pinned Poppler"));
    assert!(
        String::from_utf8(
            Process::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .unwrap()
        .contains("qpdf version 12.4.2")
    );
    assert!(
        String::from_utf8_lossy(
            &Process::new(poppler.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .lines()
        .any(|line| line == "pdftotext version 26.10.0")
    );
    let temp = tempfile::tempdir().unwrap();
    for revision in [2, 3, 4, 6] {
        let (cipher, plain) = encryption::fixture(revision, "synthetic-reader", false);
        let source = temp.path().join(format!("r{revision}-source.pdf"));
        let clear = temp.path().join(format!("r{revision}-reference-clear.pdf"));
        std::fs::write(&source, &cipher).unwrap();
        // Public synthetic reference data only. The reference tool reads its
        // test password through stdin, with no credential file or argv value.
        let mut child = Process::new(&qpdf)
            .args(["--password-file=-", "--decrypt"])
            .arg(&source)
            .arg(&clear)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"synthetic-reader\n")
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        let reference = std::fs::read(&clear).unwrap();
        let expected = PdfDocument::from_bytes(&plain, None).unwrap();
        let actual = PdfDocument::from_bytes(&reference, None).unwrap();
        assert_eq!(actual.info().unwrap().pages, expected.info().unwrap().pages);
        assert_eq!(
            actual.extract_text(&[1]).unwrap(),
            expected.extract_text(&[1]).unwrap()
        );
        let external = Process::new(poppler.join("pdftotext"))
            .args(["-enc", "UTF-8"])
            .arg(&clear)
            .arg("-")
            .output()
            .unwrap();
        assert!(external.status.success());
        let external = String::from_utf8(external.stdout).unwrap();
        assert_eq!(
            external.split_whitespace().collect::<Vec<_>>(),
            expected
                .extract_text(&[1])
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>()
        );
        document::with_test_executor(
            native.clone(),
            vec!["--pdf-worker".into()],
            pdf_worker::Limits::default(),
            || {
                let fingerprint = pdf_worker::payload_fingerprint(&cipher);
                let OpenResult::Open(document) =
                    WorkingDocument::unlock(&source, fingerprint, "synthetic-reader".into())
                        .unwrap()
                else {
                    panic!("authenticated native child");
                };
                let (result, reply) = document
                    .read(ReadOperation::Render {
                        page: 0,
                        scale: 1.0,
                        logical_zoom: 1.0,
                    })
                    .unwrap();
                assert!(matches!(result, DocumentResult::Render { .. }));
                assert_eq!(
                    reply.payload,
                    pdf_render::render_page(&plain, 0, 1.0).unwrap().rgba
                );
                assert!(document.prepare_save().is_err());
            },
        );
        assert_eq!(std::fs::read(source).unwrap(), cipher);
    }
}
