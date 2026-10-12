use super::EditorApp;
use eframe::egui;

impl EditorApp {
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let background = if ui.visuals().dark_mode {
            egui::Color32::from_gray(28)
        } else {
            egui::Color32::from_gray(213)
        };
        egui::CentralPanel::default().frame(egui::Frame::new().fill(background)).show(ui, |ui| {
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
                        ui.set_min_size(size + egui::vec2(64.0, 64.0));
                        ui.add_space(32.0);
                        ui.horizontal(|ui| {
                            ui.add_space(((viewport_width - size.x) / 2.0).max(32.0));
                            egui::Frame::new().fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(90)))
                                .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color: egui::Color32::from_black_alpha(70) })
                                .show(ui, |ui| {
                                    ui.add(egui::Image::new((texture.id(), size)).alt_text(format!("PDF page {}. Read its extracted text using the Text sidebar.", tab.page + 1)));
                                });
                        });
                        ui.add_space(32.0);
                    } else if tab.error.is_none() {
                        ui.add_space(48.0);
                        ui.centered_and_justified(|ui| { ui.label("Preparing page preview…"); });
                    }
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.28).max(40.0));
                    ui.heading("Your PDFs, on your computer.");
                    ui.add_space(12.0);
                    ui.label("Read, search and organize local documents.");
                    ui.label("No account. No upload. Original files preserved.");
                    ui.add_space(20.0);
                    if ui.add_enabled(!self.busy, egui::Button::new("Open a local PDF").min_size(egui::vec2(180.0, 36.0))).clicked() { self.open(); }
                    ui.add_space(12.0);
                    ui.small("Command/Ctrl+O to open · Command/Ctrl+K for commands");
                });
            }
        });
    }
}
