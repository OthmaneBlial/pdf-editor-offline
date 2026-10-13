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
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    focus: bool,
}
impl Dialog {
    pub(super) fn has_changes(&self) -> bool {
        self.selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index)?.text.as_deref())
            .is_some_and(|text| text != self.replacement)
    }
    fn select(&mut self, index: usize) {
        self.selected = Some(index);
        self.replacement = self
            .inventory
            .as_ref()
            .and_then(|inventory| inventory.runs.get(index))
            .and_then(|run| run.text.clone())
            .unwrap_or_default();
        self.error = None;
        self.focus = true;
    }
    fn validate(&self) -> Result<(), &'static str> {
        let run = self
            .selected
            .and_then(|index| self.inventory.as_ref()?.runs.get(index))
            .ok_or("Select a text run.")?;
        if !run.writable {
            return Err("This run is outside the supported editing tier.");
        }
        if run.validate_replacement(&self.replacement).is_err() {
            if matches!(run.encoding, Some(pdf_core::ContentTextEncoding::Standard))
                && (self.replacement.contains('\'') || self.replacement.contains('`'))
            {
                return Err("StandardEncoding quote codes are ambiguous; no value was truncated.");
            }
            return Err(
                "Keep printable ASCII length and spaces unchanged; no value was truncated.",
            );
        }
        if !self.has_changes() {
            return Err("Enter a changed value.");
        }
        Ok(())
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
            submitting: false,
            stale: false,
            error: None,
            focus: false,
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
        let mut discard = false;
        let mut next = None;
        egui::Window::new("Edit existing text").default_width(540.0).max_width((context.content_rect().width()-32.0).max(220.0)).max_height((context.content_rect().height()-32.0).max(180.0)).collapsible(false).anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).show(context,|ui|{
            ui.label(format!("Page {} · original text operands",dialog.key.page));
            ui.small("Standard Courier only: equal-length ASCII, keeping spaces in place. Some fonts cannot edit ASCII apostrophe/backtick. No reflow or secure deletion; old stream bytes remain recoverable.");
            if !editable {ui.colored_label(ui.visuals().error_fg_color,"This document is read-only.");}
            if dialog.stale {ui.colored_label(ui.visuals().error_fg_color,"The document or page changed. Copy your draft, then discard and reopen.");}
            if let Some(inventory)=&dialog.inventory {
                    if inventory.runs.is_empty(){ui.label("No direct text runs in this tier; Form XObjects and TJ arrays are not inventoried.");}
                    else {ui.label(format!("Runs {}–{} of {}",inventory.offset+1,inventory.offset as usize+inventory.runs.len(),inventory.total_runs));}
                    let mut selected=None;
                    let select_enabled=enabled&&!dialog.has_changes();
                    egui::ScrollArea::vertical().id_salt("existing-text-runs").max_height(120.0).show(ui,|ui|{
                    for (index,run) in inventory.runs.iter().enumerate() {
                        let preview=run.text.as_ref().map(|text|{let mut preview=text.chars().take(40).collect::<String>();if text.len()>40{preview.push('…');}preview}).unwrap_or_else(||"unsupported".into());
                        let label=format!("Run {} · {}",inventory.offset as usize+index+1,preview);
                        if ui.add_enabled(select_enabled,egui::Button::new(label).selected(dialog.selected==Some(index))).clicked(){selected=Some(index);}
                    }
                    });
                    if let Some(index)=selected {dialog.select(index);}
            }else {ui.label("Loading bounded inventory…");}
            egui::ScrollArea::vertical().id_salt("existing-text-controls").max_height((ui.available_height()-86.0).max(90.0)).show(ui,|ui|{
                if let Some(run)=dialog.selected.and_then(|index|dialog.inventory.as_ref()?.runs.get(index)) {
                    if let Some(reason)=&run.refusal_reason {ui.label(reason);}
                    if let Some(expected)=&run.text {
                        let mut original=expected.clone();let label=ui.label("Current text");
                        ui.add(egui::TextEdit::singleline(&mut original).interactive(false).desired_width(f32::INFINITY)).labelled_by(label.id);
                        let label=ui.label("Replacement text");
                        let response=ui.add_enabled(enabled&&editable&&run.writable,egui::TextEdit::singleline(&mut dialog.replacement).desired_width(f32::INFINITY).id_salt(("existing-text-value",dialog.key.generation))).labelled_by(label.id);
                        if std::mem::take(&mut dialog.focus){response.request_focus();}
                        apply=(response.has_focus()||response.lost_focus())&&ui.input(|input|input.key_pressed(egui::Key::Enter));
                    }
                }
                if dialog.has_changes() && let Err(reason)=dialog.validate(){ui.colored_label(ui.visuals().error_fg_color,reason);}
                if let Some(error)=&dialog.error {ui.colored_label(ui.visuals().error_fg_color,error);}
            });
            ui.horizontal_wrapped(|ui|{
                apply|=ui.add_enabled(enabled&&editable&&dialog.validate().is_ok(),crate::style::primary("Replace existing text",ui.visuals().dark_mode)).clicked();
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
        } else if let Some(offset) = next {
            self.content_batch(offset);
        }
    }
}
