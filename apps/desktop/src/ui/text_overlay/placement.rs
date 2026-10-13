use super::EditorApp;
use eframe::egui;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::ui) struct PageMap {
    pub(in crate::ui) bounds: [f64; 4],
    rotation: i64,
    image: egui::Rect,
}
impl PageMap {
    pub fn new(page: &pdf_core::PageInfo, image: egui::Rect) -> Option<Self> {
        let bounds = [
            f64::from(page.media_box[0].max(page.crop_box[0])),
            f64::from(page.media_box[1].max(page.crop_box[1])),
            f64::from(page.media_box[2].min(page.crop_box[2])),
            f64::from(page.media_box[3].min(page.crop_box[3])),
        ];
        let rotation = page.rotation.rem_euclid(360);
        (page
            .media_box
            .iter()
            .chain(&page.crop_box)
            .all(|v| v.is_finite())
            && bounds.iter().all(|v| v.is_finite())
            && bounds[2] > bounds[0]
            && bounds[3] > bounds[1]
            && image.is_finite()
            && image.width().is_finite()
            && image.height().is_finite()
            && image.width() > 0.0
            && image.height() > 0.0
            && matches!(rotation, 0 | 90 | 180 | 270))
        .then_some(Self {
            bounds,
            rotation,
            image,
        })
    }
    pub fn point(&self, p: [f64; 2]) -> egui::Pos2 {
        let u = (p[0] - self.bounds[0]) / (self.bounds[2] - self.bounds[0]);
        let v = (p[1] - self.bounds[1]) / (self.bounds[3] - self.bounds[1]);
        let (u, v) = match self.rotation {
            0 => (u, 1.0 - v),
            90 => (v, u),
            180 => (1.0 - u, v),
            _ => (1.0 - v, 1.0 - u),
        };
        self.image.min
            + egui::vec2(
                u as f32 * self.image.width(),
                v as f32 * self.image.height(),
            )
    }
    pub fn inverse(&self, p: egui::Pos2, snap: bool) -> Option<[f64; 2]> {
        if !p.is_finite() {
            return None;
        }
        let p = self.image.clamp(p);
        let u = f64::from((p.x - self.image.left()) / self.image.width());
        let v = f64::from((p.y - self.image.top()) / self.image.height());
        let (u, v) = match self.rotation {
            0 => (u, 1.0 - v),
            90 => (v, u),
            180 => (1.0 - u, v),
            _ => (1.0 - v, 1.0 - u),
        };
        let mut point = [
            self.bounds[0] + u * (self.bounds[2] - self.bounds[0]),
            self.bounds[1] + v * (self.bounds[3] - self.bounds[1]),
        ];
        if snap {
            point = point.map(f64::round);
        }
        Some([
            point[0].clamp(self.bounds[0], self.bounds[2]),
            point[1].clamp(self.bounds[1], self.bounds[3]),
        ])
    }
    pub fn rectangle(&self, r: [f64; 4]) -> Option<egui::Rect> {
        if r.iter().any(|v| !v.is_finite())
            || r[0] >= r[2]
            || r[1] >= r[3]
            || r[0] < self.bounds[0]
            || r[1] < self.bounds[1]
            || r[2] > self.bounds[2]
            || r[3] > self.bounds[3]
        {
            return None;
        }
        Some(egui::Rect::from_two_pos(
            self.point([r[0], r[1]]),
            self.point([r[2], r[3]]),
        ))
    }
    fn moved(&self, r: [f64; 4], delta: [f64; 2]) -> [f64; 4] {
        let dx = delta[0].clamp(self.bounds[0] - r[0], self.bounds[2] - r[2]);
        let dy = delta[1].clamp(self.bounds[1] - r[1], self.bounds[3] - r[3]);
        [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy]
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Create,
    Move,
    Resize(Option<usize>, Option<usize>),
}
pub(super) struct Gesture {
    map: PageMap,
    kind: Kind,
    start: [f64; 2],
    original: [f64; 4],
}
fn handles(r: [f64; 4]) -> [(Option<usize>, Option<usize>, [f64; 2]); 8] {
    [
        (Some(0), Some(1), [r[0], r[1]]),
        (Some(2), Some(1), [r[2], r[1]]),
        (Some(0), Some(3), [r[0], r[3]]),
        (Some(2), Some(3), [r[2], r[3]]),
        (Some(0), None, [r[0], (r[1] + r[3]) / 2.0]),
        (Some(2), None, [r[2], (r[1] + r[3]) / 2.0]),
        (None, Some(1), [(r[0] + r[2]) / 2.0, r[1]]),
        (None, Some(3), [(r[0] + r[2]) / 2.0, r[3]]),
    ]
}
impl Gesture {
    fn update(&self, p: [f64; 2]) -> [f64; 4] {
        let min_w = (self.map.bounds[2] - self.map.bounds[0]).min(1.0);
        let min_h = (self.map.bounds[3] - self.map.bounds[1]).min(1.0);
        match self.kind {
            Kind::Move => self
                .map
                .moved(self.original, [p[0] - self.start[0], p[1] - self.start[1]]),
            Kind::Create => {
                let mut r = [
                    self.start[0].min(p[0]),
                    self.start[1].min(p[1]),
                    self.start[0].max(p[0]),
                    self.start[1].max(p[1]),
                ];
                if r[2] - r[0] < min_w {
                    r[2] = (r[0] + min_w).min(self.map.bounds[2]);
                    r[0] = r[2] - min_w;
                }
                if r[3] - r[1] < min_h {
                    r[3] = (r[1] + min_h).min(self.map.bounds[3]);
                    r[1] = r[3] - min_h;
                }
                r
            }
            Kind::Resize(x, y) => {
                let mut r = self.original;
                let min_w = min_w.min(r[2] - r[0]);
                let min_h = min_h.min(r[3] - r[1]);
                if let Some(index) = x {
                    r[index] = if index == 0 {
                        p[0].clamp(self.map.bounds[0], r[2] - min_w)
                    } else {
                        p[0].clamp(r[0] + min_w, self.map.bounds[2])
                    };
                }
                if let Some(index) = y {
                    r[index] = if index == 1 {
                        p[1].clamp(self.map.bounds[1], r[3] - min_h)
                    } else {
                        p[1].clamp(r[1] + min_h, self.map.bounds[3])
                    };
                }
                r
            }
        }
    }
}
impl EditorApp {
    pub(in crate::ui) fn overlay_canvas(
        &mut self,
        ui: &mut egui::Ui,
        tab: u64,
        revision: u64,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
    ) {
        let allowed = ui.is_enabled()
            && !self.busy
            && self.pending_saves.is_empty()
            && self.close_target.is_none()
            && self.overlay_current();
        let Some(d) = self.overlay_dialog.as_mut().filter(|d| {
            d.key.tab == tab
                && d.key.revision == revision
                && d.request.page == page.number
                && !d.submitting
        }) else {
            return;
        };
        if !allowed {
            d.gesture = None;
            return;
        }
        let Some(map) = PageMap::new(page, image) else {
            d.gesture = None;
            return;
        };
        if d.gesture.as_ref().is_some_and(|g| g.map != map) {
            if let Some(g) = d.gesture.take() {
                d.request.rectangle = g.original;
            }
            d.error = Some("Placement cancelled because the page view changed.".into());
        }
        let response=ui.interact(image,egui::Id::new(("overlay-placement",tab,d.key.generation)),egui::Sense::click_and_drag()).on_hover_text("Drag outside the rectangle to place it; drag inside to move; drag a handle to resize. Focus this canvas and use arrow keys to nudge.");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                true,
                "New text overlay placement canvas",
            )
        });
        if response.hovered()
            && let Some(pointer) = response.hover_pos()
        {
            let rectangle = map.rectangle(d.request.rectangle);
            let handle = rectangle.and_then(|_| {
                handles(d.request.rectangle)
                    .into_iter()
                    .find(|(_, _, p)| map.point(*p).distance(pointer) <= 8.0)
            });
            let cursor = match handle {
                Some((Some(_), Some(_), p)) => {
                    let center = rectangle.map_or(image.center(), |r| r.center());
                    let point = map.point(p);
                    if (point.x - center.x) * (point.y - center.y) > 0.0 {
                        egui::CursorIcon::ResizeNwSe
                    } else {
                        egui::CursorIcon::ResizeNeSw
                    }
                }
                Some((Some(_), None, _)) => {
                    if map.rotation % 180 == 0 {
                        egui::CursorIcon::ResizeHorizontal
                    } else {
                        egui::CursorIcon::ResizeVertical
                    }
                }
                Some((None, Some(_), _)) => {
                    if map.rotation % 180 == 0 {
                        egui::CursorIcon::ResizeVertical
                    } else {
                        egui::CursorIcon::ResizeHorizontal
                    }
                }
                _ => {
                    if rectangle.is_some_and(|r| r.contains(pointer)) {
                        egui::CursorIcon::Grab
                    } else {
                        egui::CursorIcon::Crosshair
                    }
                }
            };
            ui.ctx().set_cursor_icon(cursor);
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.drag_started()
            && let Some(start) = ui.input(|i| i.pointer.press_origin())
        {
            let kind = if let Some(rect) = d.placement(page, image) {
                handles(d.request.rectangle)
                    .into_iter()
                    .find_map(|(x, y, p)| {
                        (map.point(p).distance(start) <= 8.0).then_some(Kind::Resize(x, y))
                    })
                    .unwrap_or_else(|| {
                        if rect.contains(start) {
                            Kind::Move
                        } else {
                            Kind::Create
                        }
                    })
            } else {
                Kind::Create
            };
            if let Some(start) = map.inverse(start, d.snap) {
                d.gesture = Some(Gesture {
                    map,
                    kind,
                    start,
                    original: d.request.rectangle,
                });
            }
        }
        if let Some(g) = d.gesture.as_ref()
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(point) = map.inverse(pointer, d.snap)
        {
            d.request.rectangle = g.update(point);
            d.error = None;
        }
        // Egui clears widget focus on Escape before this response is read.
        if d.gesture.is_some()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            && let Some(g) = d.gesture.take()
        {
            d.request.rectangle = g.original;
        }
        if response.has_focus() {
            let mut delta = [0.0, 0.0];
            ui.input_mut(|input| {
                let modifiers = input.modifiers;
                if modifiers.command || modifiers.ctrl {
                    return;
                }
                let step = if modifiers.shift {
                    10.0
                } else if modifiers.alt {
                    0.1
                } else {
                    1.0
                };
                for (key, dx, dy) in [
                    (egui::Key::ArrowLeft, -1.0, 0.0),
                    (egui::Key::ArrowRight, 1.0, 0.0),
                    (egui::Key::ArrowUp, 0.0, -1.0),
                    (egui::Key::ArrowDown, 0.0, 1.0),
                ] {
                    if input.consume_key(modifiers, key) {
                        delta = [dx * step, dy * step];
                    }
                }
            });
            if delta != [0.0, 0.0]
                && d.gesture.is_none()
                && map.rectangle(d.request.rectangle).is_some()
            {
                let [x, y] = delta;
                let delta = match map.rotation {
                    0 => [x, -y],
                    90 => [y, x],
                    180 => [-x, y],
                    _ => [-y, -x],
                };
                d.request.rectangle = map.moved(d.request.rectangle, delta);
                d.error = None;
            }
        }
        if response.drag_stopped() {
            d.gesture = None;
        }
        if let Some(rect) = map.rectangle(d.request.rectangle) {
            let colors = crate::style::palette(ui.visuals().dark_mode);
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.5, colors.copper),
                egui::StrokeKind::Inside,
            );
            for (_, _, point) in handles(d.request.rectangle) {
                ui.painter().rect_filled(
                    egui::Rect::from_center_size(map.point(point), egui::vec2(7.0, 7.0)),
                    1.0,
                    colors.copper,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::super::tests::app;
    use super::*;
    fn page(rotation: i64) -> pdf_core::PageInfo {
        pdf_core::PageInfo {
            number: 1,
            media_box: [-10.0, 0.0, 300.0, 200.0],
            crop_box: [20.0, 30.0, 220.0, 130.0],
            rotation,
        }
    }
    fn draw(
        app: &mut EditorApp,
        ctx: &egui::Context,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(egui::Modifiers::NONE);
        let events = std::iter::once(egui::Event::ModifiersChanged(modifiers))
            .chain(events)
            .collect();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.overlay_canvas(ui, 1, 0, page, image),
        );
        output.textures_delta.clear();
        output
    }
    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn drag(
        app: &mut EditorApp,
        ctx: &egui::Context,
        page: &pdf_core::PageInfo,
        image: egui::Rect,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        draw(app, ctx, page, image, vec![]);
        draw(
            app,
            ctx,
            page,
            image,
            vec![egui::Event::PointerMoved(from), pointer(from, true)],
        );
        draw(app, ctx, page, image, vec![egui::Event::PointerMoved(to)]);
        draw(app, ctx, page, image, vec![pointer(to, false)]);
    }
    fn assert_rect(actual: [f64; 4], expected: [f64; 4]) {
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 0.01, "{actual:?} vs {expected:?}");
        }
    }
    #[test]
    fn actual_pointer_create_move_resize_and_arrow_nudges_all_rotations_zoom_and_dpi() {
        for rotation in [0, 90, 180, 270] {
            for zoom in [0.75, 1.5] {
                for dpi in [1.0, 2.0] {
                    let ctx = egui::Context::default();
                    ctx.set_pixels_per_point(dpi);
                    let mut app = app(&ctx);
                    let page = page(rotation);
                    app.tabs[0].status.info.pages[0] = page.clone();
                    let image = egui::Rect::from_min_size(
                        egui::pos2(60.0, 80.0),
                        if rotation % 180 == 0 {
                            egui::vec2(400.0 * zoom, 200.0 * zoom)
                        } else {
                            egui::vec2(200.0 * zoom, 400.0 * zoom)
                        },
                    );
                    let map = PageMap::new(&page, image).unwrap();
                    app.overlay_dialog.as_mut().unwrap().request.rectangle =
                        [160.0, 90.0, 210.0, 120.0];
                    drag(
                        &mut app,
                        &ctx,
                        &page,
                        image,
                        map.point([30.0, 40.0]),
                        map.point([100.0, 80.0]),
                    );
                    assert_rect(
                        app.overlay_dialog.as_ref().unwrap().request.rectangle,
                        [30.0, 40.0, 100.0, 80.0],
                    );
                    drag(
                        &mut app,
                        &ctx,
                        &page,
                        image,
                        map.point([65.0, 60.0]),
                        map.point([75.0, 65.0]),
                    );
                    assert_rect(
                        app.overlay_dialog.as_ref().unwrap().request.rectangle,
                        [40.0, 45.0, 110.0, 85.0],
                    );
                    drag(
                        &mut app,
                        &ctx,
                        &page,
                        image,
                        map.point([110.0, 85.0]),
                        map.point([130.0, 100.0]),
                    );
                    assert_rect(
                        app.overlay_dialog.as_ref().unwrap().request.rectangle,
                        [40.0, 45.0, 130.0, 100.0],
                    );
                    let before = app.overlay_dialog.as_ref().unwrap().request.rectangle;
                    draw(
                        &mut app,
                        &ctx,
                        &page,
                        image,
                        vec![egui::Event::Key {
                            key: egui::Key::ArrowRight,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                    let delta = match rotation {
                        0 => [1.0, 0.0],
                        90 => [0.0, 1.0],
                        180 => [-1.0, 0.0],
                        _ => [0.0, -1.0],
                    };
                    assert_rect(
                        app.overlay_dialog.as_ref().unwrap().request.rectangle,
                        map.moved(before, delta),
                    );
                    assert!(app.needs_exit_guard());
                }
            }
        }
    }
    #[test]
    fn inverse_snap_edges_cancel_and_stale_guards_preserve_drafts() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let page = page(90);
        app.tabs[0].status.info.pages[0] = page.clone();
        let image = egui::Rect::from_min_size(egui::pos2(40.0, 60.0), egui::vec2(200.0, 400.0));
        let map = PageMap::new(&page, image).unwrap();
        app.overlay_dialog.as_mut().unwrap().request.value = "Unapplied Unicode é".into();
        app.overlay_dialog.as_mut().unwrap().request.rectangle = [160.0, 90.0, 210.0, 120.0];
        app.overlay_dialog.as_mut().unwrap().snap = true;
        drag(
            &mut app,
            &ctx,
            &page,
            image,
            map.point([30.3, 40.3]),
            map.point([99.7, 80.4]),
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            [30.0, 40.0, 100.0, 80.0],
        );
        drag(
            &mut app,
            &ctx,
            &page,
            image,
            map.point([65.0, 60.0]),
            map.point([999.0, 999.0]),
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            [150.0, 90.0, 220.0, 130.0],
        );
        let before = app.overlay_dialog.as_ref().unwrap().request.rectangle;
        let from = map.point([185.0, 110.0]);
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(from), pointer(from, true)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(map.point([170.0, 100.0]))],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            before,
        );
        draw(&mut app, &ctx, &page, image, vec![pointer(from, false)]);
        app.tabs[0].status.info.signature_fields = 1;
        drag(
            &mut app,
            &ctx,
            &page,
            image,
            map.point([25.0, 35.0]),
            map.point([80.0, 70.0]),
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            before,
        );
        assert_eq!(
            app.overlay_dialog.as_ref().unwrap().request.value,
            "Unapplied Unicode é"
        );
        assert!(map.inverse(egui::pos2(f32::NAN, 0.0), false).is_none());
        for p in [[20.0, 30.0], [40.3, 60.7], [220.0, 130.0]] {
            let inverse = map.inverse(map.point(p), false).unwrap();
            assert!((inverse[0] - p[0]).abs() < 0.001 && (inverse[1] - p[1]).abs() < 0.001);
        }
    }
    #[test]
    fn actual_pointer_rectangle_applies_through_existing_child_history_and_verified_save() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("cropped-rotated.pdf");
        let font = temp.path().join("font.ttf");
        let output = temp.path().join("placed.pdf");
        let mut source = lopdf::Document::load_mem(include_bytes!(
            "../../../../../tests/fixtures/pdfs/forms.pdf"
        ))
        .unwrap();
        let id = source.get_pages()[&1];
        let page = source.get_dictionary_mut(id).unwrap();
        page.set(
            "CropBox",
            vec![20.into(), 30.into(), 220.into(), 130.into()],
        );
        page.set("Rotate", 90);
        source.save(&input).unwrap();
        let original = std::fs::read(&input).unwrap();
        std::fs::write(
            &font,
            include_bytes!("../../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
        )
        .unwrap();
        let ctx = egui::Context::default();
        let mut app = EditorApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.send(
            crate::worker::Command::Load {
                id: 1,
                path: input.clone(),
            },
            "Loading fixture…",
        );
        let wait = |app: &mut EditorApp, ready: fn(&EditorApp) -> bool| {
            let start = std::time::Instant::now();
            while !ready(app) {
                app.poll(&ctx);
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(10),
                    "actor completion: {}",
                    app.message
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        wait(&mut app, |a| !a.busy && a.tabs.len() == 1);
        app.open_text_overlay();
        let page = app.tabs[0].status.info.pages[0].clone();
        let image = egui::Rect::from_min_size(egui::pos2(80.0, 90.0), egui::vec2(200.0, 400.0));
        let map = PageMap::new(&page, image).unwrap();
        drag(
            &mut app,
            &ctx,
            &page,
            image,
            map.point([30.0, 40.0]),
            map.point([180.0, 110.0]),
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            [30.0, 40.0, 180.0, 110.0],
        );
        let dialog = app.overlay_dialog.as_mut().unwrap();
        dialog.request.value = "Placed overlay".into();
        dialog.font_file = Some(font);
        assert!(app.apply_text_overlay());
        wait(&mut app, |a| !a.busy && a.overlay_dialog.is_none());
        assert!(app.tabs[0].status.dirty && app.tabs[0].status.can_undo);
        app.send(
            crate::worker::Command::Edit {
                id: 1,
                edit: crate::worker::Edit::Undo,
            },
            "Undo",
        );
        wait(&mut app, |a| !a.busy && !a.tabs[0].status.dirty);
        app.send(
            crate::worker::Command::Edit {
                id: 1,
                edit: crate::worker::Edit::Redo,
            },
            "Redo",
        );
        wait(&mut app, |a| !a.busy && a.tabs[0].status.dirty);
        app.send(
            crate::worker::Command::Save {
                id: 1,
                path: output.clone(),
            },
            "Save",
        );
        wait(&mut app, |a| !a.busy && !a.tabs[0].status.dirty);
        let pdf = pdf_core::PdfDocument::open(&output, None).unwrap();
        assert!(pdf.extract_text(&[1]).unwrap().contains("Placed overlay"));
        assert_eq!(
            pdf.info().unwrap().pages,
            pdf_core::PdfDocument::from_bytes(&original, None)
                .unwrap()
                .info()
                .unwrap()
                .pages
        );
        assert_eq!(std::fs::read(input).unwrap(), original);
    }
    #[test]
    fn edge_handles_modifier_nudges_and_locked_operations_are_real() {
        for rotation in [0, 90, 180, 270] {
            let ctx = egui::Context::default();
            let mut app = app(&ctx);
            let page = page(rotation);
            app.tabs[0].status.info.pages[0] = page.clone();
            let image = egui::Rect::from_min_size(egui::pos2(40.0, 60.0), egui::vec2(400.0, 300.0));
            let map = PageMap::new(&page, image).unwrap();
            for (from, to, expected) in [
                ([40.0, 70.0], [30.0, 70.0], [30.0, 45.0, 130.0, 95.0]),
                ([130.0, 70.0], [150.0, 70.0], [40.0, 45.0, 150.0, 95.0]),
                ([85.0, 45.0], [85.0, 35.0], [40.0, 35.0, 130.0, 95.0]),
                ([85.0, 95.0], [85.0, 115.0], [40.0, 45.0, 130.0, 115.0]),
            ] {
                app.overlay_dialog.as_mut().unwrap().request.rectangle = [40.0, 45.0, 130.0, 95.0];
                drag(&mut app, &ctx, &page, image, map.point(from), map.point(to));
                assert_rect(
                    app.overlay_dialog.as_ref().unwrap().request.rectangle,
                    expected,
                );
            }
            for (modifiers, step) in [(egui::Modifiers::SHIFT, 10.0), (egui::Modifiers::ALT, 0.1)] {
                app.overlay_dialog.as_mut().unwrap().request.rectangle = [40.0, 45.0, 130.0, 95.0];
                draw(
                    &mut app,
                    &ctx,
                    &page,
                    image,
                    vec![egui::Event::Key {
                        key: egui::Key::ArrowDown,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                );
                let delta = match rotation {
                    0 => [0.0, -step],
                    90 => [step, 0.0],
                    180 => [0.0, step],
                    _ => [-step, 0.0],
                };
                assert_rect(
                    app.overlay_dialog.as_ref().unwrap().request.rectangle,
                    map.moved([40.0, 45.0, 130.0, 95.0], delta),
                );
            }
            let before = app.overlay_dialog.as_ref().unwrap().request.rectangle;
            app.overlay_dialog.as_mut().unwrap().stale = true;
            drag(
                &mut app,
                &ctx,
                &page,
                image,
                map.point([25.0, 35.0]),
                map.point([80.0, 70.0]),
            );
            assert_rect(
                app.overlay_dialog.as_ref().unwrap().request.rectangle,
                before,
            );
            app.overlay_dialog.as_mut().unwrap().stale = false;
            app.tabs[0].status.revision = 1;
            drag(
                &mut app,
                &ctx,
                &page,
                image,
                map.point([25.0, 35.0]),
                map.point([80.0, 70.0]),
            );
            assert_rect(
                app.overlay_dialog.as_ref().unwrap().request.rectangle,
                before,
            );
            app.tabs[0].status.revision = 0;
            app.pending_saves.insert(
                1,
                crate::ui::recovery::PendingSave::new("pending.pdf".into(), "Pending".into()),
            );
            drag(
                &mut app,
                &ctx,
                &page,
                image,
                map.point([25.0, 35.0]),
                map.point([80.0, 70.0]),
            );
            assert_rect(
                app.overlay_dialog.as_ref().unwrap().request.rectangle,
                before,
            );
        }
    }
    #[test]
    fn real_workstation_keeps_compact_toolbar_and_zoom_footer_accessible() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.tabs[0].texture = Some(ctx.load_texture(
            "layout-only page",
            egui::ColorImage::new([595, 842], vec![egui::Color32::WHITE; 595 * 842]),
            egui::TextureOptions::LINEAR,
        ));
        app.overlay_dialog.as_mut().unwrap().canvas_mode = true;
        let draw = |app: &mut EditorApp, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    app.workstation(ui);
                    app.text_overlay_dialog(&ctx);
                },
            );
            output.textures_delta.clear();
            output
        };
        draw(&mut app, vec![]);
        let output = draw(&mut app, vec![]);
        let position = |label: &str| {
            output
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
                .expect("fully visible real workstation control")
        };
        position("Show text controls");
        position("Discard draft");
        let zoom = position("Zoom in");
        draw(
            &mut app,
            vec![egui::Event::PointerMoved(zoom), pointer(zoom, true)],
        );
        draw(&mut app, vec![pointer(zoom, false)]);
        assert!((app.tabs[0].scale - 1.25).abs() < 0.01);
        assert!(
            app.overlay_dialog.is_some(),
            "footer zoom must not create/discard an overlay rectangle"
        );
    }
    #[test]
    fn view_change_cancels_live_gesture_and_nonfinite_maps_are_refused() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        let mut page = page(0);
        app.tabs[0].status.info.pages[0] = page.clone();
        let image = egui::Rect::from_min_size(egui::pos2(40.0, 60.0), egui::vec2(400.0, 200.0));
        let map = PageMap::new(&page, image).unwrap();
        let original = [40.0, 45.0, 130.0, 95.0];
        app.overlay_dialog.as_mut().unwrap().request.rectangle = original;
        let from = map.point([85.0, 70.0]);
        draw(&mut app, &ctx, &page, image, vec![]);
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(from), pointer(from, true)],
        );
        draw(
            &mut app,
            &ctx,
            &page,
            image,
            vec![egui::Event::PointerMoved(map.point([95.0, 80.0]))],
        );
        assert!(app.overlay_dialog.as_ref().unwrap().gesture.is_some());
        draw(
            &mut app,
            &ctx,
            &page,
            image.translate(egui::vec2(20.0, 0.0)),
            vec![],
        );
        assert_rect(
            app.overlay_dialog.as_ref().unwrap().request.rectangle,
            original,
        );
        assert!(app.overlay_dialog.as_ref().unwrap().gesture.is_none());
        page.crop_box[0] = f32::NAN;
        assert!(PageMap::new(&page, image).is_none());
        page = super::tests::page(45);
        assert!(PageMap::new(&page, image).is_none());
        let infinite_size = egui::Rect::from_min_max(
            egui::pos2(-f32::MAX, -f32::MAX),
            egui::pos2(f32::MAX, f32::MAX),
        );
        assert!(PageMap::new(&super::tests::page(0), infinite_size).is_none());
    }
}
