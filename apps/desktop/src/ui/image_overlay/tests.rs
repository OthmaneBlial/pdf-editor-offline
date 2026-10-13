#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::{DocumentStatus, Tab};
use crate::worker::test_pdf as support;
pub(super) fn editor(context: &egui::Context) -> EditorApp {
    let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
    let info = pdf_core::PdfDocument::from_bytes(&support::fixture(2), None)
        .unwrap()
        .info()
        .unwrap();
    app.tabs.push(Tab::new(
        1,
        "source.pdf".into(),
        DocumentStatus {
            info,
            revision: 0,
            dirty: false,
            can_undo: false,
            can_redo: false,
        },
    ));
    app.active = Some(1);
    app.next_id = 2;
    app.open_image_overlay();
    app
}
#[test]
fn signature_actions_retain_image_redaction_and_chosen_font_dialogs() {
    let context = egui::Context::default();
    let mut app = editor(&context);
    let key = app.image_dialog.as_ref().unwrap().key;
    {
        let dialog = app.image_dialog.as_mut().unwrap();
        dialog.redaction_mode = true;
        dialog.removal_confirmed = true;
        dialog.expected_copy = Some(9);
    }
    app.open_image_signature();
    app.open_typed_signature();
    let dialog = app.image_dialog.as_ref().unwrap();
    assert!(dialog.redaction_mode && dialog.removal_confirmed);
    assert!(!dialog.visual_signature);
    assert_eq!(dialog.expected_copy, Some(9));
    assert_eq!(dialog.key, key);
    assert!(app.overlay_dialog.is_none());

    app.image_dialog = None;
    app.open_content_text();
    assert!(app.content_dialog.is_some());
    // The shared exclusive dialog guard must hold even after the asynchronous
    // inventory finishes and the actor's busy flag clears.
    app.busy = false;
    app.open_image_signature();
    app.open_typed_signature();
    assert!(app.content_dialog.is_some());
    assert!(app.image_dialog.is_none() && app.overlay_dialog.is_none());
}
#[test]
fn signature_modes_do_not_repurpose_open_image_or_bypass_readonly_guards() {
    let ctx = egui::Context::default();
    let mut app = editor(&ctx);
    let key = app.image_dialog.as_ref().unwrap().key;
    app.image_dialog.as_mut().unwrap().image_file = Some("retained.png".into());
    app.open_image_signature();
    app.open_typed_signature();
    assert!(!app.image_dialog.as_ref().unwrap().visual_signature);
    assert_eq!(app.image_dialog.as_ref().unwrap().key, key);
    assert!(app.overlay_dialog.is_none());
    app.image_dialog = None;
    app.tabs[0].status.info.signature_fields = 1;
    app.open_image_signature();
    app.open_typed_signature();
    assert!(app.image_dialog.is_none() && app.overlay_dialog.is_none());
    app.tabs[0].status.info.signature_fields = 0;
    app.busy = true;
    app.open_image_signature();
    app.open_typed_signature();
    assert!(app.image_dialog.is_none() && app.overlay_dialog.is_none());
    app.busy = false;
    app.open_image_signature();
    assert!(app.image_dialog.as_ref().unwrap().visual_signature);
    assert!(!app.image_dialog.as_ref().unwrap().has_changes());
    app.image_dialog.as_mut().unwrap().image_file = Some("signature.png".into());
    app.tabs[0].status.revision = 1;
    assert!(!app.apply_image_overlay());
    assert!(app.image_dialog.as_ref().unwrap().has_changes());
    app.request_close(1);
    assert!(app.close_target.is_some());
    assert!(app.image_dialog.is_some());
}
#[test]
fn competing_overlay_actions_preserve_the_first_open_dialog() {
    for first in 0..3 {
        let context = egui::Context::default();
        let mut app = editor(&context);
        app.image_dialog = None;
        match first {
            0 => {
                app.open_image_overlay();
                app.image_dialog.as_mut().unwrap().image_file = Some("retained.png".into());
            }
            1 => {
                app.open_text_overlay();
                app.overlay_dialog.as_mut().unwrap().request.value = "Retained draft".into();
            }
            _ => app.open_vector(),
        }
        let generation = app.forms_generation;
        app.open_image_overlay();
        app.open_vector();
        app.open_text_overlay();
        app.open_content_text();
        assert_eq!(
            [
                app.image_dialog.is_some(),
                app.overlay_dialog.is_some(),
                app.vector_dialog.is_some(),
                app.content_dialog.is_some()
            ],
            [first == 0, first == 1, first == 2, false],
            "only the first dialog may own the canvas"
        );
        assert_eq!(app.forms_generation, generation);
        if first == 0 {
            assert_eq!(
                app.image_dialog.as_ref().unwrap().image_file.as_deref(),
                Some(std::path::Path::new("retained.png"))
            );
        } else if first == 1 {
            assert_eq!(
                app.overlay_dialog.as_ref().unwrap().request.value,
                "Retained draft"
            );
        }
    }
}
fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
    let mut draw = |events| {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.image_overlay_dialog(context),
        );
        output.textures_delta.clear();
        output
    };
    let _ = draw(vec![]);
    let output = draw(vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing visible control: {label}"));
    let _ = draw(vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let _ = draw(vec![egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
}
#[test]
fn actual_rotation_insert_button_error_and_draft_close_guard_are_live() {
    let context = egui::Context::default();
    let mut app = editor(&context);
    app.image_dialog.as_mut().unwrap().image_file = Some("chosen.png".into());
    assert!(app.has_tab_draft(1));
    assert!(app.needs_exit_guard());
    app.request_close(1);
    assert_eq!(app.close_target, Some(super::super::CloseTarget::Tab(1)));
    app.close_target = None;
    click(&mut app, &context, "0°");
    click(&mut app, &context, "90°");
    assert_eq!(app.image_dialog.as_ref().unwrap().request.rotation, 90);
    click(&mut app, &context, "Insert image");
    assert!(app.image_dialog.as_ref().unwrap().submitting);
    let start = std::time::Instant::now();
    while app.image_dialog.as_ref().unwrap().submitting
        && start.elapsed() < std::time::Duration::from_secs(2)
    {
        app.poll(&context);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let dialog = app.image_dialog.as_ref().unwrap();
    assert!(dialog.error.is_some());
    assert!(!dialog.submitting);
    assert!(dialog.image_file.is_some());
    assert_eq!(app.tabs.len(), 1);
    assert!(!app.tabs[0].status.dirty);
    click(&mut app, &context, "Discard image draft");
    assert!(app.image_dialog.is_none());
}
#[test]
fn stale_readonly_busy_and_missing_file_cannot_dispatch() {
    let context = egui::Context::default();
    let mut app = editor(&context);
    assert!(!app.apply_image_overlay());
    app.image_dialog.as_mut().unwrap().image_file = Some("chosen.png".into());
    app.busy = true;
    assert!(!app.apply_image_overlay());
    app.busy = false;
    app.tabs[0].status.revision = 1;
    assert!(!app.apply_image_overlay());
    app.tabs[0].status.revision = 0;
    app.tabs[0].status.info.encrypted = true;
    assert!(!app.apply_image_overlay());
    app.tabs[0].status.info.encrypted = false;
    app.tabs[0].status.info.signature_fields = 1;
    assert!(!app.apply_image_overlay());
    assert!(app.image_dialog.as_ref().unwrap().has_changes());
}
#[test]
fn actual_controls_apply_through_worker_then_save_new_copy_and_keep_original_files() {
    exercise_image_controls(false);
}
#[test]
fn visual_signature_image_controls_history_and_new_copy_keep_assets_unchanged() {
    exercise_image_controls(true);
}
fn exercise_image_controls(signature: bool) {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let image = temp.path().join("image.png");
    let source = support::fixture(2);
    std::fs::write(&input, &source).unwrap();
    std::fs::write(&image, crate::worker::TEST_RED_PNG).unwrap();
    let context = egui::Context::default();
    let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
    assert!(app.send(
        Command::Load {
            id: 1,
            path: input.clone()
        },
        "Loading test source"
    ));
    let wait = |app: &mut EditorApp| {
        let start = std::time::Instant::now();
        while app.busy && start.elapsed() < std::time::Duration::from_secs(10) {
            app.poll(&context);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.busy);
    };
    wait(&mut app);
    assert_eq!(app.tabs.len(), 1);
    if signature {
        app.open_image_signature();
    } else {
        app.open_image_overlay();
    }
    assert_eq!(
        app.image_dialog.as_ref().unwrap().visual_signature,
        signature
    );
    app.image_dialog.as_mut().unwrap().image_file = Some(image.clone());
    app.image_dialog.as_mut().unwrap().request.rectangle = [60.0, 30.0, 100.0, 70.0];
    click(&mut app, &context, "0°");
    click(&mut app, &context, "270°");
    if signature {
        let key = app.image_dialog.as_ref().unwrap().key;
        app.open_typed_signature();
        app.open_image_signature();
        assert_eq!(app.image_dialog.as_ref().unwrap().key, key);
        assert!(app.overlay_dialog.is_none());
        assert!(app.needs_exit_guard());
    }
    click(
        &mut app,
        &context,
        if signature {
            "Add visual signature image"
        } else {
            "Insert image"
        },
    );
    wait(&mut app);
    assert!(app.image_dialog.is_none());
    assert!(app.tabs[0].status.dirty && app.tabs[0].status.can_undo);
    assert_eq!(app.tabs[0].status.revision, 1);
    for edit in [crate::worker::Edit::Undo, crate::worker::Edit::Redo] {
        assert!(app.send(Command::Edit { id: 1, edit }, "Checking signature history"));
        wait(&mut app);
    }
    assert!(app.tabs[0].status.dirty && app.tabs[0].status.can_undo);
    let output = temp.path().join("new.pdf");
    assert!(app.send(
        Command::Save {
            id: 1,
            path: output.clone()
        },
        "Saving test copy"
    ));
    wait(&mut app);
    assert!(!app.tabs[0].status.dirty);
    let bytes = std::fs::read(output).unwrap();
    let rendered = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    let offset = (250 * rendered.width as usize + 80) * 4;
    assert_eq!(&rendered.rgba[offset..offset + 4], &[255, 0, 0, 255]);
    assert_eq!(std::fs::read(input).unwrap(), source);
    assert_eq!(std::fs::read(image).unwrap(), crate::worker::TEST_RED_PNG);
}
