use super::EditorApp;
use crate::worker::{Command, Edit};
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Extract,
    Delete,
    Duplicate,
}

pub(super) struct Dialog {
    pub tab: u64,
    pub revision: u64,
    page: u32,
    count: usize,
    pub mode: Mode,
    text: String,
    focus: bool,
    confirm: bool,
    pub submitting: bool,
    pub expected_id: Option<u64>,
    pub error: Option<String>,
}
impl Dialog {
    fn selection(&self) -> Result<Vec<u32>, String> {
        if self.text.len() > 8192 {
            return Err("Page selection must fit within 8 KiB of text.".into());
        }
        let pages =
            pdf_core::selection::parse_unique(&self.text, self.count).map_err(|e| e.to_string())?;
        if self.mode == Mode::Delete && pages.len() == self.count {
            return Err("Keep at least one page in the source document.".into());
        }
        if self.mode == Mode::Duplicate
            && self
                .count
                .checked_add(pages.len())
                .is_none_or(|count| count > pdf_core::MAX_PAGES)
        {
            return Err("Duplicated pages exceed the document page limit.".into());
        }
        Ok(pages)
    }
}
impl EditorApp {
    pub(super) fn open_page_ranges(&mut self, mode: Mode) {
        if self.busy
            || !self.pending_saves.is_empty()
            || (mode == Mode::Extract && self.tabs.len() >= crate::worker::MAX_TABS)
        {
            return;
        }
        let Some(tab) = self.selected().filter(|t| t.editable()) else {
            return;
        };
        self.pages_dialog = Some(Dialog {
            tab: tab.id,
            revision: tab.status.revision,
            page: tab.page as u32 + 1,
            count: tab.status.info.page_count,
            mode,
            text: (tab.page + 1).to_string(),
            focus: true,
            confirm: false,
            submitting: false,
            expected_id: None,
            error: None,
        });
    }
    fn page_dialog_current(&self) -> bool {
        self.pages_dialog.as_ref().is_some_and(|d| {
            self.active == Some(d.tab)
                && self.tabs.iter().any(|t| {
                    t.id == d.tab
                        && t.status.revision == d.revision
                        && t.status.info.page_count == d.count
                        && t.editable()
                })
        })
    }
    fn apply_page_ranges(&mut self) -> bool {
        if self.busy || !self.pending_saves.is_empty() || !self.page_dialog_current() {
            return false;
        }
        let Some(dialog) = self.pages_dialog.as_ref().filter(|d| !d.submitting) else {
            return false;
        };
        let Ok(pages) = dialog.selection() else {
            return false;
        };
        let id = dialog.tab;
        let revision = dialog.revision;
        let mode = dialog.mode;
        let new_id = self.next_id;
        let command = match mode {
            Mode::Extract => {
                if self.tabs.len() >= crate::worker::MAX_TABS || new_id.checked_add(1).is_none() {
                    return false;
                }
                Command::ExtractPages {
                    id,
                    revision,
                    new_id,
                    pages,
                }
            }
            Mode::Delete => {
                if !dialog.confirm {
                    return false;
                }
                Command::Edit {
                    id,
                    edit: Edit::DeletePages {
                        revision,
                        pages,
                        page: dialog.page,
                    },
                }
            }
            Mode::Duplicate => Command::Edit {
                id,
                edit: Edit::DuplicatePages {
                    revision,
                    pages,
                    page: dialog.page,
                },
            },
        };
        if !self.send(
            command,
            match mode {
                Mode::Extract => "Extracting selected pages into a new PDF…",
                Mode::Delete => "Deleting selected pages…",
                Mode::Duplicate => "Duplicating selected pages…",
            },
        ) {
            return false;
        }
        if mode == Mode::Extract {
            self.next_id = new_id + 1;
        }
        if let Some(dialog) = self.pages_dialog.as_mut() {
            dialog.submitting = true;
            dialog.expected_id = (mode == Mode::Extract).then_some(new_id);
            dialog.error = None;
        }
        true
    }
    pub(super) fn page_ranges_dialog(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let current = self.page_dialog_current();
        let capacity = self.tabs.len() < crate::worker::MAX_TABS;
        let Some(dialog) = self.pages_dialog.as_mut() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        let mut apply = false;
        let title = match dialog.mode {
            Mode::Extract => "Extract page range",
            Mode::Delete => "Delete page range",
            Mode::Duplicate => "Duplicate page range",
        };
        egui::Window::new(title).open(&mut open).collapsible(false).resizable(false)
            .default_width(480.0).max_width((context.content_rect().width()-32.0).max(280.0))
            .anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).show(context, |ui| {
                egui::ScrollArea::vertical().max_height((context.content_rect().height()-150.0).max(80.0)).show(ui,|ui| {
                    ui.label("Use one-based page numbers and ascending ranges, separated by commas: 1-3, 5. Overlapping selections are refused.");
                    let label=ui.label("Pages");
                    let entry=ui.add_enabled(!dialog.submitting,egui::TextEdit::singleline(&mut dialog.text).char_limit(4096)).labelled_by(label.id);
                    if dialog.focus { entry.request_focus(); dialog.focus=false; }
                    if entry.changed() { dialog.confirm=false; dialog.error=None; }
                    match dialog.selection() {
                        Ok(pages) => { ui.label(match dialog.mode {
                            Mode::Extract => format!("{} selected page(s) → new unsaved tab. Source: {} pages, unchanged.",pages.len(),dialog.count),
                            Mode::Delete => format!("Delete {} of {} page(s); {} remain.", pages.len(),dialog.count,dialog.count-pages.len()),
                            Mode::Duplicate => format!("Duplicate {} page(s); {} total. Each copy follows its original.", pages.len(),dialog.count+pages.len()),
                        }); }
                        Err(error) => { ui.colored_label(ui.visuals().error_fg_color,error); }
                    }
                    ui.label("This guarded operation may refuse forms, bookmarks or other document structures. A refusal preserves your PDFs.");
                    if dialog.mode == Mode::Delete { ui.label("Page deletion does not securely erase content. Undo restores the prior document while its history remains available."); }
                    if !current { ui.colored_label(ui.visuals().error_fg_color,"The source document changed. Copy your selection, close this dialog and reopen it."); }
                    if dialog.mode == Mode::Extract && !capacity { ui.label("Close a tab before extracting: at most 16 PDFs may be open."); }
                    if let Some(error)=&dialog.error { ui.colored_label(ui.visuals().error_fg_color,error); }
                    if dialog.confirm { ui.label("Delete the selected pages from this open PDF? The original file is preserved; Save As writes a new file."); }
                });
                ui.separator();
                let ready=current && !self.busy && self.pending_saves.is_empty() && !dialog.submitting && dialog.selection().is_ok() && (dialog.mode != Mode::Extract || capacity);
                ui.horizontal_wrapped(|ui| {
                    if dialog.mode == Mode::Delete && !dialog.confirm {
                        if ui.add_enabled(ready,egui::Button::new("Review page deletion…")).clicked() { dialog.confirm=true; }
                    } else {
                        apply=ui.add_enabled(ready,crate::style::primary(match dialog.mode {
                            Mode::Extract => "Extract to new tab",
                            Mode::Delete => "Confirm page deletion",
                            Mode::Duplicate => "Duplicate selected pages",
                        },ui.visuals().dark_mode)).clicked();
                    }
                    close=ui.add_enabled(!dialog.submitting,egui::Button::new("Cancel page operation")).clicked();
                    if dialog.submitting { ui.spinner(); ui.label("Working…"); }
                });
            });
        if apply {
            self.apply_page_ranges();
        } else if !dialog.submitting
            && (close || !open || context.input(|i| i.key_pressed(egui::Key::Escape)))
        {
            self.pages_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DocumentStatus, Tab};
    fn status(count: usize) -> DocumentStatus {
        DocumentStatus {
            info: pdf_core::DocumentInfo {
                version: "1.7".into(),
                page_count: count,
                object_count: 5,
                encrypted: false,
                signature_fields: 0,
                pages: vec![],
            },
            revision: 4,
            dirty: true,
            can_undo: true,
            can_redo: false,
        }
    }
    fn app(mode: Mode) -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ));
        let mut tab = Tab::new(7, "source.pdf".into(), status(4));
        tab.needs_render = false;
        app.tabs.push(tab);
        app.active = Some(7);
        app.next_id = 9;
        app.open_page_ranges(mode);
        app
    }
    #[test]
    fn shared_selection_preserves_order_and_blocks_overlaps_all_page_deletion_and_large_input() {
        let mut app = app(Mode::Extract);
        let dialog = app.pages_dialog.as_mut().expect("dialog");
        dialog.text = "4,1-2".into();
        assert_eq!(dialog.selection().expect("selection"), vec![4, 1, 2]);
        for invalid in ["0", "5", "3-2", "1-2,2", ""] {
            dialog.text = invalid.into();
            assert!(dialog.selection().is_err());
        }
        dialog.text = " ".repeat(8193);
        assert!(dialog.selection().is_err());
        dialog.mode = Mode::Delete;
        dialog.text = "1-4".into();
        assert!(dialog.selection().is_err());
        dialog.text = "2-3".into();
        assert_eq!(dialog.selection().expect("delete"), vec![2, 3]);
    }
    #[test]
    fn stale_draft_and_capacity_cannot_dispatch_or_allocate_new_ids() {
        let mut app = app(Mode::Extract);
        app.pages_dialog.as_mut().expect("dialog").text = "1-2".into();
        app.tabs[0].status.revision += 1;
        assert!(!app.apply_page_ranges());
        assert_eq!(app.next_id, 9);
        assert_eq!(
            app.pages_dialog.as_ref().expect("retained selection").text,
            "1-2"
        );
        app.tabs[0].status.revision = 4;
        app.active = Some(999);
        assert!(!app.apply_page_ranges());
        app.active = Some(7);
        for id in 20..35 {
            app.tabs.push(Tab::new(id, "other.pdf".into(), status(1)));
        }
        assert_eq!(app.tabs.len(), 16);
        assert!(!app.apply_page_ranges());
        assert_eq!(app.next_id, 9);
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
            |_| app.page_ranges_dialog(ctx),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, ctx: &egui::Context, label: &str) {
        let _ = draw(app, ctx, vec![]);
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
            .expect("fully visible button");
        let _ = draw(
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
        let _ = draw(
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
    #[test]
    fn actual_egui_delete_review_sends_nothing_until_confirmation_and_cancel_retains_work() {
        let mut app = app(Mode::Delete);
        app.pages_dialog.as_mut().expect("dialog").text = "2-3".into();
        assert!(!app.apply_page_ranges());
        let context = egui::Context::default();
        click(&mut app, &context, "Review page deletion…");
        assert!(app.pages_dialog.as_ref().expect("confirmation").confirm);
        assert!(!app.busy);
        click(&mut app, &context, "Cancel page operation");
        assert!(app.pages_dialog.is_none());
        assert_eq!(app.tabs[0].status.info.page_count, 4);
        assert!(app.tabs[0].status.dirty);
    }
    #[test]
    fn typed_extract_result_opens_only_expected_unsaved_tab_and_keeps_source_history() {
        let mut app = app(Mode::Extract);
        let dialog = app.pages_dialog.as_mut().expect("dialog");
        dialog.submitting = true;
        dialog.expected_id = Some(9);
        app.busy = true;
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        let mut extracted = status(2);
        extracted.revision = 0;
        extracted.can_undo = false;
        sender
            .send(crate::worker::Event::Extracted {
                id: 9,
                path: "extracted.pdf".into(),
                status: extracted,
            })
            .expect("event");
        // Suppress background rendering of this UI-only fixture after accepting the result.
        app.close_target = Some(super::super::CloseTarget::Exit);
        app.poll(&egui::Context::default());
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, Some(9));
        assert!(app.pages_dialog.is_none());
        assert_eq!(app.tabs[0].id, 7);
        assert_eq!(app.tabs[0].status.revision, 4);
        assert!(app.tabs[0].status.can_undo);
        assert!(app.tabs[1].status.dirty);
        assert_eq!(app.tabs[1].status.info.page_count, 2);
        sender
            .send(crate::worker::Event::Extracted {
                id: 9,
                path: "stale.pdf".into(),
                status: status(1),
            })
            .expect("stale event");
        app.poll(&egui::Context::default());
        assert_eq!(app.tabs.len(), 2);
    }
    #[test]
    fn failed_page_operation_retains_selection_and_source_without_canvas_error() {
        for mode in [Mode::Delete, Mode::Duplicate] {
            let mut app = app(mode);
            let dialog = app.pages_dialog.as_mut().expect("dialog");
            dialog.text = "2".into();
            dialog.confirm = true;
            dialog.submitting = true;
            app.busy = true;
            let (sender, events) = std::sync::mpsc::channel();
            app.worker.events = events;
            sender
                .send(crate::worker::Event::Error {
                    id: 7,
                    task: None,
                    message: "Guarded subset refused this document.".into(),
                })
                .expect("error");
            app.poll(&egui::Context::default());
            let dialog = app.pages_dialog.as_ref().expect("selection retained");
            assert!(!dialog.submitting);
            assert_eq!(dialog.text, "2");
            assert!(dialog.error.is_some());
            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.tabs[0].status.revision, 4);
            assert!(app.tabs[0].error.is_none());
            assert!(app.tabs[0].status.can_undo);
        }
    }
    #[test]
    fn actual_egui_stale_selection_remains_copyable_and_extract_button_cannot_dispatch() {
        let mut app = app(Mode::Extract);
        app.pages_dialog.as_mut().expect("dialog").text = "3,1".into();
        app.tabs[0].status.revision = 5;
        let context = egui::Context::default();
        click(&mut app, &context, "Extract to new tab");
        assert!(!app.busy);
        assert_eq!(app.next_id, 9);
        assert_eq!(
            app.pages_dialog
                .as_ref()
                .expect("stale draft retained")
                .text,
            "3,1"
        );
        assert_eq!(app.tabs[0].status.revision, 5);
    }

    #[test]
    fn duplicate_ranges_allow_all_pages_without_delete_confirmation_and_enforce_growth_limit() {
        let mut app = app(Mode::Duplicate);
        let dialog = app.pages_dialog.as_mut().expect("duplicate dialog");
        dialog.text = "4,1-3".into();
        assert_eq!(
            dialog.selection().expect("all original pages"),
            vec![4, 1, 2, 3]
        );
        assert!(!dialog.confirm);
        dialog.count = pdf_core::MAX_PAGES;
        dialog.text = "1".into();
        assert!(dialog.selection().is_err());
    }

    #[test]
    fn duplicate_drafts_refuse_stale_revision_and_unresolved_save_without_losing_selection() {
        let mut app = app(Mode::Duplicate);
        app.pages_dialog.as_mut().expect("dialog").text = "3,1".into();
        app.tabs[0].status.revision = 5;
        assert!(!app.apply_page_ranges());
        app.tabs[0].status.revision = 4;
        app.pending_saves.insert(
            7,
            super::super::recovery::PendingSave::new(
                "pending.pdf".into(),
                "Unresolved save".into(),
            ),
        );
        assert!(!app.apply_page_ranges());
        assert_eq!(app.pages_dialog.as_ref().expect("draft").text, "3,1");
        assert!(!app.busy);
        assert_eq!(app.next_id, 9);
        app.pages_dialog = None;
        app.open_page_ranges(Mode::Duplicate);
        assert!(app.pages_dialog.is_none());
    }

    #[test]
    fn duplicate_result_updates_existing_tab_focus_and_closes_only_submitted_dialog() {
        let mut app = app(Mode::Duplicate);
        let dialog = app.pages_dialog.as_mut().expect("dialog");
        dialog.submitting = true;
        dialog.text = "1,3".into();
        app.busy = true;
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        let mut updated = status(6);
        updated.revision = 5;
        sender
            .send(crate::worker::Event::Updated {
                id: 7,
                status: updated,
                selected_page: Some(3),
            })
            .expect("duplicate event");
        app.close_target = Some(super::super::CloseTarget::Exit);
        app.poll(&egui::Context::default());
        assert!(app.pages_dialog.is_none());
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].page, 3);
        assert_eq!(app.tabs[0].status.info.page_count, 6);
        assert!(app.tabs[0].status.can_undo && app.tabs[0].status.dirty);
    }

    #[test]
    fn actual_egui_duplicate_action_dispatches_without_delete_confirmation_or_new_tab_id() {
        let mut app = app(Mode::Duplicate);
        app.pages_dialog.as_mut().expect("dialog").text = "1,3".into();
        click(
            &mut app,
            &egui::Context::default(),
            "Duplicate selected pages",
        );
        assert!(app.busy);
        assert!(
            app.pages_dialog
                .as_ref()
                .expect("submitted duplicate")
                .submitting
        );
        assert!(
            !app.pages_dialog
                .as_ref()
                .expect("no deletion confirmation")
                .confirm
        );
        assert_eq!(app.next_id, 9);
        assert_eq!(app.tabs.len(), 1);
    }

    #[test]
    fn actual_command_palette_duplicate_entry_opens_the_guarded_range_dialog() {
        let mut app = app(Mode::Duplicate);
        app.pages_dialog = None;
        app.palette_open = true;
        app.palette_query = "duplicate".into();
        let context = egui::Context::default();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |_| app.command_palette(&context),
        );
        output.textures_delta.clear();
        assert!(!app.palette_open);
        assert!(
            app.pages_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.mode == Mode::Duplicate)
        );
        assert!(!app.busy);
    }
}
