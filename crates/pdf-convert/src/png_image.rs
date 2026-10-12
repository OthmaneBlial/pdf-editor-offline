use crate::{Error, ImageData, MAX_IMAGE_METADATA_BYTES, Result, dimensions};
use std::io::Cursor;

pub(crate) fn decode(bytes: &[u8]) -> Result<ImageData> {
    let mut offset = 8usize;
    let mut size = None;
    let mut metadata = 0usize;
    let mut ended = false;
    let mut chunks = 0usize;
    let mut srgb_intent = None;
    while offset < bytes.len() {
        chunks += 1;
        if chunks > 100_000 {
            return Err(Error::Limit("too many PNG chunks"));
        }
        let header = bytes
            .get(offset..offset + 8)
            .ok_or(Error::Invalid("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| Error::Invalid("PNG chunk length"))?,
        ) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .ok_or(Error::Invalid("PNG chunk size overflow"))?;
        let chunk = bytes
            .get(offset + 4..end - 4)
            .ok_or(Error::Invalid("truncated PNG chunk payload"))?;
        let crc = bytes
            .get(end - 4..end)
            .ok_or(Error::Invalid("truncated PNG checksum"))?;
        if crc32fast::hash(chunk)
            != u32::from_be_bytes(crc.try_into().map_err(|_| Error::Invalid("PNG checksum"))?)
        {
            return Err(Error::Invalid("PNG CRC mismatch"));
        }
        let kind = &header[4..8];
        let data = &chunk[4..];
        if kind != b"IDAT" {
            metadata = metadata.saturating_add(length);
            if metadata > MAX_IMAGE_METADATA_BYTES {
                return Err(Error::Limit("PNG metadata exceeds 1 MiB"));
            }
        }
        match kind {
            b"IHDR" => {
                if chunks != 1 || length != 13 {
                    return Err(Error::Invalid("PNG needs a single initial IHDR"));
                }
                let width = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| Error::Invalid("PNG width"))?,
                );
                let height = u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| Error::Invalid("PNG height"))?,
                );
                if !matches!(data[9], 0 | 2 | 3 | 4 | 6)
                    || data[8] > 8
                    || (!matches!(data[9], 0 | 3) && data[8] != 8)
                    || !matches!(data[8], 1 | 2 | 4 | 8)
                {
                    return Err(Error::Unsupported(
                        "PNG color type/high-bit-depth unsupported",
                    ));
                }
                if data[12] != 0 {
                    return Err(Error::Unsupported(
                        "interlaced PNG not supported by this tier",
                    ));
                }
                // Expansion may introduce RGB and alpha: reserve the worst supported layout.
                dimensions(width, height, 4)?;
                size = Some((width, height));
            }
            b"PLTE" => {
                if length == 0 || length > 768 || !length.is_multiple_of(3) {
                    return Err(Error::Invalid("PNG palette length invalid"));
                }
            }
            b"tRNS" => {
                if length > 256 {
                    return Err(Error::Invalid("PNG transparency table too large"));
                }
            }
            b"sRGB" => {
                if length != 1 || srgb_intent.is_some() {
                    return Err(Error::Invalid("PNG sRGB intent invalid/repeated"));
                }
                srgb_intent = Some(match data[0] {
                    0 => "Perceptual",
                    1 => "RelativeColorimetric",
                    2 => "Saturation",
                    3 => "AbsoluteColorimetric",
                    _ => return Err(Error::Invalid("PNG sRGB intent outside 0..3")),
                });
            }
            b"IDAT" => {
                if size.is_none() {
                    return Err(Error::Invalid("PNG IDAT before IHDR"));
                }
            }
            b"IEND" => {
                if length != 0 || end != bytes.len() {
                    return Err(Error::Invalid("PNG IEND/trailing data invalid"));
                }
                ended = true;
            }
            _ => {
                return Err(Error::Unsupported(
                    "PNG color/orientation/animation/ancillary metadata unsupported",
                ));
            }
        }
        offset = end;
    }
    if !ended {
        return Err(Error::Invalid("PNG missing IEND"));
    }
    let (width, height) = size.ok_or(Error::Invalid("PNG missing IHDR"))?;
    let mut options = png::DecodeOptions::default();
    options.set_ignore_adler32(false);
    options.set_ignore_crc(false);
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder = png::Decoder::new_with_options(Cursor::new(bytes), options);
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|_| Error::Decode("PNG header"))?;
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err(Error::Unsupported(
            "PNG expansion did not yield 8-bit samples",
        ));
    }
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgba => 4,
        _ => return Err(Error::Unsupported("PNG expanded color type unsupported")),
    };
    let expected = dimensions(width, height, channels)?;
    if reader.output_buffer_size() != Some(expected) {
        return Err(Error::Invalid("PNG decoded size differs from header"));
    }
    let mut samples = vec![0; expected];
    let frame = reader
        .next_frame(&mut samples)
        .map_err(|_| Error::Decode("PNG pixels"))?;
    if frame.buffer_size() != expected {
        return Err(Error::Invalid("PNG pixel count mismatch"));
    }
    reader
        .finish()
        .map_err(|_| Error::Decode("PNG completion"))?;
    let alpha_channel = channels == 2 || channels == 4;
    let mut color_channels = if channels <= 2 { 1 } else { 3 };
    let (mut pixels, alpha) = if alpha_channel {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * color_channels);
        let mut alpha = Vec::with_capacity(width as usize * height as usize);
        for sample in samples.chunks_exact(channels) {
            pixels.extend_from_slice(&sample[..color_channels]);
            alpha.push(sample[color_channels]);
        }
        (pixels, Some(alpha))
    } else {
        (samples, None)
    };
    if srgb_intent.is_some() && color_channels == 1 {
        pixels = pixels
            .into_iter()
            .flat_map(|gray| [gray, gray, gray])
            .collect();
        color_channels = 3;
    }
    Ok(ImageData {
        width,
        height,
        color_space: if color_channels == 1 {
            "DeviceGray"
        } else {
            "DeviceRGB"
        },
        pixels,
        alpha,
        format: "png",
        jpeg_transform: None,
        srgb_intent,
    })
}
