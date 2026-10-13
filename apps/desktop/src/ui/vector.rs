//! New artwork drafts only: existing PDF content is never selected or erased.
use super::{EditorApp, text_overlay::placement::PageMap};
use crate::{
    state::FormsReadKey,
    worker::{Command, Edit},
};
use eframe::egui;
use pdf_core::{VectorOverlay, VectorShape, VectorStroke};
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Rectangle,
    Ellipse,
    Line,
    Freehand,
}
struct Gesture {
    map: PageMap,
    start: [f64; 2],
    original: Option<VectorShape>,
}
pub(super) struct Dialog {
    pub key: FormsReadKey,
    pub submitting: bool,
    pub stale: bool,
    pub error: Option<String>,
    page: u32,
    bounds: [f64; 4],
    tool: Tool,
    pending_tool: Option<Tool>,
    shape: Option<VectorShape>,
    stroke: [f32; 3],
    stroked: bool,
    width: f64,
    fill: [f32; 3],
    filled: bool,
    opacity: f64,
    edited: bool,
    overflow: bool,
    gesture: Option<Gesture>,
    snap: bool,
}
impl Dialog {
    pub fn has_changes(&self) -> bool {
        self.edited || self.shape.is_some() || self.gesture.is_some()
    }
    fn request(&self) -> Option<VectorOverlay> {
        if self.pending_tool.is_some()
            || self.overflow
            || self.gesture.is_some()
            || (!self.stroked && !self.filled)
        {
            return None;
        }
        Some(VectorOverlay {
            page: self.page,
            shape: self.shape.clone()?,
            stroke: self.stroked.then_some(VectorStroke {
                color: self.stroke.map(f64::from),
                width: self.width,
            }),
            fill: self.filled.then_some(self.fill.map(f64::from)),
            opacity: self.opacity,
        })
    }
    fn change_tool(&mut self, tool: Tool) {
        if tool == self.tool {
            return;
        }
        if self.shape.is_some() || self.gesture.is_some() {
            self.pending_tool = Some(tool);
        } else {
            self.discard_for_tool(tool);
        }
    }
    fn discard_for_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.pending_tool = None;
        self.shape = None;
        self.gesture = None;
        self.filled = false;
        self.stroked = true;
        self.edited = true;
        self.overflow = false;
        self.error = None;
    }
}
impl EditorApp {
    pub(super) fn open_vector(&mut self) {
        if self.busy || !self.pending_saves.is_empty() || self.has_page_content_dialog() {
            return;
        }
        let Some(tab) = self.selected().filter(|t| t.editable()) else {
            return;
        };
        let Some(page) = tab.status.info.pages.get(tab.page) else {
            return;
        };
        let Some(map) = PageMap::new(
            page,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)),
        ) else {
            return;
        };
        let (id, revision, page) = (tab.id, tab.status.revision, page.number);
        let Some(generation) = self.forms_generation.checked_add(1) else {
            self.message = "Draft request counter exhausted.".into();
            return;
        };
        self.forms_generation = generation;
        self.vector_dialog = Some(Dialog {
            key: FormsReadKey {
                tab: id,
                revision,
                generation,
            },
            page,
            bounds: map.bounds,
            tool: Tool::Rectangle,
            pending_tool: None,
            shape: None,
            stroke: [0.1, 0.25, 0.2],
            stroked: true,
            width: 2.0,
            fill: [0.7, 0.8, 0.75],
            filled: false,
            opacity: 1.0,
            edited: false,
            overflow: false,
            gesture: None,
            snap: false,
            submitting: false,
            stale: false,
            error: None,
        });
    }
    fn vector_current(&self) -> bool {
        self.vector_dialog.as_ref().is_some_and(|d| {
            !d.stale
                && self.active == Some(d.key.tab)
                && self.tabs.iter().any(|t| {
                    t.id == d.key.tab
                        && t.status.revision == d.key.revision
                        && t.page + 1 == d.page as usize
                        && t.editable()
                })
        })
    }
    fn apply_vector(&mut self) -> bool {
        if self.busy
            || self.close_target.is_some()
            || !self.pending_saves.is_empty()
            || !self.vector_current()
        {
            return false;
        }
        let Some(d) = self.vector_dialog.as_ref().filter(|d| !d.submitting) else {
            return false;
        };
        let Some(overlay) = d.request() else {
            return false;
        };
        let key = d.key;
        if !self.send(
            Command::Edit {
                id: key.tab,
                edit: Edit::VectorOverlay { key, overlay },
            },
            "Adding vector artwork locally…",
        ) {
            return false;
        }
        if let Some(d) = self.vector_dialog.as_mut() {
            d.submitting = true;
            d.error = None;
        }
        true
    }
    pub(super) fn vector_dialog_ui(&mut self, ctx: &egui::Context) {
        let current = self.vector_current();
        let controls =
            current && !self.busy && self.pending_saves.is_empty() && self.close_target.is_none();
        let may_discard = self.close_target.is_none();
        if !current && let Some(d) = self.vector_dialog.as_mut() {
            d.stale = true;
        }
        let Some(d) = self.vector_dialog.as_mut() else {
            return;
        };
        let mut apply = false;
        let mut discard = false;
        egui::Window::new("Draw new artwork").anchor(egui::Align2::RIGHT_TOP,egui::vec2(-12.0,88.0)).default_width(285.0).max_width((ctx.content_rect().width()-32.0).max(220.0)).collapsible(false).show(ctx,|ui|{
            ui.label("Drag on the paper to draw. Arrow keys nudge the selected draft. This adds artwork; it does not erase underlying content.");
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-290.0-if d.pending_tool.is_some(){90.0}else{0.0}).max(40.0)).show(ui,|ui|{
                ui.add_enabled_ui(controls&&!d.submitting&&d.pending_tool.is_none(),|ui|{
                    ui.horizontal_wrapped(|ui|{for (tool,label)in [(Tool::Rectangle,"Rectangle"),(Tool::Ellipse,"Ellipse"),(Tool::Line,"Line"),(Tool::Freehand,"Freehand")]{if ui.selectable_label(d.tool==tool,label).clicked(){d.change_tool(tool);}}});
                    d.edited|=ui.checkbox(&mut d.snap,"Snap to 1 PDF unit").changed();
                    if matches!(d.tool,Tool::Rectangle|Tool::Ellipse){d.edited|=ui.checkbox(&mut d.stroked,"Stroke shape").changed();}
                    ui.label("Stroke RGB");d.edited|=ui.color_edit_button_rgb(&mut d.stroke).changed();
                    ui.horizontal(|ui|{ui.label("Stroke width (PDF units)");d.edited|=ui.add(egui::DragValue::new(&mut d.width).range(0.01..=1024.0).speed(0.1)).changed();});
                    if matches!(d.tool,Tool::Rectangle|Tool::Ellipse){d.edited|=ui.checkbox(&mut d.filled,"Fill shape").changed();if d.filled{ui.label("Fill RGB");d.edited|=ui.color_edit_button_rgb(&mut d.fill).changed();}}
                    d.edited|=ui.add(egui::Slider::new(&mut d.opacity,0.01..=1.0).text("Paint opacity")).changed();
                    match d.shape.as_mut(){Some(VectorShape::Rectangle{rectangle})|Some(VectorShape::Ellipse{rectangle})=>{ui.label("PDF rectangle");for (label,v)in ["Left","Bottom","Right","Top"].into_iter().zip(rectangle.iter_mut()){ui.horizontal(|ui|{ui.label(label);d.edited|=ui.add(egui::DragValue::new(v).speed(0.5)).changed();});}},Some(VectorShape::Line{from,to})=>{for(label,v)in ["Start X","Start Y","End X","End Y"].into_iter().zip(from.iter_mut().chain(to.iter_mut())){ui.horizontal(|ui|{ui.label(label);d.edited|=ui.add(egui::DragValue::new(v).speed(0.5)).changed();});}},Some(VectorShape::Polyline{points})=>{ui.label(format!("{} of 4096 freehand points",points.len()));ui.collapsing("PDF point coordinates",|ui|{egui::ScrollArea::vertical().max_height(90.0).show_rows(ui,ui.text_style_height(&egui::TextStyle::Body)+8.0,points.len(),|ui,rows|{for i in rows{ui.push_id(i,|ui|{ui.horizontal(|ui|{ui.label(format!("Point {}",i+1));for (axis,v)in ["X","Y"].into_iter().zip(&mut points[i]){ui.label(axis);d.edited|=ui.add(egui::DragValue::new(v).speed(0.5)).changed();}});});}});});},None=>{ui.label("Draw a shape on the paper first.");}}
                    if ui.button("Start a new drawing").clicked(){d.shape=None;d.gesture=None;d.overflow=false;d.edited=true;d.error=None;}
                });
                if d.stale{ui.label("The document changed. This draft is retained; close and reopen drawing controls before applying.");}
                if let Some(error)=&d.error{ui.colored_label(crate::style::palette(ui.visuals().dark_mode).copper,error);}
            });
            ui.separator();
            if let Some(tool)=d.pending_tool {
                ui.label("Keep the current drawing, or explicitly discard it before switching tools.");
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(may_discard&&!d.submitting,egui::Button::new("Keep current drawing")).clicked(){d.pending_tool=None;d.gesture=None;}
                    if ui.add_enabled(controls&&!d.submitting,egui::Button::new("Discard draft and switch tool")).clicked(){d.discard_for_tool(tool);}
                });
            }
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(controls&&!d.submitting&&d.request().is_some(),egui::Button::new("Add artwork")).clicked(){apply=true;}
                if ui.add_enabled(may_discard&&!d.submitting,egui::Button::new("Discard drawing draft")).clicked(){discard=true;}
            });
        });
        if discard {
            self.vector_dialog = None;
        } else if apply {
            self.apply_vector();
        }
    }
    pub(super) fn vector_canvas(
        &mut self,
        ui: &mut egui::Ui,
        id: u64,
        revision: u64,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
    ) {
        let allowed = ui.is_enabled()
            && !self.busy
            && self.pending_saves.is_empty()
            && self.close_target.is_none()
            && self.vector_current();
        let Some(d) = self.vector_dialog.as_mut().filter(|d| {
            d.key.tab == id && d.key.revision == revision && d.page == page.number && !d.submitting
        }) else {
            return;
        };
        if !allowed {
            d.gesture = None;
            return;
        }
        let Some(map) = PageMap::new(page, image) else {
            return;
        };
        if d.pending_tool.is_none() {
            if d.gesture.as_ref().is_some_and(|g| g.map != map) {
                if let Some(g) = d.gesture.take() {
                    d.shape = g.original;
                }
                d.error = Some("Drawing cancelled because the page view changed.".into());
            }
            let response=ui.interact(image,egui::Id::new(("vector-drawing",id,d.key.generation)),egui::Sense::click_and_drag()).on_hover_text("Drag to create a new shape. Focus the canvas and use arrows to move the draft; Shift moves 10 PDF units, Alt moves 0.1.");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    "New vector artwork canvas",
                )
            });
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if response.clicked() || response.drag_started() {
                response.request_focus();
            }
            let inset = |p: [f64; 2]| {
                let margin = if d.stroked { d.width / 2.0 } else { 0.0 };
                if !margin.is_finite()
                    || margin < 0.0
                    || map.bounds[0] + margin > map.bounds[2] - margin
                    || map.bounds[1] + margin > map.bounds[3] - margin
                {
                    return None;
                }
                Some([
                    p[0].clamp(map.bounds[0] + margin, map.bounds[2] - margin),
                    p[1].clamp(map.bounds[1] + margin, map.bounds[3] - margin),
                ])
            };
            if response.drag_started()
                && let Some(start) = ui
                    .input(|i| i.pointer.press_origin())
                    .and_then(|p| map.inverse(p, d.snap))
                    .and_then(inset)
            {
                d.gesture = Some(Gesture {
                    map,
                    start,
                    original: d.shape.take(),
                });
                d.overflow = false;
                d.error = None;
                d.edited = true;
            }
            if let Some(g) = &d.gesture
                && let Some(p) = response
                    .interact_pointer_pos()
                    .and_then(|p| map.inverse(p, d.snap))
                    .and_then(inset)
            {
                d.shape = Some(match d.tool {
                    Tool::Rectangle => VectorShape::Rectangle {
                        rectangle: [
                            g.start[0].min(p[0]),
                            g.start[1].min(p[1]),
                            g.start[0].max(p[0]),
                            g.start[1].max(p[1]),
                        ],
                    },
                    Tool::Ellipse => VectorShape::Ellipse {
                        rectangle: [
                            g.start[0].min(p[0]),
                            g.start[1].min(p[1]),
                            g.start[0].max(p[0]),
                            g.start[1].max(p[1]),
                        ],
                    },
                    Tool::Line => VectorShape::Line {
                        from: g.start,
                        to: p,
                    },
                    Tool::Freehand => {
                        let mut points = match d.shape.take() {
                            Some(VectorShape::Polyline { points }) => points,
                            _ => vec![g.start],
                        };
                        if points.last().is_some_and(|last| *last != p) {
                            if points.len() < 4096 {
                                points.push(p);
                            } else {
                                d.overflow = true;
                                d.error=Some("Freehand exceeds 4096 points. Start a new drawing; this draft will not be truncated or applied.".into());
                            }
                        }
                        VectorShape::Polyline { points }
                    }
                });
            }
            if d.gesture.is_some()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                && let Some(g) = d.gesture.take()
            {
                d.shape = g.original;
                d.overflow = false;
            }
            if response.drag_stopped() {
                d.gesture = None;
            }
            if response.has_focus() && d.gesture.is_none() {
                let mut delta = egui::Vec2::ZERO;
                ui.input_mut(|i| {
                    let m = i.modifiers;
                    if m.command || m.ctrl {
                        return;
                    }
                    let step = if m.shift {
                        10.0
                    } else if m.alt {
                        0.1
                    } else {
                        1.0
                    };
                    for (key, v) in [
                        (egui::Key::ArrowLeft, egui::vec2(-step, 0.0)),
                        (egui::Key::ArrowRight, egui::vec2(step, 0.0)),
                        (egui::Key::ArrowUp, egui::vec2(0.0, -step)),
                        (egui::Key::ArrowDown, egui::vec2(0.0, step)),
                    ] {
                        if i.consume_key(m, key) {
                            delta = v;
                        }
                    }
                });
                if delta != egui::Vec2::ZERO
                    && let Some(shape) = d.shape.as_mut()
                {
                    let a = map.inverse(image.center(), false);
                    let b = map.inverse(image.center() + delta, false);
                    if let (Some(a), Some(b)) = (a, b) {
                        let scale = (map.point([map.bounds[0] + 1.0, map.bounds[1]])
                            - map.point([map.bounds[0], map.bounds[1]]))
                        .length();
                        if scale.is_finite() && scale > 0.0 {
                            translate(
                                shape,
                                [
                                    (b[0] - a[0]) * f64::from(scale),
                                    (b[1] - a[1]) * f64::from(scale),
                                ],
                            );
                            d.edited = true;
                        }
                    }
                }
            }
        }
        if let Some(shape) = d.shape.as_ref().filter(|s| finite_shape(s)) {
            let mut c = egui::Color32::from_rgb(
                (d.stroke[0] * 255.0) as u8,
                (d.stroke[1] * 255.0) as u8,
                (d.stroke[2] * 255.0) as u8,
            );
            c = c.gamma_multiply(d.opacity as f32);
            let scale = (map.point([d.bounds[0] + 1.0, d.bounds[1]])
                - map.point([d.bounds[0], d.bounds[1]]))
            .length();
            let stroke = if d.stroked {
                egui::Stroke::new((d.width as f32 * scale).max(0.5), c)
            } else {
                egui::Stroke::NONE
            };
            let fill = if d.filled {
                egui::Color32::from_rgb(
                    (d.fill[0] * 255.0) as u8,
                    (d.fill[1] * 255.0) as u8,
                    (d.fill[2] * 255.0) as u8,
                )
                .gamma_multiply(d.opacity as f32)
            } else {
                egui::Color32::TRANSPARENT
            };
            match shape {
                VectorShape::Rectangle { rectangle } => {
                    if let Some(r) = map.rectangle(*rectangle) {
                        ui.painter()
                            .rect(r, 0.0, fill, stroke, egui::StrokeKind::Middle);
                    }
                }
                VectorShape::Ellipse { rectangle } => {
                    if let Some(r) = map.rectangle(*rectangle) {
                        ui.painter().add(egui::Shape::ellipse_filled(
                            r.center(),
                            r.size() / 2.0,
                            fill,
                        ));
                        ui.painter().add(egui::Shape::ellipse_stroke(
                            r.center(),
                            r.size() / 2.0,
                            stroke,
                        ));
                    }
                }
                VectorShape::Line { from, to } => {
                    ui.painter()
                        .line_segment([map.point(*from), map.point(*to)], stroke);
                }
                VectorShape::Polyline { points } => {
                    ui.painter().add(egui::Shape::line(
                        points.iter().map(|p| map.point(*p)).collect(),
                        stroke,
                    ));
                }
            }
        }
    }
}
fn finite_shape(shape: &VectorShape) -> bool {
    match shape {
        VectorShape::Rectangle { rectangle } | VectorShape::Ellipse { rectangle } => {
            rectangle.iter().all(|v| v.is_finite())
        }
        VectorShape::Line { from, to } => from.iter().chain(to).all(|v| v.is_finite()),
        VectorShape::Polyline { points } => points.iter().flatten().all(|v| v.is_finite()),
    }
}
fn translate(shape: &mut VectorShape, delta: [f64; 2]) {
    match shape {
        VectorShape::Rectangle { rectangle } | VectorShape::Ellipse { rectangle } => {
            rectangle[0] += delta[0];
            rectangle[1] += delta[1];
            rectangle[2] += delta[0];
            rectangle[3] += delta[1];
        }
        VectorShape::Line { from, to } => {
            for p in [from, to] {
                p[0] += delta[0];
                p[1] += delta[1];
            }
        }
        VectorShape::Polyline { points } => {
            for p in points {
                p[0] += delta[0];
                p[1] += delta[1];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn app(ctx: &egui::Context, page: &pdf_core::PageInfo) -> EditorApp {
        let mut app = super::super::text_overlay::tests::app(ctx);
        app.overlay_dialog = None;
        app.tabs[0].status.info.pages = vec![page.clone()];
        app.open_vector();
        app
    }
    fn draw(
        app: &mut EditorApp,
        ctx: &egui::Context,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
        events: Vec<egui::Event>,
    ) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.vector_canvas(ui, 1, 0, page, image),
        );
        out.textures_delta.clear();
    }
    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn page(rotation: i64) -> pdf_core::PageInfo {
        pdf_core::PageInfo {
            number: 1,
            media_box: [-10.0, 0.0, 300.0, 200.0],
            crop_box: [20.0, 30.0, 220.0, 130.0],
            rotation,
        }
    }
    fn points(shape: &VectorShape) -> Vec<[f64; 2]> {
        match shape {
            VectorShape::Rectangle { rectangle } | VectorShape::Ellipse { rectangle } => {
                vec![[rectangle[0], rectangle[1]], [rectangle[2], rectangle[3]]]
            }
            VectorShape::Line { from, to } => vec![*from, *to],
            VectorShape::Polyline { points } => points.clone(),
        }
    }
    #[test]
    fn actual_drag_all_tools_crop_rotations_zoom_dpi_and_keyboard_nudges() {
        for rotation in [0, 90, 180, 270] {
            for zoom in [0.75, 1.5] {
                for dpi in [1.0, 2.0] {
                    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::Line, Tool::Freehand] {
                        let ctx = egui::Context::default();
                        ctx.set_pixels_per_point(dpi);
                        let page = page(rotation);
                        let mut app = app(&ctx, &page);
                        app.vector_dialog.as_mut().unwrap().tool = tool;
                        let size = if rotation % 180 == 0 {
                            egui::vec2(200.0, 100.0)
                        } else {
                            egui::vec2(100.0, 200.0)
                        } * zoom;
                        let image = egui::Rect::from_min_size(egui::pos2(200.0, 200.0), size);
                        let map = PageMap::new(&page, image).unwrap();
                        let start = map.point([40.0, 50.0]);
                        let end = map.point([180.0, 110.0]);
                        draw(&mut app, &ctx, &page, image, vec![]);
                        draw(
                            &mut app,
                            &ctx,
                            &page,
                            image,
                            vec![egui::Event::PointerMoved(start), pointer(start, true)],
                        );
                        draw(
                            &mut app,
                            &ctx,
                            &page,
                            image,
                            vec![egui::Event::PointerMoved(end)],
                        );
                        draw(&mut app, &ctx, &page, image, vec![pointer(end, false)]);
                        let before =
                            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap());
                        assert!((before[0][0] - 40.0).abs() < 0.02);
                        assert!((before[0][1] - 50.0).abs() < 0.02);
                        let modifiers = egui::Modifiers {
                            shift: true,
                            ..Default::default()
                        };
                        draw(
                            &mut app,
                            &ctx,
                            &page,
                            image,
                            vec![
                                egui::Event::ModifiersChanged(modifiers),
                                egui::Event::Key {
                                    key: egui::Key::ArrowRight,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers,
                                },
                            ],
                        );
                        let after =
                            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap());
                        let expected = match rotation {
                            0 => [10.0, 0.0],
                            90 => [0.0, 10.0],
                            180 => [-10.0, 0.0],
                            _ => [0.0, -10.0],
                        };
                        for (a, b) in after.iter().zip(&before) {
                            assert!((a[0] - b[0] - expected[0]).abs() < 0.03);
                            assert!((a[1] - b[1] - expected[1]).abs() < 0.03);
                        }
                        assert!(app.needs_exit_guard());
                        assert!(app.has_tab_draft(1));
                    }
                }
            }
        }
    }
    #[test]
    fn stale_busy_pending_readonly_and_freehand_limit_never_apply_or_lose_drafts() {
        let ctx = egui::Context::default();
        let page = page(0);
        let mut app = app(&ctx, &page);
        let d = app.vector_dialog.as_mut().unwrap();
        d.shape = Some(VectorShape::Polyline {
            points: vec![[40.0, 50.0]; 4096],
        });
        d.tool = Tool::Freehand;
        d.overflow = true;
        assert!(!app.apply_vector());
        app.vector_dialog.as_mut().unwrap().overflow = false;
        let preserved = points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap());
        app.vector_dialog.as_mut().unwrap().stale = true;
        assert!(!app.apply_vector());
        let image = egui::Rect::from_min_size(egui::pos2(200.0, 200.0), egui::vec2(200.0, 100.0));
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![
                egui::Event::PointerMoved(egui::pos2(250.0, 250.0)),
                pointer(egui::pos2(250.0, 250.0), true),
            ],
        );
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            preserved
        );
        app.vector_dialog.as_mut().unwrap().stale = false;
        app.busy = true;
        assert!(!app.apply_vector());
        app.busy = false;
        app.pending_saves.insert(
            1,
            crate::ui::recovery::PendingSave::new("pending.pdf".into(), "Recorded attempt".into()),
        );
        assert!(!app.apply_vector());
        app.pending_saves.clear();
        app.active = None;
        assert!(!app.apply_vector());
        app.active = Some(1);
        app.close_target = Some(super::super::CloseTarget::Tab(1));
        assert!(!app.apply_vector());
        app.close_target = None;
        app.tabs[0].status.info.signature_fields = 1;
        assert!(!app.apply_vector());
        app.vector_dialog = None;
        app.open_vector();
        assert!(app.vector_dialog.is_none());
    }
    #[test]
    fn closing_loaded_tab_retires_its_empty_vector_dialog() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.pdf");
        std::fs::write(
            &path,
            include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
        )
        .unwrap();
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        assert!(app.begin_open(path));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while (app.tabs.is_empty() || app.busy) && std::time::Instant::now() < deadline {
            app.poll(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(app.tabs.len(), 1, "{}", app.message);
        assert!(!app.busy, "{}", app.message);
        let id = app.tabs[0].id;
        app.open_vector();
        assert!(
            app.vector_dialog.is_some(),
            "busy={}, pending={}, active={:?}, info={:?}",
            app.busy,
            app.pending_saves.len(),
            app.active,
            app.tabs[0].status.info
        );
        assert!(!app.vector_dialog.as_ref().unwrap().has_changes());
        app.request_close(id);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !app.tabs.is_empty() && std::time::Instant::now() < deadline {
            app.poll(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.tabs.is_empty(), "{}", app.message);
        assert!(app.vector_dialog.is_none());
    }
    #[test]
    fn real_freehand_overflow_and_view_change_cancel_without_truncation() {
        let ctx = egui::Context::default();
        let page = page(0);
        let mut app = app(&ctx, &page);
        app.vector_dialog.as_mut().unwrap().tool = Tool::Freehand;
        let image = egui::Rect::from_min_size(egui::pos2(200.0, 200.0), egui::vec2(200.0, 100.0));
        draw(&mut app, &ctx, &page, image, vec![]);
        let start = egui::pos2(240.0, 250.0);
        let end = egui::pos2(260.0, 260.0);
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(end)],
        );
        if let Some(VectorShape::Polyline { points }) =
            &mut app.vector_dialog.as_mut().unwrap().shape
        {
            points.resize(4096, [60.0, 60.0]);
        } else {
            panic!("actual freehand did not start");
        }
        let beyond = egui::pos2(270.0, 270.0);
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(beyond)],
        );
        let d = app.vector_dialog.as_ref().unwrap();
        assert!(d.overflow);
        assert_eq!(points(d.shape.as_ref().unwrap()).len(), 4096);
        assert!(d.error.as_ref().unwrap().contains("not be truncated"));
        draw(&mut app, &ctx, &page, image, vec![pointer(beyond, false)]);
        assert!(!app.apply_vector());
        let original = VectorShape::Rectangle {
            rectangle: [40.0, 50.0, 100.0, 90.0],
        };
        let d = app.vector_dialog.as_mut().unwrap();
        d.tool = Tool::Rectangle;
        d.shape = Some(original.clone());
        d.overflow = false;
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(end)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image.translate(egui::vec2(20.0, 0.0)),
            vec![],
        );
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original)
        );
        assert!(app.vector_dialog.as_ref().unwrap().gesture.is_none());
    }
    #[test]
    fn tool_change_requires_real_confirmation_and_cancellation_keeps_geometry() {
        let ctx = egui::Context::default();
        let page = page(0);
        let mut app = app(&ctx, &page);
        let original = VectorShape::Rectangle {
            rectangle: [40.0, 50.0, 100.0, 90.0],
        };
        app.vector_dialog.as_mut().unwrap().shape = Some(original.clone());
        let render = |app: &mut EditorApp, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 420.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_| app.vector_dialog_ui(&ctx),
            );
            out.textures_delta.clear();
            out
        };
        let button = |out: &egui::FullOutput, label: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        let r = t.galley.rect.translate(t.pos.to_vec2());
                        assert!(
                            s.clip_rect.contains_rect(r),
                            "clipped decision button {label}: {r:?}"
                        );
                        assert!(r.bottom() <= 420.0);
                        Some(r.center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing actual button {label}"))
        };
        let click = |app: &mut EditorApp, label: &str| {
            render(app, vec![]);
            let out = render(app, vec![]);
            let p = button(&out, label);
            render(app, vec![egui::Event::PointerMoved(p), pointer(p, true)]);
            render(app, vec![pointer(p, false)]);
        };
        click(&mut app, "Ellipse");
        assert!(
            matches!(app.vector_dialog.as_ref().unwrap().tool, Tool::Rectangle),
            "tool changed before a discard decision"
        );
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original),
            "unapplied geometry lost before confirmation"
        );
        assert!(
            !app.apply_vector(),
            "Apply must be blocked during the tool-change decision"
        );
        let image = egui::Rect::from_min_size(egui::pos2(30.0, 100.0), egui::vec2(200.0, 100.0));
        let expected = PageMap::new(&page, image)
            .unwrap()
            .rectangle([40.0, 50.0, 100.0, 90.0])
            .unwrap();
        let mut preview = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                ..Default::default()
            },
            |ui| app.vector_canvas(ui, 1, 0, &page, image),
        );
        preview.textures_delta.clear();
        assert!(
            preview
                .shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Rect(r)if r.rect==expected)),
            "current draft outline disappeared during the decision"
        );
        let a = egui::pos2(80.0, 120.0);
        let b = egui::pos2(160.0, 180.0);
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(a), pointer(a, true)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(b)],
        );
        draw(&mut app, &ctx, &page, image, vec![pointer(b, false)]);
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original)
        );
        click(&mut app, "Keep current drawing");
        assert!(matches!(
            app.vector_dialog.as_ref().unwrap().tool,
            Tool::Rectangle
        ));
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original)
        );
        click(&mut app, "Ellipse");
        app.busy = true;
        click(&mut app, "Discard draft and switch tool");
        assert!(matches!(
            app.vector_dialog.as_ref().unwrap().tool,
            Tool::Rectangle
        ));
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original)
        );
        app.busy = false;
        app.vector_dialog.as_mut().unwrap().stale = true;
        click(&mut app, "Discard draft and switch tool");
        assert!(matches!(
            app.vector_dialog.as_ref().unwrap().tool,
            Tool::Rectangle
        ));
        assert_eq!(
            points(app.vector_dialog.as_ref().unwrap().shape.as_ref().unwrap()),
            points(&original)
        );
        app.vector_dialog.as_mut().unwrap().stale = false;
        click(&mut app, "Discard draft and switch tool");
        assert!(matches!(
            app.vector_dialog.as_ref().unwrap().tool,
            Tool::Ellipse
        ));
        assert!(app.vector_dialog.as_ref().unwrap().shape.is_none());
        assert!(app.has_tab_draft(1));
    }
    #[test]
    fn real_properties_buttons_and_small_window_footer() {
        let ctx = egui::Context::default();
        let page = page(0);
        let mut app = app(&ctx, &page);
        let mut run = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 420.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_| app.vector_dialog_ui(&ctx),
            );
            out.textures_delta.clear();
            out
        };
        run(vec![]);
        let out = run(vec![]);
        let rect = |out: &egui::FullOutput, label: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.galley.rect.translate(t.pos.to_vec2()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing {label}; text: {:?}",
                        out.shapes
                            .iter()
                            .filter_map(|s| match &s.shape {
                                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    )
                })
        };
        let ellipse = rect(&out, "Ellipse").center();
        run(vec![
            egui::Event::PointerMoved(ellipse),
            pointer(ellipse, true),
        ]);
        run(vec![pointer(ellipse, false)]);
        assert!(matches!(
            app.vector_dialog.as_ref().unwrap().tool,
            Tool::Ellipse
        ));
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 420.0),
                )),
                ..Default::default()
            },
            |_| app.vector_dialog_ui(&ctx),
        );
        out.textures_delta.clear();
        for label in ["Add artwork", "Discard drawing draft"] {
            let r = rect(&out, label);
            assert!(
                r.top() >= 0.0 && r.bottom() <= 420.0,
                "footer clipped {label}: {r:?}"
            );
        }
        assert!(app.needs_exit_guard());
        app.request_close(1);
        assert!(app.close_target.is_some());
    }
}
