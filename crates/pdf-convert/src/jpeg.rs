use crate::{Error, ImageData, MAX_IMAGE_DIMENSION, MAX_IMAGE_METADATA_BYTES, Result, dimensions};
use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};

pub(crate) fn decode(bytes: &[u8]) -> Result<ImageData> {
    let (width, height, components) = preflight(bytes)?;
    let options = DecoderOptions::default()
        .set_strict_mode(true)
        .set_max_width(MAX_IMAGE_DIMENSION as usize)
        .set_max_height(MAX_IMAGE_DIMENSION as usize)
        .jpeg_set_max_scans(64);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|_| Error::Decode("JPEG header"))?;
    if decoder.dimensions() != Some((width as usize, height as usize)) {
        return Err(Error::Invalid("JPEG dimensions disagree"));
    }
    let input = decoder
        .input_colorspace()
        .ok_or(Error::Invalid("JPEG missing color space"))?;
    let transform = match input {
        ColorSpace::Luma => None,
        ColorSpace::RGB => Some(0),
        ColorSpace::YCbCr => Some(1),
        _ => return Err(Error::Unsupported("JPEG color space unsupported")),
    };
    let output = if components == 1 {
        ColorSpace::Luma
    } else {
        ColorSpace::RGB
    };
    decoder.set_options(decoder.options().jpeg_set_out_colorspace(output));
    let expected = dimensions(width, height, components)?;
    if decoder.output_buffer_size() != Some(expected) {
        return Err(Error::Invalid("JPEG decoded allocation mismatch"));
    }
    let mut samples = vec![0; expected];
    decoder
        .decode_into(&mut samples)
        .map_err(|_| Error::Decode("JPEG pixels"))?;
    Ok(ImageData {
        width,
        height,
        color_space: if components == 1 {
            "DeviceGray"
        } else {
            "DeviceRGB"
        },
        pixels: bytes.to_vec(),
        alpha: None,
        format: "jpeg",
        jpeg_transform: transform,
        srgb_intent: None,
    })
}

fn preflight(bytes: &[u8]) -> Result<(u32, u32, usize)> {
    let mut offset = 2usize;
    let mut frame = None;
    let mut metadata = 0usize;
    let mut scans = 0usize;
    loop {
        if bytes.get(offset) != Some(&0xFF) {
            return Err(Error::Invalid("JPEG marker expected"));
        }
        while bytes.get(offset) == Some(&0xFF) {
            offset += 1;
        }
        let marker = *bytes
            .get(offset)
            .ok_or(Error::Invalid("truncated JPEG marker"))?;
        offset += 1;
        if marker == 0xD9 {
            if offset != bytes.len() || scans == 0 {
                return Err(Error::Invalid("JPEG trailing data or missing scan"));
            }
            return frame.ok_or(Error::Invalid("JPEG missing frame"));
        }
        if matches!(marker, 0x00 | 0xD8 | 0xD0..=0xD7 | 0x01) {
            return Err(Error::Invalid("unexpected standalone JPEG marker"));
        }
        let length_bytes = bytes
            .get(offset..offset + 2)
            .ok_or(Error::Invalid("truncated JPEG segment"))?;
        let length = u16::from_be_bytes(
            length_bytes
                .try_into()
                .map_err(|_| Error::Invalid("JPEG segment length"))?,
        ) as usize;
        if length < 2 {
            return Err(Error::Invalid("JPEG segment length below two"));
        }
        let end = offset
            .checked_add(length)
            .ok_or(Error::Invalid("JPEG segment overflow"))?;
        let data = bytes
            .get(offset + 2..end)
            .ok_or(Error::Invalid("truncated JPEG payload"))?;
        if marker != 0xDA {
            metadata = metadata.saturating_add(data.len());
            if metadata > MAX_IMAGE_METADATA_BYTES {
                return Err(Error::Limit("JPEG headers/metadata exceed 1 MiB"));
            }
        }
        match marker {
            0xC0 | 0xC2 => {
                if frame.is_some() || data.len() < 6 {
                    return Err(Error::Invalid("JPEG frame invalid/repeated"));
                }
                let components = data[5] as usize;
                if data[0] != 8 || !matches!(components, 1 | 3) {
                    return Err(Error::Unsupported("only 8-bit Gray/RGB JPEG supported"));
                }
                if data.len() != 6 + 3 * components {
                    return Err(Error::Invalid("JPEG frame component count invalid"));
                }
                let height = u16::from_be_bytes([data[1], data[2]]) as u32;
                let width = u16::from_be_bytes([data[3], data[4]]) as u32;
                dimensions(width, height, components)?;
                frame = Some((width, height, components));
            }
            0xDA => {
                scans += 1;
                if scans > 64 {
                    return Err(Error::Limit("more than 64 JPEG scans"));
                }
                if frame.is_none() {
                    return Err(Error::Invalid("JPEG scan before frame"));
                }
            }
            0xE0 => {
                if data.len() < 14
                    || !data.starts_with(b"JFIF\0")
                    || data.len() != 14 + 3 * data[12] as usize * data[13] as usize
                {
                    return Err(Error::Unsupported("JPEG APP0 outside JFIF tier"));
                }
            }
            0xEE => {
                if data.len() != 12 || !data.starts_with(b"Adobe") || data[11] > 1 {
                    return Err(Error::Unsupported("JPEG Adobe color transform unsupported"));
                }
            }
            0xDB | 0xC4 | 0xDD | 0xFE => {}
            _ => {
                return Err(Error::Unsupported(
                    "JPEG metadata/orientation/coding outside supported tier",
                ));
            }
        }
        offset = end;
        if marker == 0xDA {
            loop {
                let byte = *bytes
                    .get(offset)
                    .ok_or(Error::Invalid("JPEG missing end marker"))?;
                if byte != 0xFF {
                    offset += 1;
                    continue;
                }
                let start = offset;
                offset += 1;
                while bytes.get(offset) == Some(&0xFF) {
                    offset += 1;
                }
                let next = *bytes
                    .get(offset)
                    .ok_or(Error::Invalid("truncated JPEG entropy marker"))?;
                if next == 0 || (0xD0..=0xD7).contains(&next) {
                    offset += 1;
                } else {
                    offset = start;
                    break;
                }
            }
        }
    }
}
