use super::EditorApp;
use crate::{
    state::FormsReadKey,
    worker::{Command, Edit},
};
use eframe::egui;
use pdf_core::{NumberPosition, PageNumbering, TextAlignment};
use std::path::PathBuf;
pub(super) struct Dialog {
    pub key: FormsReadKey,
    range: String,
    first_number: String,
    pub request: PageNumbering,
    font_file: Option<PathBuf>,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
}
impl Dialog {
    pub fn has_changes(&self) -> bool {
        true
    }
    fn selected(&self, count: usize) -> Result<Vec<u32>, String> {
        if self.range.len() > 8192 {
            return Err("Page range exceeds 8 KiB.".into());
        }
        let mut pages =
            pdf_core::selection::parse_unique(&self.range, count).map_err(|e| e.to_string())?;
        pages.sort_unstable();
        self.first_number
            .parse::<u64>()
            .map_err(|_| "First number must be an unsigned integer".to_string())?
            .checked_add(pages.len() as u64 - 1)
            .ok_or("Number sequence overflows")?;
        Ok(pages)
    }
}
impl EditorApp {
    pub(super) fn open_numbering(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.has_page_content_dialog() {
            return;
        }
        let Some(tab) = self.selected().filter(|t| t.editable()) else {
            return;
        };
        let (id, revision, count) = (tab.id, tab.status.revision, tab.status.info.page_count);
        let Some(generation) = self.forms_generation.checked_add(1) else {
            self.message = "Draft counter exhausted".into();
            return;
        };
        self.forms_generation = generation;
        self.numbering_dialog = Some(Dialog {
            key: FormsReadKey {
                tab: id,
                revision,
                generation,
            },
            range: format!("1-{count}"),
            first_number: "1".into(),
            request: PageNumbering {
                pages: Vec::new(),
                start: 1,
                position: NumberPosition::Footer,
                margin: 18.0,
                height: 24.0,
                size: 12.0,
                color: [0.0; 3],
                alignment: TextAlignment::Center,
            },
            font_file: None,
            submitting: false,
            stale: false,
            error: None,
        });
    }
    fn numbering_current(&self) -> bool {
        self.numbering_dialog.as_ref().is_some_and(|d| {
            !d.stale
                && self.active == Some(d.key.tab)
                && self.tabs.iter().any(|t| {
                    t.id == d.key.tab && t.status.revision == d.key.revision && t.editable()
                })
        })
    }
    fn apply_numbering(&mut self) -> bool {
        if self.busy || !self.pending_saves.is_empty() || !self.numbering_current() {
            return false;
        }
        let Some(d) = self.numbering_dialog.as_ref().filter(|d| !d.submitting) else {
            return false;
        };
        let Some(tab) = self.selected() else {
            return false;
        };
        let Ok(pages) = d.selected(tab.status.info.page_count) else {
            return false;
        };
        let Some(font_file) = d.font_file.clone() else {
            return false;
        };
        let mut numbering = d.request.clone();
        numbering.pages = pages;
        let Ok(start) = d.first_number.parse::<u64>() else {
            return false;
        };
        numbering.start = start;
        if !self.send(
            Command::Edit {
                id: d.key.tab,
                edit: Edit::PageNumbers {
                    key: d.key,
                    numbering,
                    page: tab.page as u32 + 1,
                    font_file,
                },
            },
            "Adding page numbers locally…",
        ) {
            return false;
        }
        if let Some(d) = self.numbering_dialog.as_mut() {
            d.submitting = true;
            d.error = None;
        }
        true
    }
    pub(super) fn numbering_dialog_ui(&mut self, context: &egui::Context) {
        if self.close_target.is_some() {
            return;
        }
        if !self.numbering_current()
            && let Some(d) = self.numbering_dialog.as_mut()
        {
            d.stale = true;
        }
        let count = self
            .selected()
            .map(|t| t.status.info.page_count)
            .unwrap_or(0);
        let Some(d) = self.numbering_dialog.as_mut() else {
            return;
        };
        let enabled = !self.busy && !d.stale && !d.submitting;
        let mut apply = false;
        let mut discard = false;
        egui::Window::new("Number pages").collapsible(false).default_width(390.0).max_width((context.content_rect().width()-32.0).max(220.0)).max_height((context.content_rect().height()-32.0).max(180.0)).show(context,|ui|{
            ui.label("Add upright page numbers as new text. All selected pages form one undoable edit.");
            if d.stale{ui.colored_label(ui.visuals().error_fg_color,"The document changed. Copy your settings, then discard and reopen this dialog.");}
            egui::ScrollArea::vertical().max_height((ui.available_height()-76.0).max(60.0)).show(ui,|ui|{
                ui.add_enabled_ui(enabled,|ui|{
                    let label=ui.label("Pages to number");ui.add(egui::TextEdit::singleline(&mut d.range).char_limit(8192).desired_width(f32::INFINITY)).labelled_by(label.id);
                    let label=ui.label("First number");ui.add(egui::TextEdit::singleline(&mut d.first_number).char_limit(20).desired_width(150.0)).labelled_by(label.id);
                    ui.horizontal(|ui|{ui.selectable_value(&mut d.request.position,NumberPosition::Header,"Header");ui.selectable_value(&mut d.request.position,NumberPosition::Footer,"Footer");});
                    for (name,value) in [("Edge margin (PDF units)",&mut d.request.margin),("Number band height (PDF units)",&mut d.request.height),("Font size (PDF units)",&mut d.request.size)] {ui.horizontal(|ui|{ui.label(name);ui.add(egui::DragValue::new(value).speed(0.5));});}
                    egui::ComboBox::from_label("Alignment").selected_text(match d.request.alignment{TextAlignment::Left=>"Left",TextAlignment::Center=>"Center",TextAlignment::Right=>"Right"}).show_ui(ui,|ui|{for (name,value) in [("Left",TextAlignment::Left),("Center",TextAlignment::Center),("Right",TextAlignment::Right)]{if ui.selectable_label(std::mem::discriminant(&d.request.alignment)==std::mem::discriminant(&value),name).clicked(){d.request.alignment=value;}}});
                    ui.horizontal(|ui|{ui.label("Number color");let mut rgb=d.request.color.map(|v|v as f32);ui.color_edit_button_rgb(&mut rgb);d.request.color=rgb.map(f64::from);});
                    if ui.button("Choose covering font…").clicked(){d.font_file=rfd::FileDialog::new().add_filter("TrueType / OpenType",&["ttf","otf"]).pick_file().or_else(||d.font_file.clone());}
                    if let Some(font)=&d.font_file{ui.label(font.file_name().unwrap_or_default().to_string_lossy());}
                });
                match d.selected(count){Ok(pages)=>{ui.label(format!("{} pages · document order · numbers {}–{}",pages.len(),d.first_number,d.first_number.parse::<u64>().unwrap_or(0)+pages.len() as u64-1));},Err(error)=>{ui.colored_label(ui.visuals().error_fg_color,error);}}
                ui.small("Numbers are added inside the visible page edges. Existing text is preserved. Font coverage and fit are checked before any page is changed.");
            });
            if let Some(error)=&d.error{ui.colored_label(ui.visuals().error_fg_color,error);}
            ui.horizontal_wrapped(|ui|{
                if ui.button("Copy settings").clicked(){context.copy_text(format!("Pages: {}\nFirst number: {}\n{}",d.range,d.first_number,serde_json::to_string_pretty(&serde_json::json!({"position":d.request.position,"margin":d.request.margin,"height":d.request.height,"size":d.request.size,"color":d.request.color,"alignment":d.request.alignment})).unwrap_or_default()));}
                apply=ui.add_enabled(enabled&&d.font_file.is_some()&&d.selected(count).is_ok(),crate::style::primary("Add page numbers",ui.visuals().dark_mode)).clicked();discard=ui.add_enabled(!d.submitting,egui::Button::new("Discard numbering draft")).clicked();});
        });
        if discard {
            self.numbering_dialog = None;
        } else if apply {
            self.apply_numbering();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn app(ctx: &egui::Context) -> EditorApp {
        let mut app = super::super::text_overlay::tests::app(ctx);
        app.overlay_dialog = None;
        app.open_numbering();
        app
    }
    fn draw(
        app: &mut EditorApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                events,
                ..Default::default()
            },
            |_| app.numbering_dialog_ui(ctx),
        );
        out.textures_delta.clear();
        out
    }
    fn click(app: &mut EditorApp, ctx: &egui::Context, label: &str) {
        let out = draw(app, ctx, vec![]);
        let pos = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t)
                    if t.galley.text() == label
                        && s.clip_rect
                            .contains_rect(t.galley.rect.translate(t.pos.to_vec2())) =>
                {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .expect("visible labeled button");
        draw(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        draw(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn stale_numbering_draft_is_retained_readonly_and_protects_quit() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.numbering_dialog.as_mut().unwrap().first_number = "42".into();
        app.tabs[0].status.revision += 1;
        draw(&mut app, &ctx, vec![]);
        assert!(app.numbering_dialog.as_ref().unwrap().stale);
        assert_eq!(app.numbering_dialog.as_ref().unwrap().first_number, "42");
        assert!(!app.apply_numbering());
        assert!(app.needs_exit_guard());
        assert!(app.has_tab_draft(1));
        click(&mut app, &ctx, "Discard numbering draft");
        assert!(app.numbering_dialog.is_none());
        assert!(!app.needs_exit_guard());
    }
    #[test]
    fn numbering_controls_and_footer_are_reachable_and_canvas_dialogs_cannot_overlap() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "Header");
        assert_eq!(
            app.numbering_dialog.as_ref().unwrap().request.position,
            NumberPosition::Header
        );
        app.open_text_overlay();
        app.open_vector();
        app.open_image_overlay();
        app.open_content_text();
        assert!(
            app.overlay_dialog.is_none()
                && app.vector_dialog.is_none()
                && app.image_dialog.is_none()
                && app.content_dialog.is_none()
        );
        assert!(!app.apply_numbering());
        click(&mut app, &ctx, "Discard numbering draft");
        assert!(app.numbering_dialog.is_none());
    }
    #[test]
    fn selection_errors_and_readonly_pending_wrong_tab_do_not_dispatch() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let d = app.numbering_dialog.as_mut().unwrap();
        d.range = "1,1".into();
        assert!(d.selected(1).is_err());
        d.range = "1".into();
        d.font_file = Some("font.ttf".into());
        d.first_number = u64::MAX.to_string();
        assert!(d.selected(2).is_ok());
        d.range = "1-2".into();
        assert!(d.selected(2).is_err());
        d.range = "1".into();
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_numbering());
        app.tabs[0].status.info.signature_fields = 0;
        app.active = Some(99);
        assert!(!app.apply_numbering());
        app.active = Some(1);
        app.pending_saves.insert(
            1,
            super::super::recovery::PendingSave::new(
                "pending.pdf".into(),
                "Recorded attempt".into(),
            ),
        );
        assert!(!app.apply_numbering());
        assert!(!app.busy);
    }
}
