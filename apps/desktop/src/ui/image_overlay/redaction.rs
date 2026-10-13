use super::{EditorApp, instances};
use crate::{
    state::{DocumentStatus, FormsReadKey, Tab},
    worker::Command,
};
use eframe::egui;
use std::path::PathBuf;

impl EditorApp {
    pub(in crate::ui) fn open_image_redaction(&mut self) {
        if self.image_dialog.is_some() || self.tabs.len() >= crate::worker::MAX_TABS {
            return;
        }
        self.open_image_replacement();
        if let Some(d) = self.image_dialog.as_mut() {
            d.redaction_mode = true;
        }
    }
    fn apply_image_redaction(&mut self) -> bool {
        if self.busy
            || !self.pending_saves.is_empty()
            || !self.image_current()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || self.next_id == 0
            || self.next_id == u64::MAX
        {
            return false;
        }
        let Some(d) = self.image_dialog.as_ref().filter(|d| {
            d.redaction_mode
                && !d.submitting
                && d.removal_report.is_none()
                && d.removal_confirmed
                && !d.removal_selections.is_empty()
                && d.removal_selections.len() <= 10000
        }) else {
            return false;
        };
        let command = Command::RedactImages {
            key: d.key,
            new_id: self.next_id,
            selections: d.removal_selections.clone(),
        };
        if !self.send(
            command,
            "Removing and verifying complete selected image assets in a separate copy…",
        ) {
            return false;
        }
        if let Some(d) = self.image_dialog.as_mut() {
            d.submitting = true;
            d.expected_copy = Some(self.next_id);
            d.error = None;
        }
        self.next_id += 1;
        true
    }
    pub(in crate::ui) fn accept_images_redacted(
        &mut self,
        key: FormsReadKey,
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
        report: pdf_core::ImageRedactionReport,
        context: &egui::Context,
    ) {
        let expected = self.image_current()
            && self
                .image_dialog
                .as_ref()
                .is_some_and(|d| d.redaction_mode && d.key == key && d.expected_copy == Some(id));
        if !expected
            || id == 0
            || id == u64::MAX
            || self.tabs.len() >= crate::worker::MAX_TABS
            || self.tabs.iter().any(|t| t.id == id)
            || !status.dirty
            || status.can_undo
            || status.can_redo
            || !report.full_rewrite
            || report.removed_image_objects == 0
        {
            self.message =
                "Unexpected or stale image-removal result ignored; existing tabs retained.".into();
            if let Some(d) = self.image_dialog.as_mut().filter(|d| d.key == key) {
                d.submitting = false;
                d.expected_copy = None;
                d.error = Some(self.message.clone());
            }
            return;
        }
        let mut tab = Tab::new(id, path, status);
        tab.set_device_ratio(context.pixels_per_point());
        self.tabs.push(tab);
        self.active = Some(id);
        self.next_id = self.next_id.max(id + 1);
        if let Some(d) = self.image_dialog.as_mut() {
            d.removal_report = Some(report);
            d.removal_selections.clear();
            d.removal_confirmed = false;
            d.expected_copy = None;
            d.submitting = false;
        }
        self.message="Verified image-removed copy created in an unsaved tab. Original files, tabs and recovery copies remain.".into();
    }
    pub(super) fn image_redaction_dialog(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let current = self.image_current();
        let page_count = self
            .tabs
            .iter()
            .find(|t| {
                self.image_dialog
                    .as_ref()
                    .is_some_and(|d| d.key.tab == t.id)
            })
            .map_or(0, |t| t.status.info.page_count);
        let Some(d) = self.image_dialog.as_mut() else {
            return;
        };
        if !current && d.removal_report.is_none() {
            d.stale = true;
        }
        let enabled = current && !d.submitting && !self.busy && self.pending_saves.is_empty();
        let mut page_change = None;
        let mut apply = false;
        let mut discard = false;
        egui::Window::new("Permanently remove complete image assets")
            .collapsible(false).default_width(470.0)
            .max_width((context.content_rect().width()-32.0).max(240.0))
            .show(context,|ui|{
                if let Some(report)=&d.removal_report {
                    ui.label(format!("Removed {} draws, {} image assets and {} mask assets into a new copy.",report.removed_draws.len(),report.removed_image_objects,report.removed_mask_objects));
                    ui.label(format!("Cleanup: {} Info links, {} XMP links, {} embedded-file name trees, {} associated-file links, {} attachment icons, {} action links and {} automatic triggers removed.",report.cleanup.info_links_removed,report.cleanup.xmp_links_removed,report.cleanup.embedded_file_name_trees_removed,report.cleanup.associated_file_links_removed,report.cleanup.attachment_annotations_removed.len(),report.cleanup.action_links_removed,report.cleanup.automatic_triggers_removed));
                    ui.label("Use Save As on the new tab. This is whole-image removal, not partial-pixel redaction, OCR or a general privacy certificate.");
                    for limitation in &report.limitations {ui.small(limitation);}
                    if ui.button("Close report").clicked(){discard=true;}
                    return;
                }
                ui.label("Select each draw of every image asset you intend to remove. Shared uses on other pages and shared masks must also be included; the verifier refuses incomplete selections. The full rewrite also removes supported metadata, attachments and action carriers, which are listed in the result.");
                ui.horizontal(|ui|{
                    ui.label(format!("Page {} of {page_count}",d.request.page));
                    if ui.add_enabled(enabled && d.request.page>1,egui::Button::new("Previous page")).clicked(){page_change=Some(d.request.page-1);}
                    if ui.add_enabled(enabled && (d.request.page as usize)<page_count,egui::Button::new("Next page")).clicked(){page_change=Some(d.request.page+1);}
                });
                instances::controls(d,ui,enabled);
                if ui.add_enabled(enabled && d.selected_instance.and_then(|i|d.instances.get(i)).is_some_and(|i|i.replaceable),egui::Button::new("Include selected draw")).clicked()
                    && let Some(image)=d.selected_instance.and_then(|i|d.instances.get(i)) {
                    if d.removal_selections.len()>=10000 {d.error=Some("10000 selected-draw limit; selection retained".into());}
                    else if !d.removal_selections.contains(&image.selection){d.removal_selections.push(image.selection.clone());d.removal_confirmed=false;}
                }
                egui::ScrollArea::vertical().max_height(140.0).show(ui,|ui|{
                    for selection in &d.removal_selections {ui.label(format!("Page {} · Do {} · image object {} {}",selection.page,selection.operator,selection.image_id.0,selection.image_id.1));}
                });
                ui.label(format!("{} draws selected",d.removal_selections.len()));
                if ui.add_enabled(enabled && !d.removal_selections.is_empty(),egui::Button::new("Clear removal selection")).clicked(){d.removal_selections.clear();d.removal_confirmed=false;}
                ui.add_enabled(enabled,egui::Checkbox::new(&mut d.removal_confirmed,"Remove these complete assets from a separate copy"));
                ui.small("Preview: RGB/Gray 8-bit plain/Flate images and soft masks with standard-Type1 ASCII text only. JPEG, predictors, ICC, embedded fonts and opaque carriers refuse. Source plus verification output must fit 128 MiB. Original files/history/recovery copies remain; no secure erasure of them is claimed.");
                if d.stale{ui.colored_label(egui::Color32::YELLOW,"Source changed; selection retained. Discard and reopen.");}
                if let Some(error)=&d.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(enabled && d.removal_confirmed && !d.removal_selections.is_empty() && self.tabs.len()<crate::worker::MAX_TABS,egui::Button::new("Create image-redacted copy")).clicked(){apply=true;}
                    if ui.add_enabled(!d.submitting,egui::Button::new("Discard image draft")).clicked(){discard=true;}
                });
            });
        if let Some(page) = page_change {
            let Some(key) = self.image_dialog.as_ref().map(|d| d.key) else {
                return;
            };
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == key.tab) {
                tab.set_page(page as usize - 1);
            }
            if let Some(d) = self.image_dialog.as_mut() {
                d.request.page = page;
                d.instances.clear();
                d.selected_instance = None;
                d.error = None;
            }
            self.send(
                Command::ImageInstances { key, page },
                "Reading image instances on the selected page…",
            );
        }
        if apply {
            self.apply_image_redaction();
        }
        if discard {
            self.image_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn frame(
        app: &mut EditorApp,
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.image_redaction_dialog(context),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        frame(app, context, vec![]);
        let out = frame(app, context, vec![]);
        let position = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if t.galley.job.text == label
                        && s.clip_rect
                            .contains_rect(t.galley.rect.translate(t.pos.to_vec2())) =>
                {
                    Some(t.galley.rect.translate(t.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible {label}"));
        for pressed in [true, false] {
            frame(
                app,
                context,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    fn settle(app: &mut EditorApp, context: &egui::Context) {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(pdf_worker::CPU_SECONDS + 5);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll(context);
            std::thread::yield_now();
        }
        assert!(!app.busy, "{}", app.message);
    }
    #[test]
    fn actual_multi_page_selection_confirmation_failure_and_draft_guards() {
        let context = egui::Context::default();
        let mut app = super::super::tests::editor(&context);
        app.image_dialog = None;
        app.tabs[0].needs_render = false;
        app.open_image_redaction();
        settle(&mut app, &context);
        let key = app.image_dialog.as_ref().unwrap().key;
        let source = crate::worker::image_redaction_tests::fixture::fixture();
        let pdf = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
        app.accept_image_instances(key, 1, pdf.list_image_instances(1).unwrap());
        click(&mut app, &context, "Include selected draw");
        assert_eq!(
            app.image_dialog.as_ref().unwrap().removal_selections.len(),
            1
        );
        assert!(app.has_tab_draft(key.tab));
        assert!(app.needs_exit_guard());
        assert!(!app.apply_image_redaction());
        click(&mut app, &context, "Next page");
        settle(&mut app, &context);
        app.accept_image_instances(key, 2, pdf.list_image_instances(2).unwrap());
        click(&mut app, &context, "Include selected draw");
        assert_eq!(
            app.image_dialog.as_ref().unwrap().removal_selections.len(),
            2
        );
        click(
            &mut app,
            &context,
            "Remove these complete assets from a separate copy",
        );
        assert!(app.image_dialog.as_ref().unwrap().removal_confirmed);
        app.tabs[0].status.revision = 1;
        assert!(!app.apply_image_redaction());
        assert_eq!(
            app.image_dialog.as_ref().unwrap().removal_selections.len(),
            2
        );
        app.tabs[0].status.revision = 0;
        // This controls fixture has no actor document: real dispatch must fail
        // visibly and retain the confirmed selections instead of inventing a tab.
        click(&mut app, &context, "Create image-redacted copy");
        assert!(app.image_dialog.as_ref().unwrap().submitting);
        settle(&mut app, &context);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(
            app.image_dialog.as_ref().unwrap().removal_selections.len(),
            2
        );
        assert!(app.image_dialog.as_ref().unwrap().error.is_some());
        assert!(!app.image_dialog.as_ref().unwrap().submitting);
        click(&mut app, &context, "Clear removal selection");
        assert!(
            app.image_dialog
                .as_ref()
                .unwrap()
                .removal_selections
                .is_empty()
        );
        assert!(!app.image_dialog.as_ref().unwrap().removal_confirmed);
        click(&mut app, &context, "Discard image draft");
        assert!(app.image_dialog.is_none());
    }
    #[test]
    fn only_expected_copy_is_accepted_and_original_tab_is_retained() {
        let context = egui::Context::default();
        let mut app = super::super::tests::editor(&context);
        app.image_dialog = None;
        app.tabs[0].needs_render = false;
        app.open_image_redaction();
        settle(&mut app, &context);
        let key = app.image_dialog.as_ref().unwrap().key;
        let source = crate::worker::image_redaction_tests::fixture::fixture();
        let selections = crate::worker::image_redaction_tests::fixture::selections(&source);
        let pdf = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
        let (copy, report) = pdf.redact_images_copy(&selections).unwrap();
        app.image_dialog.as_mut().unwrap().expected_copy = Some(2);
        let status = || DocumentStatus {
            info: copy.info().unwrap(),
            dirty: true,
            can_undo: false,
            can_redo: false,
            revision: 0,
        };
        app.accept_images_redacted(
            key,
            3,
            PathBuf::from("wrong.pdf"),
            status(),
            report.clone(),
            &context,
        );
        assert_eq!(app.tabs.len(), 1);
        app.image_dialog.as_mut().unwrap().expected_copy = Some(2);
        app.accept_images_redacted(
            key,
            2,
            PathBuf::from("copy.pdf"),
            status(),
            report,
            &context,
        );
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, Some(2));
        assert_eq!(app.tabs[0].id, key.tab);
        assert!(app.image_dialog.as_ref().unwrap().removal_report.is_some());
        assert!(
            app.image_dialog
                .as_ref()
                .unwrap()
                .removal_selections
                .is_empty()
        );
        frame(&mut app, &context, vec![]);
        click(&mut app, &context, "Close report");
        assert!(app.image_dialog.is_none());
    }
}
