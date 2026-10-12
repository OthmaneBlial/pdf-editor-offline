use pdf_core::PageInfo;

#[derive(Debug, Clone, Copy)]
pub struct RasterPlan {
    pub scale: f32,
    pub pixel_size: [u32; 2],
    pub logical_size: [f32; 2],
    pub device_ratio: f32,
    pub limited: bool,
}

impl RasterPlan {
    pub fn display_size(self, actual_pixels: [u32; 2]) -> [f32; 2] {
        if self.limited {
            self.logical_size
        } else {
            [
                actual_pixels[0] as f32 / self.device_ratio,
                actual_pixels[1] as f32 / self.device_ratio,
            ]
        }
    }
}

pub fn plan(page: &PageInfo, zoom: f32, device_ratio: f32) -> Result<RasterPlan, &'static str> {
    if !zoom.is_finite() || zoom <= 0.0 || !device_ratio.is_finite() || device_ratio <= 0.0 {
        return Err("Zoom and display scale must be finite and positive.");
    }
    if page
        .media_box
        .iter()
        .chain(&page.crop_box)
        .any(|value| !value.is_finite())
        || page.rotation % 90 != 0
    {
        return Err("This page has invalid visible dimensions.");
    }
    let mut width =
        page.media_box[2].min(page.crop_box[2]) - page.media_box[0].max(page.crop_box[0]);
    let mut height =
        page.media_box[3].min(page.crop_box[3]) - page.media_box[1].max(page.crop_box[1]);
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("This page has no valid visible area.");
    }
    width = width.max(1.0);
    height = height.max(1.0);
    if matches!(page.rotation.rem_euclid(360), 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    let logical_size = [width * zoom, height * zoom];
    if logical_size.iter().any(|value| !value.is_finite()) {
        return Err("Page dimensions exceed the supported display range.");
    }
    let width = f64::from(width);
    let height = f64::from(height);
    let requested = f64::from(zoom) * f64::from(device_ratio);
    let fits = |scale: f64| {
        let w = (width * scale).ceil();
        let h = (height * scale).ceil();
        w >= 1.0
            && h >= 1.0
            && w <= f64::from(pdf_render::MAX_DIMENSION)
            && h <= f64::from(pdf_render::MAX_DIMENSION)
            && w * h <= pdf_render::MAX_PIXELS as f64
    };
    let mut chosen = requested;
    let limited = !fits(requested) || width * requested < 1.0 || height * requested < 1.0;
    if !fits(requested) {
        let mut lower = 0.0;
        let mut upper = requested
            .min(f64::from(pdf_render::MAX_DIMENSION) / width.max(height))
            .min((pdf_render::MAX_PIXELS as f64 / (width * height)).sqrt());
        // Ceiling each axis can exceed the area cap even when the continuous area fits.
        for _ in 0..48 {
            let middle = (lower + upper) / 2.0;
            if fits(middle) {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        chosen = lower;
    }
    let mut scale = chosen as f32;
    if !fits(f64::from(scale)) {
        scale = scale.next_down();
    }
    if !scale.is_finite() || scale <= 0.0 || !fits(f64::from(scale)) {
        return Err("Display scale is outside the supported rendering range.");
    }
    Ok(RasterPlan {
        scale,
        pixel_size: [
            (width * f64::from(scale)).ceil() as u32,
            (height * f64::from(scale)).ceil() as u32,
        ],
        logical_size,
        device_ratio,
        limited,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(width: f32, height: f32) -> PageInfo {
        PageInfo {
            number: 1,
            media_box: [0.0, 0.0, width, height],
            crop_box: [0.0, 0.0, width, height],
            rotation: 0,
        }
    }

    #[test]
    fn retina_renders_twice_the_pixels_without_changing_logical_page_size() {
        let ordinary = plan(&page(100.0, 80.0), 1.0, 1.0).expect("ordinary plan");
        let retina = plan(&page(100.0, 80.0), 1.0, 2.0).expect("retina plan");
        assert_eq!(retina.scale, 2.0);
        assert_eq!(retina.pixel_size, [200, 160]);
        assert_eq!(
            retina.display_size(retina.pixel_size),
            ordinary.display_size(ordinary.pixel_size)
        );
        assert_eq!(retina.display_size(retina.pixel_size), [100.0, 80.0]);
    }

    #[test]
    fn crops_rotations_and_pixel_caps_use_actual_visible_geometry() {
        let mut page = page(100.0, 80.0);
        page.crop_box = [10.0, 10.0, 60.0, 40.0];
        page.rotation = 90;
        let cropped = plan(&page, 1.5, 2.0).expect("cropped plan");
        assert_eq!(cropped.pixel_size, [90, 150]);
        assert_eq!(cropped.logical_size, [45.0, 75.0]);
        for (width, height, ratio) in [
            (8000.0, 8000.0, 2.0),
            (1_000_000.0, 1.0, 2.0),
            (595.0, 842.0, f32::MAX),
        ] {
            page.media_box = [0.0, 0.0, width, height];
            page.crop_box = page.media_box;
            page.rotation = 0;
            let capped = plan(&page, 4.0, ratio).expect("capped plan");
            assert!(capped.limited);
            assert!(
                capped
                    .pixel_size
                    .iter()
                    .all(|side| *side <= pdf_render::MAX_DIMENSION)
            );
            assert!(
                u64::from(capped.pixel_size[0]) * u64::from(capped.pixel_size[1])
                    <= pdf_render::MAX_PIXELS
            );
            assert_eq!(
                capped.display_size(capped.pixel_size),
                [width * 4.0, height * 4.0]
            );
        }
    }

    #[test]
    fn invalid_factors_and_visible_boxes_are_rejected() {
        for factor in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
            assert!(plan(&page(100.0, 80.0), factor, 2.0).is_err());
            assert!(plan(&page(100.0, 80.0), 1.0, factor).is_err());
        }
        let mut bad = page(100.0, 80.0);
        bad.crop_box = [200.0, 0.0, 250.0, 80.0];
        assert!(plan(&bad, 1.0, 2.0).is_err());
        bad.media_box[2] = f32::NAN;
        assert!(plan(&bad, 1.0, 2.0).is_err());
    }
}
