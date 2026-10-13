#![allow(clippy::unwrap_used)]
use super::*;
#[allow(dead_code)]
#[path = "../../../../tests/support/image_removal_fixture.rs"]
mod support;
pub(crate) mod fixture {
    pub use super::support::{PIXELS, control, selections};
    pub fn fixture() -> Vec<u8> {
        crate::worker::test_pdf::fixture(2)
    }
}
fn command(source: &WorkingDocument, new_id: u64) -> Command {
    Command::RedactImages {
        key: FormsReadKey {
            tab: 1,
            revision: source.current.revision,
            generation: 1,
        },
        new_id,
        selections: fixture::selections(&source.current.bytes),
    }
}
fn loaded(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf, Vec<u8>) {
    let path = temp.join("original.pdf");
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
fn image_copy_is_durable_before_ack_with_source_history_preserved_and_verified_save() {
    copy_and_save(false);
}
fn copy_and_save(reference: bool) {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path, source) = loaded(temp.path());
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
    let cmd = command(&docs[&1], 2);
    let selections = fixture::selections(&original);
    let Event::ImagesRedacted {
        id, status, report, ..
    } = process_with_journal(&mut docs, cmd, Some(&mut journal)).unwrap()
    else {
        panic!("copy")
    };
    assert_eq!(id, 2);
    assert!(status.dirty && !status.can_undo && !status.can_redo);
    assert_eq!(report.removed_draws.len(), 2);
    assert_eq!(docs[&1].current.bytes, original);
    assert_eq!(docs[&1].undo[0].bytes, undo);
    let recovered = journal.recover().unwrap();
    assert_eq!(recovered.len(), 2);
    assert_eq!(
        recovered
            .iter()
            .find(|d| d.id == "2")
            .unwrap()
            .current
            .bytes,
        docs[&2].current.bytes
    );
    PdfDocument::from_bytes(&original, None)
        .unwrap()
        .verify_image_redaction_bytes(&selections, &docs[&2].current.bytes)
        .unwrap();
    let output = temp.path().join("removed.pdf");
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
    PdfDocument::from_bytes(&original, None)
        .unwrap()
        .verify_image_redaction_bytes(&selections, &saved)
        .unwrap();
    assert!(!docs[&2].status().unwrap().dirty);
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
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(path).unwrap(), source);
    if reference {
        prove_copy(temp.path(), &original, &saved);
    }
}
fn prove_copy(temp: &std::path::Path, source: &[u8], output: &[u8]) {
    use std::process::Command as Process;
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        String::from_utf8_lossy(
            &Process::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .contains("12.4.2")
    );
    assert!(
        String::from_utf8_lossy(
            &Process::new(bin.join("pdftoppm"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let control = fixture::control(source);
    let mut paths = Vec::new();
    let mut texts = Vec::new();
    for (name, bytes) in [
        ("source", source),
        ("output", output),
        ("control", control.as_slice()),
    ] {
        let path = temp.join(format!("{name}-proof.pdf"));
        std::fs::write(&path, bytes).unwrap();
        assert!(
            Process::new(&qpdf)
                .arg("--check")
                .arg(&path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let result = Process::new(bin.join("pdftotext"))
            .arg("-bbox")
            .arg(&path)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success());
        texts.push(result.stdout);
        paths.push(path);
    }
    assert_eq!(texts[0], texts[1]);
    assert_eq!(texts[1], texts[2]);
    for page in 0..2 {
        assert!(
            pdf_render::render_page(output, page, 1.0).unwrap().rgba
                == pdf_render::render_page(&control, page, 1.0).unwrap().rgba
        );
        let mut images = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let prefix = temp.join(format!("pixels-{page}-{index}"));
            assert!(
                Process::new(bin.join("pdftoppm"))
                    .args([
                        "-r",
                        "72",
                        "-f",
                        &(page + 1).to_string(),
                        "-l",
                        &(page + 1).to_string(),
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
        assert!(images[0] != images[2]);
        assert!(images[1] == images[2]);
    }
    let qdf = temp.join("output.qdf");
    assert!(
        Process::new(&qpdf)
            .args([
                "--qdf",
                "--object-streams=disable",
                "--stream-data=uncompress"
            ])
            .arg(&paths[1])
            .arg(&qdf)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !std::fs::read(qdf)
            .unwrap()
            .windows(fixture::PIXELS.len())
            .any(|v| v == fixture::PIXELS)
    );
}
#[test]
fn stale_shared_pending_collision_child_journal_and_readonly_failures_are_atomic() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, path, bytes) = loaded(temp.path());
    let original = docs[&1].current.bytes.clone();
    for new_id in [0, 1, u64::MAX] {
        let cmd = command(&docs[&1], new_id);
        assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    }
    let mut stale = command(&docs[&1], 2);
    if let Command::RedactImages { key, .. } = &mut stale {
        key.revision = 1;
    }
    assert!(process_with_journal(&mut docs, stale, Some(&mut journal)).is_err());
    let mut partial = command(&docs[&1], 2);
    if let Command::RedactImages { selections, .. } = &mut partial {
        selections.pop();
    }
    assert!(process_with_journal(&mut docs, partial, Some(&mut journal)).is_err());
    document::with_test_executable(temp.path().join("absent-helper"), || {
        let cmd = command(&docs[&1], 2);
        assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    });
    let intent = journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &original)
        .unwrap();
    let cmd = command(&docs[&1], 2);
    assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    journal.cancel_save(&intent.token).unwrap();
    std::fs::write(journal.directory().join("manifest.json.tmp"), b"occupied").unwrap();
    let cmd = command(&docs[&1], 2);
    assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    std::fs::remove_file(journal.directory().join("manifest.json.tmp")).unwrap();
    assert_eq!(journal.recover().unwrap().len(), 1);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, original);
    assert!(docs[&1].undo.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    docs.get_mut(&1).unwrap().current.info.signature_fields = 1;
    let cmd = command(&docs[&1], 2);
    assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
    docs.get_mut(&1).unwrap().current.info.signature_fields = 0;
    docs.get_mut(&1).unwrap().current.info.encrypted = true;
    let cmd = command(&docs[&1], 2);
    assert!(process_with_journal(&mut docs, cmd, Some(&mut journal)).is_err());
}
#[test]
#[ignore = "requires normal native self-host binary and pinned qpdf/Poppler"]
fn production_native_self_host_image_removal_saved_copy_matches_independent_controls() {
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
