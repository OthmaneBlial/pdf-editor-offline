use super::EditorApp;
use crate::state::FormsReadKey;
use crate::worker::{Command, Edit};
use eframe::egui;
use pdf_core::{
    ButtonChange, ButtonSelection, ChoiceChange, ChoiceField, FormField, FormFieldKind, FormValue,
    TextChange,
};

pub(super) struct Dialog {
    pub key: FormsReadKey,
    pub error: Option<String>,
    pub submitting: bool,
    pub stale: bool,
    fields: Option<Vec<FormField>>,
    original: Vec<ButtonSelection>,
    drafts: Vec<ButtonSelection>,
    text_original: Vec<String>,
    text_drafts: Vec<String>,
    choices: Vec<ChoiceField>,
    choice_original: Vec<Vec<u32>>,
    choice_drafts: Vec<Vec<u32>>,
    top_drafts: Vec<Option<u32>>,
    font_file: Option<std::path::PathBuf>,
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
        FormFieldKind::Text => "Text",
        FormFieldKind::Choice => "Dropdown / list",
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
            text_original: vec![],
            text_drafts: vec![],
            choices: vec![],
            choice_original: vec![],
            choice_drafts: vec![],
            top_drafts: vec![],
            font_file: None,
            selected: 0,
        }
    }
    fn loaded(&mut self, fields: Vec<FormField>, choices: Vec<ChoiceField>) {
        self.choice_original = choices
            .iter()
            .map(|choice| choice.selected_indices.clone().unwrap_or_default())
            .collect();
        self.choice_drafts = self.choice_original.clone();
        self.top_drafts = vec![None; choices.len()];
        self.choices = choices;
        self.original = fields.iter().map(selection).collect();
        self.drafts = self.original.clone();
        self.text_original = fields
            .iter()
            .map(|field| match &field.value {
                FormValue::Text(value) => value.clone(),
                _ => String::new(),
            })
            .collect();
        self.text_drafts = self.text_original.clone();
        self.selected = fields
            .iter()
            .position(|field| {
                field.writable_button
                    || field.writable_text
                    || self
                        .choices
                        .iter()
                        .any(|choice| choice.field.object_id == field.object_id && choice.writable)
            })
            .unwrap_or(0);
        self.fields = Some(fields);
        self.error = None;
    }
    pub fn has_changes(&self) -> bool {
        self.original != self.drafts
            || self.text_original != self.text_drafts
            || self.choice_original != self.choice_drafts
            || self.top_drafts.iter().any(Option::is_some)
    }
    fn choice_changes(&self) -> Vec<ChoiceChange> {
        self.choices
            .iter()
            .enumerate()
            .filter(|(index, choice)| {
                choice.writable
                    && (self.choice_original[*index] != self.choice_drafts[*index]
                        || self.top_drafts[*index].is_some())
            })
            .map(|(index, choice)| ChoiceChange {
                name: choice.field.name.clone(),
                indices: self.choice_drafts[index].clone(),
                top_index: self.top_drafts[index],
            })
            .collect()
    }
    fn text_changes(&self) -> Vec<TextChange> {
        self.fields
            .as_ref()
            .map(|fields| {
                fields
                    .iter()
                    .zip(self.text_original.iter().zip(&self.text_drafts))
                    .filter(|(field, (original, draft))| field.writable_text && original != draft)
                    .map(|(field, (_, draft))| TextChange {
                        name: field.name.clone(),
                        value: draft.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
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
    pub(super) fn accept_forms(
        &mut self,
        key: FormsReadKey,
        fields: Vec<FormField>,
        choices: Vec<ChoiceField>,
    ) {
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
            dialog.loaded(fields, choices);
            self.message =
                "Form fields loaded. Eligible text fields require an explicit covering font."
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
        let text = dialog.text_changes();
        if text.iter().any(|change| change.value.len() > 65_536) {
            return false;
        }
        let choices = dialog.choice_changes();
        let edit = if text.is_empty() && choices.is_empty() {
            Edit::FillButtons {
                key: dialog.key,
                changes: dialog.changes(),
                page: tab.page as u32 + 1,
            }
        } else {
            let Some(font_file) = dialog.font_file.clone() else {
                return false;
            };
            Edit::FillText {
                key: dialog.key,
                changes: text,
                buttons: dialog.changes(),
                choices,
                page: tab.page as u32 + 1,
                font_file,
            }
        };
        if !self.send(
            Command::Edit { id: tab.id, edit },
            "Applying PDF form changes…",
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
            ui.label("Buttons use existing appearances. Supported text, dropdown and list fields embed one complete, editable TrueType or static CID-keyed CFF1 font; missing glyphs or unsupported layouts are refused.");
            if dialog.fields.as_ref().is_some_and(|fields|fields.iter().any(|field|field.writable_text)) || dialog.choices.iter().any(|choice|choice.writable) {
                if ui.add_enabled(editable, egui::Button::new("Choose text font…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("OpenType font", &["ttf", "otf"]).pick_file() { dialog.font_file=Some(path); }
                ui.label(dialog.font_file.as_ref().map(|path|path.file_name().unwrap_or_default().to_string_lossy().into_owned()).unwrap_or_else(||"No text font selected".into()));
            }
            if dialog.stale { ui.colored_label(ui.visuals().error_fg_color, "The document changed. Copy your draft, then close and read the form again before applying."); }
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
                if field.kind==FormFieldKind::Choice {
                    if let Some(index)=dialog.choices.iter().position(|choice|choice.field.object_id==field.object_id) {
                        let choice=&dialog.choices[index];
                        if let Some(reason)=&choice.refusal_reason { ui.label(format!("Read only: {}",preview(reason))); }
                        if choice.selected_indices.is_none(){ui.label(format!("Unresolved existing selection: {}",preview(&value_text(&choice.field.value))));}
                        let enabled=editable && choice.writable;
                        let selected=&mut dialog.choice_drafts[index];
                        if choice.combo {
                            let label=selected.first().and_then(|index|choice.options.get(*index as usize)).map(|option|format!("{} · index {}",preview(&option.display),option.index)).unwrap_or_else(||"No selection".into());
                            ui.add_enabled_ui(enabled,|ui|egui::ComboBox::from_id_salt(("form-choice",field.object_id)).selected_text(label).show_ui(ui,|ui|{
                                egui::ScrollArea::vertical().id_salt(("combo-options",field.object_id)).max_height(180.0).show_rows(ui,26.0,choice.options.len(),|ui,range|{
                                    for row in range { let option=&choice.options[row];if ui.selectable_label(selected.as_slice()==[option.index],format!("{} · index {}",preview(&option.display),option.index)).clicked(){*selected=vec![option.index];} }
                                });
                            }));
                        } else {
                            ui.label(if choice.multi_select {"Select one or more list options"} else {"Select a list option"});
                            egui::ScrollArea::vertical().id_salt(("choice-options",field.object_id)).max_height(180.0).show_rows(ui,28.0,choice.options.len(),|ui,range|{
                                for row in range {
                                    let option=&choice.options[row];
                                    let label=format!("{} · index {}",preview(&option.display),option.index);
                                    if choice.multi_select {
                                        let mut on=selected.binary_search(&option.index).is_ok();
                                        let response=ui.add_enabled(enabled,egui::Checkbox::new(&mut on,label));
                                        if response.clicked(){response.request_focus();}
                                        if response.changed(){
                                            if on {selected.push(option.index);selected.sort_unstable();}
                                            else {selected.retain(|index|*index!=option.index);}
                                        }
                                    }
                                    else if ui.add_enabled(enabled,egui::RadioButton::new(selected.as_slice()==[option.index],label)).clicked(){*selected=vec![option.index];}
                                }
                            });
                            let mut explicit=dialog.top_drafts[index].is_some();
                            ui.small(format!("Current PDF top index: {}",choice.top_index));
                            if ui.add_enabled(enabled,egui::Checkbox::new(&mut explicit,"Set first visible list index")).changed(){dialog.top_drafts[index]=explicit.then_some(choice.top_index);}
                            if let Some(top)=&mut dialog.top_drafts[index] { ui.add_enabled(enabled,egui::DragValue::new(top).range(0..=choice.options.len().saturating_sub(1) as u32).prefix("Top index ")); }
                            ui.small("Automatic scrolling reveals a selected row; explicit scroll takes precedence. Other selected indices remain selected.");
                        }
                        if ui.add_enabled(enabled && field.flags&2==0,egui::Button::new("Clear choice selection")).clicked(){selected.clear();}
                        ui.small("Indices are zero-based and distinguish duplicate display/export labels. Apply validates font coverage and fit.");
                    } else { ui.label("Choice details unavailable; this field is read only."); }
                    return;
                }
                if field.kind == FormFieldKind::Text {
                    if let Some(reason)=&field.text_refusal_reason { ui.label(format!("Read only: {}",preview(reason))); }
                    if field.writable_text {
                        let unique=fields.iter().filter(|other|other.name==field.name).take(2).count()==1;
                        let value=&mut dialog.text_drafts[dialog.selected];
                        let multiline=field.flags&(1<<12)!=0;
                        ui.label(if multiline {"Multiline text field value"} else {"Text field value"});
                        let input=if multiline {
                            egui::TextEdit::multiline(value).desired_rows(4).desired_width(ui.available_width())
                        } else {egui::TextEdit::singleline(value)};
                        ui.add_enabled(editable && unique && field.flags&1==0, input.id_salt(("form-text",field.object_id)).hint_text(if multiline {"Enter adds a line break"} else {"Text field value"}));
                        if value.len()>65_536 { ui.colored_label(ui.visuals().error_fg_color,"Value exceeds the 64 KiB UTF-8 limit; Apply will refuse it."); }
                        if ui.button("Copy text draft").clicked(){context.copy_text(value.clone());}
                        ui.small("The selected font must cover the complete value. Apply validates fit and supported layout before any document change.");
                        if multiline {ui.small("Enter adds a line break. Text wraps within the PDF field; overflow is refused.");}
                        return;
                    }
                }
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
                let text_ready = dialog.text_changes().is_empty() && dialog.choice_changes().is_empty() || dialog.font_file.is_some();
                let within_bounds=dialog.text_drafts.iter().all(|value|value.len()<=65_536);
                apply = ui.add_enabled(editable && dialog.has_changes() && text_ready && within_bounds, crate::style::primary("Apply form changes", ui.visuals().dark_mode)).clicked();
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
                writable_text: false,
                text_refusal_reason: Some("Not a text field".into()),
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
                writable_text: false,
                text_refusal_reason: Some("Not a text field".into()),
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
                writable_text: false,
                text_refusal_reason: Some("Unsupported text structure".into()),
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
        app.accept_forms(
            app.forms_dialog.as_ref().expect("dialog").key,
            fields(),
            vec![],
        );
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
    fn click(app: &mut EditorApp, context: &egui::Context, label: &str) -> egui::FullOutput {
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
        draw(
            app,
            context,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }
    #[test]
    fn actual_multiline_widget_keeps_paste_enter_and_stale_copy_without_dispatch() {
        let mut app = app();
        let dialog = app.forms_dialog.as_mut().expect("dialog");
        let field = &mut dialog.fields.as_mut().expect("fields")[2];
        field.flags = 1 << 12;
        field.writable_text = true;
        field.text_refusal_reason = None;
        dialog.selected = 2;
        let context = egui::Context::default();
        click(&mut app, &context, "Existing text remains read only");
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        let value = "Résumé e\u{301}\r\nΩЖ\nLast";
        let _ = draw(&mut app, &context, vec![egui::Event::Paste(value.into())]);
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let expected = format!("{value}\n");
        assert_eq!(
            app.forms_dialog.as_ref().expect("dialog").text_changes()[0].value,
            expected
        );
        assert!(app.needs_exit_guard());
        assert!(!app.busy, "Enter adds a line instead of applying");
        assert!(!app.apply_forms(), "a complete font must be chosen");
        let key = app.forms_dialog.as_ref().expect("dialog").key;
        app.tabs[0].status.revision = 1;
        app.forms_dialog.as_mut().expect("dialog").font_file = Some("chosen.ttf".into());
        let _ = draw(&mut app, &context, vec![]);
        assert!(app.forms_dialog.as_ref().expect("dialog").stale);
        assert!(!app.apply_forms());
        app.accept_forms(FormsReadKey { revision: 1, ..key }, vec![], vec![]);
        assert_eq!(
            app.forms_dialog
                .as_ref()
                .expect("retained stale")
                .text_drafts[2],
            expected
        );
        let output = click(&mut app, &context, "Copy text draft");
        assert!(output.platform_output.commands.iter().any(
            |command| matches!(command,egui::OutputCommand::CopyText(text) if text==&expected)
        ));
        assert!(app.needs_exit_guard());
        assert!(!app.busy);
        assert!(!app.tabs[0].status.dirty);
        app.request_close(1);
        assert_eq!(app.close_target, Some(super::super::CloseTarget::Tab(1)));
        assert!(!app.busy, "closing must retain the unapplied form draft");
    }

    #[test]
    fn oversized_multiline_draft_cannot_dispatch_or_be_truncated() {
        let mut app = app();
        let dialog = app.forms_dialog.as_mut().expect("dialog");
        let field = &mut dialog.fields.as_mut().expect("fields")[2];
        field.flags = 1 << 12;
        field.writable_text = true;
        field.text_refusal_reason = None;
        dialog.text_drafts[2] = "界".repeat(21846);
        dialog.font_file = Some("chosen.otf".into());
        assert!(!app.apply_forms());
        assert!(!app.busy);
        assert_eq!(
            app.forms_dialog.as_ref().expect("draft").text_drafts[2].len(),
            65538
        );
        assert!(app.needs_exit_guard());
    }

    #[test]
    fn actual_textbox_keeps_unicode_draft_and_requires_explicit_font_for_mixed_apply() {
        let mut app = app();
        {
            let dialog = app.forms_dialog.as_mut().expect("dialog");
            let field = &mut dialog.fields.as_mut().expect("fields")[2];
            field.writable_text = true;
            field.text_refusal_reason = None;
            dialog.selected = 2;
        }
        let context = egui::Context::default();
        click(&mut app, &context, "Existing text remains read only");
        let _ = draw(&mut app, &context, vec![egui::Event::Text(" é".into())]);
        let dialog = app.forms_dialog.as_ref().expect("dialog");
        assert_eq!(dialog.text_changes().len(), 1);
        assert!(dialog.text_changes()[0].value.contains('é'));
        assert!(app.needs_exit_guard());
        click(&mut app, &context, "Apply form changes");
        assert!(!app.busy, "missing font must not dispatch");
        let dialog = app.forms_dialog.as_mut().expect("dialog");
        dialog.font_file = Some("chosen-complete-font.ttf".into());
        dialog.drafts[0] = ButtonSelection::On { widget: (5, 0) };
        let key = dialog.key;
        click(&mut app, &context, "Apply form changes");
        assert!(app.busy && app.forms_dialog.as_ref().expect("dialog").submitting);
        let (sender, events) = std::sync::mpsc::channel();
        app.worker.events = events;
        sender
            .send(crate::worker::Event::Error {
                id: 1,
                task: Some(crate::worker::TaskKey::Forms(key)),
                message: "Font does not cover this value".into(),
            })
            .expect("error event");
        app.poll(&context);
        let dialog = app.forms_dialog.as_ref().expect("retained draft");
        assert!(!dialog.submitting && dialog.has_changes());
        assert!(
            dialog
                .error
                .as_ref()
                .expect("controlled error")
                .contains("Font")
        );
        assert_eq!(dialog.changes().len(), 1);
        assert!(dialog.text_changes()[0].value.contains('é'));
    }
    #[test]
    fn actual_dropdown_control_uses_indices_and_requires_font_before_apply() {
        let mut app = app();
        let source = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
            None,
        )
        .expect("fixture");
        let choices = source.list_choice_fields().expect("choice");
        app.forms_dialog
            .as_mut()
            .expect("dialog")
            .loaded(vec![choices[0].field.clone()], choices);
        let context = egui::Context::default();
        click(&mut app, &context, "Normal · index 1");
        click(&mut app, &context, "High · index 2");
        let dialog = app.forms_dialog.as_ref().expect("dialog");
        let changes = dialog.choice_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "priority");
        assert_eq!(changes[0].indices, vec![2]);
        assert_eq!(changes[0].top_index, None);
        assert!(!app.apply_forms());
        app.forms_dialog.as_mut().expect("dialog").font_file = Some("covering.ttf".into());
        click(&mut app, &context, "Apply form changes");
        assert!(app.busy && app.forms_dialog.as_ref().expect("dialog").submitting);
    }
    #[test]
    fn actual_list_checkboxes_distinguish_duplicate_labels_and_keyboard_toggle() {
        let mut document =
            lopdf::Document::load_mem(include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"))
                .expect("fixture");
        let field = document.get_dictionary_mut((14, 0)).expect("field");
        field.set("Ff", 1 << 21);
        field.set(
            "Opt",
            vec![
                lopdf::Object::string_literal("same"),
                lopdf::Object::string_literal("same"),
                lopdf::Object::string_literal("High"),
            ],
        );
        field.set("V", lopdf::Object::string_literal("same"));
        field.set("I", vec![lopdf::Object::Integer(0)]);
        let mut bytes = vec![];
        document.save_to(&mut bytes).expect("fixturebytes");
        let choices = pdf_core::PdfDocument::from_bytes(&bytes, None)
            .expect("PDF")
            .list_choice_fields()
            .expect("choices");
        let mut app = app();
        app.forms_dialog
            .as_mut()
            .expect("dialog")
            .loaded(vec![choices[0].field.clone()], choices);
        let context = egui::Context::default();
        click(&mut app, &context, "same · index 1");
        assert_eq!(
            app.forms_dialog.as_ref().expect("dialog").choice_drafts[0],
            vec![0, 1]
        );
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
        let _ = draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.forms_dialog.as_ref().expect("dialog").choice_drafts[0],
            vec![0]
        );
        click(&mut app, &context, "High · index 2");
        assert_eq!(
            app.forms_dialog.as_ref().expect("dialog").choice_changes()[0].indices,
            vec![0, 2]
        );
        app.forms_dialog.as_mut().expect("dialog").stale = true;
        assert!(!app.apply_forms());
        click(&mut app, &context, "same · index 1");
        assert_eq!(
            app.forms_dialog.as_ref().expect("dialog").choice_drafts[0],
            vec![0, 2]
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
        click(&mut app, &context, "Apply form changes");
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
