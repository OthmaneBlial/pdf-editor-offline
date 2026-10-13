#![allow(clippy::unwrap_used)]

use super::*;
use lopdf::{Document, Stream, dictionary};
use pdf_core::PdfDocument;

fn fixture(directory: &std::path::Path, advanced: bool) -> PathBuf {
    let path = directory.join("source.pdf");
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources = pdf.add_object(dictionary! {"Font"=>dictionary! {"F1"=>font}});
    let mut kids = Vec::new();
    for number in 1..=4 {
        let content = pdf.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 20 30 Td (Page{number}) Tj ET").into_bytes(),
        ));
        let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>content});
        kids.push(page.into());
    }
    pdf.objects.insert(pages, dictionary! {"Type"=>"Pages","Count"=>4,"Kids"=>kids,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"CropBox"=>vec![10.into(),20.into(),180.into(),280.into()],"Rotate"=>90,"Resources"=>resources}.into());
    let mut catalog = dictionary! {"Type"=>"Catalog","Pages"=>pages};
    if advanced {
        let outlines = pdf.add_object(dictionary! {"Type"=>"Outlines","Count"=>0});
        catalog.set("Outlines", outlines);
    }
    let root = pdf.add_object(catalog);
    pdf.trailer.set("Root", root);
    pdf.save(&path).unwrap();
    path
}

fn load(
    documents: &mut HashMap<u64, WorkingDocument>,
    journal: &mut Journal,
    path: &std::path::Path,
) {
    process_with_journal(
        documents,
        Command::Load {
            id: 1,
            path: path.to_owned(),
        },
        Some(journal),
    )
    .unwrap();
}
fn extract(revision: u64, pages: Vec<u32>) -> Command {
    Command::ExtractPages {
        id: 1,
        new_id: 2,
        revision,
        pages,
    }
}
fn delete(revision: u64, pages: Vec<u32>, page: u32) -> Command {
    Command::Edit {
        id: 1,
        edit: Edit::DeletePages {
            revision,
            pages,
            page,
        },
    }
}
fn duplicate(revision: u64, pages: Vec<u32>, page: u32) -> Command {
    Command::Edit {
        id: 1,
        edit: Edit::DuplicatePages {
            revision,
            pages,
            page,
        },
    }
}
fn text(document: &WorkingDocument) -> String {
    PdfDocument::from_bytes(&document.current.bytes, None)
        .unwrap()
        .extract_text(&(1..=document.current.info.page_count as u32).collect::<Vec<_>>())
        .unwrap()
}

#[test]
fn extraction_preserves_source_and_journals_ordered_unsaved_copy_with_inherited_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let original = std::fs::read(&source).unwrap();
    let journal_path = dir.path().join("journal");
    let mut journal = Journal::open(&journal_path).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let event =
        process_with_journal(&mut documents, extract(0, vec![4, 2]), Some(&mut journal)).unwrap();
    let Event::Extracted { id, path, status } = event else {
        panic!("expected new tab")
    };
    assert_eq!(id, 2);
    assert!(path.ends_with("source — extracted.pdf"));
    assert!(status.dirty);
    assert_eq!(status.revision, 0);
    assert!(!status.can_undo && !status.can_redo);
    assert_eq!(status.info.page_count, 2);
    assert!(documents[&2].path.is_none());
    let extracted_text = text(&documents[&2]);
    assert!(extracted_text.find("Page4").unwrap() < extracted_text.find("Page2").unwrap());
    assert!(!extracted_text.contains("Page1") && !extracted_text.contains("Page3"));
    for page in &status.info.pages {
        assert_eq!(page.media_box, [0.0, 0.0, 200.0, 300.0]);
        assert_eq!(page.crop_box, [10.0, 20.0, 180.0, 280.0]);
        assert_eq!(page.rotation, 90);
    }
    assert_eq!(documents[&1].current.bytes, original);
    assert_eq!(documents[&1].current.revision, 0);
    assert!(documents[&1].undo.is_empty());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    drop(journal);
    let recovered = Journal::open(journal_path).unwrap().recover().unwrap();
    assert_eq!(recovered.len(), 2);
    let copy = recovered
        .iter()
        .find(|document| document.id == "2")
        .unwrap();
    assert_eq!(copy.current.bytes, documents[&2].current.bytes);
    assert_eq!(copy.saved_revision, None);
    assert!(copy.path.is_none());
}

#[test]
fn deletion_preserves_focus_and_durable_undo_redo_without_overwriting_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let original = std::fs::read(&source).unwrap();
    let mut journal = Journal::open(dir.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let event =
        process_with_journal(&mut documents, delete(0, vec![3, 1], 3), Some(&mut journal)).unwrap();
    let Event::Updated {
        status,
        selected_page,
        ..
    } = event
    else {
        panic!("expected mutation")
    };
    assert_eq!(selected_page, Some(1)); // Removed current page3 selects surviving original page4.
    assert!(status.dirty && status.can_undo);
    assert_eq!(status.info.page_count, 2);
    let remaining = text(&documents[&1]);
    assert!(remaining.contains("Page2") && remaining.contains("Page4"));
    assert!(!remaining.contains("Page1") && !remaining.contains("Page3"));
    let edited = documents[&1].current.bytes.clone();
    // A dialog captured before deletion must not apply its old page numbers to the new revision.
    for command in [extract(0, vec![2]), delete(0, vec![2], 2)] {
        assert!(
            process_with_journal(&mut documents, command, Some(&mut journal))
                .err()
                .unwrap()
                .contains("stale")
        );
    }
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[&1].current.bytes, edited);
    assert_eq!(documents[&1].undo.len(), 1);
    process_with_journal(
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::Undo,
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(documents[&1].current.bytes, original);
    assert_eq!(journal.recover().unwrap()[0].current.bytes, original);
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
    assert_eq!(journal.recover().unwrap()[0].current.bytes, edited);
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn duplication_preserves_original_order_focus_source_and_durable_undo_redo() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let original = std::fs::read(&source).unwrap();
    let journal_path = dir.path().join("journal");
    let mut journal = Journal::open(&journal_path).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let event = process_with_journal(
        &mut documents,
        duplicate(0, vec![3, 1], 3),
        Some(&mut journal),
    )
    .unwrap();
    let Event::Updated {
        status,
        selected_page,
        ..
    } = event
    else {
        panic!("expected duplicate edit");
    };
    assert_eq!(selected_page, Some(3)); // Focus original page3, after page1 and its copy, then page2.
    assert_eq!(status.info.page_count, 6);
    assert!(status.dirty && status.can_undo);
    let pdf = PdfDocument::from_bytes(&documents[&1].current.bytes, None).unwrap();
    for (page, expected) in ["Page1", "Page1", "Page2", "Page3", "Page3", "Page4"]
        .into_iter()
        .enumerate()
    {
        assert!(
            pdf.extract_text(&[page as u32 + 1])
                .unwrap()
                .contains(expected)
        );
        assert_eq!(status.info.pages[page].crop_box, [10.0, 20.0, 180.0, 280.0]);
        assert_eq!(status.info.pages[page].rotation, 90);
    }
    let edited = documents[&1].current.bytes.clone();
    let durable = journal.recover().unwrap();
    assert_eq!(durable[0].current.bytes, edited);
    assert_eq!(durable[0].undo[0].bytes, original);
    assert!(
        process_with_journal(&mut documents, duplicate(0, vec![2], 2), Some(&mut journal))
            .err()
            .unwrap()
            .contains("stale")
    );
    assert_eq!(documents[&1].current.bytes, edited);
    for (edit, expected) in [(Edit::Undo, &original), (Edit::Redo, &edited)] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(&documents[&1].current.bytes, expected);
        assert_eq!(&journal.recover().unwrap()[0].current.bytes, expected);
    }
    drop(journal);
    assert_eq!(
        Journal::open(journal_path).unwrap().recover().unwrap()[0]
            .current
            .bytes,
        edited
    );
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn invalid_or_stale_selections_and_advanced_catalogs_leave_all_state_unchanged() {
    for advanced in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let source = fixture(dir.path(), advanced);
        let mut journal = Journal::open(dir.path().join("journal")).unwrap();
        let mut documents = HashMap::new();
        load(&mut documents, &mut journal, &source);
        let original = documents[&1].current.bytes.clone();
        let mut commands = vec![
            extract(1, vec![1]),
            extract(0, vec![]),
            extract(0, vec![0]),
            extract(0, vec![5]),
            extract(0, vec![1, 1]),
            delete(1, vec![1], 1),
            delete(0, vec![1, 2, 3, 4], 1),
            delete(0, vec![1], 0),
            duplicate(1, vec![1], 1),
            duplicate(0, vec![], 1),
            duplicate(0, vec![0], 1),
            duplicate(0, vec![5], 1),
            duplicate(0, vec![1, 1], 1),
            duplicate(0, vec![1], 0),
        ];
        if advanced {
            commands.extend([
                extract(0, vec![1]),
                delete(0, vec![1], 1),
                duplicate(0, vec![1], 1),
            ]);
        }
        for command in commands {
            assert!(process_with_journal(&mut documents, command, Some(&mut journal)).is_err());
            assert_eq!(documents.len(), 1);
            assert_eq!(documents[&1].current.bytes, original);
            assert_eq!(documents[&1].current.revision, 0);
            assert_eq!(documents[&1].next_revision, 1);
            assert!(documents[&1].undo.is_empty() && documents[&1].redo.is_empty());
            assert_eq!(journal.recover().unwrap().len(), 1);
            assert_eq!(journal.recover().unwrap()[0].current.bytes, original);
        }
    }
}

#[test]
fn journal_publication_failure_never_inserts_copy_or_commits_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let mut journal = Journal::open(dir.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let original = documents[&1].current.bytes.clone();
    let blocker = journal.directory().join("manifest.json.tmp");
    std::fs::write(&blocker, b"publication blocker").unwrap();
    for command in [
        extract(0, vec![2]),
        delete(0, vec![2], 2),
        duplicate(0, vec![2], 2),
    ] {
        assert!(process_with_journal(&mut documents, command, Some(&mut journal)).is_err());
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[&1].current.bytes, original);
        assert_eq!(documents[&1].current.revision, 0);
        assert!(documents[&1].undo.is_empty());
    }
    drop(journal);
    std::fs::remove_file(blocker).unwrap();
    let restored = Journal::open(dir.path().join("journal"))
        .unwrap()
        .recover()
        .unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].current.bytes, original);
}

#[test]
fn child_failure_and_new_id_collision_do_not_change_source_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let mut journal = Journal::open(dir.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let original = documents[&1].current.bytes.clone();
    let collided = Command::ExtractPages {
        id: 1,
        new_id: 1,
        revision: 0,
        pages: vec![1],
    };
    assert!(process_with_journal(&mut documents, collided, Some(&mut journal)).is_err());
    document::with_test_executable(dir.path().join("missing-worker"), || {
        for command in [
            extract(0, vec![2]),
            delete(0, vec![2], 2),
            duplicate(0, vec![2], 2),
        ] {
            assert!(process_with_journal(&mut documents, command, Some(&mut journal)).is_err());
        }
    });
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[&1].current.bytes, original);
    assert!(documents[&1].undo.is_empty());
    assert_eq!(journal.recover().unwrap()[0].current.bytes, original);
}

#[test]
fn extraction_respects_tab_capacity_and_pending_save_remains_bound_to_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path(), false);
    let mut journal = Journal::open(dir.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    load(&mut documents, &mut journal, &source);
    let artifact = documents[&1].prepare_save().unwrap();
    let intent = journal
        .begin_save("1", 0, &dir.path().join("future.pdf"), &artifact)
        .unwrap();
    assert!(
        process_with_journal(&mut documents, delete(0, vec![1], 1), Some(&mut journal)).is_err()
    );
    assert!(
        process_with_journal(&mut documents, duplicate(0, vec![1], 1), Some(&mut journal)).is_err()
    );
    process_with_journal(&mut documents, extract(0, vec![2]), Some(&mut journal)).unwrap();
    assert_eq!(journal.save_intents(), &[intent]);
    for id in 3..=16 {
        process_with_journal(
            &mut documents,
            Command::Load {
                id,
                path: source.clone(),
            },
            Some(&mut journal),
        )
        .unwrap();
    }
    assert!(
        process_with_journal(
            &mut documents,
            Command::ExtractPages {
                id: 1,
                new_id: 17,
                revision: 0,
                pages: vec![1]
            },
            Some(&mut journal)
        )
        .err()
        .unwrap()
        .contains("16 tabs")
    );
    assert_eq!(documents.len(), 16);
    assert_eq!(journal.recover().unwrap().len(), 16);
}
