use crate::state::Tab;
use crate::worker::{Command, Edit, Event, TaskKey, Worker};
use eframe::egui;

mod canvas;
mod dialogs;
mod navigation;
mod palette;
mod sidebar;

const MAIN_PIXEL_BUDGET: usize = 64 * 1024 * 1024;
const _: () = assert!(pdf_render::MAX_PIXELS * 4 <= MAIN_PIXEL_BUDGET as u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseTarget {
    Tab(u64),
    Exit,
}

#[derive(Clone, Copy)]
enum Action {
    Open,
    Save,
    Rotate(i64),
    Undo,
    Redo,
    Previous,
    Next,
    Page(usize),
    Zoom(f32),
    Close,
    Search,
    PageText,
    Palette,
    ToggleSidebar,
    ToggleInspector,
    Theme,
    Quit,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SidebarMode {
    #[default]
    Pages,
    Search,
    Text,
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
    show_sidebar: bool,
    show_inspector: bool,
    show_page_text: bool,
    sidebar_mode: SidebarMode,
    palette_open: bool,
    palette_query: String,
    palette_focus: bool,
    search_focus: bool,
}

impl EditorApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::configure(&context.egui_ctx);
        crate::style::configure(&context.egui_ctx, false);
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
            show_sidebar: true,
            show_inspector: true,
            show_page_text: false,
            sidebar_mode: SidebarMode::Pages,
            palette_open: false,
            palette_query: String::new(),
            palette_focus: false,
            search_focus: false,
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
        self.sync_display_ratio(context.pixels_per_point());
        while let Ok(event) = self.worker.events.try_recv() {
            self.busy = false;
            match event {
                Event::Loaded { id, path, status } => {
                    let mut tab = Tab::new(id, path, status);
                    tab.set_device_ratio(context.pixels_per_point());
                    self.tabs.push(tab);
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
                    if self
                        .tabs
                        .iter()
                        .any(|tab| tab.id == key.tab && tab.accepts(key))
                    {
                        let mut used = self
                            .tabs
                            .iter()
                            .filter(|tab| tab.id != key.tab)
                            .filter_map(|tab| tab.texture.as_ref())
                            .map(texture_bytes)
                            .sum::<usize>();
                        for tab in self.tabs.iter_mut().filter(|tab| tab.id != key.tab) {
                            if used + image.rgba.len() <= MAIN_PIXEL_BUDGET {
                                break;
                            }
                            if let Some(texture) = tab.texture.take() {
                                used -= texture_bytes(&texture);
                                tab.needs_render = true;
                            }
                        }
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [image.width as usize, image.height as usize],
                            &image.rgba,
                        );
                        let mut limited = false;
                        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == key.tab) {
                            limited = tab.raster_plan().is_ok_and(|plan| plan.limited);
                            tab.texture = Some(context.load_texture(
                                format!("pdf-{}", key.tab),
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                            tab.error = None;
                        }
                        self.message = if limited {
                            "Preview resolution limited. Document zoom remains unchanged."
                        } else {
                            "Preview rendered locally in Rust."
                        }
                        .into();
                    }
                }
                Event::Text(text) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.text_key() == text.key) {
                        tab.text = Some(text);
                        tab.text_error = None;
                    }
                }
                Event::Searched {
                    key,
                    page,
                    hit,
                    text_truncated,
                } => {
                    if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.id == key.tab && tab.status.revision == key.revision)
                        && tab.search.accept(
                            key,
                            page,
                            hit,
                            text_truncated,
                            tab.status.info.page_count,
                        )
                    {
                        self.message = tab.search.completion_message();
                    }
                }
                Event::Closed(id) => {
                    self.tabs.retain(|tab| tab.id != id);
                    if self.active == Some(id) {
                        self.active = self.tabs.last().map(|tab| tab.id);
                    }
                    self.message = "PDF closed.".into();
                }
                Event::Error { id, task, message } => {
                    self.close_after_save = None;
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        match task {
                            Some(TaskKey::Render(key)) if tab.accepts(key) => {
                                tab.error = Some(message.clone());
                                self.message = message;
                            }
                            Some(TaskKey::Text(key)) if tab.text_key() == key => {
                                tab.text_error = Some(message);
                            }
                            Some(TaskKey::Search(key)) if tab.search.key == Some(key) => {
                                tab.search.running = false;
                                tab.search.error = Some(message);
                            }
                            None => {
                                tab.error = Some(message.clone());
                                self.message = message;
                            }
                            _ => {}
                        }
                    } else {
                        self.message = message;
                    }
                }
            }
        }
        self.schedule();
    }

    fn schedule(&mut self) {
        if self.busy || self.close_target.is_some() {
            return;
        }
        if let Some(tab) = self.selected_mut().filter(|tab| tab.needs_render) {
            let key = match tab.key() {
                Ok(key) => key,
                Err(message) => {
                    tab.error = Some(message.into());
                    tab.needs_render = false;
                    self.message = message.into();
                    return;
                }
            };
            tab.needs_render = false;
            if !self.send(Command::Render(key), "Rendering page in Rust…")
                && let Some(tab) = self.selected_mut()
            {
                tab.needs_render = true;
            }
            return;
        }
        if (self.show_page_text || (self.show_sidebar && self.sidebar_mode == SidebarMode::Text))
            && let Some(tab) = self.selected_mut().filter(|tab| tab.needs_text)
        {
            let key = tab.text_key();
            tab.needs_text = false;
            if !self.send(Command::Text(key), "Extracting page text…")
                && let Some(tab) = self.selected_mut()
            {
                tab.needs_text = true;
            }
            return;
        }
        if let Some(tab) = self.selected().filter(|tab| tab.search.running)
            && let Some(key) = tab.search.key
        {
            self.send(
                Command::Search {
                    key,
                    page: tab.search.next_page,
                    query: tab.search.query.clone(),
                    match_case: tab.search.query_case,
                },
                "Searching local page text…",
            );
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

    fn sync_display_ratio(&mut self, ratio: f32) {
        for tab in &mut self.tabs {
            tab.set_device_ratio(ratio);
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

    fn request_exit(&mut self, context: &egui::Context) {
        if self.needs_exit_guard() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_target = Some(CloseTarget::Exit);
            self.palette_open = false;
        } else {
            self.allow_exit = true;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn perform(&mut self, action: Action, context: &egui::Context) {
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
            Action::Page(page) => {
                if let Some(tab) = self.selected_mut() {
                    tab.set_page(page);
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
            Action::Search => {
                self.show_sidebar = true;
                self.sidebar_mode = SidebarMode::Search;
                self.search_focus = true;
            }
            Action::PageText => {
                self.show_sidebar = true;
                self.sidebar_mode = SidebarMode::Text;
                self.show_page_text = true;
            }
            Action::Palette => {
                self.palette_open = !self.palette_open;
                self.palette_query.clear();
                self.palette_focus = self.palette_open;
            }
            Action::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Action::ToggleInspector => self.show_inspector = !self.show_inspector,
            Action::Theme => {
                crate::style::configure(context, !context.global_style().visuals.dark_mode);
            }
            Action::Quit => self.request_exit(context),
        }
    }

    fn shortcuts(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let shortcut = |key, modifiers| egui::KeyboardShortcut::new(modifiers, key);
        let wants_keyboard_input = context.egui_wants_keyboard_input();
        let action = context.input_mut(|input| {
            if input.consume_shortcut(&shortcut(egui::Key::Q, egui::Modifiers::COMMAND)) {
                Some(Action::Quit)
            } else if input.consume_shortcut(&shortcut(egui::Key::K, egui::Modifiers::COMMAND)) {
                Some(Action::Palette)
            } else if self.palette_open {
                None
            } else if input.consume_shortcut(&shortcut(egui::Key::F, egui::Modifiers::COMMAND)) {
                Some(Action::Search)
            } else if input.consume_shortcut(&shortcut(egui::Key::O, egui::Modifiers::COMMAND)) {
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
            self.perform(action, context);
        }
    }
}

fn texture_bytes(texture: &egui::TextureHandle) -> usize {
    let size = texture.size();
    size[0] * size[1] * 4
}

impl eframe::App for EditorApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(context);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.sync_display_ratio(context.pixels_per_point());
        if context.input(|input| input.viewport().close_requested()) && !self.allow_exit {
            self.request_exit(&context);
        }
        self.shortcuts(&context);
        ui.add_enabled_ui(self.close_target.is_none() && !self.palette_open, |ui| {
            self.workstation(ui)
        });
        self.command_palette(&context);
        self.close_dialog(&context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DocumentStatus;
    use pdf_core::DocumentInfo;
    fn app() -> EditorApp {
        EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ))
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

    #[test]
    fn search_action_focuses_the_real_query_and_keeps_it_for_typing() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
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
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.perform(Action::Search, &context.egui_ctx);
        assert!(app.show_sidebar);
        assert!(matches!(app.sidebar_mode, SidebarMode::Search));
        let mut output = context
            .egui_ctx
            .run_ui(Default::default(), |ui| app.sidebar(ui));
        output.textures_delta.clear();
        let focused = context.egui_ctx.memory(|memory| memory.focused());
        assert!(focused.is_some());
        assert!(!app.search_focus);
        let input = egui::RawInput {
            events: vec![egui::Event::Text("Café".into())],
            ..Default::default()
        };
        let mut output = context.egui_ctx.run_ui(input, |ui| app.sidebar(ui));
        output.textures_delta.clear();
        assert_eq!(app.tabs[0].search.draft, "Café");
        assert_eq!(context.egui_ctx.memory(|memory| memory.focused()), focused);
    }

    #[test]
    fn theme_command_toggles_the_actual_visuals_in_both_directions() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        assert!(!context.egui_ctx.global_style().visuals.dark_mode);
        app.perform(Action::Theme, &context.egui_ctx);
        assert!(context.egui_ctx.global_style().visuals.dark_mode);
        app.perform(Action::Theme, &context.egui_ctx);
        assert!(!context.egui_ctx.global_style().visuals.dark_mode);
    }

    #[test]
    fn quit_action_closes_clean_apps_and_guards_dirty_or_busy_apps() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut clean = EditorApp::new(&context);
        clean.perform(Action::Quit, &context.egui_ctx);
        assert!(clean.allow_exit);
        assert_eq!(clean.close_target, None);

        let mut busy = EditorApp::new(&context);
        busy.busy = true;
        busy.perform(Action::Quit, &context.egui_ctx);
        assert!(!busy.allow_exit);
        assert_eq!(busy.close_target, Some(CloseTarget::Exit));

        let mut dirty = EditorApp::new(&context);
        dirty.tabs.push(Tab::new(
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
        dirty.perform(Action::Quit, &context.egui_ctx);
        assert!(!dirty.allow_exit);
        assert_eq!(dirty.close_target, Some(CloseTarget::Exit));
        assert!(dirty.tabs[0].status.dirty);
    }
}
