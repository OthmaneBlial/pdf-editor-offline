use super::EditorApp;
use eframe::egui;

impl EditorApp {
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let mut overlay_page = None;
        let colors = crate::style::palette(ui.ctx().global_style().visuals.dark_mode);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(colors.canvas)).show(ui, |ui| {
            if let Some(tab) = self.selected() {
                if tab.raster_plan().is_ok_and(|plan| plan.limited) {
                    ui.small("Preview resolution limited; zoom and page dimensions are unchanged.");
                }
                if let Some(error) = &tab.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                let viewport_width = ui.available_width();
                egui::ScrollArea::both().id_salt(("pdf-page", tab.id)).auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(texture) = &tab.texture {
                        let Ok(plan) = tab.raster_plan() else { return; };
                        let pixels = texture.size();
                        let size = plan.display_size([pixels[0] as u32, pixels[1] as u32]);
                        let size = egui::vec2(size[0], size[1]);
                        ui.set_min_size(size + egui::vec2(56.0, 56.0));
                        ui.add_space(28.0);
                        ui.horizontal(|ui| {
                            ui.add_space(((viewport_width - size.x) / 2.0).max(28.0));
                            egui::Frame::new().fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1.0, if ui.visuals().dark_mode { egui::Color32::from_gray(91) } else { colors.border }))
                                .shadow(egui::epaint::Shadow { offset: [0, 5], blur: 24, spread: 0, color: egui::Color32::from_black_alpha(if ui.visuals().dark_mode { 60 } else { 24 }) })
                                .show(ui, |ui| {
                                    let image=ui.add(egui::Image::new((texture.id(), size)).alt_text(format!("PDF page {}. Read its extracted text using the Text sidebar.", tab.page + 1)));
                                    if let Some(page)=tab.status.info.pages.get(tab.page){overlay_page=Some((tab.id,tab.status.revision,page.clone(),image.rect,ui.clip_rect()));}

                                });
                        });
                        ui.add_space(28.0);
                    } else if tab.error.is_none() {
                        ui.add_space(48.0);
                        ui.centered_and_justified(|ui| { ui.label("Preparing page preview…"); });
                    }
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.28).max(40.0));
                    ui.heading("PDF Editor Offline");
                    ui.add_space(12.0);
                    ui.label("Open a PDF to read, search and organize its pages.");
                    ui.add_space(20.0);
                    if ui.add_enabled(!self.busy, crate::style::primary("Open a local PDF", ui.visuals().dark_mode).min_size(egui::vec2(180.0, 38.0))).clicked() { self.open(); }
                    ui.add_space(12.0);
                    ui.small("Command/Ctrl+O to open · Command/Ctrl+K for commands");
                });
            }
        });
        if let Some((id, revision, page, image, clip)) = overlay_page {
            let old_clip = ui.clip_rect();
            ui.set_clip_rect(old_clip.intersect(clip));
            self.overlay_canvas(ui, id, revision, &page, image);
            self.vector_canvas(ui, id, revision, &page, image);
            self.image_canvas(ui, id, revision, &page, image);
            ui.set_clip_rect(old_clip);
        }
    }
}
