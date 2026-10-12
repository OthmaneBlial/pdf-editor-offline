use super::{Action, EditorApp, SidebarMode};
use crate::search::{MAX_QUERY_CHARS, MAX_SEARCH_PAGES};
use crate::state::Tab;
use eframe::egui;

impl EditorApp {
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.sidebar_mode, SidebarMode::Pages, "Pages");
            ui.selectable_value(&mut self.sidebar_mode, SidebarMode::Search, "Search");
            if ui
                .selectable_value(&mut self.sidebar_mode, SidebarMode::Text, "Text")
                .clicked()
            {
                self.show_page_text = true;
            }
        });
        ui.separator();
        match self.sidebar_mode {
            SidebarMode::Pages => self.page_list(ui),
            SidebarMode::Search => self.search_panel(ui),
            SidebarMode::Text => {
                if let Some(tab) = self.selected() {
                    page_text(ui, tab);
                }
            }
        }
    }

    fn page_list(&mut self, ui: &mut egui::Ui) {
        let mut select = None;
        if let Some(tab) = self.selected() {
            egui::ScrollArea::vertical()
                .id_salt(("page-list", tab.id))
                .auto_shrink([false, false])
                .show_rows(ui, 48.0, tab.status.info.page_count, |ui, pages| {
                    for page in pages {
                        let detail =
                            tab.status
                                .info
                                .pages
                                .get(page)
                                .map_or_else(String::new, |info| {
                                    format!(
                                        "{:.0} × {:.0} pt",
                                        info.media_box[2] - info.media_box[0],
                                        info.media_box[3] - info.media_box[1]
                                    )
                                });
                        if ui
                            .add_sized(
                                [ui.available_width(), 44.0],
                                egui::Button::new(format!("Page {}\n{detail}", page + 1))
                                    .selected(page == tab.page),
                            )
                            .clicked()
                        {
                            select = Some(page);
                        }
                    }
                });
        }
        if let Some(page) = select {
            self.perform(Action::Page(page), ui.ctx());
        }
    }

    fn search_panel(&mut self, ui: &mut egui::Ui) {
        let mut selected_page = None;
        let mut stopped = false;
        if let Some(tab) = self.selected_mut() {
            ui.label("Find literal text");
            let response = ui.add(
                egui::TextEdit::singleline(&mut tab.search.draft)
                    .char_limit(MAX_QUERY_CHARS)
                    .hint_text("Search this document…")
                    .id_salt(("search-query", tab.id)),
            );
            ui.checkbox(&mut tab.search.match_case, "Match case");
            let mut start =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                start |= ui
                    .add_enabled(!tab.search.draft.is_empty(), egui::Button::new("Search"))
                    .clicked();
                if ui
                    .add_enabled(tab.search.running, egui::Button::new("Stop"))
                    .clicked()
                {
                    tab.search.cancel();
                    stopped = true;
                }
            });
            if start {
                tab.search.start(tab.id, tab.status.revision);
            }
            ui.small(format!(
                "{} matching page(s) · scanned {} / {}",
                tab.search.hits.len(),
                tab.search.next_page,
                tab.status.info.page_count
            ));
            if tab.search.limited {
                ui.small(format!("Stopped at {MAX_SEARCH_PAGES} matching pages. Refine the query to search further."));
            }
            if tab.search.text_was_truncated {
                ui.small("Some page text exceeded 1 MiB; only the retained prefix was searched.");
            }
            if let Some(error) = &tab.search.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(("search-hits", tab.id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for hit in &tab.search.hits {
                        if ui
                            .selectable_label(
                                tab.page == hit.page,
                                format!("Page {}", hit.page + 1),
                            )
                            .clicked()
                        {
                            selected_page = Some(hit.page);
                        }
                        ui.label(egui::RichText::new(&hit.excerpt).small());
                        ui.separator();
                    }
                    if !tab.search.running && tab.search.key.is_some() && tab.search.hits.is_empty()
                    {
                        ui.label("No literal matches in extractable text.");
                    }
                });
        }
        if let Some(page) = selected_page {
            self.perform(Action::Page(page), ui.ctx());
        }
        if stopped {
            self.message = "Search stopped.".into();
        }
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        ui.separator();
        if let Some(tab) = self.selected() {
            ui.strong(tab.title());
            ui.small(tab.path.display().to_string());
            ui.add_space(12.0);
            egui::Grid::new("document-details")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("PDF version");
                    ui.label(&tab.status.info.version);
                    ui.end_row();
                    ui.label("Pages");
                    ui.label(tab.status.info.page_count.to_string());
                    ui.end_row();
                    ui.label("Objects");
                    ui.label(tab.status.info.object_count.to_string());
                    ui.end_row();
                    ui.label("Encrypted");
                    ui.label(if tab.status.info.encrypted {
                        "Yes"
                    } else {
                        "No"
                    });
                    ui.end_row();
                    ui.label("Signature fields");
                    ui.label(tab.status.info.signature_fields.to_string());
                    ui.end_row();
                    ui.label("Changes");
                    ui.label(if tab.status.dirty { "Unsaved" } else { "Saved" });
                    ui.end_row();
                });
            ui.add_space(16.0);
            ui.strong(format!("Current page {}", tab.page + 1));
            if let Some(page) = tab.status.info.pages.get(tab.page) {
                ui.label(format!(
                    "{:.1} × {:.1} PDF points",
                    page.media_box[2] - page.media_box[0],
                    page.media_box[3] - page.media_box[1]
                ));
                ui.label(format!("Rotation: {}°", page.rotation));
                ui.small(format!(
                    "MediaBox: [{:.1}, {:.1}, {:.1}, {:.1}]",
                    page.media_box[0], page.media_box[1], page.media_box[2], page.media_box[3]
                ));
                ui.small(format!(
                    "CropBox: [{:.1}, {:.1}, {:.1}, {:.1}]",
                    page.crop_box[0], page.crop_box[1], page.crop_box[2], page.crop_box[3]
                ));
            }
            if !tab.editable() {
                ui.label("Read only: protected or signed documents are preserved.");
            }
            ui.separator();
            ui.small("Preview may omit unsupported PDF features. Text search finds matching pages; canvas highlighting is not available yet.");
        }
        ui.add_space(12.0);
        ui.checkbox(&mut self.show_page_text, "Show extracted page text");
        if self.show_page_text
            && let Some(tab) = self.selected()
        {
            page_text(ui, tab);
        }
    }
}

fn page_text(ui: &mut egui::Ui, tab: &Tab) {
    ui.strong(format!("Text · page {}", tab.page + 1));
    if let Some(error) = &tab.text_error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    } else if let Some(page) = &tab.text {
        if page.truncated {
            ui.small("Displaying the first 1 MiB of page text.");
        }
        if page.text.trim().is_empty() {
            ui.label("No extractable text on this page. Scanned content needs OCR, which is not available here yet.");
        } else {
            let mut text = page.text.as_str();
            egui::ScrollArea::vertical()
                .id_salt(("page-text", tab.id, tab.page))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(16)
                            .id_salt(("accessible-page-text", tab.id, tab.page)),
                    );
                });
        }
    } else {
        ui.label("Extracting text locally…");
    }
}
