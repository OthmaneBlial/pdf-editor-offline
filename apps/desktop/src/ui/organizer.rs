use super::EditorApp;
use crate::worker::{Command, Edit};
use eframe::egui;

pub(super) struct Dialog {
    tab: u64,
    revision: u64,
    page: u32,
    page_count: u32,
    media: [f32; 4],
    kind: Kind,
}

enum Kind {
    Blank {
        position: u32,
        width: f32,
        height: f32,
    },
    Crop {
        rectangle: [f32; 4],
    },
}

impl Dialog {
    fn valid(&self) -> bool {
        match self.kind {
            Kind::Blank {
                position,
                width,
                height,
            } => {
                position > 0
                    && position <= self.page_count + 1
                    && width.is_finite()
                    && height.is_finite()
                    && width > 0.0
                    && height > 0.0
                    && width <= 1_000_000.0
                    && height <= 1_000_000.0
            }
            Kind::Crop { rectangle: r } => {
                r.iter().all(|value| value.is_finite())
                    && r[2] > r[0]
                    && r[3] > r[1]
                    && r[0] >= self.media[0]
                    && r[1] >= self.media[1]
                    && r[2] <= self.media[2]
                    && r[3] <= self.media[3]
            }
        }
    }

    fn edit(&self) -> Edit {
        match self.kind {
            Kind::Blank {
                position,
                width,
                height,
            } => Edit::InsertBlank {
                position,
                width,
                height,
            },
            Kind::Crop { rectangle } => Edit::Crop {
                page: self.page,
                rectangle,
            },
        }
    }
}

impl EditorApp {
    pub(super) fn open_organizer_dialog(&mut self, crop: bool) {
        if self.busy {
            return;
        }
        let Some(tab) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        let Some(info) = tab.status.info.pages.get(tab.page) else {
            return;
        };
        self.organizer_dialog = Some(Dialog {
            tab: tab.id,
            revision: tab.status.revision,
            page: tab.page as u32 + 1,
            page_count: tab.status.info.page_count as u32,
            media: info.media_box,
            kind: if crop {
                Kind::Crop {
                    rectangle: info.crop_box,
                }
            } else {
                Kind::Blank {
                    position: tab.page as u32 + 1,
                    width: info.media_box[2] - info.media_box[0],
                    height: info.media_box[3] - info.media_box[1],
                }
            },
        });
    }

    pub(super) fn organizer_dialog_ui(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let Some(dialog) = self.organizer_dialog.as_mut() else {
            return;
        };
        if !self.tabs.iter().any(|tab| {
            tab.id == dialog.tab && tab.status.revision == dialog.revision && tab.editable()
        }) {
            self.organizer_dialog = None;
            self.message = "Document changed. Open the page settings again.".into();
            return;
        }
        let title = match dialog.kind {
            Kind::Blank { .. } => "Insert blank page",
            Kind::Crop { .. } => "Crop current page",
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(title).open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).default_width(360.0)
            .show(context,|ui| {
                match &mut dialog.kind {
                    Kind::Blank { position,width,height } => {
                        ui.label("Insert before this page number. The last position appends a page.");
                        ui.horizontal(|ui| {
                            let label = ui.label("Position");
                            ui.add(egui::DragValue::new(position).range(1..=dialog.page_count+1)).labelled_by(label.id);
                        });
                        ui.label("Dimensions in PDF points (72 points = 1 inch).");
                        numeric_field(ui,"Width",width,1.0..=1_000_000.0);
                        numeric_field(ui,"Height",height,1.0..=1_000_000.0);
                    }
                    Kind::Crop { rectangle } => {
                        ui.label(format!("Page {} · unrotated PDF coordinates in points",dialog.page));
                        ui.label("Cropping changes the visible area. Hidden content remains in the PDF; this is not redaction.");
                        for (index,label) in ["Left (x₀)","Bottom (y₀)","Right (x₁)","Top (y₁)"].iter().enumerate() {
                            let (lower,upper) = if index%2 == 0 { (dialog.media[0],dialog.media[2]) } else { (dialog.media[1],dialog.media[3]) };
                            numeric_field(ui,label,&mut rectangle[index],lower..=upper);
                        }
                    }
                }
                let valid = dialog.valid();
                if !valid { ui.colored_label(ui.visuals().error_fg_color,"Enter valid dimensions within the page bounds."); }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    apply = ui.add_enabled(valid && !self.busy,crate::style::primary("Apply",ui.visuals().dark_mode)).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        // Escape and the window close button cancel without editing.
        if cancel || context.input(|input| input.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        if apply {
            let id = dialog.tab;
            let edit = dialog.edit();
            if self.send(Command::Edit { id, edit }, "Applying page settings…") {
                self.organizer_dialog = None;
            }
        } else if !open {
            self.organizer_dialog = None;
        }
    }
}

fn numeric_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .range(range)
                .speed(1.0)
                .max_decimals(2),
        )
        .labelled_by(label.id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn organizer_dialogs_use_current_geometry_and_block_protected_or_busy_documents() {
        let context = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = EditorApp::new(&context);
        app.tabs.push(crate::state::Tab::new(
            1,
            "source.pdf".into(),
            crate::state::DocumentStatus {
                info: pdf_core::DocumentInfo {
                    version: "1.7".into(),
                    page_count: 1,
                    object_count: 5,
                    encrypted: false,
                    signature_fields: 0,
                    pages: vec![pdf_core::PageInfo {
                        number: 1,
                        media_box: [0.0, 0.0, 100.0, 80.0],
                        crop_box: [10.0, 10.0, 90.0, 70.0],
                        rotation: 90,
                    }],
                },
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.open_organizer_dialog(true);
        assert!(matches!(
            app.organizer_dialog.as_ref().expect("crop").edit(),
            Edit::Crop {
                page: 1,
                rectangle: [10.0, 10.0, 90.0, 70.0]
            }
        ));
        app.open_organizer_dialog(false);
        assert!(matches!(
            app.organizer_dialog.as_ref().expect("blank").edit(),
            Edit::InsertBlank {
                position: 1,
                width: 100.0,
                height: 80.0
            }
        ));
        app.organizer_dialog = None;
        app.tabs[0].status.info.signature_fields = 1;
        app.open_organizer_dialog(false);
        assert!(app.organizer_dialog.is_none());
        app.tabs[0].status.info.signature_fields = 0;
        app.tabs[0].status.info.encrypted = true;
        app.open_organizer_dialog(true);
        assert!(app.organizer_dialog.is_none());
        app.tabs[0].status.info.encrypted = false;
        app.busy = true;
        app.open_organizer_dialog(false);
        assert!(app.organizer_dialog.is_none());
    }

    #[test]
    fn dialog_validation_rejects_empty_outside_and_nonfinite_geometry() {
        let mut dialog = Dialog {
            tab: 1,
            revision: 0,
            page: 1,
            page_count: 3,
            media: [-10.0, -20.0, 100.0, 80.0],
            kind: Kind::Crop {
                rectangle: [0.0, 0.0, 90.0, 70.0],
            },
        };
        assert!(dialog.valid());
        for rectangle in [
            [0.0, 0.0, 0.0, 70.0],
            [-11.0, 0.0, 90.0, 70.0],
            [0.0, 0.0, 101.0, 70.0],
            [f32::NAN, 0.0, 90.0, 70.0],
        ] {
            dialog.kind = Kind::Crop { rectangle };
            assert!(!dialog.valid());
        }
        dialog.kind = Kind::Blank {
            position: 4,
            width: 595.0,
            height: 842.0,
        };
        assert!(dialog.valid());
        dialog.kind = Kind::Blank {
            position: 5,
            width: 595.0,
            height: 842.0,
        };
        assert!(!dialog.valid());
        dialog.kind = Kind::Blank {
            position: 1,
            width: f32::INFINITY,
            height: 842.0,
        };
        assert!(!dialog.valid());
    }
}
