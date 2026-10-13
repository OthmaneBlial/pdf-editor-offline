#![allow(clippy::unwrap_used)]
use super::*;
#[path = "../../../../tests/support/redaction_fixture.rs"]
pub(crate) mod fixture;
fn command(revision: u64, new_id: u64) -> Command {
    Command::RedactText {
        id: 1,
        new_id,
        revision,
        request: pdf_core::TextRedactionRequest {
            targets: vec![fixture::SECRET.into()],
        },
    }
}
fn source(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf, Vec<u8>) {
    let path = temp.join("source.pdf");
    let bytes = fixture::fixture();
    std::fs::write(&path, &bytes).unwrap();
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
    (docs, journal, path, bytes)
}
#[test]
fn actual_child_copy_has_no_original_undo_and_is_durable_before_ack_save_reopens() {
    copy_and_save(false);
}
fn copy_and_save(independent: bool) {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path, source) = source(temp.path());
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
    let event = process_with_journal(&mut docs, command(1, 2), Some(&mut journal)).unwrap();
    let Event::Redacted {
        id, status, report, ..
    } = event
    else {
        panic!("redacted")
    };
    assert_eq!(id, 2);
    assert!(status.dirty);
    assert!(!status.can_undo);
    assert!(report.changed_text_shows > 0);
    assert_eq!(docs[&1].current.bytes, original);
    assert_eq!(docs[&1].undo[0].bytes, undo);
    let recovered = journal.recover().unwrap();
    assert_eq!(recovered.len(), 2);
    let clean = &recovered
        .iter()
        .find(|d| d.id == "2")
        .unwrap()
        .current
        .bytes;
    pdf_core::verify_redaction_bytes(clean, None, &[fixture::SECRET.into()]).unwrap();
    assert_eq!(clean, &docs[&2].current.bytes);
    let output = temp.path().join("redacted.pdf");
    process_with_journal(
        &mut docs,
        Command::Save {
            id: 2,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let saved = std::fs::read(&output).unwrap();
    pdf_core::verify_redaction_bytes(&saved, None, &[fixture::SECRET.into()]).unwrap();
    if independent {
        let before = temp.path().join("working-source.pdf");
        std::fs::write(&before, &original).unwrap();
        prove_saved_copy(&before, &output);
    }
    assert!(!docs[&2].status().unwrap().dirty);
    assert_eq!(std::fs::read(path).unwrap(), source);
    assert!(
        process_with_journal(
            &mut docs,
            Command::Save {
                id: 2,
                path: output.clone()
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(output).unwrap(), saved);
}
fn prove_saved_copy(before: &std::path::Path, after: &std::path::Path) {
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let version = std::process::Command::new(&qpdf)
        .arg("--version")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let version = std::process::Command::new(bin.join("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
    let mut boxes = Vec::new();
    for path in [before, after] {
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--check")
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let qdf = path.with_extension("qdf");
        assert!(
            std::process::Command::new(&qpdf)
                .args(["--qdf", "--object-streams=disable"])
                .arg(path)
                .arg(&qdf)
                .output()
                .unwrap()
                .status
                .success()
        );
        let bytes = std::fs::read(qdf).unwrap();
        assert_eq!(
            bytes
                .windows(fixture::SECRET.len())
                .any(|v| v == fixture::SECRET.as_bytes()),
            path == before
        );
        let text = std::process::Command::new(bin.join("pdftotext"))
            .arg("-bbox")
            .arg(path)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let text = String::from_utf8(text.stdout).unwrap();
        assert_eq!(text.contains(fixture::SECRET), path == before);
        boxes.push(
            text.lines()
                .filter(|line| line.contains(">prefix</word>") || line.contains(">suffix</word>"))
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(boxes[0].len(), 4);
    assert_eq!(boxes[0], boxes[1]);
    for page in 1..=2 {
        let mut images = Vec::new();
        for (label, path) in [("before", before), ("after", after)] {
            let prefix = after.parent().unwrap().join(format!("{label}-{page}"));
            assert!(
                std::process::Command::new(bin.join("pdftoppm"))
                    .args([
                        "-r",
                        "72",
                        "-f",
                        &page.to_string(),
                        "-l",
                        &page.to_string(),
                        "-singlefile"
                    ])
                    .arg(path)
                    .arg(&prefix)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
        }
        assert!(
            images[0] != images[1],
            "removed glyphs must change actual Poppler pixels"
        );
    }
}
#[test]
fn stale_colliding_readonly_pending_child_and_journal_failures_preserve_all_source_state() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path, bytes) = source(temp.path());
    let original = docs[&1].current.bytes.clone();
    for cmd in [command(1, 2), command(0, 1), command(0, 0)] {
        assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    }
    document::with_test_executable(temp.path().join("missing-helper"), || {
        assert!(process_with_journal(&mut docs, command(0, 2), Some(&mut journal)).is_err())
    });
    journal
        .begin_save(
            "1",
            0,
            &temp.path().join("pending.pdf"),
            &docs[&1].current.bytes,
        )
        .unwrap();
    assert!(process_with_journal(&mut docs, command(0, 2), Some(&mut journal)).is_err());
    let token = journal.save_intents()[0].token.clone();
    journal.cancel_save(&token).unwrap();
    std::fs::write(
        journal.directory().join("manifest.json.tmp"),
        b"block publication",
    )
    .unwrap();
    assert!(process_with_journal(&mut docs, command(0, 2), Some(&mut journal)).is_err());
    std::fs::remove_file(journal.directory().join("manifest.json.tmp")).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, original);
    assert!(docs[&1].undo.is_empty());
    assert_eq!(journal.recover().unwrap().len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    docs.get_mut(&1).unwrap().current.info.signature_fields = 1;
    assert!(process_with_journal(&mut docs, command(0, 2), Some(&mut journal)).is_err());
    docs.get_mut(&1).unwrap().current.info.signature_fields = 0;
    docs.get_mut(&1).unwrap().current.info.encrypted = true;
    assert!(process_with_journal(&mut docs, command(0, 2), Some(&mut journal)).is_err());
    assert_eq!(docs.len(), 1);
}
#[test]
#[ignore = "build normal native binary and provide pinned qpdf/Poppler before the production self-host gate"]
fn production_native_self_host_redaction_copy_and_verified_save() {
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
    document::with_test_executor(
        native,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || copy_and_save(true),
    );
}
