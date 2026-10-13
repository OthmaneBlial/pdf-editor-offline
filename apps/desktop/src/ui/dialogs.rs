use super::{CloseTarget, EditorApp};
use crate::worker::Command;
use eframe::egui;

impl EditorApp {
    pub(super) fn close_dialog(&mut self, context: &egui::Context) {
        let Some(target) = self.close_target else {
            return;
        };
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(context, |ui| {
                if self.busy { ui.label("Wait for the document operation to finish before closing."); }
                else {
                    match target {
                        CloseTarget::Tab(id) => {
                            if self.has_tab_draft(id) {
                                ui.label("This tab has unapplied drafts. Return to apply or copy them, or explicitly discard them before closing. Save As saves only applied document edits.");
                                if ui.button("Return to drafts").clicked(){self.close_target=None;}
                                if ui.button("Discard tab drafts").clicked(){
                                    if self.metadata_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.metadata_dialog=None;}
                                    if self.forms_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.forms_dialog=None;}
                                    if self.overlay_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.overlay_dialog=None;}
                                    if self.notes_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.notes_dialog=None;}
                                    if self.vector_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.vector_dialog=None;}
                                    if self.image_dialog.as_ref().is_some_and(|d|d.key.tab==id){self.image_dialog=None;}
                                    self.close_target=None;self.request_close(id);
                                }
                                return;
                            }
                            ui.label("This PDF has unsaved edits. Save a new file or discard the edits before closing.");
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Save As…").clicked() { self.save_as(id, true); }
                                if ui.button("Discard edits and close").clicked() {
                                    self.close_target = None; self.send(Command::Close(id), "Closing PDF…");
                                }
                                if ui.button("Keep editing").clicked() { self.close_target = None; }
                            });
                        }
                        CloseTarget::Exit => {
                            let dirty = self.tabs.iter().filter(|tab| tab.status.dirty).count();
                            let properties_draft=self.metadata_dialog.as_ref().is_some_and(super::metadata::Dialog::has_changes) || self.forms_dialog.as_ref().is_some_and(super::forms::Dialog::has_changes) || self.overlay_dialog.as_ref().is_some_and(super::text_overlay::Dialog::has_changes) || self.notes_dialog.as_ref().is_some_and(super::notes::Dialog::has_changes);
                            let properties_draft=properties_draft || self.vector_dialog.as_ref().is_some_and(super::vector::Dialog::has_changes) || self.image_dialog.as_ref().is_some_and(super::image_overlay::Dialog::has_changes);
                            let pending_save = !self.pending_saves.is_empty();
                            if pending_save { ui.label("A recorded save attempt needs attention. Return to resolve or cancel it, or keep the recovery session and quit."); }
                            else if properties_draft { ui.label("Unapplied properties, form, note, text/image overlay or drawing drafts are not recorded in recovery. Return to apply them, or explicitly discard the drafts before quitting."); }
                            else if dirty == 0 { ui.label("The operation finished. There are no unsaved edits."); }
                            else { ui.label(format!("{dirty} PDF(s) have unsaved edits. Return to Save As, keep recorded recovery copies, or discard the edits before quitting.")); }
                            ui.horizontal(|ui| {
                                if ui.button("Return to editing").clicked() { self.close_target = None; }
                                if (dirty > 0 || pending_save || properties_draft) && ui.button(if properties_draft { "Discard unapplied drafts, keep recovery and quit" } else { "Keep recovery copies and quit" }).clicked() {
                                    self.metadata_dialog = None;
                                    self.forms_dialog = None;
                                    self.overlay_dialog = None;
                                    self.image_dialog = None;
                                    self.notes_dialog = None;
                                    self.vector_dialog = None;
                                    self.allow_exit = true; context.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                if ui.add_enabled(!pending_save, egui::Button::new(if dirty == 0 && !properties_draft { "Quit" } else { "Discard edits and quit" })).on_hover_text("Resolve or cancel recorded save attempts before discarding edits.").clicked() {
                                    self.discard_edits_and_quit(context);
                                }
                            });
                        }
                    }
                }
            });
    }
}
