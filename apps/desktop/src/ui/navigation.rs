use super::{Action, EditorApp};
use crate::state::Tab;
use eframe::egui;

impl EditorApp {
    pub(super) fn workstation(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let colors = crate::style::palette(ui.ctx().global_style().visuals.dark_mode);
        egui::Panel::top("navigation")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(colors.chrome)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 8,
                        bottom: 8,
                    }),
            )
            .show(ui, |ui| {
                crate::style::chrome(ui);
                ui.add_enabled_ui(
                    self.overlay_dialog.is_none() && self.vector_dialog.is_none(),
                    |ui| self.navigation(ui),
                );
            });
        egui::Panel::top("tab-strip")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(colors.panel)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                ui.add_enabled_ui(
                    self.overlay_dialog.is_none() && self.vector_dialog.is_none(),
                    |ui| self.document_tabs(ui),
                );
            });
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(colors.panel)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| self.status_bar(ui));
        if self.selected().is_some() && self.show_sidebar {
            egui::Panel::left("document-sidebar")
                .default_size(240.0)
                .min_size(196.0)
                .max_size((width * 0.38).clamp(196.0, 360.0))
                .frame(egui::Frame::new().fill(colors.panel).inner_margin(16))
                .show(ui, |ui| {
                    ui.add_enabled_ui(
                        self.overlay_dialog.is_none() && self.vector_dialog.is_none(),
                        |ui| self.sidebar(ui),
                    )
                });
        }
        if self.selected().is_some() && self.show_inspector && width >= 960.0 {
            egui::Panel::right("document-inspector")
                .default_size(256.0)
                .min_size(232.0)
                .max_size(340.0)
                .frame(egui::Frame::new().fill(colors.panel).inner_margin(18))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("inspector-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_enabled_ui(
                                self.overlay_dialog.is_none() && self.vector_dialog.is_none(),
                                |ui| self.inspector(ui),
                            )
                        });
                });
        }
        self.canvas(ui);
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        let dark = ui.ctx().global_style().visuals.dark_mode;
        let spacious = ui.available_width() >= 840.0;
        egui::MenuBar::new().ui(ui, |ui| {
            crate::brand::logo(ui);
            ui.label(egui::RichText::new("Editor").font(crate::style::medium(17.0)));
            ui.add_space(8.0);
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.add_enabled(!self.busy && self.tabs.len() < 16, egui::Button::new("Open PDF…")).clicked() { action = Some(Action::Open); ui.close(); }
                if ui.add_enabled(!self.busy && self.tabs.len()<crate::worker::MAX_TABS,egui::Button::new("Images to PDF…"))
                    .on_hover_text("Create an unsaved PDF from PNG/JPEG images. Unsupported orientation/color metadata is refused. One pixel becomes one PDF point.")
                    .clicked() { action=Some(Action::Images); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some_and(Tab::editable), egui::Button::new("Save As new PDF…")).clicked() { action = Some(Action::Save); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some(), egui::Button::new("Close current PDF")).clicked() { action = Some(Action::Close); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some(), egui::Button::new("Document properties…")).clicked() { action = Some(Action::Metadata); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some(), egui::Button::new("PDF form fields…")).clicked() { action = Some(Action::Forms); ui.close(); }
                if ui.button("Recover previous session…").clicked() { action = Some(Action::Recovery); ui.close(); }
                ui.separator();
                if ui.button("Quit · Command/Ctrl+Q").clicked() { action = Some(Action::Quit); ui.close(); }
            });
            if spacious && ui.add_enabled(!self.busy && self.tabs.len() < 16, crate::style::primary("Open…", dark))
                .on_hover_text("Open local PDF · Command/Ctrl+O · up to 16 tabs").clicked() { action = Some(Action::Open); }
            if spacious && ui.add_enabled(!self.busy && self.selected().is_some_and(Tab::editable), egui::Button::new("Save As…"))
                .on_hover_text("Save a new verified PDF · Command/Ctrl+S · originals are never overwritten").clicked() { action = Some(Action::Save); }
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(!self.busy && self.pending_saves.is_empty() && self.selected().is_some_and(Tab::editable),egui::Button::new("Add text overlay…")).clicked(){action=Some(Action::TextOverlay);ui.close();}
                if ui.add_enabled(!self.busy && self.pending_saves.is_empty() && self.selected().is_some_and(Tab::editable),egui::Button::new("Draw shapes and freehand…")).clicked(){action=Some(Action::VectorOverlay);ui.close();}
                if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_undo), egui::Button::new("Undo · Command/Ctrl+Z")).clicked() { action = Some(Action::Undo); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_redo), egui::Button::new("Redo · Command/Ctrl+Shift+Z")).clicked() { action = Some(Action::Redo); ui.close(); }
                ui.separator();
                if ui.add_enabled(self.selected().is_some(), egui::Button::new("Search text · Command/Ctrl+F")).clicked() { action = Some(Action::Search); ui.close(); }
                if ui.add_enabled(!self.busy && self.pending_saves.is_empty() && self.selected().is_some_and(Tab::editable) && self.tabs.len()<crate::worker::MAX_TABS,egui::Button::new("Create targeted sanitized copy…")).clicked() {action=Some(Action::Sanitize);ui.close();}
                if ui.add_enabled(!self.busy && self.pending_saves.is_empty() && self.selected().is_some_and(Tab::editable) && self.tabs.len()<crate::worker::MAX_TABS,egui::Button::new("Merge PDF files…")).clicked() {action=Some(Action::Merge);ui.close();}
                if ui.add_enabled(!self.busy && self.pending_saves.is_empty() && self.selected().is_some_and(Tab::editable) && self.tabs.len()<crate::worker::MAX_TABS,egui::Button::new("Split into new tabs…")).clicked() {action=Some(Action::Split);ui.close();}
                if ui.add_enabled(!self.busy && self.selected().is_some(),egui::Button::new("Sticky notes on this page…")).clicked(){action=Some(Action::Notes);ui.close();}
                if ui.add_enabled(self.selected().is_some(), egui::Button::new("Read extracted page text")).clicked() { action = Some(Action::PageText); ui.close(); }
            });
            ui.menu_button("Organize", |ui| {
                let editable = !self.busy && self.selected().is_some_and(Tab::editable);
                if ui.add_enabled(editable && self.tabs.len()<crate::worker::MAX_TABS, egui::Button::new("Extract page range…")).clicked() { action=Some(Action::ExtractPages); ui.close(); }
                if ui.add_enabled(editable && self.selected().is_some_and(|tab| tab.status.info.page_count>1), egui::Button::new("Delete page range…")).clicked() { action=Some(Action::DeletePages); ui.close(); }
                if ui.add_enabled(editable && self.pending_saves.is_empty() && self.selected().is_some_and(|tab| tab.status.info.page_count<pdf_core::MAX_PAGES), egui::Button::new("Duplicate page range…")).clicked() { action=Some(Action::DuplicatePages); ui.close(); }
                ui.separator();
                if ui.add_enabled(editable, egui::Button::new("Rotate current page left")).clicked() { action = Some(Action::Rotate(-90)); ui.close(); }
                if ui.add_enabled(editable, egui::Button::new("Rotate current page right")).clicked() { action = Some(Action::Rotate(90)); ui.close(); }
                ui.separator();
                if ui.add_enabled(editable && self.selected().is_some_and(|tab| tab.page > 0), egui::Button::new("Move current page earlier")).clicked() { action = Some(Action::Move(true)); ui.close(); }
                if ui.add_enabled(editable && self.selected().is_some_and(|tab| tab.page + 1 < tab.status.info.page_count), egui::Button::new("Move current page later")).clicked() { action = Some(Action::Move(false)); ui.close(); }
                if ui.add_enabled(editable && self.selected().is_some_and(|tab| tab.status.info.page_count > 1), egui::Button::new("Reverse all pages")).clicked() { action = Some(Action::Reverse); ui.close(); }
                ui.separator();
                if ui.add_enabled(editable && self.selected().is_some_and(|tab| tab.status.info.page_count < pdf_core::MAX_PAGES), egui::Button::new("Insert blank page…")).clicked() { action = Some(Action::InsertBlank); ui.close(); }
                if ui.add_enabled(editable, egui::Button::new("Crop current page…")).clicked() { action = Some(Action::Crop); ui.close(); }
            });
            ui.menu_button("View", |ui| {
                if ui.button(if self.show_sidebar { "Hide sidebar" } else { "Show sidebar" }).clicked() { action = Some(Action::ToggleSidebar); ui.close(); }
                if ui.button(if self.show_inspector { "Hide inspector" } else { "Show inspector" }).on_hover_text("The inspector collapses automatically below a 960-point window width. Page text stays available in the sidebar.").clicked() { action = Some(Action::ToggleInspector); ui.close(); }
                if ui.button(if dark { "Use light theme" } else { "Use dark theme" }).clicked() { action = Some(Action::Theme); ui.close(); }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Commands").on_hover_text("Find a working command · Command/Ctrl+K").clicked() { action = Some(Action::Palette); }
                ui.label(egui::RichText::new("OFFLINE").small().color(ui.visuals().weak_text_color()));
            });
        });
        if let Some(action) = action {
            self.perform(action, ui.ctx());
        }
    }

    fn document_tabs(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        let colors = crate::style::palette(ui.ctx().global_style().visuals.dark_mode);
        egui::ScrollArea::horizontal()
            .id_salt("document-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for tab in &self.tabs {
                        let selected = self.active == Some(tab.id);
                        let title = format!(
                            "{}{}",
                            tab.title(),
                            if tab.status.dirty { " •" } else { "" }
                        );
                        let foreground = if selected {
                            colors.on_selection
                        } else {
                            colors.text
                        };
                        let fill = if selected {
                            colors.selection
                        } else {
                            colors.inset
                        };
                        egui::Frame::new()
                            .fill(fill)
                            .corner_radius(5)
                            .stroke(egui::Stroke::new(
                                1.0,
                                if selected {
                                    colors.copper
                                } else {
                                    colors.border
                                },
                            ))
                            .inner_margin(egui::Margin::symmetric(3, 1))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            [180.0, 28.0],
                                            egui::Button::new(
                                                egui::RichText::new(title)
                                                    .font(crate::style::medium(13.0))
                                                    .color(foreground),
                                            )
                                            .selected(selected)
                                            .fill(fill)
                                            .stroke(egui::Stroke::NONE)
                                            .truncate(),
                                        )
                                        .on_hover_text(tab.path.display().to_string())
                                        .clicked()
                                    {
                                        self.active = Some(tab.id);
                                    }
                                    if ui
                                        .add_enabled(
                                            !self.busy,
                                            egui::Button::new(
                                                egui::RichText::new("Close")
                                                    .small()
                                                    .color(foreground),
                                            )
                                            .fill(fill)
                                            .stroke(egui::Stroke::NONE),
                                        )
                                        .on_hover_text(format!("Close {}", tab.title()))
                                        .clicked()
                                    {
                                        close = Some(tab.id);
                                    }
                                });
                            });
                    }
                    if self.tabs.is_empty() {
                        ui.label(
                            egui::RichText::new("Open a document to begin")
                                .small()
                                .weak(),
                        );
                    }
                });
            });
        if let Some(id) = close {
            self.request_close(id);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        ui.horizontal(|ui| {
            if self.busy {
                ui.spinner();
            }
            let status = if let Some(tab) = self.selected().filter(|tab| tab.search.running) {
                format!(
                    "Search: {} / {} pages · {} matching",
                    tab.search.next_page,
                    tab.status.info.page_count,
                    tab.search.hits.len()
                )
            } else {
                self.message.clone()
            };
            let available = (ui.available_width() - 480.0).max(80.0);
            ui.add_sized(
                [available, 22.0],
                egui::Label::new(egui::RichText::new(status).small()).truncate(),
            );
            if let Some(tab) = self.selected() {
                if tab.search.running {
                    ui.add(
                        egui::ProgressBar::new(
                            tab.search.next_page as f32 / tab.status.info.page_count.max(1) as f32,
                        )
                        .desired_width(60.0),
                    );
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.overlay_dialog.is_none()
                            && self.vector_dialog.is_none()
                            && tab.page > 0,
                        egui::Button::new("Prev").small(),
                    )
                    .on_hover_text("Previous page")
                    .clicked()
                {
                    action = Some(Action::Previous);
                }
                ui.label(format!("{} / {}", tab.page + 1, tab.status.info.page_count));
                if ui
                    .add_enabled(
                        self.overlay_dialog.is_none()
                            && self.vector_dialog.is_none()
                            && tab.page + 1 < tab.status.info.page_count,
                        egui::Button::new("Next").small(),
                    )
                    .on_hover_text("Next page")
                    .clicked()
                {
                    action = Some(Action::Next);
                }
                ui.separator();
                if ui.button("Zoom out").clicked() {
                    action = Some(Action::Zoom(tab.scale / 1.25));
                }
                if ui
                    .button(format!("{:.0}%", tab.scale * 100.0))
                    .on_hover_text("Restore actual size")
                    .clicked()
                {
                    action = Some(Action::Zoom(1.0));
                }
                if ui.button("Zoom in").clicked() {
                    action = Some(Action::Zoom(tab.scale * 1.25));
                }
            }
        });
        if let Some(action) = action {
            self.perform(action, ui.ctx());
        }
    }
}
