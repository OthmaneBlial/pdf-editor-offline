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
                            ui.label("This PDF has unsaved edits. Save a new file or discard the edits before closing.");
                            ui.horizontal(|ui| {
                                if ui.button("Save As…").clicked() { self.save_as(id, true); }
                                if ui.button("Discard edits and close").clicked() {
                                    self.close_target = None; self.send(Command::Close(id), "Closing PDF…");
                                }
                                if ui.button("Keep editing").clicked() { self.close_target = None; }
                            });
                        }
                        CloseTarget::Exit => {
                            let dirty = self.tabs.iter().filter(|tab| tab.status.dirty).count();
                            let properties_draft=self.metadata_dialog.as_ref().is_some_and(super::metadata::Dialog::has_changes);
                            if properties_draft { ui.label("Document properties contain unapplied changes. Return to editing to apply them and Save As, or discard the changes and quit."); }
                            else if dirty == 0 { ui.label("The operation finished. There are no unsaved edits."); }
                            else { ui.label(format!("{dirty} PDF(s) have unsaved edits. Return to editing to Save As, or discard all edits and quit.")); }
                            ui.horizontal(|ui| {
                                if ui.button("Return to editing").clicked() { self.close_target = None; }
                                if ui.button(if dirty == 0 && !properties_draft { "Quit" } else { "Discard edits and quit" }).clicked() {
                                    self.allow_exit = true; context.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        }
                    }
                }
            });
    }
}
