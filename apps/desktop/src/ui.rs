use crate::state::Tab;
use crate::worker::{Command, Edit, Event, TaskKey, Worker};
use eframe::egui;

mod canvas;
mod dialogs;
mod forms;
mod merge;
mod metadata;
mod navigation;
mod notes;
mod organizer;
mod pages;
mod palette;
mod password;
mod recovery;
mod sanitize;
mod sidebar;
mod text_overlay;

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
    Images,
    Save,
    Rotate(i64),
    Move(bool),
    Reverse,
    InsertBlank,
    Crop,
    Metadata,
    Notes,
    Forms,
    TextOverlay,
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
    Recovery,
    ExtractPages,
    DeletePages,
    DuplicatePages,
    Sanitize,
    Merge,
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
    thumbnails: crate::thumbnails::Cache,
    visible_thumbnails: Vec<crate::thumbnails::ThumbnailKey>,
    initial_window_show: bool,
    organizer_dialog: Option<organizer::Dialog>,
    metadata_dialog: Option<metadata::Dialog>,
    metadata_generation: u64,
    notes_generation: u64,
    notes_dialog: Option<notes::Dialog>,
    forms_dialog: Option<forms::Dialog>,
    overlay_dialog: Option<text_overlay::Dialog>,
    forms_generation: u64,
    pages_dialog: Option<pages::Dialog>,
    sanitize_dialog: Option<sanitize::Dialog>,
    merge_dialog: Option<merge::Dialog>,
    pending_open: Option<password::Opening>,
    password_dialog: Option<password::Dialog>,
    recovery: recovery::State,
    discard_quit: bool,
    pending_saves: std::collections::BTreeMap<u64, recovery::PendingSave>,
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
            thumbnails: Default::default(),
            visible_thumbnails: vec![],
            initial_window_show: true,
            organizer_dialog: None,
            metadata_dialog: None,
            metadata_generation: 0,
            notes_generation: 0,
            notes_dialog: None,
            forms_dialog: None,
            overlay_dialog: None,
            forms_generation: 0,
            pages_dialog: None,
            sanitize_dialog: None,
            merge_dialog: None,
            pending_open: None,
            password_dialog: None,
            recovery: Default::default(),
            discard_quit: false,
            pending_saves: Default::default(),
        }
    }

    fn selected(&self) -> Option<&Tab> {
        self.tabs.iter().find(|tab| Some(tab.id) == self.active)
    }

    fn show_initial_window(&mut self, context: &egui::Context) {
        if std::mem::take(&mut self.initial_window_show) {
            // Eframe 0.36.2 starts hidden, but its invisible-window path runs only logic;
            // post_rendering's automatic show requires a UI frame that this path skips.
            context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
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
            if !matches!(
                &event,
                Event::RecoveryListed { .. } | Event::Error { id: 0, .. }
            ) || self.recovery.pending.is_some()
            {
                self.busy = false;
            }
            match event {
                Event::Notes {
                    key,
                    entries,
                    next_offset,
                } => self.accept_notes(key, entries, next_offset),
                Event::PasswordRequired {
                    id,
                    path,
                    fingerprint,
                    incorrect,
                } => self.accept_password_required(id, path, fingerprint, incorrect),
                Event::Unlocked { id, path, status } => {
                    self.accept_unlocked(id, path, status, context)
                }
                Event::Merged { id, path, status } => self.accept_merged(id, path, status, context),
                Event::Sanitized {
                    id,
                    path,
                    status,
                    report,
                } => self.accept_sanitized(id, path, status, report, context),
                Event::RecoveryListed { sessions } => self.accept_recovery_list(sessions),
                Event::RecoveryRestored { documents } => self.accept_recovered(documents, context),
                Event::SavePending { id, path, message } => {
                    if self.tabs.iter().any(|tab| tab.id == id) {
                        self.pending_saves
                            .insert(id, recovery::PendingSave::new(path, message.clone()));
                        self.message = message;
                        self.close_after_save = None;
                        self.close_target = None;
                    }
                }
                Event::Extracted { id, path, status } => {
                    if self
                        .pages_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.submitting && dialog.expected_id == Some(id))
                        && id != 0
                        && id != u64::MAX
                        && !self.tabs.iter().any(|tab| tab.id == id)
                        && self.tabs.len() < crate::worker::MAX_TABS
                    {
                        let mut tab = Tab::new(id, path, status);
                        tab.set_device_ratio(context.pixels_per_point());
                        self.next_id = self.next_id.max(id + 1);
                        self.tabs.push(tab);
                        self.active = Some(id);
                        self.pages_dialog = None;
                        self.message =
                            "Pages extracted into a new unsaved PDF. Source document preserved."
                                .into();
                    } else {
                        self.message =
                            "Unexpected extraction result ignored. Existing PDFs retained.".into();
                    }
                }
                Event::Loaded { id, path, status } => {
                    if self
                        .pending_open
                        .as_ref()
                        .is_some_and(|opening| opening.id == id)
                    {
                        self.pending_open = None;
                    }
                    let mut tab = Tab::new(id, path, status);
                    tab.set_device_ratio(context.pixels_per_point());
                    self.tabs.push(tab);
                    self.active = Some(id);
                    self.message = if self.tabs.last().is_some_and(|tab|tab.status.dirty) {
                        "Images converted locally. Save As creates a verified PDF. Page size uses one point per source pixel."
                    } else { "PDF opened locally." }.into();
                }
                Event::Updated {
                    id,
                    status,
                    selected_page,
                } => {
                    if let Some(dialog) = self
                        .notes_dialog
                        .as_mut()
                        .filter(|dialog| dialog.key.tab == id)
                    {
                        if dialog.submitting {
                            self.notes_dialog = None;
                        } else {
                            dialog.stale = true;
                        }
                    }
                    if self
                        .forms_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.key.tab == id)
                    {
                        if self
                            .forms_dialog
                            .as_ref()
                            .is_some_and(|dialog| dialog.submitting)
                        {
                            self.forms_dialog = None;
                        } else if let Some(dialog) = self.forms_dialog.as_mut() {
                            dialog.stale = true;
                        }
                    }
                    if self
                        .overlay_dialog
                        .as_ref()
                        .is_some_and(|d| d.key.tab == id)
                    {
                        if self.overlay_dialog.as_ref().is_some_and(|d| d.submitting) {
                            self.overlay_dialog = None;
                        } else if let Some(d) = self.overlay_dialog.as_mut() {
                            d.stale = true;
                        }
                    }
                    if self.pages_dialog.as_ref().is_some_and(|dialog| {
                        matches!(dialog.mode, pages::Mode::Delete | pages::Mode::Duplicate)
                            && dialog.tab == id
                            && dialog.submitting
                    }) {
                        self.pages_dialog = None;
                    }
                    let save_cancelled = self
                        .pending_saves
                        .get(&id)
                        .is_some_and(|pending| pending.submitting);
                    if save_cancelled {
                        self.pending_saves.remove(&id);
                    }
                    if self
                        .metadata_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.key.tab == id)
                    {
                        if self
                            .metadata_dialog
                            .as_ref()
                            .is_some_and(|dialog| dialog.submitting)
                        {
                            self.metadata_dialog = None;
                        } else if let Some(dialog) = self.metadata_dialog.as_mut() {
                            dialog.stale = true;
                        }
                    }
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.update(status);
                        if let Some(page) = selected_page {
                            tab.set_page(page);
                        }
                        tab.error = None;
                    }
                    self.message = if save_cancelled {
                        "Recorded save attempt cancelled. Destination files preserved."
                    } else {
                        "Edit applied. Save As creates a new PDF."
                    }
                    .into();
                }
                Event::Saved { id, path, status } => {
                    self.pending_saves.remove(&id);
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
                Event::Thumbnail { key, image } => {
                    if self.thumbnail_is_current(key) && self.visible_thumbnails.contains(&key) {
                        let texture = context.load_texture(
                            format!("thumbnail-{}-{}-{}", key.tab, key.revision, key.page),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [image.width as usize, image.height as usize],
                                &image.rgba,
                            ),
                            egui::TextureOptions::LINEAR,
                        );
                        self.thumbnails.insert(key, Some(texture), None);
                    }
                }
                Event::Text(text) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.text_key() == text.key) {
                        tab.text = Some(text);
                        tab.text_error = None;
                    }
                }
                Event::Metadata { key, values } => {
                    self.accept_metadata(key, values);
                }
                Event::Forms {
                    key,
                    fields,
                    choices,
                } => self.accept_forms(key, fields, choices),
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
                    if self
                        .notes_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.key.tab == id)
                    {
                        self.notes_dialog = None;
                    }
                    if self
                        .password_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.related_to(id))
                    {
                        self.cancel_password();
                    }
                    self.pending_saves.remove(&id);
                    self.tabs.retain(|tab| tab.id != id);
                    if self.active == Some(id) {
                        self.active = self.tabs.last().map(|tab| tab.id);
                    }
                    self.message = "PDF closed.".into();
                }
                Event::Error { id, task, message } => {
                    if self.notes_dialog.as_ref().is_some_and(|dialog| {
                        dialog.key.tab == id
                            && (dialog.submitting
                                || matches!(task,Some(TaskKey::Notes(key)) if key==dialog.key))
                    }) {
                        if let Some(dialog) = self.notes_dialog.as_mut() {
                            dialog.submitting = false;
                            dialog.error = Some(message.clone());
                        }
                        self.message = message;
                        continue;
                    }

                    if task.is_none() && self.password_error(id, &message) {
                        continue;
                    }
                    if task.is_none()
                        && self
                            .pending_open
                            .as_ref()
                            .is_some_and(|opening| opening.id == id)
                    {
                        self.pending_open = None;
                    }
                    if task.is_none()
                        && self.merge_dialog.as_ref().is_some_and(|dialog| {
                            dialog.expected_id.is_some()
                                && (dialog.source == id || dialog.expected_id == Some(id))
                        })
                    {
                        if let Some(dialog) = self.merge_dialog.as_mut() {
                            dialog.expected_id = None;
                            dialog.error = Some(message.clone());
                        }
                        self.message = message;
                        continue;
                    }
                    if task.is_none()
                        && self.sanitize_dialog.as_ref().is_some_and(|dialog| {
                            dialog.expected_id.is_some()
                                && (dialog.source == id || dialog.expected_id == Some(id))
                        })
                    {
                        if let Some(dialog) = self.sanitize_dialog.as_mut() {
                            dialog.expected_id = None;
                            dialog.error = Some(message.clone());
                        }
                        self.message = message;
                        continue;
                    }
                    if task.is_none()
                        && self.pages_dialog.as_ref().is_some_and(|dialog| {
                            dialog.submitting
                                && (dialog.tab == id || dialog.expected_id == Some(id))
                        })
                    {
                        if let Some(dialog) = self.pages_dialog.as_mut() {
                            dialog.submitting = false;
                            dialog.error = Some(message.clone());
                        }
                        self.message = message;
                        continue;
                    }
                    if let Some(pending) = self.pending_saves.get_mut(&id) {
                        pending.submitting = false;
                        pending.message = message.clone();
                    }
                    if id == 0 {
                        self.recovery.pending = None;
                        self.recovery.error = Some(message.clone());
                        self.discard_quit = false;
                        self.message = message;
                        continue;
                    }
                    if task.is_none() {
                        self.discard_quit = false;
                    }
                    if let Some(TaskKey::Overlay(key)) = task {
                        if let Some(d) = self.overlay_dialog.as_mut().filter(|d| d.key == key) {
                            d.error = Some(message);
                            d.submitting = false;
                        }
                        continue;
                    }
                    if let Some(TaskKey::Forms(key)) = task {
                        if let Some(dialog) = self
                            .forms_dialog
                            .as_mut()
                            .filter(|dialog| dialog.key == key)
                        {
                            dialog.error = Some(message);
                            dialog.submitting = false;
                        }
                        continue;
                    }
                    if let Some(TaskKey::Metadata(key)) = task {
                        if let Some(dialog) = self
                            .metadata_dialog
                            .as_mut()
                            .filter(|dialog| dialog.key == key)
                        {
                            dialog.error = Some(message);
                            dialog.submitting = false;
                        }
                        continue;
                    }
                    if let Some(TaskKey::Thumbnail(key)) = task {
                        if self.thumbnail_is_current(key) {
                            self.thumbnails.insert(key, None, Some(message));
                        }
                        continue;
                    }
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
        let tabs = &self.tabs;
        self.thumbnails.retain(|key| {
            tabs.iter().any(|tab| {
                tab.id == key.tab
                    && tab.status.revision == key.revision
                    && tab.device_ratio.to_bits() == key.device_ratio_bits
            })
        });
        self.continue_discard_quit(context);
        self.schedule();
    }

    fn thumbnail_is_current(&self, key: crate::thumbnails::ThumbnailKey) -> bool {
        self.tabs.iter().any(|tab| {
            tab.id == key.tab
                && tab.status.revision == key.revision
                && tab.device_ratio.to_bits() == key.device_ratio_bits
                && key.page < tab.status.info.page_count
        })
    }

    fn schedule(&mut self) {
        if self.busy
            || self.close_target.is_some()
            || self.recovery.open
            || !self.pending_saves.is_empty()
            || self.pages_dialog.is_some()
            || self.sanitize_dialog.is_some()
            || self.merge_dialog.is_some()
            || self.notes_dialog.is_some()
            || self.password_dialog.is_some()
        {
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
            return;
        }
        if self.show_sidebar
            && self.sidebar_mode == SidebarMode::Pages
            && let Some(key) = self.visible_thumbnails.iter().copied().find(|key| {
                Some(key.tab) == self.active
                    && self.thumbnail_is_current(*key)
                    && !self.thumbnails.contains(*key)
            })
        {
            // One slot, one active operation; foreground render/text/search always wins the next dispatch.
            match self.worker.send(Command::Thumbnail(key)) {
                Ok(()) => self.busy = true,
                Err(error) => self.message = error,
            }
        }
    }

    fn open(&mut self) {
        if self.busy || self.tabs.len() >= 16 || self.password_dialog.is_some() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .pick_file()
        {
            self.begin_open(path);
        }
    }

    fn sync_display_ratio(&mut self, ratio: f32) {
        for tab in &mut self.tabs {
            tab.set_device_ratio(ratio);
        }
    }

    fn import_images(&mut self) {
        if self.busy || self.tabs.len() >= crate::worker::MAX_TABS {
            return;
        }
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("PNG / JPEG images", &["png", "jpg", "jpeg"])
            .pick_files()
        {
            self.start_image_conversion(paths);
        }
    }

    fn start_image_conversion(&mut self, paths: Vec<std::path::PathBuf>) -> bool {
        if self.busy || self.tabs.len() >= crate::worker::MAX_TABS || paths.is_empty() {
            return false;
        }
        if paths.len() > pdf_core::MAX_PAGES {
            self.message = "Select at most 20,000 image pages.".into();
            return false;
        }
        let id = self.next_id;
        let Some(next) = id.checked_add(1) else {
            self.message =
                "Document identifier limit reached. Restart after saving your work.".into();
            return false;
        };
        if self.send(
            Command::Images { id, paths },
            "Converting PNG/JPEG images locally…",
        ) {
            self.next_id = next;
            true
        } else {
            false
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

    fn has_tab_draft(&self, id: u64) -> bool {
        self.metadata_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.key.tab == id && dialog.has_changes())
            || self
                .forms_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.key.tab == id && dialog.has_changes())
            || self
                .overlay_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.key.tab == id && dialog.has_changes())
            || self
                .notes_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.key.tab == id && dialog.has_changes())
    }

    fn request_close(&mut self, id: u64) {
        if self.busy {
            return;
        }
        if self.tabs.iter().any(|tab| tab.id == id && tab.status.dirty) || self.has_tab_draft(id) {
            self.close_target = Some(CloseTarget::Tab(id));
        } else {
            self.send(Command::Close(id), "Closing PDF…");
        }
    }

    fn needs_exit_guard(&self) -> bool {
        !self.allow_exit
            && (self.busy
                || !self.pending_saves.is_empty()
                || self.tabs.iter().any(|tab| tab.status.dirty)
                || self
                    .notes_dialog
                    .as_ref()
                    .is_some_and(notes::Dialog::has_changes)
                || self
                    .metadata_dialog
                    .as_ref()
                    .is_some_and(metadata::Dialog::has_changes)
                || self
                    .forms_dialog
                    .as_ref()
                    .is_some_and(forms::Dialog::has_changes)
                || self
                    .overlay_dialog
                    .as_ref()
                    .is_some_and(text_overlay::Dialog::has_changes))
    }

    fn request_exit(&mut self, context: &egui::Context) {
        if !self.busy {
            self.cancel_password();
        }
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
            Action::Images => self.import_images(),
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
            Action::Move(earlier) => {
                if let Some(tab) = self.selected().filter(|tab| tab.editable()) {
                    self.send(
                        Command::Edit {
                            id: tab.id,
                            edit: Edit::Move {
                                page: tab.page as u32 + 1,
                                earlier,
                            },
                        },
                        "Moving page…",
                    );
                }
            }
            Action::Reverse => {
                if let Some(tab) = self.selected().filter(|tab| tab.editable()) {
                    self.send(
                        Command::Edit {
                            id: tab.id,
                            edit: Edit::Reverse {
                                page: tab.page as u32 + 1,
                            },
                        },
                        "Reversing page order…",
                    );
                }
            }
            Action::InsertBlank => self.open_organizer_dialog(false),
            Action::Crop => self.open_organizer_dialog(true),
            Action::Metadata => self.open_metadata(),
            Action::Notes => self.open_notes(),
            Action::Forms => self.open_forms(),
            Action::TextOverlay => self.open_text_overlay(),
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
            Action::Recovery => self.open_recovery(),
            Action::ExtractPages => self.open_page_ranges(pages::Mode::Extract),
            Action::DeletePages => self.open_page_ranges(pages::Mode::Delete),
            Action::DuplicatePages => self.open_page_ranges(pages::Mode::Duplicate),
            Action::Sanitize => self.open_sanitization(),
            Action::Merge => self.open_merge(),
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
        self.show_initial_window(context);
        self.poll(context);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.sync_display_ratio(context.pixels_per_point());
        if context.input(|input| input.viewport().close_requested()) && !self.allow_exit {
            self.request_exit(&context);
        }
        if self.organizer_dialog.is_none()
            && self.forms_dialog.is_none()
            && self.overlay_dialog.is_none()
            && self.metadata_dialog.is_none()
            && self.notes_dialog.is_none()
            && !self.recovery.open
            && self.pages_dialog.is_none()
            && self.sanitize_dialog.is_none()
            && self.merge_dialog.is_none()
            && self.password_dialog.is_none()
            && self.pending_saves.is_empty()
        {
            self.shortcuts(&context);
        } else if context
            .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Q))
        {
            self.request_exit(&context);
        }
        ui.add_enabled_ui(
            self.close_target.is_none()
                && !self.palette_open
                && self.organizer_dialog.is_none()
                && self.metadata_dialog.is_none()
                && self.notes_dialog.is_none()
                && self.forms_dialog.is_none()
                && !self.recovery.open
                && self.pending_saves.is_empty()
                && self.pages_dialog.is_none()
                && self.sanitize_dialog.is_none()
                && self.merge_dialog.is_none()
                && self.password_dialog.is_none(),
            |ui| self.workstation(ui),
        );
        self.command_palette(&context);
        self.close_dialog(&context);
        self.organizer_dialog_ui(&context);
        self.metadata_dialog_ui(&context);
        self.forms_dialog_ui(&context);
        self.text_overlay_dialog(&context);
        self.recovery_dialog(&context);
        self.pending_save_dialog(&context);
        self.page_ranges_dialog(&context);
        self.sanitization_dialog(&context);
        self.notes_dialog_ui(&context);
        self.merge_dialog(&context);
        self.password_dialog_ui(&context);
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
    fn image_conversion_ui_adds_real_unsaved_preview_without_replacing_existing_work() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        let mut previous = Tab::new(
            9,
            "previous.pdf".into(),
            DocumentStatus {
                info: DocumentInfo {
                    version: "1.7".into(),
                    page_count: 1,
                    object_count: 5,
                    encrypted: false,
                    signature_fields: 0,
                    pages: vec![],
                },
                revision: 3,
                dirty: true,
                can_undo: true,
                can_redo: false,
            },
        );
        previous.needs_render = false;
        app.tabs.push(previous);
        app.active = Some(9);
        let directory = tempfile::tempdir().expect("directory");
        let image = directory.path().join("red.png");
        std::fs::write(&image, crate::worker::TEST_RED_PNG).expect("fixture");
        assert!(app.start_image_conversion(vec![image]));
        // Actual bounded child work may contend with the parallel suite. Wait for
        // its production deadline plus event-delivery grace, not host speed.
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(pdf_worker::CPU_SECONDS + 5);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll(&context.egui_ctx);
            std::thread::yield_now();
        }
        assert!(!app.busy);
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[0].status.revision, 3);
        assert!(app.tabs[0].status.dirty);
        let generated = app.tabs.last().expect("new tab");
        assert!(generated.status.dirty);
        assert_eq!(generated.status.info.page_count, 1);
        assert_eq!(
            generated.texture.as_ref().expect("real preview").size(),
            [2, 3]
        );
        assert_eq!(app.active, Some(generated.id));
        assert_eq!(
            std::fs::read_dir(directory.path()).expect("files").count(),
            1
        );
        let id = generated.id;
        app.request_close(id);
        assert_eq!(app.close_target, Some(CloseTarget::Tab(id)));
    }

    #[test]
    fn failed_image_conversion_and_ui_capacity_guards_leave_tabs_and_selection_intact() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        let directory = tempfile::tempdir().expect("directory");
        let invalid = directory.path().join("invalid.png");
        std::fs::write(&invalid, b"not an image").expect("invalid fixture");
        let status = || DocumentStatus {
            info: DocumentInfo {
                version: "1.7".into(),
                page_count: 1,
                object_count: 5,
                encrypted: false,
                signature_fields: 0,
                pages: vec![],
            },
            revision: 2,
            dirty: true,
            can_undo: true,
            can_redo: false,
        };
        let mut previous = Tab::new(9, "previous.pdf".into(), status());
        previous.needs_render = false;
        app.tabs.push(previous);
        app.active = Some(9);
        assert!(!app.start_image_conversion(vec![]));
        assert!(app.start_image_conversion(vec![invalid.clone()]));
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(pdf_worker::CPU_SECONDS + 5);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll(&context.egui_ctx);
            std::thread::yield_now();
        }
        assert!(!app.busy);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, Some(9));
        assert!(app.tabs[0].status.dirty);
        assert!(app.message.contains("PNG/JPEG"));
        for id in 10..25 {
            app.tabs.push(Tab::new(id, "other.pdf".into(), status()));
        }
        assert_eq!(app.tabs.len(), crate::worker::MAX_TABS);
        let next_id = app.next_id;
        assert!(!app.start_image_conversion(vec![invalid]));
        assert_eq!(app.next_id, next_id);
        assert_eq!(app.active, Some(9));
        assert!(!app.busy);
    }

    #[test]
    fn initial_logic_shows_hidden_window_once_without_reopening_it_later() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        let first = context
            .egui_ctx
            .run_logic(&Default::default(), |ctx| app.show_initial_window(ctx));
        let commands = &first.viewport_commands[&egui::ViewportId::ROOT];
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Visible(true)))
        );
        let later = context
            .egui_ctx
            .run_logic(&Default::default(), |ctx| app.show_initial_window(ctx));
        assert!(
            !later
                .viewport_commands
                .values()
                .flatten()
                .any(|command| matches!(command, egui::ViewportCommand::Visible(true)))
        );
    }

    #[test]
    fn thousand_page_sidebar_requests_only_visible_pages_and_foreground_work_wins() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        app.tabs.push(Tab::new(
            1,
            "many.pdf".into(),
            DocumentStatus {
                info: DocumentInfo {
                    version: "1.7".into(),
                    page_count: 1000,
                    object_count: 1002,
                    encrypted: false,
                    signature_fields: 0,
                    pages: (1..=1000)
                        .map(|number| pdf_core::PageInfo {
                            number,
                            media_box: [0.0, 0.0, 100.0, 80.0],
                            crop_box: [0.0, 0.0, 100.0, 80.0],
                            rotation: 0,
                        })
                        .collect(),
                },
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        let mut output = context.egui_ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(240.0, 480.0),
                )),
                ..Default::default()
            },
            |ui| app.sidebar(ui),
        );
        output.textures_delta.clear();
        assert!(!app.visible_thumbnails.is_empty());
        assert!(app.visible_thumbnails.len() <= crate::thumbnails::MAX_VISIBLE);
        assert!(app.visible_thumbnails.iter().all(|key| key.page < 10));
        let first = app.visible_thumbnails[0];
        app.tabs[0].search.draft = "PDF".into();
        app.tabs[0].search.start(1, 0);
        app.show_page_text = true;
        let dispatch = |app: &mut EditorApp| {
            app.schedule();
            assert!(app.busy);
            // These UI-only documents intentionally are absent from the worker; its typed error proves which task was dispatched.
            let task = loop {
                match app
                    .worker
                    .events
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .expect("worker response")
                {
                    Event::RecoveryListed { .. } => continue,
                    Event::Error {
                        task: Some(task), ..
                    } => break task,
                    _ => panic!("typed task error"),
                }
            };
            app.busy = false;
            task
        };
        assert!(matches!(dispatch(&mut app), TaskKey::Render(_)));
        assert!(matches!(dispatch(&mut app), TaskKey::Text(_)));
        assert!(matches!(dispatch(&mut app), TaskKey::Search(_)));
        app.tabs[0].search.cancel();
        assert!(matches!(dispatch(&mut app),TaskKey::Thumbnail(key) if key == first));
        app.tabs[0].set_device_ratio(2.0);
        assert!(!app.thumbnail_is_current(first));
        app.tabs[0].set_device_ratio(1.0);
        app.tabs[0].status.revision = 1;
        assert!(!app.thumbnail_is_current(first));
        app.tabs.clear();
        assert!(!app.thumbnail_is_current(first));
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
