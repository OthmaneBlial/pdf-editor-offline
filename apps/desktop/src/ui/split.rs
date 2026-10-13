use super::EditorApp;
use crate::{
    state::Tab,
    worker::{Command, SplitDocument},
};
use eframe::egui;
pub(super) struct Dialog {
    pub source: u64,
    revision: u64,
    every: u32,
    pub expected: Option<Vec<u64>>,
    pub error: Option<String>,
}
impl EditorApp {
    pub(super) fn open_split(&mut self) {
        if self.busy || !self.pending_saves.is_empty() {
            return;
        }
        let Some(tab) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        self.split_dialog = Some(Dialog {
            source: tab.id,
            revision: tab.status.revision,
            every: 1,
            expected: None,
            error: None,
        });
    }
    fn split_current(&self) -> bool {
        self.split_dialog.as_ref().is_some_and(|dialog| {
            self.active == Some(dialog.source)
                && self.tabs.iter().any(|tab| {
                    tab.id == dialog.source
                        && tab.status.revision == dialog.revision
                        && tab.editable()
                })
        })
    }
    fn apply_split(&mut self) -> bool {
        if self.busy || !self.pending_saves.is_empty() || !self.split_current() {
            return false;
        }
        let Some(dialog) = self
            .split_dialog
            .as_ref()
            .filter(|dialog| dialog.expected.is_none() && dialog.every > 0)
        else {
            return false;
        };
        let Some(tab) = self.selected() else {
            return false;
        };
        let count = tab.status.info.page_count.div_ceil(dialog.every as usize);
        if count > crate::worker::MAX_TABS.saturating_sub(self.tabs.len()) || self.next_id == 0 {
            return false;
        }
        let Some(next) = self
            .next_id
            .checked_add(count as u64)
            .filter(|next| *next < u64::MAX)
        else {
            return false;
        };
        let ids = (self.next_id..next).collect::<Vec<_>>();
        if self.send(
            Command::Split {
                id: dialog.source,
                revision: dialog.revision,
                every: dialog.every,
                new_ids: ids.clone(),
            },
            "Creating verified split tabs…",
        ) {
            self.next_id = next;
            if let Some(dialog) = self.split_dialog.as_mut() {
                dialog.expected = Some(ids);
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn accept_split(
        &mut self,
        source: u64,
        revision: u64,
        documents: Vec<SplitDocument>,
        context: &egui::Context,
    ) {
        let ids = documents
            .iter()
            .map(|document| document.id)
            .collect::<Vec<_>>();
        let valid = self.split_dialog.as_ref().is_some_and(|dialog| {
            dialog.source == source
                && dialog.revision == revision
                && dialog.expected.as_ref() == Some(&ids)
        }) && self.split_current()
            && !documents.is_empty()
            && documents.len() <= crate::worker::MAX_TABS.saturating_sub(self.tabs.len())
            && ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == ids.len()
            && documents.iter().all(|document| {
                document.id != 0
                    && document.id != u64::MAX
                    && !self.tabs.iter().any(|tab| tab.id == document.id)
                    && document.status.dirty
                    && document.status.revision == 0
            });
        if !valid {
            self.message = "Unexpected split result ignored; existing tabs retained.".into();
            if let Some(dialog) = self.split_dialog.as_mut() {
                dialog.expected = None;
                dialog.error = Some(self.message.clone());
            }
            return;
        }
        let first = ids[0];
        for document in documents {
            let mut tab = Tab::new(document.id, document.path, document.status);
            tab.set_device_ratio(context.pixels_per_point());
            self.tabs.push(tab);
            self.next_id = self.next_id.max(document.id + 1);
        }
        self.active = Some(first);
        self.split_dialog = None;
        self.message="Split tabs created and journaled together. Source unchanged. Save each copy as a new file; other pages' unreachable data may remain.".into();
    }
    pub(super) fn split_dialog_ui(&mut self, context: &egui::Context) {
        if self.split_dialog.is_none() || self.close_target.is_some() {
            return;
        }
        let current = self.split_current();
        let pages = self.selected().map_or(0, |tab| tab.status.info.page_count);
        let available = crate::worker::MAX_TABS.saturating_sub(self.tabs.len());
        let mut apply = false;
        let mut close = false;
        let Some(dialog) = self.split_dialog.as_mut() else {
            return;
        };
        egui::Window::new("Split into new unsaved tabs")
            .collapsible(false)
            .resizable(false)
            .default_width(470.0)
            .max_width((context.content_rect().width() - 32.0).max(240.0))
            .show(context, |ui| {
            ui.label("Consecutive pages are copied into separate tabs. The source and its history remain unchanged; the entire new batch is journaled before success.");
            ui.label("Guarded subsets refuse structures requiring remapping, including bookmarks, forms, annotations and tags. Signed/encrypted inputs are readonly. Serialized results total at most 128 MiB; conservative copy limits apply.");
            ui.label("Splitting is not privacy cleanup or secure redaction: other pages' unreachable objects may remain in each PDF. Save As publishes each tab separately; there is no atomic multi-file save.");
            ui.horizontal(|ui| {
                ui.label("Pages per tab");
                ui.add_enabled(dialog.expected.is_none(), egui::DragValue::new(&mut dialog.every).range(1..=pages.max(1) as u32));
            });
            let count = pages.div_ceil(dialog.every.max(1) as usize);
            ui.label(format!("{count} new tabs · {available} available slots"));
            if !current {
                ui.colored_label(egui::Color32::YELLOW, "Source changed. Cancel and review the current document.");
            }
            if count > available {
                ui.label("Increase pages per tab or close existing tabs first.");
            }
            if let Some(error) = &dialog.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(current && !self.busy && self.pending_saves.is_empty() && dialog.expected.is_none() && count <= available, egui::Button::new("Create split tabs")).clicked() {
                    apply = true;
                }
                if ui.add_enabled(dialog.expected.is_none(), egui::Button::new("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        if apply {
            self.apply_split();
        }
        if close {
            self.split_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::state::DocumentStatus;
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        let mut draw = |events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_| app.split_dialog_ui(context),
            );
            output.textures_delta.clear();
            output
        };
        let _ = draw(vec![]);
        let output = draw(vec![]);
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
            .expect("visible labeled control");
        let _ = draw(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let _ = draw(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    fn editor() -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ));
        let info = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/bookmarks.pdf"),
            None,
        )
        .unwrap()
        .info()
        .unwrap();
        app.tabs.push(Tab::new(
            1,
            "original.pdf".into(),
            DocumentStatus {
                info,
                revision: 5,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.next_id = 2;
        app
    }
    #[test]
    fn actual_create_control_dispatches_and_stale_busy_readonly_cannot_apply() {
        let mut app = editor();
        app.open_split();
        let context = egui::Context::default();
        click(&mut app, &context, "Create split tabs");
        assert_eq!(
            app.split_dialog.as_ref().unwrap().expected,
            Some(vec![2, 3, 4])
        );
        assert!(app.busy);
        let mut app = editor();
        app.open_split();
        app.tabs[0].status.revision += 1;
        assert!(!app.apply_split());
        app.tabs[0].status.revision -= 1;
        app.busy = true;
        assert!(!app.apply_split());
        app.busy = false;
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_split());
    }
    #[test]
    fn whole_reply_is_checked_before_any_tab_is_inserted() {
        let mut app = editor();
        app.open_split();
        app.split_dialog.as_mut().unwrap().expected = Some(vec![2, 3, 4]);
        let info = app.tabs[0].status.info.clone();
        let status = || DocumentStatus {
            info: info.clone(),
            revision: 0,
            dirty: true,
            can_undo: false,
            can_redo: false,
        };
        let documents = |bad| {
            vec![
                SplitDocument {
                    id: 2,
                    path: "part1.pdf".into(),
                    status: status(),
                },
                SplitDocument {
                    id: 3,
                    path: "part2.pdf".into(),
                    status: status(),
                },
                SplitDocument {
                    id: if bad { 3 } else { 4 },
                    path: "part3.pdf".into(),
                    status: status(),
                },
            ]
        };
        let context = egui::Context::default();
        app.accept_split(1, 5, documents(true), &context);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, Some(1));
        assert!(app.split_dialog.as_ref().unwrap().error.is_some());
        app.split_dialog.as_mut().unwrap().expected = Some(vec![2, 3, 4]);
        app.accept_split(1, 5, documents(false), &context);
        assert_eq!(
            app.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(app.active, Some(2));
        assert_eq!(app.next_id, 5);
        assert!(app.split_dialog.is_none());
    }
}
