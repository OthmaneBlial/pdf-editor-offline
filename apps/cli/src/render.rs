use pdf_core::{Error, PdfDocument, Result, read_pdf_bytes};
use pdf_render::{RenderError, RenderedPage};
use serde_json::{Value, json};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

fn renderer_error(error: RenderError) -> Error {
    match error {
        RenderError::InputTooLarge => Error::Limit("render input exceeds 128 MiB"),
        RenderError::InvalidPdf => Error::Invalid("renderer cannot parse PDF"),
        RenderError::PasswordRequired => Error::Refused("encrypted rendering is not supported"),
        RenderError::InvalidScale => Error::Pages("scale must be finite and positive"),
        RenderError::InvalidViewZoom => Error::Pages("view zoom must be finite and positive"),
        RenderError::PageOutOfRange { .. } => Error::Pages("render page is outside document"),
        RenderError::OutputTooLarge => Error::Limit("render exceeds dimension or pixel budget"),
        RenderError::RendererFailed => Error::Refused("renderer could not complete this page"),
    }
}

fn encoding_error(error: png::EncodingError) -> Error {
    match error {
        png::EncodingError::IoError(error) => Error::Io(error),
        _ => Error::Verification("PNG encoding failed"),
    }
}

fn decoding_error(error: png::DecodingError) -> Error {
    match error {
        png::DecodingError::IoError(error) => Error::Io(error),
        _ => Error::Verification("PNG checksum or decoding failed"),
    }
}

fn verify_png(path: &Path, image: &RenderedPage) -> Result<()> {
    let mut options = png::DecodeOptions::default();
    options.set_ignore_adler32(false);
    options.set_ignore_crc(false);
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder = png::Decoder::new_with_options(BufReader::new(File::open(path)?), options);
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(decoding_error)?;
    let info = reader.info();
    if (info.width, info.height) != (image.width, image.height)
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || info.animation_control.is_some()
        || reader.output_buffer_size() != Some(image.rgba.len())
    {
        return Err(Error::Verification("PNG dimensions or format changed"));
    }
    let mut decoded = vec![0; image.rgba.len()];
    let frame = reader.next_frame(&mut decoded).map_err(decoding_error)?;
    if frame.buffer_size() != image.rgba.len() || decoded != image.rgba {
        return Err(Error::Verification("PNG pixels changed during encoding"));
    }
    reader.finish().map_err(decoding_error)?;
    Ok(())
}

fn save_png(output: &Path, image: &RenderedPage) -> Result<()> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut encoder = png::Encoder::new(temporary.as_file_mut(), image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(encoding_error)?;
        writer
            .write_image_data(&image.rgba)
            .map_err(encoding_error)?;
        writer.finish().map_err(encoding_error)?;
    }
    temporary.as_file().sync_all()?;
    verify_png(temporary.path(), image)?;
    temporary.persist_noclobber(output).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::OutputExists(output.to_owned())
        } else {
            Error::Io(error.error)
        }
    })?;
    Ok(())
}

pub(crate) fn render_png(
    input: &Path,
    output: &Path,
    page: u32,
    scale: f32,
    dry_run: bool,
    password: Option<&str>,
) -> Result<Value> {
    if page == 0 {
        return Err(Error::Pages("render page numbers are one-based"));
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(Error::Pages("scale must be finite and positive"));
    }
    if !dry_run && output.symlink_metadata().is_ok() {
        return Err(Error::OutputExists(output.to_owned()));
    }
    let bytes = read_pdf_bytes(input)?;
    let document = PdfDocument::from_bytes(&bytes, password)?;
    if document.info()?.encrypted {
        return Err(Error::Refused("encrypted rendering is not supported"));
    }
    document.validate_selection(&[page])?;
    let image =
        pdf_render::render_page(&bytes, page as usize - 1, scale).map_err(renderer_error)?;
    let expected = image.width as usize * image.height as usize * 4;
    if image.rgba.len() != expected || image.rgba.chunks_exact(4).any(|pixel| pixel[3] != 255) {
        return Err(Error::Verification("renderer output is not opaque RGBA8"));
    }
    if !dry_run {
        save_png(output, &image)?;
    }
    Ok(
        json!({"operation": "render-page", "page": page, "scale": scale,
        "format": "png", "width": image.width, "height": image.height,
        "pixel_format": "rgba8_opaque", "output": output, "dry_run": dry_run,
        "output_verified": !dry_run}),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn verification_rejects_corrupted_png_checksums() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.png");
        let image = RenderedPage {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        };
        save_png(&path, &image).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[29] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(verify_png(&path, &image).is_err());
    }
}
