use super::EditorApp;
use crate::worker::{Command, ContentTextKey, Edit};
use eframe::egui;
use pdf_worker::document::ContentTextPage;
#[cfg(test)]
mod tests;

pub(super) struct Dialog {
    pub key: ContentTextKey,
    inventory: Option<ContentTextPage>,
    selected: Option<usize>,
    replacement: String,
    allow_width_change: bool,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    focus: bool,
    size_enabled: bool,
    position_enabled: bool,
    rgb_enabled: bool,
    size: f32,
    position: [f32; 2],
    rgb: [f32; 3],
    properties_mode: bool,
    face_mode: bool,
    target_face: Option<pdf_core::ContentTextFont>,
}
impl Dialog {
    pub(super) fn has_changes(&self) -> bool {
        self.text_changed() || self.property_draft() || self.face_draft()
    }
    fn face_draft(&self) -> bool {
        self.target_face.is_some_and(|font| {
            self.selected
                .and_then(|index| self.inventory.as_ref()?.runs.get(index)?.standard_font)
                != Some(font)
        })
    }
    fn property_draft(&self) -> bool {
        self.size_enabled || self.position_enabled || self.rgb_enabled
    }
    fn text_changed(&self) -> bool {
        self.selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index)?.text.as_deref())
            .is_some_and(|text| text != self.replacement)
    }
    fn select(&mut self, index: usize) {
        self.size_enabled = false;
        self.position_enabled = false;
        self.rgb_enabled = false;
        self.rgb = [0.0; 3];
        self.allow_width_change = false;
        self.target_face = None;
        self.selected = Some(index);
        self.replacement = self
            .inventory
            .as_ref()
            .and_then(|inventory| inventory.runs.get(index))
            .and_then(|run| run.text.clone())
            .unwrap_or_default();
        self.error = None;
        if let Some(layout) = self
            .inventory
            .as_ref()
            .and_then(|inventory| inventory.runs.get(index))
            .and_then(|run| run.layout.as_ref())
        {
            self.size = layout.font_size as f32;
            self.position = [layout.text_matrix[4] as f32, layout.text_matrix[5] as f32];
        }
        self.focus = true;
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.face_draft() {
            return Err("Apply or reset the font draft before replacing text.");
        }
        if self.property_draft() {
            return Err("Apply or reset the property draft before replacing text.");
        }
        let run = self
            .selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index))
            .ok_or("Select a text run.")?;
        if !run.writable {
            return Err("This run is outside the supported editing tier.");
        }
        if run
            .validate_replacement_with_policy(&self.replacement, self.layout_policy())
            .is_err()
        {
            if matches!(run.encoding, Some(pdf_core::ContentTextEncoding::Standard))
                && (self.replacement.contains('\'') || self.replacement.contains('`'))
            {
                return Err("StandardEncoding quote codes are ambiguous; no value was truncated.");
            }
            return Err(
                "Preserve each word's nominal advance, or explicitly enable bounded width change. Use unambiguous printable ASCII; no value was truncated.",
            );
        }
        if !self.text_changed() {
            return Err("Enter a changed value.");
        }
        Ok(())
    }
    fn layout_policy(&self) -> pdf_core::ContentTextLayoutPolicy {
        if self.allow_width_change {
            pdf_core::ContentTextLayoutPolicy::AllowWidthChange
        } else {
            Default::default()
        }
    }
    fn property_change(&self) -> Result<pdf_core::ContentTextPropertyChange, &'static str> {
        if self.face_draft() {
            return Err("Apply or reset the font draft before applying properties.");
        }
        if self.text_changed() {
            return Err("Replace or reset the text draft before applying properties.");
        }
        let run = self
            .selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index))
            .ok_or("Select a text run.")?;
        let layout = run
            .layout
            .as_ref()
            .ok_or("Bounded text layout is unavailable.")?;
        let change = pdf_core::ContentTextPropertyChange {
            identity: run.identity.clone(),
            expected: run.text.clone().ok_or("Text is unavailable.")?,
            font_size: (self.size_enabled && self.size != layout.font_size as f32)
                .then_some(self.size),
            matrix_translation: (self.position_enabled
                && self.position != [layout.text_matrix[4] as f32, layout.text_matrix[5] as f32])
            .then_some(self.position),
            fill_rgb: self.rgb_enabled.then_some(self.rgb),
        };
        run.property_position(&change).map_err(|_|"Choose a supported run and finite bounded properties; RGB must be between zero and one.")?;
        Ok(change)
    }
    fn face_change(&self) -> Result<pdf_core::ContentTextFaceChange, &'static str> {
        if self.text_changed() || self.property_draft() {
            return Err("Apply or reset other drafts before changing the font face.");
        }
        let run = self
            .selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index))
            .ok_or("Select a text run.")?;
        let font = self
            .target_face
            .filter(|font| Some(*font) != run.standard_font)
            .ok_or("Choose a different font face.")?;
        let change = pdf_core::ContentTextFaceChange {
            identity: run.identity.clone(),
            expected: run.text.clone().ok_or("Text is unavailable.")?,
            font,
            replacement: None,
            layout_policy: self.layout_policy(),
        };
        run.face_position(&change).map_err(|_|"Choose a supported run and explicitly allow width changes if the new face changes word or space advances.")?;
        Ok(change)
    }
}
impl EditorApp {
    pub(super) fn open_content_text(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.has_page_content_dialog() {
            return;
        }
        let Some(tab) = self.selected() else {
            return;
        };
        let (id, revision, page) = (tab.id, tab.status.revision, tab.page as u32 + 1);
        self.content_generation = self.content_generation.wrapping_add(1);
        let key = ContentTextKey {
            tab: id,
            revision,
            page,
            offset: 0,
            generation: self.content_generation,
        };
        self.content_dialog = Some(Dialog {
            key,
            inventory: None,
            selected: None,
            replacement: String::new(),
            allow_width_change: false,
            submitting: false,
            stale: false,
            error: None,
            focus: false,
            size_enabled: false,
            position_enabled: false,
            rgb_enabled: false,
            size: 12.0,
            position: [0.0; 2],
            rgb: [0.0; 3],
            properties_mode: false,
            face_mode: false,
            target_face: None,
        });
        self.send(Command::ContentText(key), "Inspecting existing page text…");
    }
    fn content_current(&self, key: ContentTextKey) -> bool {
        self.selected().is_some_and(|tab| {
            tab.id == key.tab
                && tab.status.revision == key.revision
                && tab.page as u32 + 1 == key.page
        })
    }
    pub(super) fn accept_content_text(&mut self, key: ContentTextKey, inventory: ContentTextPage) {
        if !self.content_current(key) {
            return;
        }
        let Some(dialog) = self
            .content_dialog
            .as_mut()
            .filter(|dialog| dialog.key == key && !dialog.stale)
        else {
            return;
        };
        let selected = inventory
            .runs
            .iter()
            .position(|run| run.writable)
            .or_else(|| (!inventory.runs.is_empty()).then_some(0));
        dialog.inventory = Some(inventory);
        dialog.error = None;
        if let Some(index) = selected {
            dialog.select(index);
        }
    }
    fn content_batch(&mut self, offset: u32) {
        let Some(dialog) = self.content_dialog.as_ref() else {
            return;
        };
        if self.busy
            || dialog.submitting
            || dialog.stale
            || dialog.has_changes()
            || !self.pending_saves.is_empty()
            || !self.content_current(dialog.key)
        {
            return;
        }
        self.content_generation = self.content_generation.wrapping_add(1);
        let key = ContentTextKey {
            offset,
            generation: self.content_generation,
            ..dialog.key
        };
        if self.send(
            Command::ContentText(key),
            "Inspecting another batch of existing runs…",
        ) && let Some(dialog) = &mut self.content_dialog
        {
            dialog.key = key;
            dialog.inventory = None;
            dialog.selected = None;
            dialog.replacement.clear();
            dialog.error = None;
        }
    }
    fn apply_content_text(&mut self) -> bool {
        let Some(dialog) = self.content_dialog.as_ref() else {
            return false;
        };
        if self.busy
            || !self.pending_saves.is_empty()
            || dialog.submitting
            || dialog.stale
            || !self.content_current(dialog.key)
            || !self.selected().is_some_and(|tab| tab.editable())
            || dialog.validate().is_err()
        {
            return false;
        }
        let Some(inventory) = &dialog.inventory else {
            return false;
        };
        let Some(run) = dialog.selected.and_then(|index| inventory.runs.get(index)) else {
            return false;
        };
        let Some(expected) = &run.text else {
            return false;
        };
        let key = dialog.key;
        let command = Command::Edit {
            id: key.tab,
            edit: Edit::ContentText {
                key,
                source_fingerprint: inventory.source_fingerprint,
                change: pdf_core::ContentTextChange {
                    layout_policy: dialog.layout_policy(),
                    identity: run.identity.clone(),
                    expected: expected.clone(),
                    replacement: dialog.replacement.clone(),
                },
            },
        };
        if self.send(command, "Replacing existing text in document history…") {
            if let Some(dialog) = &mut self.content_dialog {
                dialog.submitting = true;
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    fn apply_content_properties(&mut self) -> bool {
        let Some(dialog) = self.content_dialog.as_ref() else {
            return false;
        };
        if self.busy
            || !self.pending_saves.is_empty()
            || dialog.submitting
            || dialog.stale
            || !self.content_current(dialog.key)
            || !self.selected().is_some_and(|tab| tab.editable())
        {
            return false;
        }
        let Ok(change) = dialog.property_change() else {
            return false;
        };
        let Some(inventory) = &dialog.inventory else {
            return false;
        };
        let key = dialog.key;
        if self.send(
            Command::Edit {
                id: key.tab,
                edit: Edit::ContentTextProperties {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change,
                },
            },
            "Updating original text properties in document history…",
        ) {
            if let Some(dialog) = &mut self.content_dialog {
                dialog.submitting = true;
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    fn apply_content_face(&mut self) -> bool {
        let Some(dialog) = self.content_dialog.as_ref() else {
            return false;
        };
        if self.busy
            || !self.pending_saves.is_empty()
            || dialog.submitting
            || dialog.stale
            || !self.content_current(dialog.key)
            || !self.selected().is_some_and(|tab| tab.editable())
        {
            return false;
        }
        let Ok(change) = dialog.face_change() else {
            return false;
        };
        let Some(inventory) = &dialog.inventory else {
            return false;
        };
        let key = dialog.key;
        if self.send(
            Command::Edit {
                id: key.tab,
                edit: Edit::ContentTextFace {
                    key,
                    source_fingerprint: inventory.source_fingerprint,
                    change,
                },
            },
            "Changing the original text font in document history…",
        ) {
            if let Some(dialog) = &mut self.content_dialog {
                dialog.submitting = true;
                dialog.error = None;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn content_text_dialog(&mut self, context: &egui::Context) {
        if self.content_dialog.is_none() || self.close_target.is_some() {
            return;
        }
        if self
            .content_dialog
            .as_ref()
            .is_some_and(|dialog| !self.content_current(dialog.key))
            && let Some(dialog) = &mut self.content_dialog
        {
            dialog.stale = true;
        }
        let editable = self.selected().is_some_and(|tab| tab.editable());
        let Some(dialog) = &mut self.content_dialog else {
            return;
        };
        let enabled =
            !self.busy && !dialog.submitting && !dialog.stale && self.pending_saves.is_empty();
        let mut apply = false;
        let mut apply_properties = false;
        let mut apply_face = false;
        let mut discard = false;
        let mut next = None;
        egui::Window::new("Edit existing text").default_width(540.0).max_width((context.content_rect().width()-32.0).max(220.0)).max_height((context.content_rect().height()-32.0).max(180.0)).collapsible(false).anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).show(context,|ui|{
            ui.label(format!("Page {} · original text operands",dialog.key.page));
            ui.small("Standard Courier/Helvetica/Times ASCII. Default: preserve word advances. Value edits keep the font; face edits keep the text and matrix. No wrapping or fitting. Old bytes remain recoverable.");
            if !editable {ui.colored_label(ui.visuals().error_fg_color,"This document is read-only.");}
            if dialog.stale {ui.colored_label(ui.visuals().error_fg_color,"The document or page changed. Copy your draft, then discard and reopen.");}
            if let Some(inventory)=&dialog.inventory {
                    if inventory.runs.is_empty(){ui.label("No direct text runs in this tier; Form XObjects and TJ arrays are not inventoried.");}
                    else {ui.label(format!("Runs {}–{} of {}",inventory.offset+1,inventory.offset as usize+inventory.runs.len(),inventory.total_runs));}
                    let mut selected=None;
                    let select_enabled=enabled&&!dialog.has_changes();
                    let run_label=|index:usize,run:&pdf_core::ContentTextRun|{let preview=run.text.as_ref().map(|text|{let mut preview=text.chars().take(40).collect::<String>();if text.len()>40{preview.push('…');}preview}).unwrap_or_else(||"unsupported".into());format!("Run {} · {}",inventory.offset as usize+index+1,preview)};
                    let draw_runs=|ui:&mut egui::Ui|{
                    for (index,run) in inventory.runs.iter().enumerate() {
                        let label=run_label(index,run);
                        if ui.add_enabled(select_enabled,egui::Button::new(label).selected(dialog.selected==Some(index))).clicked(){selected=Some(index);}
                    }
                    };
                    if dialog.properties_mode || dialog.face_mode {let title=dialog.selected.and_then(|index|inventory.runs.get(index).map(|run|run_label(index,run))).unwrap_or_else(||"Select a run".into());egui::ComboBox::from_id_salt("existing-property-run").selected_text(title).show_ui(ui,draw_runs);}
                    else {egui::ScrollArea::vertical().id_salt("existing-text-runs").max_height(120.0).show(ui,draw_runs);}
                    if let Some(index)=selected {dialog.select(index);}
            }else {ui.label("Loading bounded inventory…");}
            ui.add_enabled_ui(enabled,|ui|{ui.horizontal(|ui|{
                if ui.selectable_label(!dialog.properties_mode&&!dialog.face_mode,"Text value").clicked(){dialog.properties_mode=false;dialog.face_mode=false;}
                if ui.selectable_label(dialog.properties_mode,"Text properties").clicked(){dialog.properties_mode=true;dialog.face_mode=false;}
                if ui.selectable_label(dialog.face_mode,"Font face").clicked(){if !dialog.face_mode {dialog.allow_width_change=false;}dialog.properties_mode=false;dialog.face_mode=true;}
            });});
            if dialog.properties_mode {ui.label("Original text properties · same font and text");}
            if !dialog.properties_mode && !dialog.face_mode && let Some(run)=dialog.selected.and_then(|index|dialog.inventory.as_ref()?.runs.get(index)) {
                ui.add_enabled(enabled&&editable&&run.writable&&run.layout.is_some(),egui::Checkbox::new(&mut dialog.allow_width_change,"Allow line width change"));
                if let Some(expected)=&run.text && let (Ok(before),Ok(after))=(run.replacement_position(expected),run.replacement_position(&dialog.replacement)) {
                    ui.label(format!("Nominal advance: {:.3} → {:.3} content-space units",before.nominal_advance,after.nominal_advance));
                }
            }
            egui::ScrollArea::vertical().id_salt("existing-text-controls").max_height((ui.available_height()-86.0).max(90.0)).show(ui,|ui|{
                if let Some(run)=dialog.selected.and_then(|index|dialog.inventory.as_ref()?.runs.get(index)) {
                    if let Some(reason)=&run.refusal_reason {ui.label(reason);}
                    if !dialog.properties_mode && !dialog.face_mode && let Some(expected)=&run.text {
                        let mut original=expected.clone();let label=ui.label("Current text");
                        ui.add(egui::TextEdit::singleline(&mut original).interactive(false).desired_width(f32::INFINITY)).labelled_by(label.id);
                        let label=ui.label("Replacement text");
                        let response=ui.add_enabled(enabled&&editable&&run.writable,egui::TextEdit::singleline(&mut dialog.replacement).desired_width(f32::INFINITY).id_salt(("existing-text-value",dialog.key.generation))).labelled_by(label.id);
                        if std::mem::take(&mut dialog.focus){response.request_focus();}
                        if let Some(font)=run.standard_font {ui.label(format!("Original font: {}",font.pdf_name()));}
                        if let (Ok(before),Ok(after))=(run.replacement_position(expected),run.replacement_position(&dialog.replacement)) {
                            ui.label(format!("Baseline start: ({:.3}, {:.3}); endpoint: ({:.3}, {:.3}) → ({:.3}, {:.3})",before.baseline_start[0],before.baseline_start[1],before.baseline_end[0],before.baseline_end[1],after.baseline_end[0],after.baseline_end[1]));
                        }
                        if dialog.allow_width_change {ui.label("Width changes can overlap nearby artwork or leave the page. Font and baseline stay fixed; there is no wrapping, fitting or old-ink clipping.");}
                        apply=(response.has_focus()||response.lost_focus())&&ui.input(|input|input.key_pressed(egui::Key::Enter));
                    }
                    if dialog.properties_mode {
                    if let Some(reason)=&run.properties_refusal_reason {ui.label(reason);}
                    ui.add_enabled_ui(enabled&&editable&&run.properties_writable,|ui|{
                        ui.horizontal(|ui|{ui.checkbox(&mut dialog.size_enabled,"Change font size");let label=ui.label("Font size");ui.add_enabled(dialog.size_enabled,egui::DragValue::new(&mut dialog.size).speed(0.25).prefix("Size ")).labelled_by(label.id);});
                        ui.horizontal(|ui|{ui.checkbox(&mut dialog.position_enabled,"Move text matrix");let x=ui.label("Matrix X");ui.add_enabled(dialog.position_enabled,egui::DragValue::new(&mut dialog.position[0]).speed(0.5).prefix("X ")).labelled_by(x.id);let y=ui.label("Matrix Y");ui.add_enabled(dialog.position_enabled,egui::DragValue::new(&mut dialog.position[1]).speed(0.5).prefix("Y ")).labelled_by(y.id);});
                        ui.horizontal(|ui|{ui.checkbox(&mut dialog.rgb_enabled,"Set RGB fill");let label=ui.label("Desired RGB");ui.add_enabled_ui(dialog.rgb_enabled,|ui|{ui.color_edit_button_rgb(&mut dialog.rgb).labelled_by(label.id);});});
                        ui.add_enabled_ui(dialog.rgb_enabled,|ui|{ui.horizontal(|ui|{for (index,name) in ["Red","Green","Blue"].into_iter().enumerate(){let label=ui.label(name);ui.add(egui::DragValue::new(&mut dialog.rgb[index]).speed(0.01).range(0.0..=1.0).prefix(["R ","G ","B "][index])).labelled_by(label.id);}});});
                    });
                    ui.small("Matrix X/Y replace e/f in the original PDF text matrix. Size and width can overlap artwork or leave the page; there is no wrapping or fitting. RGB is an explicit new fill, not a guessed current color.");
                    }
                    if dialog.face_mode {
                        if let Some(reason)=&run.properties_refusal_reason {ui.label(reason);}
                        if let Some(font)=run.standard_font {ui.label(format!("Original font: {}",font.pdf_name()));}
                        let label=ui.label("New standard font face");
                        ui.add_enabled_ui(enabled&&editable&&run.properties_writable,|ui|{
                            egui::ComboBox::from_id_salt("existing-font-face").selected_text(dialog.target_face.map(|font|font.pdf_name()).unwrap_or("Choose a face")).show_ui(ui,|ui|{for font in pdf_core::ContentTextFont::ALL {ui.selectable_value(&mut dialog.target_face,Some(font),font.pdf_name());}}).response.labelled_by(label.id);
                            ui.checkbox(&mut dialog.allow_width_change,"Allow font width change");
                        });
                        if let (Some(expected),Ok(change))=(&run.text,dialog.face_change()) && let (Ok(before),Ok(after))=(run.replacement_position(expected),run.face_position(&change)) {
                            ui.label(format!("Nominal advance: {:.3} → {:.3} content-space units",before.nominal_advance,after.nominal_advance));
                        }
                        ui.small("Keeps the original size, matrix and text. New glyph ink and width may overlap artwork or leave the page. No wrapping, fitting or old-ink clipping. User-selected embedded fonts are not yet supported here.");
                    }
                }
                if dialog.text_changed() && let Err(reason)=dialog.validate(){ui.colored_label(ui.visuals().error_fg_color,reason);}
                if dialog.property_draft() && let Err(reason)=dialog.property_change(){ui.colored_label(ui.visuals().error_fg_color,reason);}
                if dialog.face_draft() && let Err(reason)=dialog.face_change(){ui.colored_label(ui.visuals().error_fg_color,reason);}
                if let Some(error)=&dialog.error {ui.colored_label(ui.visuals().error_fg_color,error);}
            });
            ui.horizontal_wrapped(|ui|{
                if dialog.face_mode {apply_face=ui.add_enabled(enabled&&editable&&dialog.face_change().is_ok(),egui::Button::new("Apply font face")).clicked();}
                else if !dialog.properties_mode {apply|=ui.add_enabled(enabled&&editable&&dialog.validate().is_ok(),crate::style::primary("Replace existing text",ui.visuals().dark_mode)).clicked();}
                else {apply_properties=ui.add_enabled(enabled&&editable&&dialog.property_change().is_ok(),egui::Button::new("Apply text properties")).clicked();}
                discard=ui.add_enabled(!dialog.submitting,egui::Button::new("Discard draft and close")).clicked();
                if ui.add_enabled(enabled&&dialog.has_changes(),egui::Button::new("Reset replacement")).clicked() && let Some(index)=dialog.selected {dialog.select(index);}
                if ui.add_enabled(enabled&&!dialog.has_changes()&&dialog.key.offset>0,egui::Button::new("First runs")).clicked(){next=Some(0);}
                if let Some(offset)=dialog.inventory.as_ref().and_then(|inventory|inventory.next_offset) && ui.add_enabled(enabled&&!dialog.has_changes(),egui::Button::new("Next runs")).clicked(){next=Some(offset);}
            });
        });
        if discard {
            self.content_dialog = None;
        } else if apply {
            self.apply_content_text();
        } else if apply_properties {
            self.apply_content_properties();
        } else if apply_face {
            self.apply_content_face();
        } else if let Some(offset) = next {
            self.content_batch(offset);
        }
    }
}
