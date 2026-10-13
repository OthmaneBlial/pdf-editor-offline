use super::EditorApp;
use crate::{
    state::FormsReadKey,
    worker::{Command, Edit},
};
use eframe::egui;
use pdf_core::{TextAlignment, TextOverlay};
use std::path::PathBuf;
mod placement;

pub(super) struct Dialog {
    pub key: FormsReadKey,
    pub request: TextOverlay,
    initial_rectangle: [f64; 4],
    bounds: [f64; 4],
    font_file: Option<PathBuf>,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    focus: bool,
    canvas_mode: bool,
    snap: bool,
    gesture: Option<placement::Gesture>,
}
impl Dialog {
    pub fn has_changes(&self) -> bool {
        !self.request.value.is_empty()
            || self.font_file.is_some()
            || self.request.rectangle != self.initial_rectangle
            || self.request.size != 12.0
            || self.request.color != [0.0; 3]
            || !matches!(self.request.alignment, TextAlignment::Left)
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.request.value.is_empty() {
            return Err("Enter the text to add.");
        }
        if self.request.value.len() > 64 * 1024 {
            return Err("Text must fit within 64 KiB; it has not been truncated.");
        }
        let [x0, y0, x1, y1] = self.request.rectangle;
        if self.request.rectangle.iter().any(|v| !v.is_finite())
            || x0 >= x1
            || y0 >= y1
            || x0 < self.bounds[0]
            || y0 < self.bounds[1]
            || x1 > self.bounds[2]
            || y1 > self.bounds[3]
        {
            return Err("Choose a nonempty rectangle within the visible page.");
        }
        if !self.request.size.is_finite()
            || !(1.0..=1024.0).contains(&self.request.size)
            || self
                .request
                .color
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Font size must be 1–1024 PDF units; RGB values must be 0–1.");
        }
        if self.font_file.is_none() {
            return Err("Choose a TrueType or CID CFF1 font covering the complete text.");
        }
        Ok(())
    }
    pub fn placement(&self, page: &pdf_core::PageInfo, image: egui::Rect) -> Option<egui::Rect> {
        if self.stale || self.request.page != page.number {
            return None;
        }
        placement::PageMap::new(page, image)?.rectangle(self.request.rectangle)
    }
}
impl EditorApp {
    pub(super) fn open_text_overlay(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.overlay_dialog.is_some() {
            return;
        }
        let Some(tab) = self.selected().filter(|t| t.editable()) else {
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
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let rectangle = [
            bounds[0] + w * 0.1,
            bounds[1] + h * 0.55,
            bounds[2] - w * 0.1,
            bounds[3] - h * 0.1,
        ];
        let id = tab.id;
        let revision = tab.status.revision;
        let page = tab.page as u32 + 1;
        let Some(generation) = self.forms_generation.checked_add(1) else {
            self.message = "Draft request counter exhausted.".into();
            return;
        };
        self.forms_generation = generation;
        self.overlay_dialog = Some(Dialog {
            key: FormsReadKey {
                tab: id,
                revision,
                generation,
            },
            request: TextOverlay {
                page,
                rectangle,
                value: String::new(),
                size: 12.0,
                color: [0.0; 3],
                alignment: TextAlignment::Left,
            },
            initial_rectangle: rectangle,
            bounds,
            font_file: None,
            submitting: false,
            stale: false,
            error: None,
            focus: true,
            canvas_mode: false,
            snap: false,
            gesture: None,
        });
    }
    pub(super) fn overlay_current(&self) -> bool {
        self.overlay_dialog.as_ref().is_some_and(|d| {
            !d.stale
                && self.active == Some(d.key.tab)
                && self.tabs.iter().any(|t| {
                    t.id == d.key.tab
                        && t.status.revision == d.key.revision
                        && t.page + 1 == d.request.page as usize
                        && t.editable()
                })
        })
    }
    pub(super) fn apply_text_overlay(&mut self) -> bool {
        if self.busy || !self.pending_saves.is_empty() || !self.overlay_current() {
            return false;
        }
        let Some(dialog) = self
            .overlay_dialog
            .as_ref()
            .filter(|d| !d.submitting && d.validate().is_ok())
        else {
            return false;
        };
        let Some(font_file) = dialog.font_file.clone() else {
            return false;
        };
        let command = Command::Edit {
            id: dialog.key.tab,
            edit: Edit::TextOverlay {
                key: dialog.key,
                overlay: dialog.request.clone(),
                font_file,
            },
        };
        if !self.send(command, "Adding text overlay locally…") {
            return false;
        }
        if let Some(d) = self.overlay_dialog.as_mut() {
            d.submitting = true;
            d.error = None;
        }
        true
    }
    pub(super) fn text_overlay_dialog(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        if !self.overlay_current()
            && let Some(d) = self.overlay_dialog.as_mut()
        {
            d.stale = true;
        }
        let Some(dialog) = self.overlay_dialog.as_mut() else {
            return;
        };
        let enabled = !dialog.submitting && !dialog.stale && !self.busy;
        if dialog.canvas_mode {
            let mut apply = false;
            let mut discard = false;
            egui::Window::new("Place new text overlay").collapsible(false).resizable(false).anchor(egui::Align2::RIGHT_TOP,egui::vec2(-12.0,88.0)).show(context,|ui|{
                ui.small("Drag to place · handles resize · inside moves · arrows nudge (Shift 10, Alt 0.1 PDF units)");
                ui.add_enabled(enabled,egui::Checkbox::new(&mut dialog.snap,"Snap to 1 PDF unit"));
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Show text controls").clicked(){dialog.canvas_mode=false;dialog.gesture=None;}
                    apply=ui.add_enabled(enabled && dialog.validate().is_ok(),crate::style::primary("Add overlay",ui.visuals().dark_mode)).clicked();
                    discard=ui.add_enabled(!dialog.submitting,egui::Button::new("Discard draft")).clicked();
                });
                if dialog.stale{ui.colored_label(ui.visuals().error_fg_color,"The document changed. Return to text controls to copy the draft.");}
                if let Some(error)=&dialog.error{ui.colored_label(ui.visuals().error_fg_color,error);}
            });
            if discard {
                self.overlay_dialog = None;
            } else if apply {
                self.apply_text_overlay();
            }
            return;
        }
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Add text overlay").default_width(380.0).max_width((context.content_rect().width()-32.0).max(220.0)).max_height((context.content_rect().height()-32.0).max(180.0)).collapsible(false).anchor(egui::Align2::RIGHT_CENTER,egui::vec2(-16.0,0.0)).show(context,|ui|{
            ui.label(format!("Page {} · adds new text over the existing page",dialog.request.page));
            if dialog.stale {ui.colored_label(ui.visuals().error_fg_color,"The document or page changed. Copy your text, then close and reopen this dialog.");}
            egui::ScrollArea::vertical().max_height((ui.available_height()-86.0).max(70.0)).show(ui,|ui|{
                let label=ui.label("Overlay text");let response=ui.add_enabled(enabled,egui::TextEdit::multiline(&mut dialog.request.value).desired_rows(5).desired_width(f32::INFINITY).hint_text("Text to add to this page").id_salt("overlay-value")).labelled_by(label.id);
                if std::mem::take(&mut dialog.focus){response.request_focus();}

                if dialog.request.value.chars().any(|c| matches!(u32::from(c),0x590..=0x8ff|0xfb1d..=0xfdff|0xfe70..=0xfeff|0x1ee00..=0x1eeff)) {
                    ui.small("Right-to-left text may look different in this input box; the PDF is shaped with your chosen font.");
                }
                ui.separator();ui.label("Placement · unrotated PDF coordinates");
                if ui.add_enabled(enabled,egui::Button::new("Place on page")).clicked(){dialog.canvas_mode=true;dialog.focus=false;}
                ui.add_enabled(enabled,egui::Checkbox::new(&mut dialog.snap,"Snap to 1 PDF unit"));
                ui.add_enabled_ui(enabled,|ui|{
                    egui::Grid::new("overlay-rectangle").num_columns(2).show(ui,|ui|{for (name,value) in ["Left X","Bottom Y","Right X","Top Y"].into_iter().zip(dialog.request.rectangle.iter_mut()){let label=ui.label(name);ui.add(egui::DragValue::new(value).speed(0.25)).labelled_by(label.id);ui.end_row();}});
                    ui.horizontal(|ui|{let label=ui.label("Font size · PDF units");ui.add(egui::DragValue::new(&mut dialog.request.size).speed(0.25)).labelled_by(label.id);});
                    ui.horizontal(|ui|{ui.label("Alignment");egui::ComboBox::from_id_salt("overlay-alignment").selected_text(match dialog.request.alignment{TextAlignment::Left=>"Left",TextAlignment::Center=>"Center",TextAlignment::Right=>"Right"}).show_ui(ui,|ui|{for (name,alignment) in [("Left",TextAlignment::Left),("Center",TextAlignment::Center),("Right",TextAlignment::Right)]{if ui.selectable_label(std::mem::discriminant(&dialog.request.alignment)==std::mem::discriminant(&alignment),name).clicked(){dialog.request.alignment=alignment;}}});});
                    let mut rgb=dialog.request.color.map(|v|v as f32);ui.horizontal(|ui|{ui.label("Text color · RGB");if ui.color_edit_button_rgb(&mut rgb).changed(){dialog.request.color=rgb.map(f64::from);}});
                    if ui.button("Choose covering font…").clicked() && let Some(path)=rfd::FileDialog::new().add_filter("TrueType or OpenType font",&["ttf","otf"]).pick_file(){dialog.font_file=Some(path);}
                });
                if let Some(path)=&dialog.font_file {ui.label(path.file_name().map(|name|name.to_string_lossy()).unwrap_or_default());}
                ui.small("Drag on the page to place a rectangle, move it, or resize its handles. PDF coordinates remain available here. Missing glyphs, font restrictions and text that does not fit are refused. Existing text is unchanged; this is not redaction.");
                if let Err(message)=dialog.validate(){ui.small(message);}
                if let Some(message)=&dialog.error{ui.colored_label(ui.visuals().error_fg_color,message);}
            });
            ui.separator();ui.horizontal_wrapped(|ui|{if ui.button("Copy overlay text").clicked(){context.copy_text(dialog.request.value.clone());}apply=ui.add_enabled(enabled && dialog.validate().is_ok(),crate::style::primary("Add overlay",ui.visuals().dark_mode)).clicked();cancel=ui.add_enabled(!dialog.submitting,egui::Button::new(if dialog.has_changes(){"Discard draft"}else{"Cancel"})).clicked();});
            if dialog.submitting {ui.label("Validating font, fit and PDF output…");}
        });
        if cancel {
            self.overlay_dialog = None;
        } else if apply {
            self.apply_text_overlay();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    pub(super) fn app(context: &egui::Context) -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
        let pdf = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
            None,
        )
        .unwrap();
        app.tabs.push(crate::state::Tab::new(
            1,
            "forms.pdf".into(),
            crate::state::DocumentStatus {
                info: pdf.info().unwrap(),
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.open_text_overlay();
        app
    }
    fn draw(
        app: &mut EditorApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.text_overlay_dialog(ctx),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, ctx: &egui::Context, label: &str) -> egui::FullOutput {
        let output = draw(app, ctx, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text)
                    if text.galley.text() == label
                        && shape
                            .clip_rect
                            .contains_rect(text.galley.rect.translate(text.pos.to_vec2())) =>
                {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .expect("fully visible labeled control");
        draw(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        draw(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }
    #[test]
    fn actual_text_input_focus_linebreak_stale_copy_footer_and_exit_guard() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        draw(&mut app, &ctx, vec![]);
        let value = "Résumé e\u{301}\r\nSecond line ΩЖ";
        draw(&mut app, &ctx, vec![egui::Event::Paste(value.into())]);
        draw(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let expected = format!("{value}\n");
        assert_eq!(app.overlay_dialog.as_ref().unwrap().request.value, expected);
        assert!(app.needs_exit_guard());
        assert!(!app.apply_text_overlay());
        assert!(!app.busy);
        let output = draw(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::epaint::Shape::Text(text) if text.galley.text()=="Add overlay" && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))));
        app.overlay_dialog.as_mut().unwrap().font_file = Some("font.ttf".into());
        app.tabs[0].status.revision = 1;
        draw(&mut app, &ctx, vec![]);
        assert!(app.overlay_dialog.as_ref().unwrap().stale);
        assert!(!app.apply_text_overlay());
        app.tabs[0].status.revision = 0;
        assert!(
            !app.apply_text_overlay(),
            "undo cannot silently revalidate a stale draft"
        );
        let copied = click(&mut app, &ctx, "Copy overlay text");
        assert!(
            copied
                .platform_output
                .commands
                .iter()
                .any(|cmd| matches!(cmd,egui::OutputCommand::CopyText(text) if text==&expected))
        );
        app.request_close(1);
        assert_eq!(app.close_target, Some(super::super::CloseTarget::Tab(1)));
        assert!(app.overlay_dialog.is_some());
    }
    #[test]
    fn readonly_pending_wrong_tab_and_oversized_values_never_dispatch() {
        let mut app = app(&egui::Context::default());
        let dialog = app.overlay_dialog.as_mut().unwrap();
        dialog.request.value = "界".repeat(21_846);
        dialog.font_file = Some("font.ttf".into());
        assert!(!app.apply_text_overlay());
        assert_eq!(
            app.overlay_dialog.as_ref().unwrap().request.value.len(),
            65_538
        );
        assert!(!app.busy);
        app.overlay_dialog.as_mut().unwrap().request.value = "Text".into();
        app.pending_saves.insert(
            1,
            super::super::recovery::PendingSave::new(
                "pending.pdf".into(),
                "Recorded attempt".into(),
            ),
        );
        assert!(!app.apply_text_overlay());
        app.pending_saves.clear();
        app.active = Some(99);
        assert!(!app.apply_text_overlay());
        app.active = Some(1);
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_text_overlay());
        app.overlay_dialog = None;
        app.open_text_overlay();
        assert!(app.overlay_dialog.is_none());
    }
    #[test]
    fn true_outline_mapping_uses_crop_origin_rotation_and_actual_image_bounds() {
        let mut app = app(&egui::Context::default());
        let d = app.overlay_dialog.as_mut().unwrap();
        d.request.rectangle = [40.0, 40.0, 120.0, 80.0];
        let mut page = pdf_core::PageInfo {
            number: 1,
            media_box: [0.0, 0.0, 300.0, 200.0],
            crop_box: [20.0, 30.0, 220.0, 130.0],
            rotation: 0,
        };
        for (rotation, size, expected) in [
            (0, [400.0, 200.0], [140.0, 300.0, 300.0, 380.0]),
            (90, [200.0, 400.0], [120.0, 240.0, 200.0, 400.0]),
            (180, [400.0, 200.0], [300.0, 220.0, 460.0, 300.0]),
            (270, [200.0, 400.0], [200.0, 400.0, 280.0, 560.0]),
        ] {
            page.rotation = rotation;
            let rect = d
                .placement(
                    &page,
                    egui::Rect::from_min_size(
                        egui::pos2(100.0, 200.0),
                        egui::vec2(size[0], size[1]),
                    ),
                )
                .unwrap();
            for (actual, expected) in [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
                .into_iter()
                .zip(expected)
            {
                assert!((actual - expected).abs() < 0.01);
            }
        }
        d.stale = true;
        assert!(d.placement(&page, egui::Rect::EVERYTHING).is_none());
    }
}
