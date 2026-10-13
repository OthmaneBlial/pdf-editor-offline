#![allow(clippy::unwrap_used)]
use super::*;
use crate::worker::content_tests::fixture;
use std::time::{Duration, Instant};
fn wait(app: &mut EditorApp, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(pdf_worker::CPU_SECONDS + 5);
    while app.busy && Instant::now() < deadline {
        app.poll(ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !app.busy,
        "bounded child reply did not arrive: {}",
        app.message
    );
}
fn app() -> (
    tempfile::TempDir,
    EditorApp,
    egui::Context,
    std::path::PathBuf,
    Vec<u8>,
) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("original.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&path, &bytes).unwrap();
    let ctx = egui::Context::default();
    let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
    app.begin_open(path.clone());
    wait(&mut app, &ctx);
    assert_eq!(app.tabs.len(), 1, "{}", app.message);
    app.open_content_text();
    wait(&mut app, &ctx);
    assert!(
        app.content_dialog.as_ref().unwrap().inventory.is_some(),
        "{}",
        app.message
    );
    (temp, app, ctx, path, bytes)
}
#[test]
fn competing_canvas_actions_retain_loaded_existing_text_draft() {
    let (_temp, mut app, _ctx, _path, _bytes) = app();
    let dialog = app.content_dialog.as_mut().unwrap();
    dialog.selected = Some(0);
    dialog.replacement = "Replaced".into();
    let key = dialog.key;
    assert!(dialog.has_changes());
    assert!(!app.busy);
    app.open_image_overlay();
    app.open_vector();
    app.open_text_overlay();
    assert!(app.image_dialog.is_none());
    assert!(app.vector_dialog.is_none());
    assert!(app.overlay_dialog.is_none());
    let dialog = app.content_dialog.as_ref().unwrap();
    assert_eq!(dialog.key, key);
    assert_eq!(dialog.replacement, "Replaced");
    assert!(dialog.has_changes());
}
fn draw(app: &mut EditorApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |_| app.content_text_dialog(ctx),
    );
    output.textures_delta.clear();
    output
}
fn click(app: &mut EditorApp, ctx: &egui::Context, label: &str) {
    // egui's first window frame sizes/fades the surface before painting labels.
    draw(app, ctx, vec![]);
    let output = draw(app, ctx, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text)
                if text.galley.text() == label
                    && shape
                        .clip_rect
                        .contains_rect(text.galley.rect.translate(text.pos.to_vec2())) =>
            {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "visible labeled control {label}; labels: {:?}",
                output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) => Some((
                            text.galley.text(),
                            text.pos,
                            shape.clip_rect,
                            text.galley.rect
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            )
        });
    draw(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}
fn replacement(app: &mut EditorApp, ctx: &egui::Context, value: &str) {
    draw(app, ctx, vec![]);
    draw(
        app,
        ctx,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Paste(value.into()),
        ],
    );
    assert_eq!(app.content_dialog.as_ref().unwrap().replacement, value);
}
#[test]
fn content_edit_widget_click_applies_actual_history_and_verified_save() {
    let (_temp, mut app, ctx, path, bytes) = app();
    replacement(&mut app, &ctx, "Replaced");
    assert!(app.has_tab_draft(app.active.unwrap()));
    click(&mut app, &ctx, "Replace existing text");
    assert!(app.busy);
    wait(&mut app, &ctx);
    assert!(app.content_dialog.is_none());
    assert_eq!(app.selected().unwrap().status.revision, 1);
    assert!(app.selected().unwrap().status.can_undo && app.selected().unwrap().status.dirty);
    app.perform(super::super::Action::Undo, &ctx);
    wait(&mut app, &ctx);
    assert_eq!(app.selected().unwrap().status.revision, 0);
    assert!(app.selected().unwrap().status.can_redo);
    app.perform(super::super::Action::Redo, &ctx);
    wait(&mut app, &ctx);
    assert_eq!(app.selected().unwrap().status.revision, 1);
    let output = path.with_file_name("saved.pdf");
    let id = app.active.unwrap();
    app.send(
        Command::Save {
            id,
            path: output.clone(),
        },
        "Saving actual existing-text edit…",
    );
    wait(&mut app, &ctx);
    assert!(!app.selected().unwrap().status.dirty);
    let pdf = pdf_core::PdfDocument::open(output, None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Replaced\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn content_edit_enter_submission_and_real_run_selection_are_accessible() {
    let (_temp, mut app, ctx, path, bytes) = app();
    click(&mut app, &ctx, "Run 2 · A B");
    assert_eq!(app.content_dialog.as_ref().unwrap().replacement, "A B");
    replacement(&mut app, &ctx, "C D");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(app.busy);
    wait(&mut app, &ctx);
    assert!(app.content_dialog.is_none());
    assert_eq!(app.selected().unwrap().status.revision, 1);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn content_edit_stale_readonly_pending_save_and_draft_cleanup_guards() {
    let (_temp, mut app, ctx, path, bytes) = app();
    replacement(&mut app, &ctx, "Replaced");
    let key = app.content_dialog.as_ref().unwrap().key;
    app.active = None;
    assert!(!app.apply_content_text());
    app.active = Some(key.tab);
    app.content_dialog.as_mut().unwrap().key.revision = 99;
    assert!(!app.apply_content_text());
    app.content_dialog.as_mut().unwrap().key = key;
    app.selected_mut().unwrap().page = 1;
    assert!(!app.apply_content_text());
    app.selected_mut().unwrap().page = 0;
    app.selected_mut().unwrap().status.info.signature_fields = 1;
    assert!(!app.apply_content_text());
    app.selected_mut().unwrap().status.info.signature_fields = 0;
    app.pending_saves.insert(
        key.tab,
        super::super::recovery::PendingSave::new(
            path.with_file_name("uncertain.pdf"),
            "pending".into(),
        ),
    );
    assert!(!app.apply_content_text());
    app.pending_saves.clear();
    app.content_dialog.as_mut().unwrap().replacement = "A".repeat(65_537);
    assert!(!app.apply_content_text());
    app.content_dialog.as_mut().unwrap().replacement = "Replaced".into();
    app.open_recovery();
    assert!(!app.recovery.open);
    assert!(app.content_dialog.is_some());
    assert!(app.message.contains("existing-text draft"));
    app.request_close(key.tab);
    assert!(app.close_target.is_some());
    app.close_target = None;
    app.request_exit(&ctx);
    assert!(app.close_target.is_some());
    assert!(!app.allow_exit);
    app.close_target = None;
    click(&mut app, &ctx, "Discard draft and close");
    assert!(app.content_dialog.is_none());
    assert!(!app.has_tab_draft(key.tab));
    assert_eq!(app.selected().unwrap().status.revision, 0);
    app.open_content_text();
    wait(&mut app, &ctx);
    replacement(&mut app, &ctx, "Replaced");
    app.discard_edits_and_quit(&ctx);
    assert!(app.content_dialog.is_none());
    assert!(app.allow_exit);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
