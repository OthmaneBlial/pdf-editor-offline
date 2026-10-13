use super::{Action, EditorApp};
use crate::state::Tab;
use eframe::egui;

impl EditorApp {
    pub(super) fn command_palette(&mut self, context: &egui::Context) {
        if !self.palette_open || self.close_target.is_some() {
            return;
        }
        let mut execute = None;
        let mut open = true;
        egui::Window::new("Commands · Command/Ctrl+K")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .default_width(420.0)
            .show(context, |ui| {
                let query = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text("Find a command…")
                        .char_limit(128),
                );
                if self.palette_focus {
                    query.request_focus();
                    self.palette_focus = false;
                }
                let needle = self.palette_query.to_lowercase();
                let selected = self.selected();
                let has_document = selected.is_some();
                let editable = !self.busy && selected.is_some_and(Tab::editable);
                let commands = [
                    (
                        "Edit existing text",
                        Action::ContentText,
                        !self.busy && has_document && self.pending_saves.is_empty(),
                    ),
                    (
                        "Add text overlay",
                        Action::TextOverlay,
                        editable && self.pending_saves.is_empty(),
                    ),
                    (
                        "Sticky notes on this page",
                        Action::Notes,
                        !self.busy && has_document,
                    ),
                    (
                        "Draw shapes and freehand",
                        Action::VectorOverlay,
                        editable && self.pending_saves.is_empty(),
                    ),
                    ("PDF form fields", Action::Forms, !self.busy && has_document),
                    ("Recover previous session", Action::Recovery, true),
                    ("Open PDF", Action::Open, !self.busy && self.tabs.len() < 16),
                    (
                        "Images to PDF",
                        Action::Images,
                        !self.busy && self.tabs.len() < crate::worker::MAX_TABS,
                    ),
                    ("Save As new PDF", Action::Save, editable),
                    (
                        "Document properties",
                        Action::Metadata,
                        !self.busy && has_document,
                    ),
                    ("Search document text", Action::Search, has_document),
                    ("Read extracted page text", Action::PageText, has_document),
                    (
                        "Extract page range",
                        Action::ExtractPages,
                        editable && self.tabs.len() < crate::worker::MAX_TABS,
                    ),
                    (
                        "Insert image content",
                        Action::ImageOverlay,
                        editable && self.pending_saves.is_empty(),
                    ),
                    (
                        "Split into new tabs",
                        Action::Split,
                        editable
                            && self.pending_saves.is_empty()
                            && self.tabs.len() < crate::worker::MAX_TABS,
                    ),
                    (
                        "Merge PDF files",
                        Action::Merge,
                        editable
                            && self.pending_saves.is_empty()
                            && self.tabs.len() < crate::worker::MAX_TABS,
                    ),
                    (
                        "Create targeted sanitized copy",
                        Action::Sanitize,
                        editable
                            && self.pending_saves.is_empty()
                            && self.tabs.len() < crate::worker::MAX_TABS,
                    ),
                    (
                        "Delete page range",
                        Action::DeletePages,
                        editable && selected.is_some_and(|tab| tab.status.info.page_count > 1),
                    ),
                    (
                        "Duplicate page range",
                        Action::DuplicatePages,
                        editable
                            && self.pending_saves.is_empty()
                            && selected.is_some_and(|tab| {
                                tab.status.info.page_count < pdf_core::MAX_PAGES
                            }),
                    ),
                    ("Rotate current page left", Action::Rotate(-90), editable),
                    ("Rotate current page right", Action::Rotate(90), editable),
                    (
                        "Move current page earlier",
                        Action::Move(true),
                        editable && selected.is_some_and(|tab| tab.page > 0),
                    ),
                    (
                        "Move current page later",
                        Action::Move(false),
                        editable
                            && selected
                                .is_some_and(|tab| tab.page + 1 < tab.status.info.page_count),
                    ),
                    (
                        "Reverse all pages",
                        Action::Reverse,
                        editable && selected.is_some_and(|tab| tab.status.info.page_count > 1),
                    ),
                    (
                        "Insert blank page",
                        Action::InsertBlank,
                        editable
                            && selected.is_some_and(|tab| {
                                tab.status.info.page_count < pdf_core::MAX_PAGES
                            }),
                    ),
                    ("Crop current page", Action::Crop, editable),
                    (
                        "Undo",
                        Action::Undo,
                        !self.busy && selected.is_some_and(|tab| tab.status.can_undo),
                    ),
                    (
                        "Redo",
                        Action::Redo,
                        !self.busy && selected.is_some_and(|tab| tab.status.can_redo),
                    ),
                    (
                        "Previous page",
                        Action::Previous,
                        selected.is_some_and(|tab| tab.page > 0),
                    ),
                    (
                        "Next page",
                        Action::Next,
                        selected.is_some_and(|tab| tab.page + 1 < tab.status.info.page_count),
                    ),
                    ("Restore actual size", Action::Zoom(1.0), has_document),
                    ("Show or hide sidebar", Action::ToggleSidebar, true),
                    ("Show or hide inspector", Action::ToggleInspector, true),
                    ("Switch dark or light theme", Action::Theme, true),
                    (
                        "Close current PDF",
                        Action::Close,
                        !self.busy && has_document,
                    ),
                    ("Quit application", Action::Quit, true),
                ];
                let mut first = None;
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        for (label, action, enabled) in commands {
                            if label.to_lowercase().contains(&needle) {
                                if enabled && first.is_none() {
                                    first = Some(action);
                                }
                                if ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::new(label)
                                            .min_size(egui::vec2(ui.available_width(), 28.0)),
                                    )
                                    .clicked()
                                {
                                    execute = Some(action);
                                }
                            }
                        }
                    });
                if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    execute = first;
                }
            });
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.palette_open = open;
        if let Some(action) = execute {
            self.palette_open = false;
            self.perform(action, context);
        }
    }
}
