use super::EditorApp;
use crate::state::{DocumentStatus, Tab};
use crate::worker::Command;
use eframe::egui;
use std::path::PathBuf;

pub(super) struct Opening {
    pub id: u64,
    path: PathBuf,
    active: Option<u64>,
}

pub(super) struct Dialog {
    opening: Opening,
    fingerprint: [u8; 32],
    password: String,
    submitting: bool,
    error: Option<String>,
    focus: bool,
}
impl Dialog {
    pub(super) fn related_to(&self, id: u64) -> bool {
        self.opening.id == id || self.opening.active == Some(id)
    }
}

impl EditorApp {
    pub(super) fn begin_open(&mut self, path: PathBuf) -> bool {
        if self.busy
            || self.password_dialog.is_some()
            || self.pending_open.is_some()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || self.next_id == 0
            || self.next_id == u64::MAX
        {
            return false;
        }
        let id = self.next_id;
        let opening = Opening {
            id,
            path: path.clone(),
            active: self.active,
        };
        if self.send(Command::Load { id, path }, "Opening and validating PDF…") {
            self.next_id += 1;
            self.pending_open = Some(opening);
            true
        } else {
            false
        }
    }

    fn password_current(&self) -> bool {
        self.password_dialog.as_ref().is_some_and(|dialog| {
            self.active == dialog.opening.active
                && !self.tabs.iter().any(|tab| tab.id == dialog.opening.id)
                && dialog.opening.id != 0
                && dialog.opening.id < self.next_id
                && self.tabs.len() < crate::worker::MAX_TABS
                && self.pending_saves.is_empty()
        })
    }

    pub(super) fn accept_password_required(
        &mut self,
        id: u64,
        path: PathBuf,
        fingerprint: [u8; 32],
        incorrect: bool,
    ) {
        if incorrect {
            if self.password_current()
                && let Some(dialog) = self.password_dialog.as_mut().filter(|dialog| {
                    dialog.submitting
                        && dialog.opening.id == id
                        && dialog.opening.path == path
                        && dialog.fingerprint == fingerprint
                })
            {
                dialog.submitting = false;
                dialog.password.clear();
                dialog.focus = true;
                dialog.error = Some("Password was not accepted. Try again. Unsupported encryption may also be refused.".into());
                self.message =
                    "Encrypted PDF remains closed. Enter the password again or cancel.".into();
                return;
            }
        } else if let Some(opening) = self.pending_open.take().filter(|opening| {
            opening.id == id
                && opening.path == path
                && opening.active == self.active
                && id != 0
                && id < self.next_id
                && self.tabs.len() < crate::worker::MAX_TABS
                && !self.tabs.iter().any(|tab| tab.id == id)
        }) {
            self.password_dialog = Some(Dialog {
                opening,
                fingerprint,
                password: String::new(),
                submitting: false,
                error: None,
                focus: true,
            });
            self.message = "This encrypted PDF needs explicit session-only opening.".into();
            return;
        }
        self.message = "Stale password request ignored. Existing PDFs retained.".into();
    }

    fn apply_password(&mut self) -> bool {
        if self.busy || !self.password_current() {
            return false;
        }
        let Some(dialog) = self
            .password_dialog
            .as_mut()
            .filter(|dialog| !dialog.submitting)
        else {
            return false;
        };
        if dialog.password.len() > pdf_worker::document::MAX_PASSWORD_BYTES {
            dialog.error = Some("Password exceeds 4096 UTF-8 bytes.".into());
            return false;
        }
        let command = Command::Unlock {
            id: dialog.opening.id,
            path: dialog.opening.path.clone(),
            fingerprint: dialog.fingerprint,
            password: std::mem::take(&mut dialog.password),
        };
        if self.send(
            command,
            "Opening encrypted PDF read-only in a bounded worker…",
        ) {
            if let Some(dialog) = self.password_dialog.as_mut() {
                dialog.submitting = true;
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }

    pub(super) fn accept_unlocked(
        &mut self,
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
        context: &egui::Context,
    ) {
        let current = self.password_current()
            && self.password_dialog.as_ref().is_some_and(|dialog| {
                dialog.submitting && dialog.opening.id == id && dialog.opening.path == path
            })
            && status.info.encrypted
            && status.revision == 0
            && !status.dirty
            && !status.can_undo
            && !status.can_redo
            && status.info.page_count > 0
            && status.info.page_count <= pdf_core::MAX_PAGES
            && status.info.page_count == status.info.pages.len()
            && status.info.object_count <= pdf_core::MAX_OBJECTS;
        if current {
            self.cancel_password();
            let mut tab = Tab::new(id, path, status);
            tab.set_device_ratio(context.pixels_per_point());
            self.tabs.push(tab);
            self.active = Some(id);
            self.message = "Encrypted PDF opened read-only for this session. Password and decrypted content are not saved in recovery.".into();
        } else {
            self.cancel_password();
            // A completed child may have opened an actor document before the user
            // changed tabs. Retire that orphan so its transient credential is dropped.
            if id != 0 && id != u64::MAX && !self.tabs.iter().any(|tab| tab.id == id) {
                self.send(Command::Close(id), "Retiring stale password opening…");
            }
            self.message = "Stale password opening ignored. Existing PDFs retained.".into();
        }
    }

    pub(super) fn password_error(&mut self, id: u64, message: &str) -> bool {
        if let Some(dialog) = self
            .password_dialog
            .as_mut()
            .filter(|dialog| dialog.submitting && dialog.opening.id == id)
        {
            dialog.submitting = false;
            dialog.password.clear();
            dialog.error = Some(message.to_owned());
            dialog.focus = true;
            self.message =
                "Password opening failed. Source ciphertext and existing documents retained."
                    .into();
            true
        } else {
            false
        }
    }

    pub(super) fn cancel_password(&mut self) {
        self.password_dialog = None;
        self.pending_open = None;
    }

    pub(super) fn password_dialog_ui(&mut self, context: &egui::Context) {
        let current = self.password_current();
        let Some(dialog) = &mut self.password_dialog else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Open encrypted PDF")
            .collapsible(false).resizable(false).default_width(520.0).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label("Read-only, session-only viewing");
                ui.label("The original stays encrypted. Passwords and decrypted PDF content are not saved in recovery. After a restart, reopen this file and enter its password again.");
                ui.label("Editing, Save As, encrypted export and protection changes are unavailable.");
                if !current { ui.colored_label(egui::Color32::YELLOW, "This opening is stale or a save needs attention. Cancel and open again."); }
                ui.add_enabled_ui(current && !self.busy && !dialog.submitting, |ui| {
                    ui.label("Password (an empty password is allowed)");
                    // Keep one over-limit character so an oversized paste is refused,
                    // never silently used as a valid truncated password.
                    let mut output = egui::TextEdit::singleline(&mut dialog.password)
                        .id(egui::Id::new(("pdf-password", dialog.opening.id)))
                        .password(true).hint_text("PDF password").char_limit(4097).show(ui);
                    // Egui normally retains text undo history, even for masked edits.
                    // A password must not survive cancellation in that history.
                    output.state.clear_undoer();
                    output.state.store(ui.ctx(), output.response.id);
                    let input = output.response;
                    if std::mem::take(&mut dialog.focus) { input.request_focus(); }
                    let enter = input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    apply = ui.button("Open read-only for this session").clicked() || enter;
                });
                if let Some(error) = &dialog.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
                cancel = ui.add_enabled(!dialog.submitting && !self.busy, egui::Button::new("Cancel")).clicked();
            });
        if cancel {
            self.cancel_password();
        }
        if apply {
            self.apply_password();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::worker::password_tests::encryption;
    use std::time::{Duration, Instant};

    fn wait(app: &mut EditorApp, context: &egui::Context, done: impl Fn(&EditorApp) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(35);
        while Instant::now() < deadline {
            app.poll(context);
            if !app.busy && done(app) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("password operation must finish within the worker deadline plus grace");
    }
    fn opened_dialog(password: &str) -> (EditorApp, egui::Context, tempfile::TempDir, Vec<u8>) {
        let context = egui::Context::default();
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
        let temp = tempfile::tempdir().unwrap();
        let (cipher, _) = encryption::fixture(4, password, false);
        let path = temp.path().join("encrypted.pdf");
        std::fs::write(&path, &cipher).unwrap();
        assert!(app.begin_open(path));
        wait(&mut app, &context, |app| app.password_dialog.is_some());
        assert!(app.tabs.is_empty());
        (app, context, temp, cipher)
    }
    fn draw(
        app: &mut EditorApp,
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.password_dialog_ui(context),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        draw(app, context, vec![]);
        let output = draw(app, context, vec![]);
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
            .expect("fully visible labeled password control");
        draw(
            app,
            context,
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
            context,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    #[test]
    fn real_masked_widget_retries_wrong_password_and_opens_read_only_with_actual_pixels() {
        let (mut app, context, temp, cipher) = opened_dialog("synthetic-reader");
        draw(&mut app, &context, vec![]);
        draw(&mut app, &context, vec![]);
        let output = draw(
            &mut app,
            &context,
            vec![egui::Event::Paste("wrong-secret".into())],
        );
        assert_eq!(
            app.password_dialog.as_ref().unwrap().password,
            "wrong-secret"
        );
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text().contains("wrong-secret"))));
        let state =
            egui::text_edit::TextEditState::load(&context, egui::Id::new(("pdf-password", 1_u64)))
                .unwrap();
        assert!(!state.undoer().has_undo(&(
            egui::text::CCursorRange::one(egui::text::CCursor::new(0)),
            String::new()
        )));
        click(&mut app, &context, "Open read-only for this session");
        assert!(app.password_dialog.as_ref().unwrap().password.is_empty());
        wait(&mut app, &context, |app| {
            app.password_dialog
                .as_ref()
                .is_some_and(|dialog| !dialog.submitting)
        });
        assert!(app.password_dialog.as_ref().unwrap().error.is_some());
        draw(&mut app, &context, vec![]);
        draw(&mut app, &context, vec![]);
        draw(
            &mut app,
            &context,
            vec![egui::Event::Paste("synthetic-reader".into())],
        );
        click(&mut app, &context, "Open read-only for this session");
        wait(&mut app, &context, |app| {
            app.tabs.len() == 1 && app.tabs[0].texture.is_some()
        });
        assert!(app.password_dialog.is_none() && app.pending_open.is_none());
        assert!(app.tabs[0].status.info.encrypted && !app.tabs[0].editable());
        assert!(!app.tabs[0].status.dirty && !app.tabs[0].status.can_undo);
        assert_eq!(
            std::fs::read(temp.path().join("encrypted.pdf")).unwrap(),
            cipher
        );
        app.request_close(1);
        wait(&mut app, &context, |app| app.tabs.is_empty());
        assert!(app.close_target.is_none());
    }

    #[test]
    fn empty_password_still_requires_disclosure_and_actual_enter_dispatch() {
        let (mut app, context, _temporary, _cipher) = opened_dialog("");
        draw(&mut app, &context, vec![]);
        draw(&mut app, &context, vec![]);
        draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.busy && app.password_dialog.as_ref().unwrap().submitting);
        wait(&mut app, &context, |app| {
            app.tabs.len() == 1 && app.tabs[0].texture.is_some()
        });
        assert!(app.tabs[0].status.info.encrypted && !app.tabs[0].editable());
        assert!(app.password_dialog.is_none());
    }

    #[test]
    fn stale_tab_pending_save_oversized_input_and_cancel_cannot_dispatch() {
        let (mut app, context, _temporary, _cipher) = opened_dialog("reader");
        app.password_dialog.as_mut().unwrap().password = "sensitive draft".into();
        app.active = Some(99);
        assert!(!app.apply_password());
        app.active = None;
        app.pending_saves.insert(
            99,
            super::super::recovery::PendingSave::new("pending.pdf".into(), "pending".into()),
        );
        assert!(!app.apply_password());
        app.pending_saves.clear();
        app.password_dialog.as_mut().unwrap().password = "x".repeat(4097);
        assert!(!app.apply_password());
        assert!(!app.busy);
        assert!(app.tabs.is_empty());
        click(&mut app, &context, "Cancel");
        assert!(app.password_dialog.is_none() && app.pending_open.is_none());
    }

    #[test]
    fn stale_success_retires_actor_credential_and_existing_tabs_are_retained() {
        let (mut app, context, _temporary, _cipher) = opened_dialog("reader");
        app.password_dialog.as_mut().unwrap().password = "reader".into();
        assert!(app.apply_password());
        let (_, plain) = encryption::fixture(3, "reader", false);
        let info = pdf_core::PdfDocument::from_bytes(&plain, None)
            .unwrap()
            .info()
            .unwrap();
        let mut previous = Tab::new(
            99,
            "existing.pdf".into(),
            DocumentStatus {
                info,
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        );
        previous.needs_render = false;
        app.tabs.push(previous);
        app.active = Some(99);
        wait(&mut app, &context, |app| app.password_dialog.is_none());
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].id, 99);
        app.worker
            .send(Command::Forms(crate::state::FormsReadKey {
                tab: 1,
                revision: 0,
                generation: 0,
            }))
            .unwrap();
        let reply = app
            .worker
            .events
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(
            matches!(reply, crate::worker::Event::Error { id: 1, .. }),
            "stale encrypted actor tab must be closed"
        );
    }
}
