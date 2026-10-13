use super::EditorApp;
use crate::worker::{Command, RecoveredDocument, RecoverySession};
use eframe::egui;

#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    offered: bool,
    pub sessions: Vec<RecoverySession>,
    pub pending: Option<Pending>,
    confirm_discard: Option<String>,
    pub error: Option<String>,
}

pub(super) struct PendingSave {
    pub path: std::path::PathBuf,
    pub message: String,
    pub submitting: bool,
    confirm_cancel: bool,
}
impl PendingSave {
    pub fn new(path: std::path::PathBuf, message: String) -> Self {
        Self {
            path,
            message,
            submitting: false,
            confirm_cancel: false,
        }
    }
}

pub(super) enum Pending {
    List,
    Restore,
    Discard,
}

fn restore_blocker(session: &RecoverySession, tabs: usize, busy: bool) -> Option<&'static str> {
    if busy {
        Some("Wait for the current operation to finish.")
    } else if session.busy {
        Some("This session is still in use by another app window.")
    } else if session.error.is_some() {
        Some("This session needs attention and cannot be restored.")
    } else if session.documents == 0 {
        Some("This session has no documents to restore.")
    } else if session.documents > crate::worker::MAX_TABS.saturating_sub(tabs) {
        Some("This session exceeds the available 16-tab capacity.")
    } else {
        None
    }
}

impl EditorApp {
    pub(super) fn open_recovery(&mut self) {
        if self.content_dialog.is_some() {
            self.message =
                "Apply or discard the existing-text draft before opening recovery.".into();
            return;
        }
        self.recovery.open = true;
        self.recovery.confirm_discard = None;
        self.refresh_recovery();
    }

    fn refresh_recovery(&mut self) {
        if self.send(Command::RecoveryList, "Checking local recovery copies…") {
            self.recovery.pending = Some(Pending::List);
            self.recovery.error = None;
        }
    }

    pub(super) fn accept_recovery_list(&mut self, sessions: Vec<RecoverySession>) {
        if matches!(
            self.recovery.pending,
            Some(Pending::List | Pending::Discard)
        ) {
            self.message = if matches!(self.recovery.pending, Some(Pending::Discard)) {
                "Recovery copies discarded. Original PDFs and saved files preserved."
            } else {
                "Recovery sessions checked. No documents opened."
            }
            .into();
            self.recovery.pending = None;
        }
        if !self.recovery.offered {
            self.recovery.offered = true;
            self.recovery.open |= sessions
                .iter()
                .any(|s| !s.busy && s.error.is_none() && s.documents > 0);
        }
        self.recovery.sessions = sessions;
        self.recovery.confirm_discard = None;
    }

    fn restore_session(&mut self, id: &str) {
        if self.recovery.pending.is_some()
            || !self
                .recovery
                .sessions
                .iter()
                .any(|s| s.id == id && restore_blocker(s, self.tabs.len(), self.busy).is_none())
        {
            return;
        }
        if self.send(
            Command::RestoreSession { session: id.into() },
            "Validating recovery copies…",
        ) {
            self.recovery.pending = Some(Pending::Restore);
            self.recovery.error = None;
        }
    }

    fn discard_session(&mut self, id: &str) {
        if self.busy
            || self.recovery.pending.is_some()
            || self.recovery.confirm_discard.as_deref() != Some(id)
            || !self.recovery.sessions.iter().any(|s| s.id == id && !s.busy)
        {
            return;
        }
        if self.send(
            Command::DiscardSession { session: id.into() },
            "Discarding this session's recovery copies…",
        ) {
            self.recovery.pending = Some(Pending::Discard);
            self.recovery.error = None;
        }
    }

    pub(super) fn accept_recovered(
        &mut self,
        documents: Vec<RecoveredDocument>,
        context: &egui::Context,
    ) {
        if !matches!(self.recovery.pending, Some(Pending::Restore)) {
            self.message =
                "Unexpected recovery result ignored. Open PDFs have been retained.".into();
            return;
        }
        self.recovery.pending = None;
        let mut ids: std::collections::HashSet<u64> = self.tabs.iter().map(|t| t.id).collect();
        let valid = !documents.is_empty()
            && self.tabs.len().saturating_add(documents.len()) <= crate::worker::MAX_TABS
            && documents
                .iter()
                .all(|d| d.id != 0 && d.id != u64::MAX && ids.insert(d.id));
        if !valid {
            self.recovery.error = Some(
                "Recovery result could not be added safely. Existing tabs have been retained."
                    .into(),
            );
            return;
        }
        for document in documents {
            if let Some(path) = document.pending_save {
                self.pending_saves.insert(document.id, PendingSave::new(path, "A save attempt was interrupted. Verify the recorded destination or cancel the attempt.".into()));
            }
            self.next_id = self.next_id.max(document.id + 1);
            let mut tab = crate::state::Tab::new(document.id, document.path, document.status);
            tab.set_device_ratio(context.pixels_per_point());
            if self.active.is_none() {
                self.active = Some(tab.id);
            }
            self.tabs.push(tab);
        }
        self.recovery.open = false;
        self.message =
            "Session recovered. Review the PDFs and Save As to create verified files.".into();
    }

    pub(super) fn recovery_dialog(&mut self, context: &egui::Context) {
        if !self.recovery.open
            || self.close_target.is_some()
            || self.metadata_dialog.is_some()
            || self.forms_dialog.is_some()
            || self.content_dialog.is_some()
            || self.organizer_dialog.is_some()
        {
            return;
        }
        let mut open = true;
        let mut restore = None;
        let mut confirm = None;
        let mut discard = None;
        let mut cancel = false;
        let mut close = false;
        let mut refresh = false;
        let pending = self.recovery.pending.is_some();
        egui::Window::new("Recover previous session").open(&mut open).collapsible(false)
            .default_width(510.0).max_width((context.content_rect().width() - 32.0).max(280.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(context, |ui| {
                ui.label("Recovery copies are stored on this computer. Quitting retains recorded sessions until you discard them. Nothing opens until you choose Restore.");
                if let Some(error) = &self.recovery.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                egui::ScrollArea::vertical().max_height((context.content_rect().height()-220.0).max(80.0)).show(ui, |ui| {
                    if self.recovery.sessions.is_empty() && !pending { ui.label("No previous sessions are available."); }
                    for (index, session) in self.recovery.sessions.iter().enumerate() {
                        ui.push_id(&session.id, |ui| {
                            ui.separator();
                            ui.label(egui::RichText::new(format!("Session {} · {} PDF(s) · {:.1} MiB", index+1, session.documents, session.bytes as f64 / 1_048_576.0)).strong());
                            if let Some(error) = &session.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                            let reason = restore_blocker(session, self.tabs.len(), self.busy);
                            if let Some(reason) = reason { ui.label(reason); }
                            if self.recovery.confirm_discard.as_deref() == Some(&session.id) {
                                ui.label("Discard this session's app-owned recovery copies? Original PDFs and saved files are preserved. Recovery copies will no longer be available through this app; this is not secure erasure.");
                                ui.horizontal_wrapped(|ui| {
                                    if ui.add_enabled(!self.busy && !pending && !session.busy, egui::Button::new("Discard recovery copies")).clicked() { discard = Some(session.id.clone()); }
                                    if ui.add_enabled(!pending, egui::Button::new("Keep recovery copies")).clicked() { cancel = true; }
                                });
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    if ui.add_enabled(reason.is_none() && !pending, crate::style::primary(&format!("Restore session {}", index+1), ui.visuals().dark_mode)).clicked() { restore = Some(session.id.clone()); }
                                    if ui.add_enabled(!self.busy && !pending && !session.busy, egui::Button::new(format!("Discard session {}…", index+1))).clicked() { confirm = Some(session.id.clone()); }
                                });
                            }
                        });
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    refresh = ui.add_enabled(!self.busy && !pending, egui::Button::new("Refresh sessions")).clicked();
                    if ui.add_enabled(!pending, egui::Button::new("Close recovery")).clicked() { close = true; }
                    if pending { ui.spinner(); ui.label("Working…"); }
                });
            });
        if !pending {
            self.recovery.open =
                open && !close && !context.input(|input| input.key_pressed(egui::Key::Escape));
            if !self.recovery.open {
                self.recovery.confirm_discard = None;
            }
        }
        if refresh {
            self.refresh_recovery();
        }
        if cancel {
            self.recovery.confirm_discard = None;
        }
        if let Some(id) = confirm {
            self.recovery.confirm_discard = Some(id);
        }
        if let Some(id) = restore {
            self.restore_session(&id);
        }
        if let Some(id) = discard {
            self.discard_session(&id);
        }
    }

    pub(super) fn pending_save_dialog(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let Some((&id, pending)) = self.pending_saves.first_key_value() else {
            return;
        };
        let path = pending.path.display().to_string();
        let message = pending.message.clone();
        let submitting = pending.submitting;
        let confirm = pending.confirm_cancel;
        let mut retry = false;
        let mut ask_cancel = false;
        let mut cancel = false;
        let mut keep = false;
        egui::Window::new("Resolve recorded save attempt").collapsible(false).resizable(false)
            .default_width(510.0).max_width((context.content_rect().width()-32.0).max(280.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(context, |ui| {
                egui::ScrollArea::vertical().max_height((context.content_rect().height()-150.0).max(80.0)).show(ui, |ui| {
                    ui.label(&message);
                    ui.label("Recorded destination:"); ui.label(&path);
                    ui.label("Retry verifies the same destination and completes the recorded save. A different existing file is preserved. Cancel leaves the current PDF and its edits unchanged.");
                    if confirm { ui.label("Cancel this recorded attempt? Destination files and temporary copies are not deleted. The document keeps its previous unsaved state."); }
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if confirm {
                        cancel = ui.add_enabled(!self.busy && !submitting,egui::Button::new("Confirm cancel save attempt")).clicked();
                        keep = ui.add_enabled(!submitting,egui::Button::new("Keep recorded attempt")).clicked();
                    } else {
                        retry = ui.add_enabled(!self.busy && !submitting,crate::style::primary("Retry verification and save",ui.visuals().dark_mode)).clicked();
                        ask_cancel = ui.add_enabled(!self.busy && !submitting,egui::Button::new("Cancel recorded save…")).clicked();
                    }
                });
                if submitting { ui.spinner(); ui.label("Checking recorded save…"); }
            });
        if let Some(pending) = self.pending_saves.get_mut(&id) {
            if ask_cancel {
                pending.confirm_cancel = true;
            }
            if keep {
                pending.confirm_cancel = false;
            }
        }
        let command = if retry {
            Some(Command::ResolveSave { id })
        } else if cancel {
            Some(Command::CancelSave { id })
        } else {
            None
        };
        if let Some(command) = command
            && self.send(command, "Resolving recorded save attempt…")
            && let Some(pending) = self.pending_saves.get_mut(&id)
        {
            pending.submitting = true;
        }
    }

    pub(super) fn discard_edits_and_quit(&mut self, context: &egui::Context) {
        if self.busy || !self.pending_saves.is_empty() {
            return;
        }
        self.metadata_dialog = None;
        self.notes_dialog = None;
        self.forms_dialog = None;
        self.overlay_dialog = None;
        self.vector_dialog = None;
        self.image_dialog = None;
        self.content_dialog = None;
        self.organizer_dialog = None;
        self.discard_quit = true;
        self.continue_discard_quit(context);
    }

    pub(super) fn continue_discard_quit(&mut self, context: &egui::Context) {
        if !self.discard_quit || self.busy {
            return;
        }
        if let Some(id) = self.tabs.iter().find(|t| t.status.dirty).map(|t| t.id) {
            if !self.send(
                Command::Close(id),
                "Discarding recorded edits before quitting…",
            ) {
                self.discard_quit = false;
            }
        } else {
            self.close_target = None;
            self.allow_exit = true;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DocumentStatus, Tab};

    fn app() -> EditorApp {
        EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ))
    }
    fn session(busy: bool) -> RecoverySession {
        RecoverySession {
            id: "opaque-session".into(),
            documents: 2,
            bytes: 4096,
            busy,
            error: None,
        }
    }
    fn status() -> DocumentStatus {
        DocumentStatus {
            info: pdf_core::DocumentInfo {
                version: "1.7".into(),
                page_count: 1,
                object_count: 5,
                encrypted: false,
                signature_fields: 0,
                pages: vec![],
            },
            revision: 8,
            dirty: true,
            can_undo: true,
            can_redo: true,
        }
    }
    #[test]
    fn startup_offers_only_inactive_recoverable_summary_and_never_opens_documents() {
        let mut app = app();
        app.busy = true;
        app.accept_recovery_list(vec![session(false)]);
        assert!(app.recovery.open);
        assert!(app.tabs.is_empty());
        assert!(app.busy);
        app.recovery.open = false;
        app.accept_recovery_list(vec![session(false)]);
        assert!(
            !app.recovery.open,
            "refresh does not reopen a dismissed startup offer"
        );
        let mut app = self::app();
        app.accept_recovery_list(vec![session(true)]);
        assert!(!app.recovery.open);
    }
    #[test]
    fn restore_guards_active_errors_busy_and_capacity_but_allows_existing_tabs() {
        assert!(restore_blocker(&session(false), 1, false).is_none());
        assert!(restore_blocker(&session(true), 0, false).is_some());
        assert!(restore_blocker(&session(false), 0, true).is_some());
        assert!(restore_blocker(&session(false), 15, false).is_some());
        let mut broken = session(false);
        broken.error = Some("Unreadable recovery manifest".into());
        assert!(restore_blocker(&broken, 0, false).is_some());
    }
    #[test]
    fn restored_ids_history_flags_and_existing_dirty_work_are_preserved_atomically() {
        let mut app = app();
        app.tabs.push(Tab::new(4, "original.pdf".into(), status()));
        app.active = Some(4);
        app.next_id = 50;
        app.recovery.pending = Some(Pending::Restore);
        app.accept_recovered(
            vec![RecoveredDocument {
                id: 7,
                path: "recovered.pdf".into(),
                status: status(),
                pending_save: None,
            }],
            &egui::Context::default(),
        );
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, Some(4));
        assert_eq!(app.next_id, 50);
        assert_eq!(app.tabs[0].status.revision, 8);
        assert!(app.tabs[0].status.dirty);
        assert!(
            app.tabs[1].status.can_undo && app.tabs[1].status.can_redo && app.tabs[1].status.dirty
        );
        app.recovery.pending = Some(Pending::Restore);
        app.accept_recovered(
            vec![RecoveredDocument {
                id: 4,
                path: "wrong.pdf".into(),
                status: status(),
                pending_save: None,
            }],
            &egui::Context::default(),
        );
        assert_eq!(app.tabs.len(), 2);
        assert!(app.recovery.error.is_some());
        app.recovery.pending = Some(Pending::Restore);
        app.accept_recovered(
            vec![RecoveredDocument {
                id: 90,
                path: "new.pdf".into(),
                status: status(),
                pending_save: None,
            }],
            &egui::Context::default(),
        );
        assert_eq!(app.next_id, 91);
        app.accept_recovered(
            vec![RecoveredDocument {
                id: 92,
                path: "stale.pdf".into(),
                status: status(),
                pending_save: None,
            }],
            &egui::Context::default(),
        );
        assert_eq!(app.tabs.len(), 3);
        assert_eq!(app.next_id, 91);
    }
    fn draw(
        app: &mut EditorApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                events,
                ..Default::default()
            },
            |_| {
                if app.pending_saves.is_empty() {
                    app.recovery_dialog(ctx);
                } else {
                    app.pending_save_dialog(ctx);
                }
            },
        );
        output.textures_delta.clear();
        output
    }
    fn click_label(app: &mut EditorApp, ctx: &egui::Context, label: &str) {
        let _ = draw(app, ctx, vec![]);
        let output = draw(app, ctx, vec![]);
        let position = output
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
            .expect("visible button text");
        let _ = draw(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let _ = draw(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn real_egui_discard_needs_confirmation_and_cancel_preserves_session() {
        let mut app = app();
        app.recovery.open = true;
        app.recovery.sessions = vec![session(false)];
        let context = egui::Context::default();
        click_label(&mut app, &context, "Discard session 1…");
        assert_eq!(
            app.recovery.confirm_discard.as_deref(),
            Some("opaque-session")
        );
        assert!(
            app.recovery.pending.is_none(),
            "first step sends no deletion command"
        );
        click_label(&mut app, &context, "Keep recovery copies");
        assert!(app.recovery.confirm_discard.is_none());
        assert_eq!(app.recovery.sessions.len(), 1);
    }
    #[test]
    fn real_egui_active_session_restore_is_disabled_and_footer_reachable() {
        let mut app = app();
        app.recovery.open = true;
        app.recovery.sessions = vec![session(true)];
        let context = egui::Context::default();
        click_label(&mut app, &context, "Restore session 1");
        assert!(app.recovery.pending.is_none());
        assert!(app.tabs.is_empty());
        click_label(&mut app, &context, "Close recovery");
        assert!(!app.recovery.open);
    }
    #[test]
    fn global_recovery_error_preserves_dirty_tabs_and_cancels_pending_exit() {
        let mut app = app();
        let mut tab = Tab::new(4, "original.pdf".into(), status());
        tab.needs_render = false;
        app.tabs.push(tab);
        app.active = Some(4);
        app.recovery.pending = Some(Pending::Restore);
        app.discard_quit = true;
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        sender
            .send(crate::worker::Event::Error {
                id: 0,
                task: None,
                message: "Recovery session changed; refresh and retry.".into(),
            })
            .expect("event");
        app.poll(&egui::Context::default());
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, Some(4));
        assert!(app.tabs[0].status.dirty && app.tabs[0].error.is_none());
        assert!(app.recovery.error.is_some());
        assert!(app.recovery.pending.is_none());
        assert!(!app.discard_quit && !app.allow_exit);
        assert!(app.message.contains("Recovery session changed"));
    }
    #[test]
    fn explicit_discard_quit_waits_for_close_and_failure_retains_the_dirty_document() {
        let mut app = app();
        let mut tab = Tab::new(4, "original.pdf".into(), status());
        tab.needs_render = false;
        app.tabs.push(tab);
        app.active = Some(4);
        app.close_target = Some(super::super::CloseTarget::Exit);
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        let context = egui::Context::default();
        app.discard_edits_and_quit(&context);
        assert!(app.busy && app.discard_quit && !app.allow_exit);
        assert_eq!(
            app.tabs.len(),
            1,
            "close request alone must not remove a tab"
        );
        sender
            .send(crate::worker::Event::Error {
                id: 4,
                task: None,
                message: "Recovery commit failed.".into(),
            })
            .expect("event");
        app.poll(&context);
        assert!(!app.discard_quit && !app.allow_exit);
        assert_eq!(app.tabs.len(), 1);
        assert!(app.tabs[0].status.dirty);
    }
    #[test]
    fn real_egui_pending_save_cancel_is_explicit_and_failed_retry_keeps_work() {
        let mut app = app();
        let mut tab = Tab::new(4, "original.pdf".into(), status());
        tab.needs_render = false;
        app.tabs.push(tab);
        app.active = Some(4);
        app.pending_saves.insert(
            4,
            PendingSave::new(
                "destination.pdf".into(),
                "Save completion is uncertain.".into(),
            ),
        );
        let context = egui::Context::default();
        click_label(&mut app, &context, "Cancel recorded save…");
        assert!(app.pending_saves[&4].confirm_cancel);
        assert!(!app.pending_saves[&4].submitting && !app.busy);
        click_label(&mut app, &context, "Keep recorded attempt");
        assert!(!app.pending_saves[&4].confirm_cancel);
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        click_label(&mut app, &context, "Retry verification and save");
        assert!(app.pending_saves[&4].submitting && app.busy);
        sender
            .send(crate::worker::Event::Error {
                id: 4,
                task: None,
                message: "Destination differs; preserved.".into(),
            })
            .expect("event");
        app.poll(&context);
        assert!(!app.pending_saves[&4].submitting);
        assert_eq!(app.tabs.len(), 1);
        assert!(app.tabs[0].status.dirty);
        assert!(app.needs_exit_guard());
        app.pending_saves.get_mut(&4).expect("attempt").submitting = true;
        sender
            .send(crate::worker::Event::Updated {
                id: 4,
                status: status(),
                selected_page: None,
            })
            .expect("cancel acknowledgement");
        app.poll(&context);
        assert!(app.pending_saves.is_empty());
        assert!(app.tabs[0].status.dirty);
    }
}
