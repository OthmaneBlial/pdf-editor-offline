#![allow(clippy::unwrap_used)]
use super::*;
fn exercise() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let font = temp.path().join("font.ttf");
    let source = test_pdf::fixture(4);
    std::fs::write(&input, &source).unwrap();
    std::fs::write(
        &font,
        include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
    )
    .unwrap();
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
    let old = documents[&1].current.bytes.clone();
    let key = FormsReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    let numbering = pdf_core::PageNumbering {
        pages: vec![4, 2, 1, 3],
        start: 7,
        position: pdf_core::NumberPosition::Header,
        margin: 10.0,
        height: 24.0,
        size: 12.0,
        color: [0.0; 3],
        alignment: pdf_core::TextAlignment::Center,
    };
    let edit = |key, numbering| Edit::PageNumbers {
        key,
        numbering,
        page: 2,
        font_file: font.clone(),
    };
    for bad in [
        edit(
            FormsReadKey {
                revision: 99,
                ..key
            },
            numbering.clone(),
        ),
        edit(FormsReadKey { tab: 2, ..key }, numbering.clone()),
        edit(
            key,
            pdf_core::PageNumbering {
                pages: vec![1, 5],
                ..numbering.clone()
            },
        ),
    ] {
        assert!(
            process_with_journal(
                &mut documents,
                Command::Edit { id: 1, edit: bad },
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(documents[&1].current.bytes, old);
        assert!(documents[&1].undo.is_empty());
    }
    assert!(
        documents
            .get_mut(&1)
            .unwrap()
            .edit_durable(edit(key, numbering.clone()), HISTORY_BYTES, "1", |_| Err(
                "forced journal failure".into()
            ))
            .is_err()
    );
    assert_eq!(documents[&1].current.bytes, old);
    let Event::Updated {
        selected_page,
        status,
        ..
    } = process_with_journal(
        &mut documents,
        Command::Edit {
            id: 1,
            edit: edit(key, numbering),
        },
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("updated event")
    };
    assert_eq!(selected_page, Some(1));
    assert!(status.dirty && status.can_undo);
    assert_eq!(documents[&1].undo.len(), 1);
    let changed = documents[&1].current.bytes.clone();
    let pdf = pdf_core::PdfDocument::from_bytes(&changed, None).unwrap();
    for page in 1..=4 {
        assert!(
            pdf.extract_text(&[page])
                .unwrap()
                .contains(&(6 + page).to_string())
        );
    }
    assert_eq!(journal.recover().unwrap()[0].current.bytes, changed);
    for (change, expected) in [(Edit::Undo, &old), (Edit::Redo, &changed)] {
        process_with_journal(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: change,
            },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(&documents[&1].current.bytes, expected);
    }
    let output = temp.path().join("numbered.pdf");
    process_with_journal(
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert!(!documents[&1].status().unwrap().dirty);
    assert_eq!(std::fs::read(&input).unwrap(), source);
    assert_eq!(
        pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .info()
            .unwrap()
            .pages,
        pdf.info().unwrap().pages
    );
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
}
#[test]
fn numbering_batch_is_one_durable_history_edit_with_stale_and_failure_guards() {
    exercise();
}
#[test]
#[ignore = "requires freshly built normal native executable"]
fn production_native_numbering_child_undo_redo_and_save() {
    let binary = std::env::current_exe()
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
    assert!(binary.is_file());
    document::with_test_executor(
        binary,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        exercise,
    );
}
