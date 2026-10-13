//! Original fallback FileAttachment vectors; no appearance streams or PDF bytes are generated.
//! Geometry authored for this project under the vendored crate's Apache-2.0 OR MIT license.
use super::{annotation_adjustment, finite_rect, numeric_array};
use crate::color::{Color, ColorComponents, ColorSpace};
use crate::context::Context;
use crate::device::Device;
use crate::util::RectExt;
use crate::{BlendMode, ClipPath, DrawMode, DrawProps, FillRule, Paint, StrokeProps};
use hayro_syntax::object::{Array, Dict, Name, Object, Rect};
use hayro_syntax::page::Rotation;
use kurbo::{Affine, BezPath, Cap, Circle, Join, RoundedRect, Shape};
use smallvec::smallvec;

enum Icon {
    Graph,
    Paperclip,
    PushPin,
    Tag,
}

fn absent(dict: &Dict<'_>, key: &[u8]) -> bool {
    !dict.contains_key(key) || matches!(dict.get::<Object<'_>>(key), Some(Object::Null(_)))
}

fn background(dict: &Dict<'_>) -> Option<Option<Color>> {
    if absent(dict, b"C") {
        return Some(Some(Color::new(
            ColorSpace::device_rgb(),
            smallvec![1.0, 1.0, 1.0],
            1.0,
        )));
    }
    let array = dict.get::<Array<'_>>(b"C")?;
    let count = array.raw_iter().take(5).count();
    let space = match count {
        0 => return Some(None),
        1 => ColorSpace::device_gray(),
        3 => ColorSpace::device_rgb(),
        4 => ColorSpace::device_cmyk(),
        _ => return None,
    };
    let mut values = ColorComponents::new();
    for value in array.iter::<f64>().take(4) {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return None;
        }
        values.push(value as f32);
    }
    if values.len() != count {
        return None;
    }
    Some(Some(Color::new(space, values, 1.0)))
}

pub(super) fn draw<'a>(
    annot: &Dict<'a>,
    rotation: Rotation,
    flags: u32,
    context: &Context<'a>,
    device: &mut impl Device<'a>,
) -> Option<()> {
    let raw = numeric_array::<4>(annot, b"Rect")?;
    if raw.iter().any(|value| !(*value as f32).is_finite()) {
        return None;
    }
    let rect = annot.get::<Rect>(b"Rect")?.to_kurbo();
    let zoom = context.settings.annotation_view_zoom;
    if !finite_rect(rect) || !zoom.is_finite() || zoom <= 0.0 {
        return None;
    }
    let adjustment = annotation_adjustment(rect, rotation, zoom, flags);
    let transform = context.get().ctm
        * adjustment
        * Affine::translate((rect.x0, rect.y0))
        * Affine::scale_non_uniform(rect.width() / 24.0, rect.height() / 24.0);
    if transform
        .as_coeffs()
        .iter()
        .any(|value| !value.is_finite() || !(*value as f32).is_finite())
    {
        return None;
    }
    let clip = transform * kurbo::Rect::new(0.0, 0.0, 24.0, 24.0).to_path(0.1);
    let bounds = clip.bounding_box();
    if !finite_rect(bounds)
        || [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .any(|value| !(*value as f32).is_finite())
    {
        return None;
    }
    let background = background(annot)?;
    let opacity = if absent(annot, b"CA") {
        1.0
    } else {
        annot.get::<f64>(b"CA")?
    };
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return None;
    }
    let icon = if absent(annot, b"Name") {
        Icon::PushPin
    } else {
        match annot.get::<Name<'_>>(b"Name")?.as_ref() {
            b"Graph" => Icon::Graph,
            b"Paperclip" => Icon::Paperclip,
            b"Tag" => Icon::Tag,
            _ => Icon::PushPin,
        }
    };
    if opacity == 0.0 || !context.ocg_state.is_visible() {
        return Some(());
    }

    // All paths and clip coordinates are local constants. Direct DrawProps prevent
    // page paint, alpha, soft-mask, blend, transform, pending-path or clip-state leakage.
    device.push_clip_path(&ClipPath {
        path: clip,
        fill: FillRule::NonZero,
    });
    device.push_transparency_group(opacity as f32, None, BlendMode::Normal);
    let ink = Color::new(ColorSpace::device_rgb(), smallvec![0.15, 0.20, 0.25], 1.0);
    let props = |color: Color| DrawProps {
        transform,
        paint: Paint::Color(color),
        soft_mask: None,
        blend_mode: BlendMode::Normal,
    };
    let stroke = DrawMode::Stroke(StrokeProps {
        line_width: 1.2,
        line_cap: Cap::Round,
        line_join: Join::Round,
        ..Default::default()
    });
    let mut body = if matches!(icon, Icon::Tag) {
        let mut path = polygon(&[
            (2.0, 5.0),
            (15.0, 5.0),
            (22.0, 12.0),
            (15.0, 19.0),
            (2.0, 19.0),
        ]);
        path.extend(
            Circle::new((16.0, 12.0), 1.6)
                .to_path(0.1)
                .elements()
                .iter()
                .copied(),
        );
        path
    } else {
        RoundedRect::new(1.5, 1.5, 22.5, 22.5, 2.0).to_path(0.1)
    };
    if let Some(color) = background {
        device.draw_path(&body, props(color), &DrawMode::Fill(FillRule::EvenOdd));
    }
    device.draw_path(&body, props(ink.clone()), &stroke);
    match icon {
        Icon::Graph => {
            for (x, height) in [(5.0, 6.0), (10.0, 10.0), (15.0, 14.0)] {
                device.draw_rect(
                    &kurbo::Rect::new(x, 5.0, x + 3.0, 5.0 + height),
                    props(ink.clone()),
                    &DrawMode::Fill(FillRule::NonZero),
                );
            }
        }
        Icon::PushPin => {
            device.draw_path(
                &RoundedRect::new(7.0, 15.0, 17.0, 20.0, 1.0).to_path(0.1),
                props(ink.clone()),
                &DrawMode::Fill(FillRule::NonZero),
            );
            device.draw_rect(
                &kurbo::Rect::new(10.0, 10.0, 14.0, 15.0),
                props(ink.clone()),
                &DrawMode::Fill(FillRule::NonZero),
            );
            device.draw_path(
                &polygon(&[(6.0, 9.0), (18.0, 9.0), (15.0, 13.0), (9.0, 13.0)]),
                props(ink.clone()),
                &DrawMode::Fill(FillRule::NonZero),
            );
            body = BezPath::new();
            body.move_to((12.0, 9.0));
            body.line_to((12.0, 3.0));
            device.draw_path(&body, props(ink), &stroke);
        }
        Icon::Paperclip => {
            body = BezPath::new();
            body.move_to((8.0, 6.0));
            body.line_to((16.0, 14.0));
            body.curve_to((19.0, 17.0), (17.0, 21.0), (14.0, 18.0));
            body.line_to((6.0, 10.0));
            body.curve_to((2.0, 6.0), (7.0, 1.0), (11.0, 5.0));
            body.line_to((18.0, 12.0));
            body.curve_to((21.0, 15.0), (17.0, 18.0), (15.0, 16.0));
            body.line_to((8.0, 9.0));
            device.draw_path(&body, props(ink), &stroke);
        }
        Icon::Tag => {}
    }
    device.pop_transparency_group();
    device.pop_clip();
    Some(())
}

fn polygon(points: &[(f64, f64)]) -> BezPath {
    let mut path = BezPath::new();
    for (index, &point) in points.iter().enumerate() {
        if index == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close_path();
    path
}
