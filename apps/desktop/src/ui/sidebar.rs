use super::{Action, EditorApp, SidebarMode};
use crate::search::{MAX_QUERY_CHARS, MAX_SEARCH_PAGES};
use crate::state::Tab;
use eframe::egui;

impl EditorApp {
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.sidebar_mode, SidebarMode::Pages, "Pages");
            if ui
                .selectable_value(&mut self.sidebar_mode, SidebarMode::Search, "Search")
                .clicked()
            {
                self.search_focus = true;
            }
            if ui
                .selectable_value(&mut self.sidebar_mode, SidebarMode::Text, "Text")
                .clicked()
            {
                self.show_page_text = true;
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
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
        self.visible_thumbnails.clear();
        if let Some(tab) = self.tabs.iter().find(|tab| Some(tab.id) == self.active) {
            ui.label(
                egui::RichText::new(format!("{} pages", tab.status.info.page_count))
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt(("page-list", tab.id))
                .auto_shrink([false, false])
                .show_rows(ui, 98.0, tab.status.info.page_count, |ui, pages| {
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
                        let key = tab.status.info.pages.get(page).and_then(|info| {
                            crate::thumbnails::ThumbnailKey::new(
                                tab.id,
                                tab.status.revision,
                                page,
                                info,
                                tab.device_ratio,
                            )
                            .ok()
                        });
                        if let Some(key) = key
                            && self.visible_thumbnails.len() < crate::thumbnails::MAX_VISIBLE
                        {
                            self.visible_thumbnails.push(key);
                        }
                        let entry = key.and_then(|key| self.thumbnails.get(key));
                        let label = format!("Page {}\n{detail}", page + 1);
                        let button = if let (Some(key), Some(texture)) =
                            (key, entry.and_then(|entry| entry.texture.as_ref()))
                        {
                            egui::Button::image_and_text(
                                egui::Image::new((texture.id(), key.display_size())),
                                label,
                            )
                        } else {
                            egui::Button::new(label)
                        };
                        let response = ui.add_sized(
                            [ui.available_width(), 90.0],
                            button.selected(page == tab.page),
                        );
                        if let Some(error) = entry.and_then(|entry| entry.error.as_ref()) {
                            response
                                .clone()
                                .on_hover_text(format!("Thumbnail unavailable: {error}"));
                        }
                        if response.clicked() {
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
        let request_focus = self.search_focus;
        if self.selected().is_some() {
            self.search_focus = false;
        }
        if let Some(tab) = self.selected_mut() {
            ui.label(egui::RichText::new("Search document").font(crate::style::medium(16.0)));
            ui.add_space(4.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut tab.search.draft)
                    .char_limit(MAX_QUERY_CHARS)
                    .hint_text("Search this document…")
                    .id_salt(("search-query", tab.id)),
            );
            if request_focus {
                response.request_focus();
            }
            ui.checkbox(&mut tab.search.match_case, "Match case");
            let mut start =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                start |= ui
                    .add_enabled(
                        !tab.search.draft.is_empty(),
                        crate::style::primary("Search", ui.visuals().dark_mode),
                    )
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
                response.request_focus();
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
                        let colors = crate::style::palette(ui.visuals().dark_mode);
                        egui::Frame::new()
                            .fill(colors.inset)
                            .corner_radius(5)
                            .inner_margin(10)
                            .show(ui, |ui| {
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
                            });
                        ui.add_space(6.0);
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
        let colors = crate::style::palette(ui.visuals().dark_mode);
        ui.label(egui::RichText::new("Document").font(crate::style::medium(16.0)));
        ui.separator();
        ui.add_space(8.0);
        if let Some(tab) = self.selected() {
            ui.add(
                egui::Label::new(egui::RichText::new(tab.title()).font(crate::style::medium(14.0)))
                    .truncate(),
            )
            .on_hover_text(tab.path.display().to_string());
            ui.label(
                egui::RichText::new(if tab.status.dirty {
                    "Unsaved changes"
                } else {
                    "No unsaved changes"
                })
                .small()
                .color(if tab.status.dirty {
                    colors.copper
                } else {
                    colors.muted
                }),
            );
            ui.add_space(16.0);
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
                });
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("Page {}", tab.page + 1))
                    .font(crate::style::medium(16.0)),
            );
            if let Some(page) = tab.status.info.pages.get(tab.page) {
                ui.label(format!(
                    "{:.1} × {:.1} pt · media size",
                    page.media_box[2] - page.media_box[0],
                    page.media_box[3] - page.media_box[1]
                ));
                ui.label(format!(
                    "{:.1} × {:.1} pt · crop size",
                    page.crop_box[2] - page.crop_box[0],
                    page.crop_box[3] - page.crop_box[1]
                ));
                ui.label(format!("Rotation: {}°", page.rotation));
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Page boxes and file details").show(ui, |ui| {
                    ui.small(format!(
                        "MediaBox: [{:.1}, {:.1}, {:.1}, {:.1}]",
                        page.media_box[0], page.media_box[1], page.media_box[2], page.media_box[3]
                    ));
                    ui.small(format!(
                        "CropBox: [{:.1}, {:.1}, {:.1}, {:.1}]",
                        page.crop_box[0], page.crop_box[1], page.crop_box[2], page.crop_box[3]
                    ));
                    ui.small(format!("{} PDF objects", tab.status.info.object_count));
                });
            }
            if !tab.editable() {
                ui.label("Read only: protected or signed documents are preserved.");
            }
            ui.add_space(12.0);
            egui::CollapsingHeader::new("About previews").show(ui, |ui| {
                ui.small("Preview may omit unsupported PDF features. Text search finds matching pages; canvas highlighting is not available yet.");
            });
        }
        ui.add_space(16.0);
        ui.separator();
        ui.checkbox(&mut self.show_page_text, "Show extracted page text");
        if self.show_page_text
            && let Some(tab) = self.selected()
        {
            page_text(ui, tab);
        }
    }
}

fn page_text(ui: &mut egui::Ui, tab: &Tab) {
    ui.label(
        egui::RichText::new(format!("Text · page {}", tab.page + 1))
            .font(crate::style::medium(14.0)),
    );
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
