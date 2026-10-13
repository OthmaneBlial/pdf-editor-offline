use super::EditorApp;
use crate::state::FormsReadKey;
use crate::worker::{Command, Edit};
use eframe::egui;
use pdf_core::{ButtonChange, ButtonSelection, FormField, FormFieldKind, FormValue};

pub(super) struct Dialog {
    pub key: FormsReadKey,
    pub error: Option<String>,
    pub submitting: bool,
    pub stale: bool,
    fields: Option<Vec<FormField>>,
    original: Vec<ButtonSelection>,
    drafts: Vec<ButtonSelection>,
    selected: usize,
}

fn selection(field: &FormField) -> ButtonSelection {
    field
        .widgets
        .iter()
        .find(|widget| widget.on_state.is_some() && widget.appearance_state == widget.on_state)
        .map_or(ButtonSelection::Off, |widget| ButtonSelection::On {
            widget: widget.object_id,
        })
}

fn preview(text: &str) -> String {
    let mut value: String = text.chars().take(180).collect();
    if text.chars().nth(180).is_some() {
        value.push('…');
    }
    value
}

fn kind_label(kind: &FormFieldKind) -> &'static str {
    match kind {
        FormFieldKind::CheckBox => "Checkbox",
        FormFieldKind::RadioButton => "Radio button",
        FormFieldKind::Text => "Text · read only",
        FormFieldKind::Choice => "Choice · read only",
        FormFieldKind::PushButton => "Push button · read only",
        FormFieldKind::Signature => "Signature · read only",
        FormFieldKind::Other => "Other · read only",
    }
}

fn widget_label(widget: &pdf_core::FormWidget, index: usize) -> String {
    let state = widget
        .on_state
        .as_ref()
        .map(|bytes| {
            std::str::from_utf8(bytes).map(preview).unwrap_or_else(|_| {
                format!("Name bytes prefix {:02x?}…", &bytes[..bytes.len().min(12)])
            })
        })
        .unwrap_or_else(|| "Unavailable".into());
    format!("Option {} · {} · page {}", index + 1, state, widget.page)
}

fn value_text(value: &FormValue) -> String {
    match value {
        FormValue::Text(value) => value.clone(),
        FormValue::TextArray(values) => values.join("\n"),
        FormValue::Name(bytes) => format!("PDF name bytes: {:02x?}", bytes),
        FormValue::Empty => "Empty".into(),
        FormValue::Unsupported => "Unsupported value representation".into(),
        FormValue::PasswordOmitted => "Password value omitted".into(),
    }
}

impl Dialog {
    fn loading(key: FormsReadKey) -> Self {
        Self {
            key,
            error: None,
            submitting: false,
            stale: false,
            fields: None,
            original: vec![],
            drafts: vec![],
            selected: 0,
        }
    }
    fn loaded(&mut self, fields: Vec<FormField>) {
        self.original = fields.iter().map(selection).collect();
        self.drafts = self.original.clone();
        self.selected = fields
            .iter()
            .position(|field| field.writable_button)
            .unwrap_or(0);
        self.fields = Some(fields);
        self.error = None;
    }
    pub fn has_changes(&self) -> bool {
        self.original != self.drafts
    }
    fn changes(&self) -> Vec<ButtonChange> {
        self.fields
            .as_ref()
            .map(|fields| {
                fields
                    .iter()
                    .zip(self.original.iter().zip(&self.drafts))
                    .filter(|(_, (original, draft))| original != draft)
                    .map(|(field, (_, draft))| ButtonChange {
                        name: field.name.clone(),
                        selection: draft.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl EditorApp {
    pub(super) fn open_forms(&mut self) {
        if self.busy || self.forms_dialog.is_some() || self.metadata_dialog.is_some() {
            return;
        }
        let Some(tab) = self.selected() else {
            return;
        };
        let Some(generation) = self.forms_generation.checked_add(1) else {
            self.message = "Form request counter exhausted.".into();
            return;
        };
        let key = FormsReadKey {
            tab: tab.id,
            revision: tab.status.revision,
            generation,
        };
        self.forms_generation = generation;
        if self.send(Command::Forms(key), "Reading PDF form fields…") {
            self.forms_dialog = Some(Dialog::loading(key));
        }
    }
    pub(super) fn accept_forms(&mut self, key: FormsReadKey, fields: Vec<FormField>) {
        if self.active == Some(key.tab)
            && self
                .tabs
                .iter()
                .any(|tab| tab.id == key.tab && tab.status.revision == key.revision)
            && let Some(dialog) = self
                .forms_dialog
                .as_mut()
                .filter(|dialog| dialog.key == key && !dialog.stale)
        {
            dialog.loaded(fields);
            self.message =
                "Form fields loaded. Only eligible checkboxes and radio buttons can be changed."
                    .into();
        }
    }
    fn apply_forms(&mut self) -> bool {
        let Some(dialog) = self.forms_dialog.as_ref() else {
            return false;
        };
        let Some(tab) = self.selected().filter(|tab| {
            tab.id == dialog.key.tab && tab.status.revision == dialog.key.revision && tab.editable()
        }) else {
            return false;
        };
        if self.busy
            || !self.pending_saves.is_empty()
            || dialog.submitting
            || dialog.stale
            || !dialog.has_changes()
        {
            return false;
        }
        let edit = Edit::FillButtons {
            key: dialog.key,
            changes: dialog.changes(),
            page: tab.page as u32 + 1,
        };
        if !self.send(
            Command::Edit { id: tab.id, edit },
            "Applying checkbox and radio changes…",
        ) {
            return false;
        }
        if let Some(dialog) = self.forms_dialog.as_mut() {
            dialog.submitting = true;
            dialog.error = None;
        }
        true
    }
    pub(super) fn forms_dialog_ui(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        let Some(dialog) = self.forms_dialog.as_mut() else {
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
        let editable = !self.busy
            && !dialog.submitting
            && !dialog.stale
            && self.pending_saves.is_empty()
            && tab.is_some_and(crate::state::Tab::editable);
        let mut open = true;
        let mut apply = false;
        let mut close = false;
        let mut window = egui::Window::new("PDF form fields")
            .collapsible(false)
            .resizable(true)
            .default_width(540.0)
            .max_width((context.content_rect().width() - 32.0).max(280.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        if !dialog.submitting {
            window = window.open(&mut open);
        }
        window.show(context, |ui| {
            ui.label("Checkboxes and radio buttons use existing PDF appearances. Text, choice and other fields are read only.");
            if dialog.stale { ui.colored_label(ui.visuals().error_fg_color, "This draft is stale. Close and read the form again before applying."); }
            if let Some(error) = &dialog.error { ui.colored_label(ui.visuals().error_fg_color, preview(error)); }
            if !self.pending_saves.is_empty() { ui.label("Resolve the recorded save attempt before applying form changes."); }
            egui::ScrollArea::vertical().id_salt("forms-body").max_height((context.content_rect().height()-240.0).clamp(90.0,420.0)).show(ui, |ui| {
                let Some(fields) = &dialog.fields else { if dialog.error.is_none() { ui.spinner(); ui.label("Reading form fields…"); } return; };
                if fields.is_empty() { ui.label("No AcroForm fields were discovered."); return; }
                ui.label(format!("{} field(s)", fields.len()));
                egui::ScrollArea::vertical().id_salt("form-field-list").max_height(120.0).show_rows(ui, 26.0, fields.len(), |ui, range| {
                    for index in range {
                        let field = &fields[index];
                        if ui.selectable_label(dialog.selected == index, format!("{} · {}", preview(&field.name), kind_label(&field.kind))).clicked() { dialog.selected = index; }
                    }
                });
                let field = &fields[dialog.selected];
                ui.separator();
                ui.label(egui::RichText::new(preview(&field.name)).strong());
                if let Some(reason) = &field.refusal_reason { ui.label(format!("Read only: {}", preview(reason))); }
                if !field.writable_button {
                    let value = value_text(&field.value);
                    ui.label(preview(&value));
                    if !matches!(field.value, FormValue::PasswordOmitted) && ui.button("Copy read-only value").clicked() { context.copy_text(value); }
                    return;
                }
                let unique = fields.iter().filter(|other| other.name == field.name).take(2).count() == 1;
                if !unique { ui.label("Read only: the field name is ambiguous."); }
                let enabled = editable && unique && field.flags & 1 == 0;
                let clear = !(field.flags & 2 != 0 || field.kind == FormFieldKind::RadioButton && field.flags & (1 << 14) != 0);
                let draft = &mut dialog.drafts[dialog.selected];
                if ui.add_enabled(enabled && clear, egui::RadioButton::new(*draft == ButtonSelection::Off, "Off / clear button field")).clicked() { *draft = ButtonSelection::Off; }
                if !clear { ui.small("Required or NoToggleToOff: clearing this button is refused."); }
                egui::ScrollArea::vertical().id_salt("form-widget-list").max_height(140.0).show_rows(ui, 28.0, field.widgets.len(), |ui, range| {
                    for index in range {
                        let widget = &field.widgets[index];
                        let selected = match draft {
                            ButtonSelection::Off => false,
                            ButtonSelection::On { widget: selected } => {
                                if field.kind == FormFieldKind::RadioButton && field.flags & (1 << 25) == 0 { *selected == widget.object_id }
                                else { field.widgets.iter().find(|entry| entry.object_id == *selected).is_some_and(|entry| entry.on_state == widget.on_state) }
                            }
                        };
                        let label = widget_label(widget, index);
                        if field.kind == FormFieldKind::CheckBox {
                            let mut on = selected;
                            let response = ui.add_enabled(enabled && (!selected || clear), egui::Checkbox::new(&mut on, label));
                            if response.clicked() { response.request_focus(); }
                            if response.changed() {
                                *draft = if on { ButtonSelection::On { widget: widget.object_id } } else { ButtonSelection::Off };
                            }
                        } else if ui.add_enabled(enabled, egui::RadioButton::new(selected, label)).clicked() { *draft = ButtonSelection::On { widget: widget.object_id }; }
                    }
                });
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                apply = ui.add_enabled(editable && dialog.has_changes(), crate::style::primary("Apply button changes", ui.visuals().dark_mode)).clicked();
                close = ui.add_enabled(!dialog.submitting, egui::Button::new("Close form fields")).clicked();
                if dialog.submitting { ui.spinner(); ui.label("Working…"); }
            });
        });
        if apply {
            self.apply_forms();
        } else if !dialog.submitting
            && (close || !open || context.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.forms_dialog = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DocumentStatus, Tab};

    fn fields() -> Vec<FormField> {
        let widget = |id| pdf_core::FormWidget {
            object_id: (id, 0),
            page: 1,
            rectangle: [10.0, 10.0, 30.0, 30.0],
            appearance_state: Some(b"Off".to_vec()),
            on_state: Some(b"Yes".to_vec()),
        };
        vec![
            FormField {
                object_id: (5, 0),
                name: "Consent".into(),
                kind: FormFieldKind::CheckBox,
                flags: 0,
                value: FormValue::Name(b"Off".to_vec()),
                widgets: vec![widget(5)],
                writable_button: true,
                refusal_reason: None,
            },
            FormField {
                object_id: (9, 0),
                name: "Delivery".into(),
                kind: FormFieldKind::RadioButton,
                flags: 1 << 15,
                value: FormValue::Name(b"Off".to_vec()),
                widgets: vec![widget(10), widget(11)],
                writable_button: true,
                refusal_reason: None,
            },
            FormField {
                object_id: (20, 0),
                name: "Name".into(),
                kind: FormFieldKind::Text,
                flags: 0,
                value: FormValue::Text("Existing text remains read only".into()),
                widgets: vec![widget(20)],
                writable_button: false,
                refusal_reason: Some("Text appearance generation is not implemented".into()),
            },
        ]
    }
    fn app() -> EditorApp {
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(
            egui::Context::default(),
        ));
        let info = pdf_core::DocumentInfo {
            version: "1.7".into(),
            page_count: 1,
            object_count: 25,
            encrypted: false,
            signature_fields: 0,
            pages: vec![],
        };
        let mut tab = Tab::new(
            1,
            "form.pdf".into(),
            DocumentStatus {
                info,
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        );
        tab.needs_render = false;
        app.tabs.push(tab);
        app.active = Some(1);
        app.forms_dialog = Some(Dialog::loading(FormsReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        }));
        app.accept_forms(app.forms_dialog.as_ref().expect("dialog").key, fields());
        app
    }
    fn draw(
        app: &mut EditorApp,
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.forms_dialog_ui(context),
        );
        output.textures_delta.clear();
        output
    }
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) {
        let _ = draw(app, context, vec![]);
        let output = draw(app, context, vec![]);
        let position = output
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
        let _ = draw(
            app,
            context,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let _ = draw(
            app,
            context,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn actual_checkbox_and_radio_controls_build_precise_changes_and_guard_exit() {
        let mut app = app();
        let context = egui::Context::default();
        click(&mut app, &context, "Option 1 · Yes · page 1");
        assert!(app.needs_exit_guard());
        app.forms_dialog.as_mut().expect("dialog").selected = 1;
        click(&mut app, &context, "Option 2 · Yes · page 1");
        let changes = app.forms_dialog.as_ref().expect("dialog").changes();
        assert_eq!(
            changes,
            vec![
                ButtonChange {
                    name: "Consent".into(),
                    selection: ButtonSelection::On { widget: (5, 0) }
                },
                ButtonChange {
                    name: "Delivery".into(),
                    selection: ButtonSelection::On { widget: (11, 0) }
                }
            ]
        );
        click(&mut app, &context, "Apply button changes");
        assert!(app.busy && app.forms_dialog.as_ref().expect("dialog").submitting);
    }
    #[test]
    fn stale_cross_tab_protected_and_pending_save_drafts_cannot_dispatch() {
        let mut app = app();
        let key = app.forms_dialog.as_ref().expect("dialog").key;
        app.accept_forms(
            FormsReadKey {
                generation: 2,
                ..key
            },
            vec![],
        );
        assert_eq!(
            app.forms_dialog
                .as_ref()
                .expect("dialog")
                .fields
                .as_ref()
                .expect("fields")
                .len(),
            3
        );
        app.forms_dialog.as_mut().expect("dialog").drafts[0] =
            ButtonSelection::On { widget: (5, 0) };
        app.tabs[0].status.revision = 1;
        assert!(!app.apply_forms());
        app.tabs[0].status.revision = 0;
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_forms());
        app.tabs[0].status.info.signature_fields = 0;
        app.pending_saves.insert(
            1,
            super::super::recovery::PendingSave::new("pending.pdf".into(), "Unresolved".into()),
        );
        assert!(!app.apply_forms());
        assert!(
            app.forms_dialog
                .as_ref()
                .expect("retained draft")
                .has_changes()
        );
        assert!(!app.busy);
    }
    #[test]
    fn read_only_value_is_truthful_and_form_errors_retain_draft_without_canvas_error() {
        let mut app = app();
        app.forms_dialog.as_mut().expect("dialog").selected = 2;
        let context = egui::Context::default();
        let _ = draw(&mut app, &context, vec![]);
        let output = draw(&mut app, &context, vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text().contains("Existing text remains read only"))));
        app.forms_dialog.as_mut().expect("dialog").drafts[0] =
            ButtonSelection::On { widget: (5, 0) };
        let key = app.forms_dialog.as_ref().expect("dialog").key;
        app.forms_dialog.as_mut().expect("dialog").submitting = true;
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        sender
            .send(crate::worker::Event::Error {
                id: 1,
                task: Some(crate::worker::TaskKey::Forms(key)),
                message: "Button appearance refused".into(),
            })
            .expect("event");
        app.close_target = Some(super::super::CloseTarget::Exit);
        app.poll(&egui::Context::default());
        let dialog = app.forms_dialog.as_ref().expect("draft retained");
        assert!(dialog.has_changes() && dialog.error.is_some() && !dialog.submitting);
        assert!(app.tabs[0].error.is_none());
    }

    #[test]
    fn checkbox_keyboard_space_toggles_the_focused_labeled_control() {
        let mut app = app();
        let context = egui::Context::default();
        click(&mut app, &context, "Option 1 · Yes · page 1");
        assert!(app.forms_dialog.as_ref().expect("dialog").has_changes());
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            !app.forms_dialog
                .as_ref()
                .expect("keyboard-cleared draft")
                .has_changes()
        );
    }

    #[test]
    fn signed_source_disables_real_button_controls_even_if_reply_claims_writable() {
        let mut app = app();
        app.tabs[0].status.info.signature_fields = 1;
        click(
            &mut app,
            &egui::Context::default(),
            "Option 1 · Yes · page 1",
        );
        assert!(!app.forms_dialog.as_ref().expect("read only").has_changes());
        assert!(!app.apply_forms());
    }
}
