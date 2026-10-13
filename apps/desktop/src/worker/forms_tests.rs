#![allow(clippy::unwrap_used)]
use super::*;
use pdf_core::{ButtonChange, ButtonSelection, FormFieldKind, PdfDocument};

#[test]
fn real_form_read_fill_save_and_durable_undo_preserve_original_and_other_fields() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("forms.pdf");
    let original = include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf");
    std::fs::write(&path, original).unwrap();
    let journal_path = directory.path().join("journal");
    let mut journal = Journal::open(&journal_path).unwrap();
    let mut documents = HashMap::new();
    process_with_journal(
        &mut documents,
        Command::Load {
            id: 1,
            path: path.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let key = FormsReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    let Event::Forms { fields, .. } =
        process_with_journal(&mut documents, Command::Forms(key), Some(&mut journal)).unwrap()
    else {
        panic!("forms response");
    };
    assert!(
        fields
            .iter()
            .any(|field| field.kind == FormFieldKind::Text && !field.writable_button)
    );
    assert!(
        fields
            .iter()
            .any(|field| field.kind == FormFieldKind::Choice && !field.writable_button)
    );
    let checkbox = fields
        .iter()
        .find(|field| field.kind == FormFieldKind::CheckBox && field.writable_button)
        .unwrap();
    let change = ButtonChange {
        name: checkbox.name.clone(),
        selection: ButtonSelection::On {
            widget: checkbox.widgets[0].object_id,
        },
    };
    let radio = fields
        .iter()
        .find(|field| field.kind == FormFieldKind::RadioButton && field.writable_button)
        .unwrap();
    let radio_widget = radio
        .widgets
        .iter()
        .find(|widget| widget.appearance_state != widget.on_state)
        .unwrap();
    let radio_change = ButtonChange {
        name: radio.name.clone(),
        selection: ButtonSelection::On {
            widget: radio_widget.object_id,
        },
    };
    let Event::Updated {
        selected_page,
        status,
        ..
    } = process_with_journal(
        &mut documents,
        Command::Edit {
            id: 1,
            edit: Edit::FillButtons {
                key,
                changes: vec![change.clone(), radio_change],
                page: 1,
            },
        },
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("fill response");
    };
    assert_eq!(selected_page, Some(0));
    assert!(status.dirty && status.can_undo);
    let edited = documents[&1].current.bytes.clone();
    let after = PdfDocument::from_bytes(&edited, None)
        .unwrap()
        .list_form_fields()
        .unwrap();
    let filled = after
        .iter()
        .find(|field| field.name == checkbox.name)
        .unwrap();
    assert_eq!(
        filled.widgets[0].appearance_state,
        filled.widgets[0].on_state
    );
    let filled_radio = after.iter().find(|field| field.name == radio.name).unwrap();
    let selected_radio = filled_radio
        .widgets
        .iter()
        .find(|widget| widget.object_id == radio_widget.object_id)
        .unwrap();
    assert_eq!(selected_radio.appearance_state, selected_radio.on_state);
    for field in fields
        .iter()
        .filter(|field| field.name != checkbox.name && field.name != radio.name)
    {
        assert_eq!(
            after
                .iter()
                .find(|candidate| candidate.object_id == field.object_id)
                .unwrap(),
            field
        );
    }
    for (edit, bytes) in [
        (Edit::Undo, original.as_slice()),
        (Edit::Redo, edited.as_slice()),
    ] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(documents[&1].current.bytes, bytes);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, bytes);
    }
    let stale = Command::Edit {
        id: 1,
        edit: Edit::FillButtons {
            key,
            changes: vec![change],
            page: 1,
        },
    };
    assert!(process_with_journal(&mut documents, stale, Some(&mut journal)).is_err());
    assert_eq!(documents[&1].current.bytes, edited);
    let output = directory.path().join("filled.pdf");
    process_with_journal(
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert_eq!(
        PdfDocument::open(output, None)
            .unwrap()
            .list_form_fields()
            .unwrap(),
        after
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
    drop(journal);
    assert_eq!(
        Journal::open(journal_path).unwrap().recover().unwrap()[0]
            .current
            .bytes,
        edited
    );
}

#[test]
fn invalid_cross_tab_and_pending_save_form_edits_never_change_committed_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("forms.pdf");
    std::fs::write(
        &path,
        include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
    )
    .unwrap();
    let mut documents = HashMap::new();
    let mut journal = Journal::open(directory.path().join("journal")).unwrap();
    process_with_journal(
        &mut documents,
        Command::Load { id: 1, path },
        Some(&mut journal),
    )
    .unwrap();
    let original = documents[&1].current.bytes.clone();
    let change = ButtonChange {
        name: "not a real field".into(),
        selection: ButtonSelection::Off,
    };
    for key in [
        FormsReadKey {
            tab: 2,
            revision: 0,
            generation: 1,
        },
        FormsReadKey {
            tab: 1,
            revision: 1,
            generation: 1,
        },
        FormsReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        },
    ] {
        assert!(
            process_with_journal(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::FillButtons {
                        key,
                        changes: vec![change.clone()],
                        page: 1
                    }
                },
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(documents[&1].current.bytes, original);
        assert!(documents[&1].undo.is_empty());
    }
    let artifact = documents[&1].prepare_save().unwrap();
    journal
        .begin_save("1", 0, &directory.path().join("pending.pdf"), &artifact)
        .unwrap();
    assert!(
        process_with_journal(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::FillButtons {
                    key: FormsReadKey {
                        tab: 1,
                        revision: 0,
                        generation: 2
                    },
                    changes: vec![change],
                    page: 1
                }
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(journal.recover().unwrap()[0].current.bytes, original);
    assert_eq!(journal.save_intents().len(), 1);
}
