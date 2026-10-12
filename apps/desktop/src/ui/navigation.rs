use super::{Action, EditorApp};
use crate::state::Tab;
use eframe::egui;

impl EditorApp {
    pub(super) fn workstation(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        egui::Panel::top("navigation")
            .resizable(false)
            .show(ui, |ui| {
                self.navigation(ui);
                self.document_tabs(ui);
            });
        egui::Panel::bottom("status")
            .resizable(false)
            .show(ui, |ui| self.status_bar(ui));
        if self.selected().is_some() && self.show_sidebar {
            egui::Panel::left("document-sidebar")
                .default_size(220.0)
                .min_size(180.0)
                .max_size((width * 0.38).clamp(180.0, 340.0))
                .show(ui, |ui| self.sidebar(ui));
        }
        if self.selected().is_some() && self.show_inspector && width >= 960.0 {
            egui::Panel::right("document-inspector")
                .default_size(250.0)
                .min_size(210.0)
                .max_size(320.0)
                .show(ui, |ui| self.inspector(ui));
        }
        self.canvas(ui);
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.strong("PDF Editor");
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.add_enabled(!self.busy && self.tabs.len() < 16, egui::Button::new("Open PDF…")).clicked() { action = Some(Action::Open); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some_and(Tab::editable), egui::Button::new("Save As new PDF…")).clicked() { action = Some(Action::Save); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some(), egui::Button::new("Close current PDF")).clicked() { action = Some(Action::Close); ui.close(); }
                ui.separator();
                if ui.button("Quit · Command/Ctrl+Q").clicked() { action = Some(Action::Quit); ui.close(); }
            });
            if ui.add_enabled(!self.busy && self.tabs.len() < 16, egui::Button::new("Open…"))
                .on_hover_text("Open local PDF · Command/Ctrl+O · up to 16 tabs").clicked() { action = Some(Action::Open); }
            if ui.add_enabled(!self.busy && self.selected().is_some_and(Tab::editable), egui::Button::new("Save As…"))
                .on_hover_text("Save a new verified PDF · Command/Ctrl+S · originals are never overwritten").clicked() { action = Some(Action::Save); }
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_undo), egui::Button::new("Undo · Command/Ctrl+Z")).clicked() { action = Some(Action::Undo); ui.close(); }
                if ui.add_enabled(!self.busy && self.selected().is_some_and(|tab| tab.status.can_redo), egui::Button::new("Redo · Command/Ctrl+Shift+Z")).clicked() { action = Some(Action::Redo); ui.close(); }
                ui.separator();
                if ui.add_enabled(self.selected().is_some(), egui::Button::new("Search text · Command/Ctrl+F")).clicked() { action = Some(Action::Search); ui.close(); }
                if ui.add_enabled(self.selected().is_some(), egui::Button::new("Read extracted page text")).clicked() { action = Some(Action::PageText); ui.close(); }
            });
            ui.menu_button("Organize", |ui| {
                let editable = !self.busy && self.selected().is_some_and(Tab::editable);
                if ui.add_enabled(editable, egui::Button::new("Rotate current page left")).clicked() { action = Some(Action::Rotate(-90)); ui.close(); }
                if ui.add_enabled(editable, egui::Button::new("Rotate current page right")).clicked() { action = Some(Action::Rotate(90)); ui.close(); }
            });
            ui.menu_button("View", |ui| {
                if ui.button(if self.show_sidebar { "Hide sidebar" } else { "Show sidebar" }).clicked() { action = Some(Action::ToggleSidebar); ui.close(); }
                if ui.button(if self.show_inspector { "Hide inspector" } else { "Show inspector" }).on_hover_text("The inspector collapses automatically below a 960-point window width. Page text stays available in the sidebar.").clicked() { action = Some(Action::ToggleInspector); ui.close(); }
                if ui.button(if ui.visuals().dark_mode { "Use light theme" } else { "Use dark theme" }).clicked() { action = Some(Action::Theme); ui.close(); }
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
        egui::ScrollArea::horizontal()
            .id_salt("document-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for tab in &self.tabs {
                        let title = format!(
                            "{}{}",
                            tab.title(),
                            if tab.status.dirty { " •" } else { "" }
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
                    if self.tabs.is_empty() {
                        ui.label(egui::RichText::new("No document open").small().weak());
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
            let available = (ui.available_width() - 380.0).max(100.0);
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
                    .add_enabled(tab.page > 0, egui::Button::new("Prev").small())
                    .on_hover_text("Previous page")
                    .clicked()
                {
                    action = Some(Action::Previous);
                }
                ui.label(format!("{} / {}", tab.page + 1, tab.status.info.page_count));
                if ui
                    .add_enabled(
                        tab.page + 1 < tab.status.info.page_count,
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
