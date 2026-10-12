//! Bounded, offline PDF rasterization without an external renderer.

#![forbid(unsafe_code)]

use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Input limit shared with the document engine.
pub const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
/// Maximum output dimension, in pixels.
pub const MAX_DIMENSION: u32 = 8192;
/// Maximum output area (64 MiB of RGBA pixels).
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;

/// Opaque RGBA8 pixels, top-to-bottom rows with no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Errors safe for a desktop caller to present without crashing the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    InputTooLarge,
    InvalidPdf,
    PasswordRequired,
    InvalidScale,
    InvalidViewZoom,
    PageOutOfRange { requested: usize, page_count: usize },
    OutputTooLarge,
    RendererFailed,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => f.write_str("PDF exceeds the 128 MiB input limit"),
            Self::InvalidPdf => f.write_str("PDF cannot be parsed"),
            Self::PasswordRequired => {
                f.write_str("PDF requires a password or uses unsupported encryption")
            }
            Self::InvalidScale => f.write_str("Render scale must be finite and greater than zero"),
            Self::InvalidViewZoom => f.write_str("View zoom must be finite and greater than zero"),
            Self::PageOutOfRange {
                requested,
                page_count,
            } => {
                write!(
                    f,
                    "Page index {requested} is outside this {page_count}-page PDF"
                )
            }
            Self::OutputTooLarge => {
                f.write_str("Render exceeds the output dimension or pixel limit")
            }
            Self::RendererFailed => f.write_str("PDF renderer could not complete this page"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Render one zero-based PDF page with `scale` pixels per PDF point.
///
/// Dimensions include CropBox and page rotation; fractional dimensions round up.
/// The output uses an opaque white background. No file, network, subprocess, GPU,
/// service or platform PDF library is needed.
///
/// Input and output limits bound the input copy and raster buffer. They do not
/// bound Hayro's decompression, internal allocations, or execution time. Ordinary
/// dependency panics become errors; process aborts and allocation failure cannot
/// be caught. Hostile documents require an isolated worker with OS resource limits.
pub fn render_page(
    bytes: &[u8],
    page_index: usize,
    scale: f32,
) -> Result<RenderedPage, RenderError> {
    render_view_page(bytes, page_index, scale, 1.0)
}

/// Render for on-screen viewing. `scale` is raster pixels per PDF point; `view_zoom`
/// is logical screen magnification. NoZoom annotations cancel only `view_zoom`,
/// preserving density/DPR and capped-preview resolution independently.
pub fn render_view_page(
    bytes: &[u8],
    page_index: usize,
    scale: f32,
    view_zoom: f32,
) -> Result<RenderedPage, RenderError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RenderError::InputTooLarge);
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(RenderError::InvalidScale);
    }
    if !view_zoom.is_finite() || view_zoom <= 0.0 {
        return Err(RenderError::InvalidViewZoom);
    }
    std::panic::catch_unwind(|| render_page_inner(bytes, page_index, scale, view_zoom))
        .map_err(|_| RenderError::RendererFailed)?
}

fn render_page_inner(
    bytes: &[u8],
    page_index: usize,
    scale: f32,
    view_zoom: f32,
) -> Result<RenderedPage, RenderError> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|error| match error {
        LoadPdfError::Invalid => RenderError::InvalidPdf,
        LoadPdfError::Decryption(_) => RenderError::PasswordRequired,
    })?;
    let page = pdf
        .pages()
        .get(page_index)
        .ok_or(RenderError::PageOutOfRange {
            requested: page_index,
            page_count: pdf.pages().len(),
        })?;
    let (page_width, page_height) = page.render_dimensions();
    let (width, height) = output_dimensions(page_width, page_height, scale)?;
    let mut context = RenderContext::new(width as u16, height as u16);
    let transform = hayro::kurbo::Affine::scale(f64::from(scale))
        * hayro::hayro_interpret::util::TransformExt::to_kurbo(&page.initial_transform(true));
    let invalid_geometry = Arc::new(AtomicBool::new(false));
    let geometry_warning = invalid_geometry.clone();
    hayro::render_into(
        page,
        &hayro::RenderCache::new(),
        &hayro::hayro_interpret::InterpreterSettings {
            annotation_view_zoom: f64::from(view_zoom),
            warning_sink: Arc::new(move |warning| {
                if matches!(
                    warning,
                    hayro::hayro_interpret::InterpreterWarning::InvalidAnnotationGeometry
                ) {
                    geometry_warning.store(true, Ordering::Relaxed);
                }
            }),
            ..Default::default()
        },
        &Default::default(),
        &mut context,
        transform,
    );
    if invalid_geometry.load(Ordering::Relaxed) {
        return Err(RenderError::RendererFailed);
    }
    context.flush();
    let mut pixmap = Pixmap::new(width as u16, height as u16);
    context.render_with(
        &mut pixmap,
        &mut Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(hayro::vello_cpu::color::palette::css::WHITE),
            ..Default::default()
        },
    );
    Ok(RenderedPage {
        width,
        height,
        rgba: pixmap.take_rgba8(hayro::vello_cpu::peniko::ImageAlphaType::Alpha),
    })
}

fn output_dimensions(
    page_width: f32,
    page_height: f32,
    scale: f32,
) -> Result<(u32, u32), RenderError> {
    let width = (f64::from(page_width) * f64::from(scale)).ceil();
    let height = (f64::from(page_height) * f64::from(scale)).ceil();
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(MAX_DIMENSION)
        || height > f64::from(MAX_DIMENSION)
        || width * height > MAX_PIXELS as f64
    {
        return Err(RenderError::OutputTooLarge);
    }
    Ok((width as u32, height as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_or_non_positive_scale_before_parsing() {
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                render_page(b"bad PDF", 0, scale),
                Err(RenderError::InvalidScale)
            );
        }
    }

    #[test]
    fn output_limits_reject_bombs_and_round_up_fractional_pixels() {
        assert_eq!(output_dimensions(100.1, 80.1, 1.0), Ok((101, 81)));
        for (width, height, scale) in [
            (10000.0, 1.0, 1.0),
            (8192.0, 8192.0, 1.0),
            (100.0, 100.0, f32::MAX),
            (f32::NAN, 100.0, 1.0),
            (f32::INFINITY, 100.0, 1.0),
            (0.0, 100.0, 1.0),
        ] {
            assert_eq!(
                output_dimensions(width, height, scale),
                Err(RenderError::OutputTooLarge)
            );
        }
        assert_eq!(output_dimensions(4096.0, 4096.0, 1.0), Ok((4096, 4096)));
    }
}
