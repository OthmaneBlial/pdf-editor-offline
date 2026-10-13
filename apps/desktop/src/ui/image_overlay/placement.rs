use super::{EditorApp, PageMap};
use eframe::egui;
#[derive(Clone, Copy)]
enum Kind {
    Create,
    Move,
    Resize(usize, usize),
}
pub(super) struct Gesture {
    map: PageMap,
    origin: [f64; 2],
    original: [f64; 4],
    kind: Kind,
}
fn moved(rectangle: [f64; 4], delta: [f64; 2], bounds: [f64; 4]) -> [f64; 4] {
    let x = delta[0].clamp(bounds[0] - rectangle[0], bounds[2] - rectangle[2]);
    let y = delta[1].clamp(bounds[1] - rectangle[1], bounds[3] - rectangle[3]);
    [
        rectangle[0] + x,
        rectangle[1] + y,
        rectangle[2] + x,
        rectangle[3] + y,
    ]
}
impl EditorApp {
    pub(in crate::ui) fn image_canvas(
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
            && self.image_current();
        let Some(dialog) = self.image_dialog.as_mut().filter(|dialog| {
            dialog.key.tab == tab
                && dialog.key.revision == revision
                && dialog.request.page == page.number
                && !dialog.submitting
        }) else {
            return;
        };
        let Some(map) = PageMap::new(page, image) else {
            dialog.gesture = None;
            return;
        };
        if dialog
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.map != map)
        {
            if let Some(gesture) = dialog.gesture.take() {
                dialog.request.rectangle = gesture.original;
            }
            dialog.error = Some("Placement cancelled because the page view changed.".into());
        }
        let response = if allowed && dialog.canvas_mode {
            Some(
                ui.interact(
                    image,
                    egui::Id::new(("image-placement", tab, dialog.key.generation)),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_text(
                    "Drag to place or move the new image frame; corners resize. Arrow keys nudge.",
                ),
            )
        } else {
            dialog.gesture = None;
            None
        };
        if let Some(response) = &response {
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    "New image placement canvas",
                )
            });
            if response.clicked() {
                response.request_focus();
            }
            if response.drag_started() {
                response.request_focus();
                if let Some(pointer) = ui
                    .input(|input| input.pointer.press_origin())
                    .or(response.interact_pointer_pos())
                    && let Some(origin) = map.inverse(pointer, false)
                {
                    let corner = [(0, 1), (2, 1), (0, 3), (2, 3)].into_iter().find(|(x, y)| {
                        map.point([dialog.request.rectangle[*x], dialog.request.rectangle[*y]])
                            .distance(pointer)
                            <= 8.0
                    });
                    let kind = if let Some((x, y)) = corner {
                        Kind::Resize(x, y)
                    } else if map
                        .rectangle(dialog.request.rectangle)
                        .is_some_and(|rectangle| rectangle.contains(pointer))
                    {
                        Kind::Move
                    } else {
                        Kind::Create
                    };
                    dialog.gesture = Some(Gesture {
                        map,
                        origin,
                        original: dialog.request.rectangle,
                        kind,
                    });
                }
            }
            if response.dragged()
                && let Some(gesture) = &dialog.gesture
                && let Some(pointer) = response.interact_pointer_pos()
                && let Some(point) = map.inverse(pointer, false)
            {
                dialog.request.rectangle = match gesture.kind {
                    Kind::Create => [
                        gesture.origin[0].min(point[0]),
                        gesture.origin[1].min(point[1]),
                        gesture.origin[0].max(point[0]),
                        gesture.origin[1].max(point[1]),
                    ],
                    Kind::Move => moved(
                        gesture.original,
                        [point[0] - gesture.origin[0], point[1] - gesture.origin[1]],
                        dialog.bounds,
                    ),
                    Kind::Resize(x, y) => {
                        let mut rectangle = gesture.original;
                        rectangle[x] = point[0];
                        rectangle[y] = point[1];
                        [
                            rectangle[0].min(rectangle[2]),
                            rectangle[1].min(rectangle[3]),
                            rectangle[0].max(rectangle[2]),
                            rectangle[1].max(rectangle[3]),
                        ]
                    }
                };
            }
            if response.drag_stopped() {
                dialog.gesture = None;
            }
            if response.has_focus() {
                let mut delta = [0.0; 2];
                ui.input(|input| {
                    let step = if input.modifiers.alt {
                        0.1
                    } else if input.modifiers.shift {
                        10.0
                    } else {
                        1.0
                    };
                    let origin = map.inverse(image.center(), false).unwrap_or([0.0; 2]);
                    for (key, screen) in [
                        (egui::Key::ArrowLeft, egui::vec2(-1.0, 0.0)),
                        (egui::Key::ArrowRight, egui::vec2(1.0, 0.0)),
                        (egui::Key::ArrowUp, egui::vec2(0.0, -1.0)),
                        (egui::Key::ArrowDown, egui::vec2(0.0, 1.0)),
                    ] {
                        if input.key_pressed(key)
                            && let Some(point) = map.inverse(image.center() + screen, false)
                        {
                            let vector = [point[0] - origin[0], point[1] - origin[1]];
                            let scale = vector[0].abs().max(vector[1].abs());
                            if scale > 0.0 {
                                delta[0] += vector[0] / scale * step;
                                delta[1] += vector[1] / scale * step;
                            }
                        }
                    }
                });
                if delta != [0.0; 2] {
                    dialog.request.rectangle =
                        moved(dialog.request.rectangle, delta, dialog.bounds);
                }
                if ui.input(|input| input.key_pressed(egui::Key::Escape))
                    && let Some(gesture) = dialog.gesture.take()
                {
                    dialog.request.rectangle = gesture.original;
                }
            }
        }
        if let Some(rectangle) = map.rectangle(dialog.request.rectangle) {
            let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(image));
            let color = egui::Color32::from_rgb(194, 100, 46);
            painter.rect_stroke(
                rectangle,
                0.0,
                egui::Stroke::new(2.0, color),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rectangle.min + egui::vec2(5.0, 5.0),
                egui::Align2::LEFT_TOP,
                format!("New image · {}°", dialog.request.rotation),
                egui::FontId::proportional(12.0),
                color,
            );
            if dialog.canvas_mode && allowed {
                for (x, y) in [(0, 1), (2, 1), (0, 3), (2, 3)] {
                    let point =
                        map.point([dialog.request.rectangle[x], dialog.request.rectangle[y]]);
                    painter.rect_filled(
                        egui::Rect::from_center_size(point, egui::vec2(7.0, 7.0)),
                        1.0,
                        color,
                    );
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn draw(
        app: &mut EditorApp,
        context: &egui::Context,
        events: Vec<egui::Event>,
        image: egui::Rect,
    ) {
        let page = app.tabs[0].status.info.pages[0].clone();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.image_canvas(ui, 1, 0, &page, image),
        );
        output.textures_delta.clear();
    }
    fn drag(
        app: &mut EditorApp,
        context: &egui::Context,
        image: egui::Rect,
        start: egui::Pos2,
        end: egui::Pos2,
    ) {
        draw(
            app,
            context,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            image,
        );
        draw(
            app,
            context,
            vec![egui::Event::PointerMoved(start + (end - start) * 0.5)],
            image,
        );
        draw(app, context, vec![egui::Event::PointerMoved(end)], image);
        draw(
            app,
            context,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            image,
        );
    }
    fn close(a: [f64; 4], b: [f64; 4]) {
        for (a, b) in a.into_iter().zip(b) {
            assert!((a - b).abs() < 0.001, "{a} != {b}");
        }
    }
    #[test]
    fn real_pointer_creation_movement_resize_and_keyboard_nudge_use_pdf_coordinates() {
        let context = egui::Context::default();
        let mut app = super::super::tests::editor(&context);
        app.image_dialog.as_mut().unwrap().canvas_mode = true;
        let image = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(200.0, 300.0));
        draw(&mut app, &context, vec![], image);
        drag(
            &mut app,
            &context,
            image,
            egui::pos2(110.0, 130.0),
            egui::pos2(160.0, 90.0),
        );
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            [100.0, 180.0, 150.0, 220.0],
        );
        drag(
            &mut app,
            &context,
            image,
            egui::pos2(135.0, 110.0),
            egui::pos2(145.0, 125.0),
        );
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            [110.0, 165.0, 160.0, 205.0],
        );
        drag(
            &mut app,
            &context,
            image,
            egui::pos2(120.0, 145.0),
            egui::pos2(130.0, 155.0),
        );
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            [120.0, 155.0, 160.0, 205.0],
        );
        draw(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            image,
        );
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            [121.0, 155.0, 161.0, 205.0],
        );
        assert!(app.has_tab_draft(1));
    }
    #[test]
    fn changed_page_view_cancels_gesture_and_stale_or_busy_frame_never_moves() {
        let context = egui::Context::default();
        let mut app = super::super::tests::editor(&context);
        app.image_dialog.as_mut().unwrap().canvas_mode = true;
        let image = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(200.0, 300.0));
        draw(&mut app, &context, vec![], image);
        let original = app.image_dialog.as_ref().unwrap().request.rectangle;
        let start = egui::pos2(110.0, 130.0);
        draw(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            image,
        );
        draw(
            &mut app,
            &context,
            vec![egui::Event::PointerMoved(egui::pos2(140.0, 100.0))],
            image,
        );
        assert!(app.image_dialog.as_ref().unwrap().gesture.is_some());
        draw(
            &mut app,
            &context,
            vec![],
            image.translate(egui::vec2(10.0, 0.0)),
        );
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            original,
        );
        assert!(app.image_dialog.as_ref().unwrap().gesture.is_none());
        app.busy = true;
        drag(&mut app, &context, image, start, egui::pos2(160.0, 90.0));
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            original,
        );
        app.busy = false;
        app.tabs[0].status.revision = 1;
        drag(&mut app, &context, image, start, egui::pos2(160.0, 90.0));
        close(
            app.image_dialog.as_ref().unwrap().request.rectangle,
            original,
        );
    }
}
