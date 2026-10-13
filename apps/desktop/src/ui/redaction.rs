use super::EditorApp;
#[cfg(test)]
use crate::worker::redaction_tests::fixture;
use crate::{
    state::{DocumentStatus, Tab},
    worker::Command,
};
use eframe::egui;
use pdf_core::{TextRedactionReport, TextRedactionRequest};
use std::path::PathBuf;
pub(super) struct Dialog {
    pub source: u64,
    revision: u64,
    targets: String,
    confirmed: bool,
    pub expected_id: Option<u64>,
    pub error: Option<String>,
    report: Option<TextRedactionReport>,
}

impl Dialog {
    pub fn has_changes(&self) -> bool {
        self.report.is_none() && !self.targets.is_empty()
    }
    fn request(&self) -> Result<TextRedactionRequest, String> {
        if self.targets.len() > 65536 {
            return Err("Target draft exceeds 64 KiB; original draft retained".into());
        }
        let request = TextRedactionRequest {
            targets: self.targets.lines().map(str::to_owned).collect(),
        };
        request.validate().map_err(|e| e.to_string())?;
        Ok(request)
    }
}
impl EditorApp {
    pub(super) fn open_redaction(&mut self) {
        if self.busy
            || !self.pending_saves.is_empty()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || self.close_target.is_some()
            || self.recovery.open
            || self.redaction_dialog.is_some()
            || self.sanitize_dialog.is_some()
            || self.merge_dialog.is_some()
            || self.split_dialog.is_some()
            || self.pages_dialog.is_some()
            || self.password_dialog.is_some()
            || self.organizer_dialog.is_some()
            || self.forms_dialog.is_some()
            || self.metadata_dialog.is_some()
            || self.notes_dialog.is_some()
            || self.overlay_dialog.is_some()
            || self.vector_dialog.is_some()
            || self.image_dialog.is_some()
        {
            return;
        }
        let Some(tab) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        self.redaction_dialog = Some(Dialog {
            source: tab.id,
            revision: tab.status.revision,
            targets: String::new(),
            confirmed: false,
            expected_id: None,
            error: None,
            report: None,
        });
    }
    fn redaction_current(&self) -> bool {
        self.redaction_dialog.as_ref().is_some_and(|d| {
            self.active == Some(d.source)
                && self
                    .tabs
                    .iter()
                    .any(|t| t.id == d.source && t.status.revision == d.revision && t.editable())
        })
    }
    fn apply_redaction(&mut self) -> bool {
        if self.busy
            || !self.pending_saves.is_empty()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || !self.redaction_current()
            || self.next_id == 0
            || self.next_id == u64::MAX
        {
            return false;
        }
        let Some(d) = self
            .redaction_dialog
            .as_ref()
            .filter(|d| d.confirmed && d.expected_id.is_none() && d.report.is_none())
        else {
            return false;
        };
        let request = match d.request() {
            Ok(request) => request,
            Err(error) => {
                if let Some(d) = self.redaction_dialog.as_mut() {
                    d.error = Some(error);
                }
                return false;
            }
        };
        let command = Command::RedactText {
            id: d.source,
            new_id: self.next_id,
            revision: d.revision,
            request,
        };
        if self.send(
            command,
            "Creating and verifying a separate redacted copy locally…",
        ) {
            if let Some(d) = self.redaction_dialog.as_mut() {
                d.expected_id = Some(self.next_id);
                d.error = None;
            }
            self.next_id += 1;
            true
        } else {
            false
        }
    }
    pub(super) fn accept_redacted(
        &mut self,
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
        report: TextRedactionReport,
        context: &egui::Context,
    ) {
        if self
            .redaction_dialog
            .as_ref()
            .is_some_and(|d| d.expected_id == Some(id))
            && self.redaction_current()
            && id != 0
            && id != u64::MAX
            && !self.tabs.iter().any(|t| t.id == id)
            && self.tabs.len() < crate::worker::MAX_TABS
            && status.dirty
            && !status.can_undo
            && !status.can_redo
            && report.full_rewrite
            && report.changed_text_shows > 0
        {
            let mut tab = Tab::new(id, path, status);
            tab.set_device_ratio(context.pixels_per_point());
            self.next_id = self.next_id.max(id + 1);
            self.tabs.push(tab);
            self.active = Some(id);
            if let Some(d) = self.redaction_dialog.as_mut() {
                d.expected_id = None;
                d.targets.clear();
                d.confirmed = false;
                d.report = Some(report);
            }
            self.message="Verified redacted copy created in a new unsaved tab. Original tabs, files, history and recovery copies remain.".into();
        } else {
            self.message =
                "Unexpected or stale redaction result ignored; existing tabs retained.".into();
            if let Some(d) = self
                .redaction_dialog
                .as_mut()
                .filter(|d| d.expected_id == Some(id))
            {
                d.expected_id = None;
                d.error = Some(self.message.clone());
            }
        }
    }
    pub(super) fn redaction_dialog_ui(&mut self, context: &egui::Context) {
        if self.redaction_dialog.is_none() {
            return;
        }
        let current = self.redaction_current();
        let enabled = current
            && !self.busy
            && self.pending_saves.is_empty()
            && self.tabs.len() < crate::worker::MAX_TABS;
        let mut apply = false;
        let mut close = false;
        let Some(d) = self.redaction_dialog.as_mut() else {
            return;
        };
        egui::Window::new("Permanent text redacted copy").anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).collapsible(false).resizable(false).default_width(560.0).max_width((context.content_rect().width()-32.0).max(280.0)).show(context,|ui|{
            egui::ScrollArea::vertical().max_height((context.content_rect().height()-280.0).max(120.0)).show(ui,|ui|{
            if let Some(report)=&d.report {
                ui.label("Exact target glyphs removed and actual output bytes verified within the supported profile.");
                ui.label(format!("Matches: {} · changed text shows: {} · affected pages: {}",report.matched_targets,report.changed_text_shows,report.affected_pages.len()));
                ui.label(format!("Info links: {} · XMP links: {} · embedded-file trees: {} · actions/triggers: {}",report.cleanup.info_links_removed,report.cleanup.xmp_links_removed,report.cleanup.embedded_file_name_trees_removed,report.cleanup.action_links_removed+report.cleanup.automatic_triggers_removed));
                ui.label(format!("Comment carriers cleared: {} · old/unreachable objects discarded: {}",report.content_comments_removed,report.cleanup.unreachable_objects_removed+report.rewritten_stream_orphans_removed));
                ui.label("Use Save As to publish a verified new file. This is not arbitrary-PDF redaction or a general privacy certificate.");
            }else{
                ui.label("Remove exact case-sensitive ASCII targets from PDF text instructions. Unmatched text retains its nominal position; no white covering rectangle is used.");
                ui.small("Supported preview: standard Courier/Helvetica/Times semantic plaintext and one uniform full-page background with explicit text state. Other painted paths, clipping, transforms, images, custom/embedded fonts, Forms, retained annotations, layers, tagging and unsupported layout/state refuse. This is not arbitrary visual or steganographic redaction.");
                ui.label("One exact target per line (1–100; 4096 bytes each, 64 KiB combined):");
                let before=d.targets.clone();
                ui.add_enabled(enabled && d.expected_id.is_none(),egui::TextEdit::multiline(&mut d.targets).id(egui::Id::new("redaction-targets")).desired_rows(5).desired_width(f32::INFINITY));
                if d.targets.len()>65536 {d.targets=before;d.error=Some("Target draft exceeds 64 KiB; previous draft retained".into());}
                if !d.targets.is_empty() && let Err(error)=d.request(){ui.colored_label(egui::Color32::YELLOW,error);}
                ui.add_enabled(enabled && d.expected_id.is_none(),egui::Checkbox::new(&mut d.confirmed,"Remove matching text and supported metadata, attachments, actions and comment carriers from a separate copy"));
                if !current{ui.colored_label(egui::Color32::YELLOW,"The source changed or became read-only. Discard this target draft and review the source again.");}
                ui.small("A failed request keeps the original and your draft. A successful copy has no original-text Undo history.");
            }
            if let Some(error)=&d.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            });
            ui.separator();ui.small("Original tabs, files, histories, recovery journals, backups and other copies remain. This copy does not erase those sources or prove absence of all other private information.");
            ui.horizontal(|ui|{
                if d.report.is_none() && ui.add_enabled(enabled && d.expected_id.is_none() && d.confirmed && d.request().is_ok(),egui::Button::new("Create verified redacted copy")).clicked(){apply=true;}
                if ui.add_enabled(d.expected_id.is_none(),egui::Button::new(if d.report.is_some(){"Done"}else{"Discard targets"})).clicked(){close=true;}
            });
        });
        if apply {
            self.apply_redaction();
        }
        if close {
            self.redaction_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn app(context: &egui::Context) -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(context.clone()));
        let info = pdf_core::PdfDocument::from_bytes(&fixture::fixture(), None)
            .unwrap()
            .info()
            .unwrap();
        app.tabs.push(Tab::new(
            1,
            "source.pdf".into(),
            DocumentStatus {
                info,
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.next_id = 2;
        app
    }
    fn frame(
        app: &mut EditorApp,
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.redaction_dialog_ui(context),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        frame(app, context, Vec::new());
        let output = frame(app, context, Vec::new());
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                let labels = output
                    .shapes
                    .iter()
                    .filter_map(|s| {
                        if let egui::Shape::Text(t) = &s.shape {
                            Some(t.galley.job.text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                panic!("missing {label}; visible labels: {labels:?}")
            });
        for pressed in [true, false] {
            frame(
                app,
                context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn real_keyboard_confirmation_apply_and_discard_controls_keep_stale_or_failed_drafts() {
        let context = egui::Context::default();
        let mut app = app(&context);
        app.open_redaction();
        frame(&mut app, &context, Vec::new());
        frame(&mut app, &context, Vec::new());
        context.memory_mut(|memory| memory.request_focus(egui::Id::new("redaction-targets")));
        frame(
            &mut app,
            &context,
            vec![egui::Event::Text(fixture::SECRET.into())],
        );
        assert_eq!(
            app.redaction_dialog.as_ref().unwrap().targets,
            fixture::SECRET
        );
        assert!(app.has_tab_draft(1));
        assert!(app.needs_exit_guard());
        assert!(!app.apply_redaction());
        click(
            &mut app,
            &context,
            "Remove matching text and supported metadata, attachments, actions and comment carriers from a separate copy",
        );
        assert!(app.redaction_dialog.as_ref().unwrap().confirmed);
        click(&mut app, &context, "Create verified redacted copy");
        assert_eq!(app.redaction_dialog.as_ref().unwrap().expected_id, Some(2));
        assert!(app.busy);
        assert_eq!(app.next_id, 3);
        app.busy = false;
        let d = app.redaction_dialog.as_mut().unwrap();
        d.expected_id = None;
        d.error = Some("Unsupported source; retained".into());
        app.tabs[0].status.revision = 1;
        assert!(!app.apply_redaction());
        assert_eq!(
            app.redaction_dialog.as_ref().unwrap().targets,
            fixture::SECRET
        );
        click(&mut app, &context, "Discard targets");
        assert!(app.redaction_dialog.is_none());
    }
    #[test]
    fn report_creates_only_fresh_unsaved_tab_clears_targets_and_close_protects_draft() {
        let context = egui::Context::default();
        let mut app = app(&context);
        app.open_redaction();
        app.redaction_dialog.as_mut().unwrap().targets = fixture::SECRET.into();
        app.request_close(1);
        assert!(matches!(
            app.close_target,
            Some(super::super::CloseTarget::Tab(1))
        ));
        app.close_target = None;
        let pdf = pdf_core::PdfDocument::from_bytes(&fixture::fixture(), None).unwrap();
        let (clean, report) = pdf.redact_text_copy(&[fixture::SECRET.into()]).unwrap();
        let status = DocumentStatus {
            info: clean.info().unwrap(),
            revision: 0,
            dirty: true,
            can_undo: false,
            can_redo: false,
        };
        app.redaction_dialog.as_mut().unwrap().expected_id = Some(2);
        app.tabs[0].status.revision = 1;
        app.accept_redacted(
            2,
            "ignored.pdf".into(),
            DocumentStatus {
                info: status.info.clone(),
                revision: status.revision,
                dirty: status.dirty,
                can_undo: status.can_undo,
                can_redo: status.can_redo,
            },
            report.clone(),
            &context,
        );
        assert_eq!(app.tabs.len(), 1);
        assert!(app.redaction_dialog.as_ref().unwrap().error.is_some());
        app.tabs[0].status.revision = 0;
        app.redaction_dialog.as_mut().unwrap().expected_id = Some(2);
        app.accept_redacted(2, "redacted.pdf".into(), status, report, &context);
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, Some(2));
        assert!(!app.tabs[0].status.dirty);
        assert!(app.redaction_dialog.as_ref().unwrap().targets.is_empty());
        assert!(!app.redaction_dialog.as_ref().unwrap().has_changes());
        assert!(app.redaction_dialog.as_ref().unwrap().report.is_some());
    }
    #[test]
    fn readonly_pending_capacity_and_oversized_targets_cannot_dispatch() {
        let context = egui::Context::default();
        let mut app = app(&context);
        app.tabs[0].status.info.signature_fields = 1;
        app.open_redaction();
        assert!(app.redaction_dialog.is_none());
        app.tabs[0].status.info.signature_fields = 0;
        app.tabs[0].status.info.encrypted = true;
        app.open_redaction();
        assert!(app.redaction_dialog.is_none());
        app.tabs[0].status.info.encrypted = false;
        app.pending_saves.insert(
            1,
            super::super::recovery::PendingSave::new("pending.pdf".into(), "Pending".into()),
        );
        app.open_redaction();
        assert!(app.redaction_dialog.is_none());
        app.pending_saves.clear();
        app.open_redaction();
        let d = app.redaction_dialog.as_mut().unwrap();
        d.targets = "X".repeat(65537);
        d.confirmed = true;
        assert!(!app.apply_redaction());
        assert!(!app.busy);
        assert_eq!(app.next_id, 2);
        app.redaction_dialog = None;
        for id in 2..=crate::worker::MAX_TABS as u64 {
            app.tabs.push(Tab::new(
                id,
                "source.pdf".into(),
                DocumentStatus {
                    info: app.tabs[0].status.info.clone(),
                    revision: 0,
                    dirty: false,
                    can_undo: false,
                    can_redo: false,
                },
            ));
        }
        app.open_redaction();
        assert!(app.redaction_dialog.is_none());
    }
}
