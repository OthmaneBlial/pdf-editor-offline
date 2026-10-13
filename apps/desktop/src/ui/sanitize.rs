use super::EditorApp;
use crate::{
    state::{DocumentStatus, Tab},
    worker::Command,
};
use eframe::egui;
use pdf_core::SanitizationReport;
use std::path::PathBuf;

pub(super) struct Dialog {
    pub source: u64,
    revision: u64,
    pub expected_id: Option<u64>,
    report: Option<SanitizationReport>,
    pub error: Option<String>,
}

const LIMITATION: &str = "Targeted removal is not a privacy certificate or secure text/image redaction. Form values, hidden page/layer content, duplicated strings, custom data and font/image/internal metadata can remain. Original files and filesystem history are retained.";
impl EditorApp {
    pub(super) fn open_sanitization(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.tabs.len() >= crate::worker::MAX_TABS
        {
            return;
        }
        let Some(source) = self.selected().filter(|tab| tab.editable()) else {
            return;
        };
        self.sanitize_dialog = Some(Dialog {
            source: source.id,
            revision: source.status.revision,
            expected_id: None,
            report: None,
            error: None,
        });
    }
    fn sanitization_current(&self) -> bool {
        self.sanitize_dialog.as_ref().is_some_and(|dialog| {
            self.active == Some(dialog.source)
                && self.tabs.iter().any(|tab| {
                    tab.id == dialog.source
                        && tab.status.revision == dialog.revision
                        && tab.editable()
                })
        })
    }
    fn apply_sanitization(&mut self) -> bool {
        if self.busy
            || !self.pending_saves.is_empty()
            || self.tabs.len() >= crate::worker::MAX_TABS
            || !self.sanitization_current()
            || self.next_id == 0
            || self.next_id == u64::MAX
        {
            return false;
        }
        let Some(dialog) = self
            .sanitize_dialog
            .as_ref()
            .filter(|dialog| dialog.expected_id.is_none() && dialog.report.is_none())
        else {
            return false;
        };
        let new_id = self.next_id;
        if self.send(
            Command::Sanitize {
                id: dialog.source,
                new_id,
                revision: dialog.revision,
            },
            "Creating a verified targeted copy…",
        ) {
            self.next_id += 1;
            if let Some(dialog) = self.sanitize_dialog.as_mut() {
                dialog.expected_id = Some(new_id);
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn accept_sanitized(
        &mut self,
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
        report: SanitizationReport,
        context: &egui::Context,
    ) {
        if self
            .sanitize_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.expected_id == Some(id))
            && self.sanitization_current()
            && id != 0
            && id != u64::MAX
            && !self.tabs.iter().any(|tab| tab.id == id)
            && self.tabs.len() < crate::worker::MAX_TABS
            && status.dirty
        {
            let mut tab = Tab::new(id, path, status);
            tab.set_device_ratio(context.pixels_per_point());
            self.next_id = self.next_id.max(id + 1);
            self.tabs.push(tab);
            self.active = Some(id);
            if let Some(dialog) = self.sanitize_dialog.as_mut() {
                dialog.expected_id = None;
                dialog.report = Some(report);
            }
            self.message="Targeted copy created in a new unsaved tab. Original document retained; private data may remain.".into();
        } else {
            self.message =
                "Unexpected targeted-copy result ignored. Existing PDFs retained.".into();
            if let Some(dialog) = self
                .sanitize_dialog
                .as_mut()
                .filter(|dialog| dialog.expected_id == Some(id))
            {
                dialog.expected_id = None;
                dialog.error = Some(self.message.clone());
            }
        }
    }
    pub(super) fn sanitization_dialog(&mut self, context: &egui::Context) {
        if self.sanitize_dialog.is_none() {
            return;
        }
        let current = self.sanitization_current();
        let mut apply = false;
        let mut close = false;
        let Some(dialog) = self.sanitize_dialog.as_ref() else {
            return;
        };
        egui::Window::new("Targeted sanitized copy").collapsible(false).resizable(false).default_width(480.0).max_width((context.content_rect().width()-32.0).max(280.0)).max_height((context.content_rect().height()-60.0).max(200.0)).vscroll(true).show(context,|ui| {
            if let Some(report)=&dialog.report {
                ui.label("Removed from the new copy:");
                for (name,count) in [("Info metadata links",report.info_links_removed),("XMP links",report.xmp_links_removed),("Associated-file links",report.associated_file_links_removed),("Embedded-file name trees",report.embedded_file_name_trees_removed),("JavaScript name trees",report.javascript_name_trees_removed),("Collections",report.collections_removed),("Action links",report.action_links_removed),("Automatic triggers",report.automatic_triggers_removed),("Attachment annotation occurrences",report.attachment_annotations_removed.len()),("Unreachable objects (including carriers)",report.unreachable_objects_removed),("Old transport containers",report.transport_containers_removed)] {ui.label(format!("{name}: {count}"));}
                ui.label("Save As publishes a verified new file. The original tab is unchanged.");
            } else {
                ui.label("Remove supported Info/XMP metadata, file associations, attachment icons, actions and automatic triggers. Discard unreachable data through a full rewrite into a separate unsaved tab.");
                ui.label("Linked/tagged/locked attachments, unsupported appearances, XFA, rich media or retained private aliases may be refused. A failed request preserves your original tab.");
                if !current {ui.colored_label(egui::Color32::YELLOW,"The source changed or became unavailable. Close this dialog and review it again.");}
            }
            ui.separator();ui.label(LIMITATION);
            if let Some(error)=&dialog.error {ui.colored_label(egui::Color32::LIGHT_RED,error);}
            ui.horizontal(|ui| {
                if dialog.report.is_none() && ui.add_enabled(current && !self.busy && self.pending_saves.is_empty() && dialog.expected_id.is_none() && self.tabs.len()<crate::worker::MAX_TABS && self.next_id!=0 && self.next_id!=u64::MAX,egui::Button::new("Create separate copy")).clicked() {apply=true;}
                if ui.add_enabled(dialog.expected_id.is_none(),egui::Button::new(if dialog.report.is_some(){"Done"}else{"Cancel"})).clicked() {close=true;}
            });
        });
        if apply {
            self.apply_sanitization();
        }
        if close {
            self.sanitize_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn app() -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ));
        let info = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/attachment.pdf"),
            None,
        )
        .unwrap()
        .info()
        .unwrap();
        app.tabs.push(Tab::new(
            1,
            "source.pdf".into(),
            DocumentStatus {
                info,
                revision: 7,
                dirty: true,
                can_undo: true,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.next_id = 2;
        app
    }
    #[test]
    fn preview_stale_readonly_busy_and_save_guards_do_not_dispatch() {
        let mut app = app();
        app.open_sanitization();
        assert!(app.sanitize_dialog.is_some());
        app.tabs[0].status.revision += 1;
        assert!(!app.apply_sanitization());
        assert_eq!(app.next_id, 2);
        assert!(!app.busy);
        app.sanitize_dialog = None;
        app.tabs[0].status.info.signature_fields = 1;
        app.open_sanitization();
        assert!(app.sanitize_dialog.is_none());
        app.tabs[0].status.info.signature_fields = 0;
        app.busy = true;
        app.open_sanitization();
        assert!(app.sanitize_dialog.is_none());
        app.busy = false;
        app.pending_saves.insert(
            1,
            crate::ui::recovery::PendingSave::new("uncertain.pdf".into(), "Pending".into()),
        );
        app.open_sanitization();
        assert!(app.sanitize_dialog.is_none());
    }
    #[test]
    fn accepted_copy_retains_source_and_displays_targeted_report_and_limitations() {
        let mut app = app();
        let original_revision = app.tabs[0].status.revision;
        app.open_sanitization();
        app.sanitize_dialog.as_mut().unwrap().expected_id = Some(2);
        let source = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/attachment.pdf"),
            None,
        )
        .unwrap();
        let (clean, report) = source.sanitized_copy().unwrap();
        app.accept_sanitized(
            2,
            "targeted.pdf".into(),
            DocumentStatus {
                info: clean.info().unwrap(),
                revision: 0,
                dirty: true,
                can_undo: false,
                can_redo: false,
            },
            report,
            &egui::Context::default(),
        );
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[0].status.revision, original_revision);
        assert!(app.tabs[0].status.dirty);
        assert_eq!(app.active, Some(2));
        assert!(app.tabs[1].status.dirty);
        assert!(app.sanitize_dialog.as_ref().unwrap().report.is_some());
        assert!(app.message.contains("private data may remain"));
    }

    #[test]
    fn failed_or_stale_completion_keeps_the_original_and_allows_dialog_cancellation() {
        let mut app = app();
        app.open_sanitization();
        app.sanitize_dialog.as_mut().unwrap().expected_id = Some(2);
        app.busy = true;
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        sender
            .send(crate::worker::Event::Error {
                id: 1,
                task: None,
                message: "Linked attachment refused".into(),
            })
            .unwrap();
        app.poll(&egui::Context::default());
        assert!(!app.busy);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, Some(1));
        assert!(app.sanitize_dialog.as_ref().unwrap().expected_id.is_none());
        assert!(app.sanitize_dialog.as_ref().unwrap().error.is_some());
        app.sanitize_dialog.as_mut().unwrap().expected_id = Some(2);
        app.tabs[0].status.revision += 1;
        let source = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/attachment.pdf"),
            None,
        )
        .unwrap();
        let (copy, report) = source.sanitized_copy().unwrap();
        app.accept_sanitized(
            2,
            "ignored.pdf".into(),
            DocumentStatus {
                info: copy.info().unwrap(),
                revision: 0,
                dirty: true,
                can_undo: false,
                can_redo: false,
            },
            report,
            &egui::Context::default(),
        );
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, Some(1));
        assert!(app.sanitize_dialog.as_ref().unwrap().expected_id.is_none());
        assert!(app.sanitize_dialog.as_ref().unwrap().report.is_none());
    }
}
