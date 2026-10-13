use super::{Dialog, EditorApp, PageMap};
use crate::{state::FormsReadKey, worker::Command};
use eframe::egui;
pub(super) fn controls(d: &mut Dialog, ui: &mut egui::Ui, enabled: bool) {
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_label("Existing image instance")
            .selected_text(d.selected_instance.map_or_else(
                || "Select an instance".into(),
                |i| format!("Instance {}", i + 1),
            ))
            .show_ui(ui, |ui| {
                for (index, image) in d.instances.iter().enumerate() {
                    ui.selectable_value(
                        &mut d.selected_instance,
                        Some(index),
                        format!(
                            "Instance {} · {} × {} pixels · Do {}",
                            index + 1,
                            image.pixel_width,
                            image.pixel_height,
                            image.selection.operator
                        ),
                    );
                }
            });
    });
    if d.instances.is_empty() {
        ui.label("No supported direct page image instances found, or discovery is pending.");
    }
    if let Some(image) = d.selected_instance.and_then(|i| d.instances.get(i)) {
        ui.label(format!(
            "Image object {} {} · resource /{}",
            image.selection.image_id.0,
            image.selection.image_id.1,
            String::from_utf8_lossy(&image.selection.resource_name)
        ));
        if let Some(reason) = &image.refusal_reason {
            ui.label(reason);
        }
    }
    ui.small("Preview: direct page Image Do only. Nested Forms, inline images and property-marked content need later support; tagged/signed/decrypted inputs cannot be replaced.");
}
impl EditorApp {
    pub(in crate::ui) fn open_image_replacement(&mut self) {
        if self.image_dialog.is_some() {
            return;
        }
        self.open_image_overlay();
        let Some(d) = self.image_dialog.as_mut() else {
            return;
        };
        d.replacement_mode = true;
        d.canvas_mode = false;
        let key = d.key;
        let page = d.request.page;
        self.send(
            Command::ImageInstances { key, page },
            "Reading source-bound image instances locally…",
        );
    }
    pub(in crate::ui) fn accept_image_instances(
        &mut self,
        key: FormsReadKey,
        page: u32,
        instances: Vec<pdf_core::ImageInstance>,
    ) {
        if !self.image_current() {
            return;
        }
        let Some(d) = self
            .image_dialog
            .as_mut()
            .filter(|d| d.replacement_mode && d.key == key && d.request.page == page)
        else {
            return;
        };
        if instances.len() > 10_000
            || instances.iter().any(|i| {
                i.selection.page != page
                    || i.selection.operator == 0
                    || i.selection.source_hash.len() != 64
                    || i.matrix
                        .iter()
                        .chain(i.corners.iter().flatten())
                        .any(|n| !n.is_finite())
            })
        {
            d.error = Some("Invalid image instance reply; source retained.".into());
            return;
        }
        d.selected_instance = (!instances.is_empty()).then_some(0);
        d.instances = instances;
        d.error = None;
    }
    pub(super) fn existing_image_outline(
        d: &Dialog,
        ui: &egui::Ui,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
    ) {
        if let Some(instance) = d.selected_instance.and_then(|i| d.instances.get(i))
            && let Some(map) = PageMap::new(page, image)
        {
            let mut points = instance
                .corners
                .iter()
                .map(|p| map.point(*p))
                .collect::<Vec<_>>();
            points.push(points[0]);
            ui.painter().with_clip_rect(image).add(egui::Shape::line(
                points,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(190, 95, 40)),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn exercise_replacement_controls(size: egui::Vec2, image_file: std::path::PathBuf) {
        let context = egui::Context::default();
        let mut app = super::super::tests::editor(&context);
        app.image_dialog = None;
        // This controls-only fixture has no worker document or preview request.
        app.tabs[0].needs_render = false;
        app.open_image_replacement();
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(pdf_worker::CPU_SECONDS + 5);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll(&context);
            std::thread::yield_now();
        }
        assert!(!app.busy, "image read did not settle: {}", app.message);
        let key = app.image_dialog.as_ref().unwrap().key;
        let pdf =
            pdf_core::PdfDocument::from_bytes(&crate::worker::test_pdf::fixture(2), None).unwrap();
        let instances = pdf.list_image_instances(1).unwrap();
        app.accept_image_instances(key, 1, instances.clone());
        assert_eq!(
            app.image_dialog.as_ref().unwrap().selected_instance,
            Some(0)
        );
        let frame = |app: &mut EditorApp, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |_| app.image_overlay_dialog(&context),
            );
            output.textures_delta.clear();
            output
        };
        let click = |app: &mut EditorApp, label: &str| {
            frame(app, vec![]);
            let out = frame(app, vec![]);
            let pos = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t)
                        if t.galley.job.text == label
                            && s.clip_rect
                                .contains_rect(t.galley.rect.translate(t.pos.to_vec2()))
                            && context
                                .content_rect()
                                .contains_rect(t.galley.rect.translate(t.pos.to_vec2())) =>
                    {
                        Some(t.galley.rect.translate(t.pos.to_vec2()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"));
            for pressed in [true, false] {
                frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
        };
        click(&mut app, "Instance 1");
        click(
            &mut app,
            &format!(
                "Instance 1 · 1 × 1 pixels · Do {}",
                instances[0].selection.operator
            ),
        );
        app.image_dialog.as_mut().unwrap().image_file = Some(image_file);
        assert!(app.has_tab_draft(1));
        assert!(app.needs_exit_guard());
        let page = app.tabs[0].status.info.pages[0].clone();
        let image = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(200.0, 300.0));
        let mut out = context.run_ui(egui::RawInput::default(), |ui| {
            app.image_canvas(ui, 1, 0, &page, image)
        });
        out.textures_delta.clear();
        assert!(
            out.shapes
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Path(_)))
        );
        app.tabs[0].status.revision = 1;
        assert!(!app.apply_image_overlay());
        assert!(app.image_dialog.as_ref().unwrap().image_file.is_some());
        app.tabs[0].status.revision = 0;
        click(&mut app, "Replace selected instance");
        assert!(
            app.image_dialog.as_ref().unwrap().submitting,
            "replacement was not queued: {}",
            app.message
        );
        app.busy = false;
        app.image_dialog.as_mut().unwrap().submitting = false;
        app.image_dialog.as_mut().unwrap().error = Some("Decode failed; draft retained".into());
        let before = app.image_dialog.as_ref().unwrap().instances[0]
            .selection
            .clone();
        app.accept_image_instances(
            crate::state::FormsReadKey {
                revision: 99,
                ..key
            },
            1,
            vec![],
        );
        assert_eq!(
            app.image_dialog.as_ref().unwrap().instances[0].selection,
            before
        );
        click(&mut app, "Discard image draft");
        assert!(app.image_dialog.is_none());
    }
    #[test]
    fn real_replacement_controls_selection_outline_stale_and_error_retention() {
        exercise_replacement_controls(egui::vec2(1000.0, 900.0), "chosen.png".into());
    }
    #[test]
    fn short_replacement_footer_remains_visible_with_long_native_asset_name() {
        let temp = tempfile::tempdir().unwrap();
        exercise_replacement_controls(
            egui::vec2(640.0, 420.0),
            temp.path()
                .join(format!("{}chosen.png", "signature-".repeat(18))),
        );
    }
}
