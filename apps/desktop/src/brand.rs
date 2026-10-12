use eframe::egui::{self, Color32, Rect, Sense, Stroke, vec2};

/// Native vector counterpart of docs/assets/logo.svg; no bitmap or network loader.
pub fn logo(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let scale = rect.width() / 256.0;
    let point = |x: f32, y: f32| rect.min + vec2(x * scale, y * scale);
    let bounds = |x: f32, y: f32, width: f32, height: f32| {
        Rect::from_min_size(point(x, y), vec2(width, height) * scale)
    };
    let forest = Color32::from_rgb(6, 62, 50);
    let copper = Color32::from_rgb(173, 91, 47);
    painter.rect_filled(rect, 58.0 * scale, forest);
    painter.rect_filled(bounds(43.0, 83.0, 143.0, 136.0), 12.0 * scale, copper);
    painter.rect_filled(
        bounds(56.0, 66.0, 140.0, 146.0),
        12.0 * scale,
        Color32::from_rgb(216, 211, 189),
    );
    painter.add(egui::Shape::convex_polygon(
        [
            (80.0, 43.0),
            (150.0, 43.0),
            (197.0, 90.0),
            (197.0, 189.0),
            (193.5, 197.5),
            (185.0, 201.0),
            (80.0, 201.0),
            (71.5, 197.5),
            (68.0, 189.0),
            (68.0, 55.0),
            (71.5, 46.5),
        ]
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect(),
        Color32::from_rgb(250, 246, 234),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        [
            point(150.0, 43.0),
            point(197.0, 90.0),
            point(162.0, 90.0),
            point(153.5, 86.5),
            point(150.0, 78.0),
        ]
        .into(),
        Color32::from_rgb(200, 217, 203),
        Stroke::NONE,
    ));
    painter.line_segment(
        [point(88.0, 112.0), point(88.0, 164.0)],
        Stroke::new(3.0 * scale, copper),
    );
    for y in [112.0, 164.0] {
        painter.circle_filled(point(88.0, y), 6.0 * scale, copper);
    }
    for (y, length) in [(112.0, 48.0), (138.0, 36.0), (164.0, 25.0)] {
        painter.line_segment(
            [point(107.0, y), point(107.0 + length, y)],
            Stroke::new(10.0 * scale, forest),
        );
        for x in [107.0, 107.0 + length] {
            painter.circle_filled(point(x, y), 5.0 * scale, forest);
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Image,
            ui.is_enabled(),
            "PDF Editor Offline",
        )
    });
    response.on_hover_text("PDF Editor Offline")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mark_tessellates_in_both_themes_at_multiple_display_densities() {
        for dark in [false, true] {
            for density in [1.0, 2.0, 3.0] {
                let context = egui::Context::default();
                context.set_pixels_per_point(density);
                context.set_visuals(if dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
                let mut output = context.run_ui(Default::default(), |ui| {
                    let response = logo(ui);
                    assert_eq!(response.rect.size(), vec2(28.0, 28.0));
                    assert!(!response.clicked());
                });
                output.textures_delta.clear();
                let primitives = context.tessellate(output.shapes, output.pixels_per_point);
                assert!(!primitives.is_empty());
                for primitive in primitives {
                    if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                        assert!(!mesh.indices.is_empty());
                        assert!(
                            mesh.vertices
                                .iter()
                                .all(|vertex| vertex.pos.x.is_finite() && vertex.pos.y.is_finite())
                        );
                    }
                }
            }
        }
    }
}
