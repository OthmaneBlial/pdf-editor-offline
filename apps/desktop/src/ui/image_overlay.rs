use super::{EditorApp, text_overlay::placement::PageMap};
use crate::{
    state::FormsReadKey,
    worker::{Command, Edit},
};
use eframe::egui;
use std::path::PathBuf;
mod placement;
pub(super) struct Dialog {
    pub key: FormsReadKey,
    pub request: pdf_core::ImageOverlay,
    initial: [f64; 4],
    bounds: [f64; 4],
    image_file: Option<PathBuf>,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    pub canvas_mode: bool,
    gesture: Option<placement::Gesture>,
}
impl Dialog {
    pub fn has_changes(&self) -> bool {
        self.image_file.is_some()
            || self.request.rectangle != self.initial
            || self.request.rotation != 0
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.image_file.is_none() {
            return Err("Choose a PNG or JPEG image.");
        }
        let [x0, y0, x1, y1] = self.request.rectangle;
        if self
            .request
            .rectangle
            .iter()
            .any(|value| !value.is_finite())
            || x0 >= x1
            || y0 >= y1
            || x0 < self.bounds[0]
            || y0 < self.bounds[1]
            || x1 > self.bounds[2]
            || y1 > self.bounds[3]
        {
            return Err("Choose a nonempty rectangle within the visible page.");
        }
        if !matches!(self.request.rotation, 0 | 90 | 180 | 270) {
            return Err("Image rotation must be 0, 90, 180 or 270 degrees.");
        }
        Ok(())
    }
}
impl EditorApp {
    pub(super) fn open_image_overlay(&mut self) {
        if self.busy
            || !self.pending_saves.is_empty()
            || self.image_dialog.is_some()
            || self.overlay_dialog.is_some()
        {
            return;
        }
        let Some(tab) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        let Some(page) = tab.status.info.pages.get(tab.page) else {
            return;
        };
        let bounds = [
            f64::from(page.media_box[0].max(page.crop_box[0])),
            f64::from(page.media_box[1].max(page.crop_box[1])),
            f64::from(page.media_box[2].min(page.crop_box[2])),
            f64::from(page.media_box[3].min(page.crop_box[3])),
        ];
        let w = bounds[2] - bounds[0];
        let h = bounds[3] - bounds[1];
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return;
        }
        let initial = [
            bounds[0] + w * 0.1,
            bounds[1] + h * 0.1,
            bounds[0] + w * 0.4,
            bounds[1] + h * 0.4,
        ];
        let id = tab.id;
        let revision = tab.status.revision;
        let page = page.number;
        let Some(generation) = self.forms_generation.checked_add(1) else {
            self.message = "Draft counter exhausted".into();
            return;
        };
        self.forms_generation = generation;
        self.image_dialog = Some(Dialog {
            key: FormsReadKey {
                tab: id,
                revision,
                generation,
            },
            request: pdf_core::ImageOverlay {
                page,
                rectangle: initial,
                rotation: 0,
            },
            initial,
            bounds,
            image_file: None,
            submitting: false,
            stale: false,
            error: None,
            canvas_mode: false,
            gesture: None,
        });
    }
    pub(super) fn image_current(&self) -> bool {
        self.image_dialog.as_ref().is_some_and(|dialog| {
            !dialog.stale
                && self.active == Some(dialog.key.tab)
                && self.tabs.iter().any(|tab| {
                    tab.id == dialog.key.tab
                        && tab.status.revision == dialog.key.revision
                        && tab.page + 1 == dialog.request.page as usize
                        && tab.editable()
                })
        })
    }
    pub(super) fn apply_image_overlay(&mut self) -> bool {
        if self.busy || !self.pending_saves.is_empty() || !self.image_current() {
            return false;
        }
        let Some(dialog) = self
            .image_dialog
            .as_ref()
            .filter(|dialog| !dialog.submitting && dialog.validate().is_ok())
        else {
            return false;
        };
        let Some(image_file) = dialog.image_file.clone() else {
            return false;
        };
        let command = Command::Edit {
            id: dialog.key.tab,
            edit: Edit::ImageOverlay {
                key: dialog.key,
                overlay: dialog.request.clone(),
                image_file,
            },
        };
        if !self.send(command, "Decoding and inserting image locally…") {
            return false;
        }
        if let Some(dialog) = self.image_dialog.as_mut() {
            dialog.submitting = true;
            dialog.error = None;
            dialog.gesture = None;
        }
        true
    }
    pub(super) fn image_overlay_dialog(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        if !self.image_current()
            && let Some(dialog) = self.image_dialog.as_mut()
        {
            dialog.stale = true;
            dialog.gesture = None;
        }
        let Some(dialog) = self.image_dialog.as_mut() else {
            return;
        };
        let enabled =
            !dialog.submitting && !dialog.stale && !self.busy && self.pending_saves.is_empty();
        let mut pick = false;
        let mut apply = false;
        let mut discard = false;
        let window = egui::Window::new("Insert new image content")
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .default_height((context.content_rect().height() - 140.0).clamp(220.0, 660.0))
            .max_height((context.content_rect().height() - 140.0).max(220.0))
            .default_width(440.0)
            .max_width((context.content_rect().width() - 32.0).max(240.0));
        let window = if dialog.canvas_mode {
            window.anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 88.0))
        } else {
            window
        };
        window.show(context,|ui|{
            ui.label("Adds a new image over existing artwork. Existing images remain unchanged; this is not image replacement or redaction.");
            if let Some(path)=&dialog.image_file{ui.label(path.file_name().unwrap_or(path.as_os_str()).to_string_lossy());}else{ui.label("No image selected");}
            if ui.add_enabled(enabled,egui::Button::new("Choose PNG/JPEG…")).clicked(){pick=true;}
            ui.small("The frame marks placement. Image pixels appear after insertion; files stay local.");
            ui.add_enabled_ui(enabled,|ui|{
                egui::ComboBox::from_label("Counterclockwise rotation").selected_text(format!("{}°",dialog.request.rotation)).show_ui(ui,|ui|{for rotation in [0,90,180,270]{ui.selectable_value(&mut dialog.request.rotation,rotation,format!("{rotation}°"));}});
                if dialog.canvas_mode{ui.small("Drag outside frame to place · inside to move · corners resize · arrows nudge");if ui.button("Show coordinate controls").clicked(){dialog.canvas_mode=false;dialog.gesture=None;}}
                else{egui::Grid::new("image-coordinate-grid").show(ui,|ui|{for (index,label)in["X0","Y0","X1","Y1"].into_iter().enumerate(){ui.label(label);ui.add(egui::DragValue::new(&mut dialog.request.rectangle[index]).speed(1.0));if index%2==1{ui.end_row();}}});if ui.button("Place on page").clicked(){dialog.canvas_mode=true;}}
            });
            if let Some(error)=&dialog.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            if dialog.stale{ui.colored_label(egui::Color32::YELLOW,"Source changed; the draft is retained. Discard and reopen on the current page.");}
            if let Err(reason)=dialog.validate(){ui.label(reason);}
            ui.horizontal(|ui|{
                if ui.add_enabled(enabled && dialog.validate().is_ok(),egui::Button::new("Insert image")).clicked(){apply=true;}
                if ui.add_enabled(!dialog.submitting,egui::Button::new("Discard image draft")).clicked(){discard=true;}
            });
            ui.small("Coordinates use unrotated PDF space; the rectangle must fit the crop/media boxes. Its aspect ratio can stretch the image. PNG/JPEG: 32 MiB, 8192 per side, 8 million pixels; combined PDF/image 128 MiB. Unsupported color/orientation metadata and tagged pages are refused. Signed/encrypted PDFs are read only. Image metadata is not sanitized.");
        });
        if pick
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG/JPEG image", &["png", "jpg", "jpeg"])
                .pick_file()
            && let Some(dialog) = self.image_dialog.as_mut()
        {
            dialog.image_file = Some(path);
            dialog.error = None;
        }
        if apply {
            self.apply_image_overlay();
        }
        if discard {
            self.image_dialog = None;
        }
    }
}
#[cfg(test)]
mod tests;
