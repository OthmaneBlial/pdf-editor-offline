use crate::state::Tab;
use crate::worker::{Command, Edit, Event, Worker};
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseTarget {
    Tab(u64),
    Exit,
}

enum Action {
    Open,
    Save,
    Rotate(i64),
    Undo,
    Redo,
    Previous,
    Next,
    Zoom(f32),
    Close,
}

pub struct EditorApp {
    worker: Worker,
    tabs: Vec<Tab>,
    active: Option<u64>,
    next_id: u64,
    busy: bool,
    message: String,
    close_target: Option<CloseTarget>,
    close_after_save: Option<u64>,
    allow_exit: bool,
}

impl EditorApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            worker: Worker::new(context.egui_ctx.clone()),
            tabs: vec![],
            active: None,
            next_id: 1,
            busy: false,
            message: "Everything stays on this computer.".into(),
            close_target: None,
            close_after_save: None,
            allow_exit: false,
        }
    }

    fn selected(&self) -> Option<&Tab> {
        self.tabs.iter().find(|tab| Some(tab.id) == self.active)
    }
    fn selected_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| Some(tab.id) == self.active)
    }

    fn send(&mut self, command: Command, message: &str) -> bool {
        if self.busy {
            return false;
        }
        match self.worker.send(command) {
            Ok(()) => {
                self.busy = true;
                self.message = message.into();
                true
            }
            Err(error) => {
                self.message = error;
                false
            }
        }
    }

    fn poll(&mut self, context: &egui::Context) {
        while let Ok(event) = self.worker.events.try_recv() {
            self.busy = false;
            match event {
                Event::Loaded { id, path, status } => {
                    self.tabs.push(Tab::new(id, path, status));
                    self.active = Some(id);
                    self.message = "PDF opened locally.".into();
                }
                Event::Updated { id, status } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.update(status);
                        tab.error = None;
                    }
                    self.message = "Edit applied. Save As creates a new PDF.".into();
                }
                Event::Saved { id, path, status } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.update(status);
                        tab.path = path;
                        tab.error = None;
                    }
                    self.message =
                        "Saved and reopened successfully. Original file preserved.".into();
                    if self.close_after_save.take() == Some(id) {
                        self.close_target = None;
                        self.send(Command::Close(id), "Closing saved PDF…");
                    }
                }
                Event::Rendered { key, image } => {
                    if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.id == key.tab && tab.accepts(key))
                    {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [image.width as usize, image.height as usize],
                            &image.rgba,
                        );
                        tab.texture = Some(context.load_texture(
                            format!("pdf-{}", key.tab),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                        tab.error = None;
                        self.message = "Preview rendered in Rust. No remote service.".into();
                    }
                }
                Event::Closed(id) => {
                    self.tabs.retain(|tab| tab.id != id);
                    if self.active == Some(id) {
                        self.active = self.tabs.last().map(|tab| tab.id);
                    }
                    self.message = "PDF closed.".into();
                }
                Event::Error {
                    id,
                    render_key,
                    message,
                } => {
                    self.close_after_save = None;
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        if render_key.is_none_or(|key| tab.accepts(key)) {
                            tab.error = Some(message.clone());
                            self.message = message;
                        }
                    } else {
                        self.message = message;
                    }
                }
            }
        }
        if !self.busy
            && let Some(tab) = self.selected_mut().filter(|tab| tab.needs_render)
        {
            tab.needs_render = false;
            let key = tab.key();
            if !self.send(Command::Render(key), "Rendering page in Rust…")
                && let Some(tab) = self.selected_mut()
            {
                tab.needs_render = true;
            }
        }
    }

    fn open(&mut self) {
        if self.busy || self.tabs.len() >= 16 {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .pick_file()
        {
            let id = self.next_id;
            self.next_id += 1;
            self.send(Command::Load { id, path }, "Opening and validating PDF…");
        }
    }

    fn save_as(&mut self, id: u64, close_after: bool) {
        if self.busy {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id && tab.editable()) else {
            return;
        };
        let stem = tab.path.file_stem().map_or_else(
            || "document".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        let mut dialog = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .set_file_name(format!("{stem}-edited.pdf"));
        if let Some(parent) = tab.path.parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.save_file()
            && self.send(Command::Save { id, path }, "Saving and verifying new PDF…")
            && close_after
        {
            self.close_after_save = Some(id);
        }
    }

    fn request_close(&mut self, id: u64) {
        if self.busy {
            return;
        }
        if self.tabs.iter().any(|tab| tab.id == id && tab.status.dirty) {
            self.close_target = Some(CloseTarget::Tab(id));
        } else {
            self.send(Command::Close(id), "Closing PDF…");
        }
    }

    fn needs_exit_guard(&self) -> bool {
        !self.allow_exit && (self.busy || self.tabs.iter().any(|tab| tab.status.dirty))
    }

    fn action(&mut self, action: Action) {
        match action {
            Action::Open => self.open(),
            Action::Save => {
                if let Some(id) = self.active {
                    self.save_as(id, false);
                }
            }
            Action::Close => {
                if let Some(id) = self.active {
                    self.request_close(id);
                }
            }
            Action::Previous => {
                if let Some(tab) = self.selected_mut() {
                    tab.set_page(tab.page.saturating_sub(1));
                }
            }
            Action::Next => {
                if let Some(tab) = self.selected_mut() {
                    tab.set_page(tab.page + 1);
                }
            }
            Action::Zoom(scale) => {
                if let Some(tab) = self.selected_mut() {
                    tab.set_scale(scale);
                }
            }
            Action::Rotate(degrees) => {
                if let Some(tab) = self.selected().filter(|tab| tab.editable()) {
                    self.send(
                        Command::Edit {
                            id: tab.id,
                            edit: Edit::Rotate(tab.page as u32 + 1, degrees),
                        },
                        "Rotating current page…",
                    );
                }
            }
            Action::Undo => {
                if let Some(tab) = self.selected().filter(|tab| tab.status.can_undo) {
                    self.send(
                        Command::Edit {
                            id: tab.id,
                            edit: Edit::Undo,
                        },
                        "Undoing edit…",
                    );
                }
            }
            Action::Redo => {
                if let Some(tab) = self.selected().filter(|tab| tab.status.can_redo) {
                    self.send(
                        Command::Edit {
                            id: tab.id,
                            edit: Edit::Redo,
                        },
                        "Redoing edit…",
                    );
                }
            }
        }
    }

    fn shortcuts(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let shortcut = |key, modifiers| egui::KeyboardShortcut::new(modifiers, key);
        let wants_keyboard_input = context.egui_wants_keyboard_input();
        let action = context.input_mut(|input| {
            if input.consume_shortcut(&shortcut(egui::Key::O, egui::Modifiers::COMMAND)) {
                Some(Action::Open)
            } else if input.consume_shortcut(&shortcut(egui::Key::S, egui::Modifiers::COMMAND)) {
                Some(Action::Save)
            } else if input.consume_shortcut(&shortcut(
                egui::Key::Z,
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
            )) {
                Some(Action::Redo)
            } else if input.consume_shortcut(&shortcut(egui::Key::Z, egui::Modifiers::COMMAND)) {
                Some(Action::Undo)
            } else if input.consume_shortcut(&shortcut(egui::Key::W, egui::Modifiers::COMMAND)) {
                Some(Action::Close)
            } else if !wants_keyboard_input
                && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                Some(Action::Next)
            } else if !wants_keyboard_input
                && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                Some(Action::Previous)
            } else {
                None
            }
        });
        if let Some(action) = action {
            self.action(action);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            ui.heading("PDF Editor Offline");
            ui.separator();
            if ui.add_enabled(!self.busy && self.tabs.len() < 16, egui::Button::new("Open PDF"))
                .on_hover_text("Open a local PDF · Command/Ctrl+O · up to 16 tabs").clicked() { action = Some(Action::Open); }
            let editable = self.selected().is_some_and(Tab::editable);
            if ui.add_enabled(!self.busy && editable, egui::Button::new("Save As…"))
                .on_hover_text("Create a new verified PDF · Command/Ctrl+S · existing files are never overwritten").clicked() { action = Some(Action::Save); }
            if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_undo), egui::Button::new("Undo"))
                .on_hover_text("Command/Ctrl+Z · keeps up to 8 history entries").clicked() { action = Some(Action::Undo); }
            if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_redo), egui::Button::new("Redo"))
                .on_hover_text("Command/Ctrl+Shift+Z").clicked() { action = Some(Action::Redo); }
            ui.separator();
            let dark = ui.visuals().dark_mode;
            if ui.button(if dark { "Light theme" } else { "Dark theme" }).clicked() {
                ui.ctx().set_visuals(if dark { egui::Visuals::light() } else { egui::Visuals::dark() });
            }
        });
        if let Some(action) = action {
            self.action(action);
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        egui::ScrollArea::horizontal()
            .id_salt("document-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for tab in &self.tabs {
                        let title = format!(
                            "{}{}",
                            tab.title(),
                            if tab.status.dirty { " *" } else { "" }
                        );
                        if ui
                            .selectable_label(self.active == Some(tab.id), title)
                            .on_hover_text(tab.path.display().to_string())
                            .clicked()
                        {
                            self.active = Some(tab.id);
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Close").small())
                            .on_hover_text(format!("Close {}", tab.title()))
                            .clicked()
                        {
                            close = Some(tab.id);
                        }
                        ui.separator();
                    }
                });
            });
        if let Some(id) = close {
            self.request_close(id);
        }
    }

    fn page_controls(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        if let Some(tab) = self.selected() {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(tab.page > 0, egui::Button::new("Previous page"))
                    .clicked()
                {
                    action = Some(Action::Previous);
                }
                ui.label(format!(
                    "Page {} / {}",
                    tab.page + 1,
                    tab.status.info.page_count
                ));
                if ui
                    .add_enabled(
                        tab.page + 1 < tab.status.info.page_count,
                        egui::Button::new("Next page"),
                    )
                    .clicked()
                {
                    action = Some(Action::Next);
                }
                ui.separator();
                if ui.button("Zoom out").clicked() {
                    action = Some(Action::Zoom(tab.scale / 1.25));
                }
                ui.label(format!("{:.0}%", tab.scale * 100.0));
                if ui.button("Zoom in").clicked() {
                    action = Some(Action::Zoom(tab.scale * 1.25));
                }
                if ui.button("Actual size").clicked() {
                    action = Some(Action::Zoom(1.0));
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !self.busy && tab.editable(),
                        egui::Button::new("Rotate left"),
                    )
                    .clicked()
                {
                    action = Some(Action::Rotate(-90));
                }
                if ui
                    .add_enabled(
                        !self.busy && tab.editable(),
                        egui::Button::new("Rotate right"),
                    )
                    .clicked()
                {
                    action = Some(Action::Rotate(90));
                }
            });
            if !tab.editable() {
                ui.label("Read only: editing and saving signed or encrypted PDFs is blocked to preserve document protection.");
            }
            ui.label("Preview may omit unsupported PDF features. Signing, password protection, OCR and format conversion are not available in this native milestone.");
        }
        if let Some(action) = action {
            self.action(action);
        }
    }

    fn close_dialog(&mut self, context: &egui::Context) {
        let Some(target) = self.close_target else {
            return;
        };
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(context, |ui| {
                if self.busy { ui.label("Wait for the document operation to finish before closing."); }
                else {
                    match target {
                        CloseTarget::Tab(id) => {
                            ui.label("This PDF has unsaved edits. Save a new file or discard the edits before closing.");
                            ui.horizontal(|ui| {
                                if ui.button("Save As…").clicked() { self.save_as(id, true); }
                                if ui.button("Discard edits and close").clicked() {
                                    self.close_target = None;
                                    self.send(Command::Close(id), "Closing PDF…");
                                }
                                if ui.button("Keep editing").clicked() { self.close_target = None; }
                            });
                        }
                        CloseTarget::Exit => {
                            let dirty = self.tabs.iter().filter(|tab| tab.status.dirty).count();
                            if dirty == 0 {
                                ui.label("The operation finished. There are no unsaved edits.");
                            } else {
                                ui.label(format!("{dirty} PDF(s) have unsaved edits. Return to editing to Save As, or discard all edits and quit."));
                            }
                            ui.horizontal(|ui| {
                                if ui.button("Return to editing").clicked() { self.close_target = None; }
                                if ui.button(if dirty == 0 { "Quit" } else { "Discard edits and quit" }).clicked() {
                                    self.allow_exit = true;
                                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        }
                    }
                }
            });
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if context.input(|input| input.viewport().close_requested()) && self.needs_exit_guard() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_target = Some(CloseTarget::Exit);
        }
        self.shortcuts(&context);
        ui.add_enabled_ui(self.close_target.is_none(), |ui| {
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            self.toolbar(ui);
            ui.separator();
            self.tabs(ui);
            self.page_controls(ui);
            ui.separator();
            ui.horizontal(|ui| {
                if self.busy { ui.spinner(); }
                ui.label(&self.message);
            });
            ui.separator();
            if let Some(tab) = self.selected() {
                if let Some(error) = &tab.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                egui::ScrollArea::both().id_salt(("pdf-page", tab.id)).show(ui, |ui| {
                    if let Some(texture) = &tab.texture {
                        let size = texture.size_vec2();
                        ui.add(egui::Image::new((texture.id(), size)).alt_text(format!("PDF page {}. Raster preview; text is not exposed to assistive technology.", tab.page + 1)));
                    } else if tab.error.is_none() { ui.label("Preparing local page preview…"); }
                });
            } else {
                ui.add_space(70.0);
                ui.heading("Your PDFs, on your computer.");
                ui.label("Open a PDF to read pages, zoom, rotate, undo and save a new copy.");
                ui.label("Native Rust engine · no account · no upload · original files preserved");
                ui.add_space(16.0);
                if ui.add_enabled(!self.busy, egui::Button::new("Open a local PDF")).clicked() { self.open(); }
            }
        });
        });
        self.close_dialog(&context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DocumentStatus;
    use pdf_core::DocumentInfo;

    fn app() -> EditorApp {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        EditorApp::new(&context)
    }

    #[test]
    fn dirty_tab_close_and_exit_require_a_decision() {
        let mut app = app();
        app.tabs.push(Tab::new(
            1,
            "original.pdf".into(),
            DocumentStatus {
                info: DocumentInfo {
                    version: "1.7".into(),
                    page_count: 1,
                    object_count: 5,
                    encrypted: false,
                    signature_fields: 0,
                    pages: vec![],
                },
                revision: 1,
                dirty: true,
                can_undo: true,
                can_redo: false,
            },
        ));
        app.request_close(1);
        assert_eq!(app.close_target, Some(CloseTarget::Tab(1)));
        assert_eq!(app.tabs.len(), 1);
        assert!(app.needs_exit_guard());
        app.tabs[0].status.dirty = false;
        assert!(!app.needs_exit_guard());
    }

    #[test]
    fn in_flight_operation_prevents_exit_until_completed() {
        let mut app = app();
        app.busy = true;
        assert!(app.needs_exit_guard());
        app.busy = false;
        assert!(!app.needs_exit_guard());
        app.busy = true;
        app.allow_exit = true;
        assert!(!app.needs_exit_guard());
    }
}
