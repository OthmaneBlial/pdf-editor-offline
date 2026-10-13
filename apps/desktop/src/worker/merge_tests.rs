#![allow(clippy::unwrap_used)]
use super::*;
use lopdf::{Document, Object, Stream, dictionary};
fn fixture(text: &str, rotation: i64) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 20 Tf 30 120 Td ({text}) Tj ET").into_bytes(),
    ));
    let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>content});
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)],"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}.into());
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn load(temp: &std::path::Path) -> (HashMap<u64, WorkingDocument>, Journal, PathBuf, PathBuf) {
    let a = temp.join("source.pdf");
    let b = temp.join("additional.pdf");
    std::fs::write(&a, fixture("ALPHA", 0)).unwrap();
    std::fs::write(&b, fixture("BETA", 90)).unwrap();
    let mut journal = Journal::open(temp.join("journal")).unwrap();
    let mut docs = HashMap::new();
    process_with_journal(
        &mut docs,
        Command::Load {
            id: 1,
            path: a.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    (docs, journal, a, b)
}
fn command(path: &std::path::Path, revision: u64) -> Command {
    Command::Merge {
        id: 1,
        new_id: 2,
        revision,
        paths: vec![path.to_path_buf()],
    }
}
#[test]
fn merge_child_preserves_sources_and_creates_durable_unsaved_copy_saved_and_reopened() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, a, b) = load(temp.path());
    let source = std::fs::read(&a).unwrap();
    let additional = std::fs::read(&b).unwrap();
    process_with_journal(
        &mut docs,
        Command::Edit {
            id: 1,
            edit: Edit::Rotate(1, 90),
        },
        Some(&mut journal),
    )
    .unwrap();
    let current = docs[&1].current.bytes.clone();
    let undo = docs[&1].undo[0].bytes.clone();
    let Event::Merged { id, status, .. } =
        process_with_journal(&mut docs, command(&b, 1), Some(&mut journal)).unwrap()
    else {
        panic!("merged");
    };
    assert_eq!(id, 2);
    assert!(status.dirty);
    assert_eq!(status.info.page_count, 2);
    assert!(!status.can_undo);
    assert_eq!(docs[&1].current.bytes, current);
    assert_eq!(docs[&1].undo[0].bytes, undo);
    assert_eq!(docs[&1].current.revision, 1);
    assert_eq!(docs[&1].saved_revision, Some(0));
    assert_eq!(journal.recover().unwrap().len(), 2);
    let merged = pdf_core::PdfDocument::from_bytes(&docs[&2].current.bytes, None).unwrap();
    assert!(merged.extract_text(&[1]).unwrap().contains("ALPHA"));
    assert!(merged.extract_text(&[2]).unwrap().contains("BETA"));
    assert_eq!(merged.info().unwrap().pages[1].rotation, 90);
    for (index, input) in [&current, &additional].into_iter().enumerate() {
        let before = pdf_render::render_page(input, 0, 1.0).unwrap();
        let after = pdf_render::render_page(&docs[&2].current.bytes, index, 1.0).unwrap();
        assert_eq!((before.width, before.height), (after.width, after.height));
        assert_eq!(before.rgba, after.rgba);
    }
    let output = temp.path().join("new.pdf");
    process_with_journal(
        &mut docs,
        Command::Save {
            id: 2,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(
        pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .page_count(),
        2
    );
    assert_eq!(std::fs::read(a).unwrap(), source);
    assert_eq!(std::fs::read(b).unwrap(), additional);
    let published = std::fs::read(&output).unwrap();
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
    assert_eq!(std::fs::read(output).unwrap(), published);
    assert!(journal.save_intents().is_empty());
}
#[test]
fn stale_collision_unsupported_input_and_journal_failure_leave_original_committed_state() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _, b) = load(temp.path());
    let source = docs[&1].current.bytes.clone();
    assert!(process_with_journal(&mut docs, command(&b, 9), Some(&mut journal)).is_err());
    assert!(
        process_with_journal(
            &mut docs,
            Command::Merge {
                id: 1,
                new_id: 1,
                revision: 0,
                paths: vec![b.clone()]
            },
            Some(&mut journal)
        )
        .is_err()
    );
    let advanced = temp.path().join("forms.pdf");
    std::fs::write(
        &advanced,
        include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
    )
    .unwrap();
    assert!(process_with_journal(&mut docs, command(&advanced, 0), Some(&mut journal)).is_err());
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"block").unwrap();
    assert!(process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).is_err());
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, source);
    assert_eq!(journal.recover().unwrap().len(), 1);
}

#[test]
fn limits_readonly_sources_and_pending_saves_refuse_without_new_document() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _, b) = load(temp.path());
    let before = docs[&1].current.bytes.clone();
    docs.get_mut(&1).unwrap().current.info.signature_fields = 1;
    assert!(process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).is_err());
    docs.get_mut(&1).unwrap().current.info.signature_fields = 0;
    docs.get_mut(&1).unwrap().current.info.encrypted = true;
    assert!(process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).is_err());
    docs.get_mut(&1).unwrap().current.info.encrypted = false;
    let huge = temp.path().join("huge.pdf");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(pdf_core::MAX_INPUT_BYTES as u64)
        .unwrap();
    assert!(process_with_journal(&mut docs, command(&huge, 0), Some(&mut journal)).is_err());
    assert!(process_with_journal(&mut docs, command(temp.path(), 0), Some(&mut journal)).is_err());
    journal
        .begin_save("1", 0, &temp.path().join("pending.pdf"), &before)
        .unwrap();
    assert!(process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).is_err());
    assert_eq!(journal.save_intents().len(), 1);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[&1].current.bytes, before);
    let token = journal.save_intents()[0].token.clone();
    journal.cancel_save(&token).unwrap();
    for id in 2..=MAX_TABS as u64 {
        docs.insert(
            id,
            WorkingDocument {
                current: document::Snapshot {
                    bytes: before.clone(),
                    info: docs[&1].current.info.clone(),
                    revision: 0,
                },
                undo: vec![],
                redo: vec![],
                saved_revision: None,
                next_revision: 1,
                path: None,
                password: None,
            },
        );
    }
    assert!(
        process_with_journal(
            &mut docs,
            Command::Merge {
                id: 1,
                new_id: 99,
                revision: 0,
                paths: vec![b]
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(docs.len(), MAX_TABS);
    assert_eq!(journal.recover().unwrap().len(), 1);
}

#[test]
#[ignore = "build test helper before explicit fault-preservation gate"]
fn crashed_timed_out_and_corrupt_merge_children_cannot_commit() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, _, b) = load(temp.path());
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
                process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).is_err(),
                "{mode}"
            )
        });
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[&1].current.bytes, before);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, before);
    }
}

#[test]
#[ignore = "requires normal native executable plus pinned qpdf12.4.2 and Poppler26.10"]
fn production_native_merge_and_saved_copy_pass_independent_text_pixels_geometry_checks() {
    let temp = tempfile::tempdir().unwrap();
    let (mut docs, mut journal, a, b) = load(temp.path());
    let left = std::fs::read(&a).unwrap();
    let right = std::fs::read(&b).unwrap();
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
            process_with_journal(&mut docs, command(&b, 0), Some(&mut journal)).unwrap();
            process_with_journal(
                &mut docs,
                Command::Save {
                    id: 2,
                    path: temp.path().join("merged.pdf"),
                },
                Some(&mut journal),
            )
            .unwrap();
        },
    );
    let output = temp.path().join("merged.pdf");
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        std::process::Command::new(qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    for (index, path) in [&a, &b].into_iter().enumerate() {
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
        assert_eq!(
            reference("pdftotext", path, 1),
            reference("pdftotext", &output, index + 1)
        );
        assert_eq!(
            reference("pdftoppm", path, 1),
            reference("pdftoppm", &output, index + 1)
        );
        let before = pdf_core::PdfDocument::open(path, None)
            .unwrap()
            .info()
            .unwrap()
            .pages[0]
            .clone();
        let mut after = pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .info()
            .unwrap()
            .pages[index]
            .clone();
        after.number = 1;
        assert_eq!(before, after);
    }
    assert_eq!(std::fs::read(a).unwrap(), left);
    assert_eq!(std::fs::read(b).unwrap(), right);
    assert_eq!(journal.recover().unwrap().len(), 2);
}
