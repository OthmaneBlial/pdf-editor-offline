use super::EditorApp;
use crate::worker::{Command, Edit, NotesKey};
use eframe::egui;
use pdf_worker::document::{NoteChange, NoteEntry};
pub(super) struct Dialog {
    pub key: NotesKey,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    entries: Option<Vec<NoteEntry>>,
    next_offset: Option<u32>,
    selected: Option<usize>,
    contents: String,
    author: String,
    rectangle: [f32; 4],
    initial_rectangle: [f32; 4],
    confirm_remove: bool,
}

impl Dialog {
    pub(super) fn has_changes(&self) -> bool {
        match self
            .selected
            .and_then(|index| self.entries.as_ref()?.get(index))
        {
            Some(entry) => self.contents != entry.note.contents || self.author != entry.note.author,
            None => {
                !self.contents.is_empty()
                    || !self.author.is_empty()
                    || self.rectangle != self.initial_rectangle
            }
        }
    }
    fn valid(&self) -> bool {
        self.contents.len() <= pdf_core::MAX_NOTE_CONTENT_BYTES
            && self.author.len() <= pdf_core::MAX_NOTE_AUTHOR_BYTES
            && !self.contents.contains('\0')
            && !self.author.contains('\0')
            && self.rectangle.iter().all(|n| n.is_finite())
    }
    fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        self.confirm_remove = false;
        self.error = None;
        if let Some(entry) = index.and_then(|index| self.entries.as_ref()?.get(index)) {
            self.contents = entry.note.contents.clone();
            self.author = entry.note.author.clone();
            self.rectangle = entry.note.rectangle;
        } else {
            self.contents.clear();
            self.author.clear();
            self.rectangle = self.initial_rectangle;
        }
    }
}
impl EditorApp {
    pub(super) fn open_notes(&mut self) {
        if self.busy || self.notes_dialog.is_some() {
            return;
        }
        let Some(tab) = self.selected() else {
            return;
        };
        let (tab, revision, page, geometry) = (
            tab.id,
            tab.status.revision,
            tab.page as u32 + 1,
            tab.status.info.pages[tab.page].clone(),
        );
        self.notes_generation = self.notes_generation.wrapping_add(1);
        let key = NotesKey {
            tab,
            revision,
            page,
            offset: 0,
            generation: self.notes_generation,
        };
        let left = geometry.media_box[0].max(geometry.crop_box[0]);
        let top = geometry.media_box[3].min(geometry.crop_box[3]);
        self.notes_dialog = Some(Dialog {
            key,
            submitting: false,
            stale: false,
            error: None,
            entries: None,
            next_offset: None,
            selected: None,
            contents: String::new(),
            author: String::new(),
            rectangle: [left + 12.0, top - 36.0, left + 36.0, top - 12.0],
            initial_rectangle: [left + 12.0, top - 36.0, left + 36.0, top - 12.0],
            confirm_remove: false,
        });
        if !self.send(Command::Notes(key), "Reading sticky notes locally…")
            && let Some(dialog) = self.notes_dialog.as_mut()
        {
            dialog.error = Some("Cannot request notes".into());
        }
    }
    pub(super) fn accept_notes(
        &mut self,
        key: NotesKey,
        entries: Vec<NoteEntry>,
        next_offset: Option<u32>,
    ) {
        if self.active == Some(key.tab)
            && self
                .tabs
                .iter()
                .any(|tab| tab.id == key.tab && tab.status.revision == key.revision)
            && let Some(dialog) = self
                .notes_dialog
                .as_mut()
                .filter(|dialog| dialog.key == key && !dialog.stale)
        {
            dialog.entries = Some(entries);
            dialog.next_offset = next_offset;
            dialog.error = None;
        }
    }
    fn apply_note(&mut self, remove: bool) -> bool {
        let Some(dialog) = self.notes_dialog.as_ref() else {
            return false;
        };
        if self.busy
            || !self.pending_saves.is_empty()
            || dialog.submitting
            || dialog.stale
            || !dialog.valid()
            || dialog.entries.is_none()
            || !self.tabs.iter().any(|tab| {
                Some(tab.id) == self.active
                    && tab.id == dialog.key.tab
                    && tab.status.revision == dialog.key.revision
                    && tab.editable()
            })
        {
            return false;
        }
        let selected = dialog
            .selected
            .and_then(|index| dialog.entries.as_ref()?.get(index));
        let change = if let Some(entry) = selected {
            if !entry.editable {
                return false;
            }
            if remove {
                if !dialog.confirm_remove {
                    return false;
                }
                NoteChange::Remove {
                    page: dialog.key.page,
                    annotation_index: entry.note.annotation_index,
                    expected_id: entry.note.object_id,
                }
            } else {
                if !dialog.has_changes() {
                    return false;
                }
                NoteChange::Edit {
                    page: dialog.key.page,
                    annotation_index: entry.note.annotation_index,
                    expected_id: entry.note.object_id,
                    contents: Some(dialog.contents.clone()),
                    author: Some(dialog.author.clone()),
                }
            }
        } else {
            if remove {
                return false;
            }
            NoteChange::Add {
                page: dialog.key.page,
                rectangle: dialog.rectangle,
                contents: dialog.contents.clone(),
                author: dialog.author.clone(),
            }
        };
        let key = dialog.key;
        if self.send(
            Command::Edit {
                id: key.tab,
                edit: Edit::Note { key, change },
            },
            if remove {
                "Removing note occurrence…"
            } else {
                "Saving note to document history…"
            },
        ) {
            if let Some(dialog) = self.notes_dialog.as_mut() {
                dialog.submitting = true;
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn notes_dialog_ui(&mut self, context: &egui::Context) {
        if self.notes_dialog.is_none() || self.close_target.is_some() {
            return;
        }
        let editable = self.selected().is_some_and(|tab| tab.editable());
        let mut apply = false;
        let mut remove = false;
        let mut close = false;
        let mut offset = None;
        let Some(dialog) = self.notes_dialog.as_mut() else {
            return;
        };
        egui::Window::new("Sticky notes on this page").collapsible(false).resizable(false).default_width(520.0).max_width((context.content_rect().width()-32.0).max(280.0)).max_height((context.content_rect().height()-60.0).max(200.0)).vscroll(true).show(context,|ui|{
            ui.label(format!("Page {} · Contents and author are annotation metadata, not page text.",dialog.key.page));
            ui.label("New notes use the supported yellow vector icon. Editing preserves existing geometry, flags and appearance. Removal is ordinary editing: old values can remain recoverable.");
            if dialog.stale {ui.colored_label(egui::Color32::YELLOW,"The source changed. Close and reopen; your draft remains here until then.");}
            if let Some(entries)=&dialog.entries {
                let mut select=None;
                if ui.add_enabled(!dialog.submitting && !dialog.has_changes(),egui::Button::new("New yellow note")).clicked(){select=Some(None);}
                for (index,entry) in entries.iter().enumerate() {if ui.add_enabled(!dialog.submitting && !dialog.has_changes(),egui::Button::new(format!("Note occurrence {} · {}",entry.note.annotation_index,if entry.editable{"editable"}else{"read only"})).selected(dialog.selected==Some(index))).clicked(){select=Some(Some(index));}}
                if let Some(index)=select {dialog.select(index);}
            } else {ui.label("Reading this page's notes…");}
            let selected=dialog.selected.and_then(|index|dialog.entries.as_ref()?.get(index));
            let allowed=editable && !dialog.stale && !dialog.submitting && !self.busy && self.pending_saves.is_empty() && selected.is_none_or(|entry|entry.editable);
            if let Some(reason)=selected.and_then(|entry|entry.refusal_reason.as_ref()){ui.label(reason);}
            ui.add_enabled_ui(allowed,|ui|{
                ui.label("Contents (up to 64 KiB UTF-8)");ui.add(egui::TextEdit::multiline(&mut dialog.contents).desired_rows(4));ui.label("Author/title (up to 1 KiB UTF-8)");ui.text_edit_singleline(&mut dialog.author);
                if dialog.selected.is_none(){ui.label("Rectangle in unrotated PDF points, origin at bottom-left: x₀, y₀, x₁, y₁");ui.horizontal(|ui|{for value in &mut dialog.rectangle {ui.add(egui::DragValue::new(value).speed(1.0));}});}
            });
            if !dialog.valid(){ui.colored_label(egui::Color32::LIGHT_RED,"Text exceeds its byte limit, contains NUL, or position is non-finite. Draft retained; Apply disabled.");}
            if let Some(error)=&dialog.error {ui.colored_label(egui::Color32::LIGHT_RED,error);}
            if dialog.has_changes() && ui.button("Copy draft").clicked(){context.copy_text(format!("Author/title: {}\nRectangle: {:?}\nContents:\n{}",dialog.author,dialog.rectangle,dialog.contents));}
            ui.horizontal(|ui|{
                if ui.add_enabled(allowed && dialog.entries.is_some() && dialog.valid() && (dialog.selected.is_none() || dialog.has_changes()),egui::Button::new(if dialog.selected.is_some(){"Apply note changes"}else{"Add note"})).clicked(){apply=true;}
                if ui.add_enabled(allowed && dialog.selected.is_some(),egui::Button::new(if dialog.confirm_remove{"Confirm remove occurrence"}else{"Remove note…"})).clicked(){if dialog.confirm_remove{remove=true;}else{dialog.confirm_remove=true;}}
                if ui.add_enabled(!dialog.submitting,egui::Button::new(if dialog.has_changes(){"Discard draft and close"}else{"Close"})).clicked(){close=true;}
            });
            if !dialog.has_changes() && !dialog.submitting {ui.horizontal(|ui|{
                if ui.add_enabled(dialog.key.offset>0 && !self.busy,egui::Button::new("First notes")).clicked(){offset=Some(0);}
                if let Some(next)=dialog.next_offset && ui.add_enabled(!self.busy,egui::Button::new("Next notes")).clicked(){offset=Some(next);}
            });}
        });
        if let Some(offset) = offset
            && let Some(dialog) = self.notes_dialog.as_mut()
        {
            self.notes_generation = self.notes_generation.wrapping_add(1);
            dialog.key.offset = offset;
            dialog.key.generation = self.notes_generation;
            dialog.entries = None;
            dialog.selected = None;
            let key = dialog.key;
            self.send(Command::Notes(key), "Reading next notes…");
        }
        if apply {
            self.apply_note(false);
        }
        if remove {
            self.apply_note(true);
        }
        if close {
            self.notes_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::state::{DocumentStatus, Tab};
    fn app() -> EditorApp {
        let context = egui::Context::default();
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
        let info = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/bookmarks.pdf"),
            None,
        )
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
        app.open_notes();
        // Complete the real discovery request before injecting UI fixture entries.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.notes_dialog.as_ref().unwrap().error.is_none()
            && std::time::Instant::now() < deadline
        {
            app.poll(&context);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.notes_dialog.as_ref().unwrap().error.is_some());
        assert!(!app.busy);
        let key = app.notes_dialog.as_ref().unwrap().key;
        app.accept_notes(
            key,
            vec![NoteEntry {
                note: pdf_core::Note {
                    page: 1,
                    annotation_index: 1,
                    object_id: Some((90, 0)),
                    rectangle: [30.0, 30.0, 54.0, 54.0],
                    contents: "Old comment".into(),
                    author: "Reviewer".into(),
                    flags: 4,
                },
                editable: true,
                refusal_reason: None,
            }],
            None,
        );
        app
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
                    egui::vec2(800.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.notes_dialog_ui(context),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        let _ = draw(app, context, vec![]);
        let output = draw(app, context, vec![]);
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
            .expect("fully visible control");
        let _ = draw(
            app,
            context,
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
            context,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn actual_selection_keyboard_draft_byte_limits_and_stale_apply_preserve_draft() {
        let mut app = app();
        let context = egui::Context::default();
        click(&mut app, &context, "Note occurrence 1 · editable");
        click(&mut app, &context, "Old comment");
        let _ = draw(&mut app, &context, vec![egui::Event::Text(" 中文".into())]);
        assert!(app.notes_dialog.as_ref().unwrap().contents.contains("中文"));
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.notes_dialog.as_ref().unwrap().contents.contains('\n'));
        assert!(!app.notes_dialog.as_ref().unwrap().submitting);
        app.tabs[0].status.revision += 1;
        assert!(!app.apply_note(false));
        assert!(app.notes_dialog.as_ref().unwrap().contents.contains("中文"));
        app.notes_dialog.as_mut().unwrap().author = "😀".repeat(257);
        assert!(!app.notes_dialog.as_ref().unwrap().valid());
        assert!(!app.apply_note(false));
        assert_eq!(app.notes_dialog.as_ref().unwrap().author.len(), 1028);
    }
    #[test]
    fn removal_requires_confirmation_and_failed_actor_keeps_original_and_dialog() {
        let mut app = app();
        let context = egui::Context::default();
        click(&mut app, &context, "Note occurrence 1 · editable");
        assert!(!app.apply_note(true));
        click(&mut app, &context, "Remove note…");
        assert!(!app.busy);
        assert!(app.notes_dialog.as_ref().unwrap().confirm_remove);
        click(&mut app, &context, "Confirm remove occurrence");
        assert!(
            app.notes_dialog.as_ref().unwrap().submitting,
            "confirmation did not submit: message={:?}, busy={}, stale={}, selected={:?}, confirm={}, valid={}",
            app.message,
            app.busy,
            app.notes_dialog.as_ref().unwrap().stale,
            app.notes_dialog.as_ref().unwrap().selected,
            app.notes_dialog.as_ref().unwrap().confirm_remove,
            app.notes_dialog.as_ref().unwrap().valid(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.notes_dialog.as_ref().unwrap().submitting && std::time::Instant::now() < deadline
        {
            app.poll(&context);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!app.notes_dialog.as_ref().unwrap().submitting);
        assert!(app.notes_dialog.as_ref().unwrap().error.is_some());
        assert_eq!(app.tabs.len(), 1);
        assert!(!app.tabs[0].status.dirty);
        assert_eq!(app.notes_dialog.as_ref().unwrap().contents, "Old comment");
    }
    #[test]
    fn readonly_eligibility_and_pending_saves_never_apply() {
        let mut app = app();
        app.notes_dialog.as_mut().unwrap().select(Some(0));
        app.notes_dialog.as_mut().unwrap().entries.as_mut().unwrap()[0].editable = false;
        app.notes_dialog.as_mut().unwrap().contents = "Changed".into();
        assert!(!app.apply_note(false));
        app.notes_dialog.as_mut().unwrap().entries.as_mut().unwrap()[0].editable = true;
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_note(false));
        assert_eq!(app.notes_dialog.as_ref().unwrap().contents, "Changed");
        app.tabs[0].status.info.signature_fields = 0;
        app.pending_saves.insert(
            1,
            super::super::recovery::PendingSave::new(
                "pending.pdf".into(),
                "Resolve recorded save".into(),
            ),
        );
        assert!(!app.apply_note(false));
        assert_eq!(app.notes_dialog.as_ref().unwrap().contents, "Changed");
    }
    #[test]
    fn clean_tab_note_draft_requires_explicit_close_choice_and_remains_on_return() {
        let mut app = app();
        app.notes_dialog.as_mut().unwrap().contents = "Unapplied draft".into();
        assert!(!app.tabs[0].status.dirty);
        assert!(app.needs_exit_guard());
        app.request_close(1);
        assert_eq!(app.close_target, Some(super::super::CloseTarget::Tab(1)));
        assert!(!app.busy);
        app.close_target = None;
        assert_eq!(
            app.notes_dialog.as_ref().unwrap().contents,
            "Unapplied draft"
        );
        assert_eq!(app.tabs.len(), 1);
    }
}
