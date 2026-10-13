use super::EditorApp;
use crate::{
    state::{DocumentStatus, Tab},
    worker::Command,
};
use eframe::egui;
use std::path::PathBuf;

pub(super) struct Dialog {
    pub source: u64,
    revision: u64,
    paths: Vec<PathBuf>,
    pub expected_id: Option<u64>,
    pub error: Option<String>,
}

const RESTRICTIONS: &str = "Guarded merge supports simple catalogs with matching inert metadata. Forms, bookmarks, attachments, layers, tags, advanced links/actions, signed or encrypted inputs and metadata conflicts may be refused. The open PDF comes first; additional files follow the order below. Inputs together must fit 128 MiB. Originals remain unchanged. This is not sanitization or secure erasure.";
impl EditorApp {
    pub(super) fn open_merge(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.tabs.len() >= crate::worker::MAX_TABS
        {
            return;
        }
        let Some(tab) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        self.merge_dialog = Some(Dialog {
            source: tab.id,
            revision: tab.status.revision,
            paths: vec![],
            expected_id: None,
            error: None,
        });
    }
    fn merge_current(&self) -> bool {
        self.merge_dialog.as_ref().is_some_and(|dialog| {
            self.active == Some(dialog.source)
                && self.tabs.iter().any(|tab| {
                    tab.id == dialog.source
                        && tab.status.revision == dialog.revision
                        && tab.editable()
                })
        })
    }
    fn apply_merge(&mut self) -> bool {
        if self.busy
            || !self.pending_saves.is_empty()
            || !self.merge_current()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || self.next_id == 0
            || self.next_id == u64::MAX
        {
            return false;
        }
        let Some(dialog) = self
            .merge_dialog
            .as_ref()
            .filter(|dialog| dialog.expected_id.is_none() && !dialog.paths.is_empty())
        else {
            return false;
        };
        let id = self.next_id;
        if self.send(
            Command::Merge {
                id: dialog.source,
                new_id: id,
                revision: dialog.revision,
                paths: dialog.paths.clone(),
            },
            "Merging into a separate verified document…",
        ) {
            self.next_id += 1;
            if let Some(dialog) = self.merge_dialog.as_mut() {
                dialog.expected_id = Some(id);
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn accept_merged(
        &mut self,
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
        context: &egui::Context,
    ) {
        if self
            .merge_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.expected_id == Some(id))
            && self.merge_current()
            && id != 0
            && id != u64::MAX
            && !self.tabs.iter().any(|tab| tab.id == id)
            && self.tabs.len() < crate::worker::MAX_TABS
            && status.dirty
        {
            let mut tab = Tab::new(id, path, status);
            tab.set_device_ratio(context.pixels_per_point());
            self.tabs.push(tab);
            self.active = Some(id);
            self.next_id = self.next_id.max(id + 1);
            self.merge_dialog = None;
            self.message="Merged copy created in a separate unsaved tab. Save As creates a new file; originals are unchanged.".into();
        } else {
            self.message = "Unexpected merge result ignored; existing documents retained.".into();
            if let Some(dialog) = self
                .merge_dialog
                .as_mut()
                .filter(|dialog| dialog.expected_id == Some(id))
            {
                dialog.expected_id = None;
                dialog.error = Some(self.message.clone());
            }
        }
    }
    pub(super) fn merge_dialog(&mut self, context: &egui::Context) {
        if self.merge_dialog.is_none() {
            return;
        }
        let current = self.merge_current();
        let mut apply = false;
        let mut close = false;
        let mut pick = false;
        let Some(dialog) = self.merge_dialog.as_mut() else {
            return;
        };
        egui::Window::new("Merge PDFs into a new document")
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .max_width((context.content_rect().width() - 32.0).max(280.0))
            .max_height((context.content_rect().height() - 60.0).max(200.0))
            .vscroll(true)
            .show(context, |ui| {
                ui.label(RESTRICTIONS);
                if !current {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "The source changed. Cancel and review the current document.",
                    );
                }
                let enabled = dialog.expected_id.is_none();
                if ui
                    .add_enabled(enabled && current, egui::Button::new("Add PDF files…"))
                    .clicked()
                {
                    pick = true;
                }
                let mut movement = None;
                let mut remove = None;
                for (index, path) in dialog.paths.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}. {}",
                            index + 1,
                            path.file_name()
                                .unwrap_or(path.as_os_str())
                                .to_string_lossy()
                        ));
                        if ui
                            .add_enabled(enabled && index > 0, egui::Button::new("↑"))
                            .clicked()
                        {
                            movement = Some((index, index - 1));
                        }
                        if ui
                            .add_enabled(
                                enabled && index + 1 < dialog.paths.len(),
                                egui::Button::new("↓"),
                            )
                            .clicked()
                        {
                            movement = Some((index, index + 1));
                        }
                        if ui
                            .add_enabled(enabled, egui::Button::new("Remove"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                }
                if let Some((a, b)) = movement {
                    dialog.paths.swap(a, b);
                }
                if let Some(index) = remove {
                    dialog.paths.remove(index);
                }
                if let Some(error) = &dialog.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            enabled
                                && current
                                && !self.busy
                                && self.pending_saves.is_empty()
                                && !dialog.paths.is_empty(),
                            egui::Button::new("Create merged copy"),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        if pick
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .pick_files()
            && let Some(dialog) = self.merge_dialog.as_mut()
        {
            if dialog.paths.len().saturating_add(paths.len()) >= pdf_core::MAX_PAGES {
                dialog.error = Some("Select at most 19,999 additional PDF files".into());
            } else {
                dialog.paths.extend(paths);
                dialog.error = None;
            }
        }
        if apply {
            self.apply_merge();
        }
        if close {
            self.merge_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
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
                |_| app.merge_dialog(context),
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
    fn app() -> EditorApp {
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
    fn empty_stale_busy_and_readonly_merge_dialogs_cannot_dispatch() {
        let mut app = app();
        app.open_merge();
        assert!(!app.apply_merge());
        app.merge_dialog
            .as_mut()
            .unwrap()
            .paths
            .push("additional.pdf".into());
        app.busy = true;
        assert!(!app.apply_merge());
        app.busy = false;
        app.tabs[0].status.revision += 1;
        assert!(!app.apply_merge());
        assert_eq!(app.next_id, 2);
        app.merge_dialog = None;
        app.tabs[0].status.info.signature_fields = 1;
        app.open_merge();
        assert!(app.merge_dialog.is_none());
    }
    #[test]
    fn actual_dialog_controls_reorder_remove_and_disable_stale_apply() {
        let mut app = app();
        app.open_merge();
        app.merge_dialog.as_mut().unwrap().paths = vec!["first.pdf".into(), "second.pdf".into()];
        let context = egui::Context::default();
        click(&mut app, &context, "↓");
        assert_eq!(
            app.merge_dialog.as_ref().unwrap().paths,
            vec![PathBuf::from("second.pdf"), PathBuf::from("first.pdf")]
        );
        click(&mut app, &context, "Remove");
        assert_eq!(
            app.merge_dialog.as_ref().unwrap().paths,
            vec![PathBuf::from("first.pdf")]
        );
        app.tabs[0].status.revision += 1;
        click(&mut app, &context, "Create merged copy");
        assert!(!app.busy);
        assert_eq!(app.next_id, 2);
        assert!(app.merge_dialog.as_ref().unwrap().expected_id.is_none());
        click(&mut app, &context, "Cancel");
        assert!(app.merge_dialog.is_none());
    }
    #[test]
    fn accepted_copy_is_a_new_unsaved_tab_and_stale_result_retains_source() {
        let mut app = app();
        app.open_merge();
        app.merge_dialog.as_mut().unwrap().expected_id = Some(2);
        let info = app.tabs[0].status.info.clone();
        let status = || DocumentStatus {
            info: info.clone(),
            revision: 0,
            dirty: true,
            can_undo: false,
            can_redo: false,
        };
        app.accept_merged(2, "merged.pdf".into(), status(), &egui::Context::default());
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, Some(2));
        assert_eq!(app.tabs[0].status.revision, 5);
        assert!(!app.tabs[0].status.dirty);
        assert!(app.tabs[1].status.dirty);
        let mut app = super::tests::app();
        app.open_merge();
        app.merge_dialog.as_mut().unwrap().expected_id = Some(2);
        app.tabs[0].status.revision += 1;
        app.accept_merged(2, "merged.pdf".into(), status(), &egui::Context::default());
        assert_eq!(app.tabs.len(), 1);
        assert!(app.merge_dialog.as_ref().unwrap().expected_id.is_none());
        assert!(app.merge_dialog.as_ref().unwrap().error.is_some());
    }
}
