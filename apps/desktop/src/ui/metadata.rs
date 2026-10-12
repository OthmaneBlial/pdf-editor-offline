use super::EditorApp;
use crate::state::MetadataReadKey;
use crate::worker::{Command, Edit};
use eframe::egui;
use pdf_core::{DocumentMetadata, MAX_METADATA_VALUE_BYTES, MetadataKey};

const FIELDS: [(MetadataKey, &str); 6] = [
    (MetadataKey::Title, "Title"),
    (MetadataKey::Author, "Author"),
    (MetadataKey::Subject, "Subject"),
    (MetadataKey::Keywords, "Keywords"),
    (MetadataKey::Creator, "Creator"),
    (MetadataKey::Producer, "Producer"),
];

pub(super) struct Dialog {
    pub key: MetadataReadKey,
    pub error: Option<String>,
    pub submitting: bool,
    pub stale: bool,
    source: Option<DocumentMetadata>,
    drafts: [String; 6],
}

impl Dialog {
    fn loading(key: MetadataReadKey) -> Self {
        Self {
            key,
            error: None,
            submitting: false,
            stale: false,
            source: None,
            drafts: Default::default(),
        }
    }

    pub fn loaded(&mut self, values: DocumentMetadata) {
        self.drafts = std::array::from_fn(|index| {
            values
                .fields
                .get(&FIELDS[index].0)
                .cloned()
                .unwrap_or_default()
        });
        self.source = Some(values);
        self.error = None;
        self.submitting = false;
    }

    fn changes(&self) -> Vec<(MetadataKey, Option<String>)> {
        let Some(source) = &self.source else {
            return vec![];
        };
        FIELDS
            .iter()
            .zip(&self.drafts)
            .filter_map(|((key, _), draft)| {
                let old = source.fields.get(key);
                if old.map_or("", String::as_str) == draft {
                    None
                } else {
                    Some((*key, (!draft.is_empty()).then(|| draft.clone())))
                }
            })
            .collect()
    }

    pub(super) fn has_changes(&self) -> bool {
        self.source.as_ref().is_some_and(|source| {
            FIELDS
                .iter()
                .zip(&self.drafts)
                .any(|((key, _), draft)| source.fields.get(key).map_or("", String::as_str) != draft)
        })
    }

    fn valid(&self) -> bool {
        self.source.is_some()
            && self
                .drafts
                .iter()
                .all(|value| value.len() <= MAX_METADATA_VALUE_BYTES && !value.contains('\0'))
    }
}

impl EditorApp {
    fn apply_metadata(&mut self) -> bool {
        let Some(dialog) = self.metadata_dialog.as_ref() else {
            return false;
        };
        if self.busy
            || dialog.stale
            || !dialog.valid()
            || !dialog.has_changes()
            || !self.tabs.iter().any(|tab| {
                Some(tab.id) == self.active
                    && tab.id == dialog.key.tab
                    && tab.status.revision == dialog.key.revision
                    && tab.editable()
            })
        {
            return false;
        }
        let id = dialog.key.tab;
        let edit = Edit::Metadata {
            key: dialog.key,
            changes: dialog.changes(),
        };
        if !self.send(Command::Edit { id, edit }, "Updating document properties…") {
            return false;
        }
        if let Some(dialog) = self.metadata_dialog.as_mut() {
            dialog.submitting = true;
            dialog.error = None;
        }
        true
    }
    pub(super) fn accept_metadata(&mut self, key: MetadataReadKey, values: DocumentMetadata) {
        if self.active == Some(key.tab)
            && self
                .tabs
                .iter()
                .any(|tab| tab.id == key.tab && tab.status.revision == key.revision)
            && let Some(dialog) = self
                .metadata_dialog
                .as_mut()
                .filter(|dialog| dialog.key == key && !dialog.stale)
        {
            dialog.loaded(values);
            self.message = "Document properties loaded locally.".into();
        }
    }
    pub(super) fn open_metadata(&mut self) {
        if self.busy || self.metadata_dialog.is_some() {
            return;
        }
        let Some(tab) = self.selected() else {
            return;
        };
        let tab_id = tab.id;
        let revision = tab.status.revision;
        self.metadata_generation += 1;
        let key = MetadataReadKey {
            tab: tab_id,
            revision,
            generation: self.metadata_generation,
        };
        if self.send(Command::Metadata(key), "Reading document properties…") {
            self.metadata_dialog = Some(Dialog::loading(key));
        }
    }

    pub(super) fn metadata_dialog_ui(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let Some(dialog) = self.metadata_dialog.as_mut() else {
            return;
        };
        let tab = self.tabs.iter().find(|tab| {
            Some(tab.id) == self.active
                && tab.id == dialog.key.tab
                && tab.status.revision == dialog.key.revision
        });
        if tab.is_none() {
            dialog.stale = true;
        }
        let editable = !dialog.stale && tab.is_some_and(crate::state::Tab::editable);
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let window = egui::Window::new("Document properties")
            .collapsible(false)
            .resizable(true)
            .vscroll(true)
            .default_height((context.content_rect().height() - 90.0).clamp(200.0, 600.0))
            .max_height((context.content_rect().height() - 60.0).max(200.0));
        let window = if dialog.submitting {
            window
        } else {
            window.open(&mut open)
        };
        window
            .anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).default_width(440.0)
            .show(context,|ui| {
                if let Some(error)=&dialog.error { ui.colored_label(ui.visuals().error_fg_color,error); }
                if dialog.stale { ui.label("The document changed. Copy any values you need, then close and reopen Document properties before applying changes."); }
                let Some(source)=&dialog.source else { if dialog.error.is_none() && !dialog.stale { ui.spinner(); ui.label("Reading document properties…"); } cancel=ui.button("Close").clicked(); return; };
                if !editable && !dialog.stale { ui.label("Read only: this document is signed or encrypted."); }
                ui.small("Standard document fields · each field allows up to 64 KiB of UTF-8 text.");
                egui::ScrollArea::vertical().id_salt("metadata-fields")
                    // Reserve the title, instructions and action footer even on short windows.
                    .max_height((context.content_rect().height()-470.0).clamp(70.0,280.0)).show(ui,|ui| {
                    for (index,(_,name)) in FIELDS.iter().enumerate() {
                        let label=ui.label(egui::RichText::new(*name).font(crate::style::medium(13.0)));
                        let response=if editable && !dialog.submitting {
                            ui.add(egui::TextEdit::multiline(&mut dialog.drafts[index]).char_limit(MAX_METADATA_VALUE_BYTES)
                                .desired_rows(2).desired_width(f32::INFINITY).id_salt(("metadata-field",dialog.key.generation,index)))
                        } else {
                            let mut value=dialog.drafts[index].as_str();
                            ui.add(egui::TextEdit::multiline(&mut value).desired_rows(2).desired_width(f32::INFINITY)
                                .id_salt(("metadata-field",dialog.key.generation,index)))
                        };
                        response.labelled_by(label.id);
                        ui.add_space(5.0);
                    }
                });
                ui.small(if source.xmp_present { "Embedded XMP stays unchanged and may contain different values." }
                    else { "These fields are separate from XMP metadata." });
                ui.small("Dates and custom fields are preserved. Clearing a changed field removes its active entry; earlier copies may remain in the file.");
                if source.catalog_info_present { ui.small("Additional catalog metadata is preserved separately."); }
                if !dialog.valid() { ui.colored_label(ui.visuals().error_fg_color,"Each field must fit within 64 KiB of UTF-8 and contain no NUL characters."); }
                ui.horizontal(|ui| {
                    if editable { apply=ui.add_enabled(dialog.valid() && dialog.has_changes() && !self.busy,
                        crate::style::primary("Apply changes",ui.visuals().dark_mode)).clicked(); }
                    cancel=ui.add_enabled(!dialog.submitting,egui::Button::new(if editable { "Cancel" } else { "Close" })).clicked();
                });
                if dialog.submitting { ui.spinner(); ui.label("Updating document properties…"); }
            });
        if apply {
            self.apply_metadata();
        } else if !dialog.submitting
            && (cancel || !open || context.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.metadata_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn app() -> EditorApp {
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
                    pages: vec![],
                },
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        ));
        app.active = Some(1);
        app.metadata_dialog = Some(Dialog::loading(MetadataReadKey {
            tab: 1,
            revision: 0,
            generation: 2,
        }));
        app
    }

    fn values() -> DocumentMetadata {
        DocumentMetadata {
            fields: BTreeMap::from([(MetadataKey::Title, "Original".into())]),
            info_present: true,
            xmp_present: false,
            catalog_info_present: false,
            xmp_synchronized: false,
            secure_erasure: false,
        }
    }

    #[test]
    fn stale_metadata_results_and_drafts_cannot_overwrite_another_request_tab_or_revision() {
        let mut app = app();
        let key = app.metadata_dialog.as_ref().expect("dialog").key;
        app.accept_metadata(
            MetadataReadKey {
                generation: 1,
                ..key
            },
            values(),
        );
        assert!(
            app.metadata_dialog
                .as_ref()
                .expect("dialog")
                .source
                .is_none()
        );
        app.accept_metadata(MetadataReadKey { revision: 1, ..key }, values());
        assert!(
            app.metadata_dialog
                .as_ref()
                .expect("dialog")
                .source
                .is_none()
        );
        app.active = None;
        app.accept_metadata(key, values());
        assert!(
            app.metadata_dialog
                .as_ref()
                .expect("dialog")
                .source
                .is_none()
        );
        app.active = Some(1);
        app.accept_metadata(key, values());
        app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "Changed".into();
        app.tabs[0].status.revision = 1;
        assert!(!app.apply_metadata());
        draw(&mut app, 640.0, 420.0);
        let dialog = app.metadata_dialog.as_ref().expect("stale draft retained");
        assert!(dialog.stale);
        assert_eq!(dialog.drafts[0], "Changed");
        assert!(app.needs_exit_guard());
        app.tabs[0].status.revision = 0; // Undo cannot make this stale draft editable again.
        app.accept_metadata(key, values());
        assert_eq!(
            app.metadata_dialog.as_ref().expect("retained").drafts[0],
            "Changed"
        );
        assert!(!app.apply_metadata());
    }

    fn draw(app: &mut EditorApp, width: f32, height: f32) {
        let context = egui::Context::default();
        crate::fonts::configure(&context);
        crate::style::configure(&context, false);
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                ..Default::default()
            },
            |_| app.metadata_dialog_ui(&context),
        );
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }

    #[test]
    fn actual_ui_pass_retains_stale_tab_drafts_at_small_and_large_viewports() {
        let mut app = app();
        let key = app.metadata_dialog.as_ref().expect("dialog").key;
        app.accept_metadata(key, values());
        app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "Unapplied résumé 東京".into();
        app.active = None;
        draw(&mut app, 640.0, 420.0);
        assert_eq!(
            app.metadata_dialog.as_ref().expect("copyable draft").drafts[0],
            "Unapplied résumé 東京"
        );
        assert!(app.metadata_dialog.as_ref().expect("dialog").stale);
        assert!(app.needs_exit_guard());
        assert!(!app.apply_metadata());
        app.active = Some(1);
        draw(&mut app, 1260.0, 860.0);
        assert!(!app.apply_metadata());
        app.open_metadata(); // A new read must not silently replace this retained draft.
        assert_eq!(
            app.metadata_dialog
                .as_ref()
                .expect("retained until explicit close")
                .drafts[0],
            "Unapplied résumé 東京"
        );
        assert!(!app.busy);
    }

    #[test]
    fn metadata_drafts_guard_quit_and_read_only_busy_or_invalid_apply_is_blocked() {
        let mut app = app();
        let key = app.metadata_dialog.as_ref().expect("dialog").key;
        app.accept_metadata(key, values());
        assert!(!app.needs_exit_guard());
        app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "Changed".into();
        assert!(app.needs_exit_guard());
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_metadata());
        app.tabs[0].status.info.signature_fields = 0;
        app.tabs[0].status.info.encrypted = true;
        assert!(!app.apply_metadata());
        app.tabs[0].status.info.encrypted = false;
        app.busy = true;
        assert!(!app.apply_metadata());
        app.busy = false;
        app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "NUL\0".into();
        assert!(!app.apply_metadata());
        assert!(
            app.metadata_dialog
                .as_ref()
                .expect("draft retained")
                .has_changes()
        );
        app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "Changed".into();
        assert!(app.apply_metadata());
        assert!(
            app.metadata_dialog
                .as_ref()
                .expect("draft awaiting result")
                .submitting
        );
        // The UI-only document is absent from the worker: rejection must preserve the draft for correction.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll(&egui::Context::default());
            std::thread::yield_now();
        }
        let dialog = app
            .metadata_dialog
            .as_ref()
            .expect("draft retained on failure");
        assert_eq!(dialog.drafts[0], "Changed");
        assert!(dialog.error.is_some() && !dialog.submitting);
        assert!(!app.tabs[0].status.dirty);
    }

    #[test]
    fn property_actions_are_painted_inside_the_visible_dialog_without_scrolling() {
        for size in [egui::vec2(640.0, 420.0), egui::vec2(1260.0, 718.0)] {
            let mut app = app();
            let key = app.metadata_dialog.as_ref().expect("dialog").key;
            app.accept_metadata(key, values());
            app.metadata_dialog.as_mut().expect("dialog").drafts[0] = "Changed".into();
            let context = egui::Context::default();
            crate::fonts::configure(&context);
            crate::style::configure(&context, false);
            for frame in 0..4 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        time: Some(f64::from(frame) * 0.1),
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |_| app.metadata_dialog_ui(&context),
                );
                output.textures_delta.clear();
                if frame < 3 {
                    continue;
                }
                for label in ["Apply changes", "Cancel"] {
                    let painted = output.shapes.iter().any(|shape| {
                        if let egui::epaint::Shape::Text(text) = &shape.shape {
                            text.galley.job.text == label
                                && shape
                                    .clip_rect
                                    .contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
                        } else {
                            false
                        }
                    });
                    assert!(
                        painted,
                        "{label} is clipped at {size:?}: {:?}",
                        output
                            .shapes
                            .iter()
                            .filter_map(|shape| {
                                if let egui::epaint::Shape::Text(text) = &shape.shape {
                                    (text.galley.job.text == label).then_some((
                                        shape.clip_rect,
                                        text.pos,
                                        text.galley.rect,
                                    ))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }

    #[test]
    fn drafts_change_only_modified_fields_and_validate_utf8_bytes() {
        let mut dialog = Dialog::loading(MetadataReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        });
        dialog.loaded(DocumentMetadata {
            fields: BTreeMap::from([
                (MetadataKey::Title, "Résumé 東京".into()),
                (MetadataKey::Author, "Ada".into()),
                (MetadataKey::Subject, String::new()),
            ]),
            info_present: true,
            xmp_present: true,
            catalog_info_present: true,
            xmp_synchronized: false,
            secure_erasure: false,
        });
        assert!(dialog.valid() && dialog.changes().is_empty());
        dialog.drafts[0] = "Résumé 東京 ✓".into();
        dialog.drafts[1].clear();
        assert_eq!(
            dialog.changes(),
            vec![
                (MetadataKey::Title, Some("Résumé 東京 ✓".into())),
                (MetadataKey::Author, None)
            ]
        );
        dialog.drafts[0] = "é".repeat(MAX_METADATA_VALUE_BYTES / 2 + 1);
        assert!(!dialog.valid());
        dialog.drafts[0] = "NUL\0".into();
        assert!(!dialog.valid());
    }
}
