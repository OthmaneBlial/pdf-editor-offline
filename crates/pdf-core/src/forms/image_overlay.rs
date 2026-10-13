//! Insert one bounded image asset; never replace or remove existing page content.
use super::{Budget, appearance, overlay};
use crate::{Error, PdfDocument, Result};
use lopdf::{
    Dictionary, Object, Stream,
    content::{Content, Operation},
    dictionary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageOverlay {
    /// One-based page; rectangle in unrotated PDF user space.
    pub page: u32,
    pub rectangle: [f64; 4],
    /// Counterclockwise quarter-turns of the image within its destination rectangle.
    pub rotation: u16,
}
struct Asset {
    image: Stream,
    mask: Option<Stream>,
    profile: Option<Stream>,
}
fn allow(dict: &Dictionary, keys: &[&[u8]]) -> Result<()> {
    if dict.iter().any(|(key, _)| !keys.contains(&key.as_slice())) {
        return Err(Error::Refused(
            "image asset has unsupported dictionary keys",
        ));
    }
    Ok(())
}
fn integer(dict: &Dictionary, key: &[u8]) -> Result<i64> {
    Ok(dict.get(key)?.as_i64()?)
}
fn samples(stream: &Stream, width: i64, height: i64, channels: usize, dct: bool) -> Result<()> {
    if stream.content.len() > crate::MAX_STREAM_BYTES {
        return Err(Error::Limit("image asset stream exceeds 32 MiB"));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(channels))
        .filter(|n| *n <= crate::MAX_STREAM_BYTES)
        .ok_or(Error::Limit("image decoded samples exceed 32 MiB"))?;
    match stream.dict.get(b"Filter") {
        Err(_) if stream.content.len() == expected => {}
        Ok(Object::Name(name)) if name == b"FlateDecode" => {
            if stream.decompressed_content_with_limit(expected)?.len() != expected {
                return Err(Error::Invalid(
                    "image decoded sample length differs from dimensions",
                ));
            }
        }
        Ok(Object::Name(name)) if dct && name == b"DCTDecode" => {
            // Raw image decoding belongs to pdf-convert. This API imports an already
            // decoded/validated image PDF; it does not independently decode JPEG.
            if !stream.content.starts_with(b"\xff\xd8") || !stream.content.ends_with(b"\xff\xd9") {
                return Err(Error::Invalid("image JPEG codestream markers are invalid"));
            }
        }
        _ => {
            return Err(Error::Refused(
                "image asset filter or sample encoding is unsupported",
            ));
        }
    }
    Ok(())
}
fn image_asset(pdf: &PdfDocument) -> Result<Asset> {
    pdf.ensure_editable()?;
    if pdf.page_count() != 1 {
        return Err(Error::Refused("overlay donor must have one image page"));
    }
    let mut budget = Budget::new();
    let resources = crate::structure::inherited_raw(&pdf.document, pdf.pages[0], b"Resources")?
        .ok_or(Error::Invalid("image donor resources missing"))?;
    let resources = budget.resolve(&pdf.document, resources)?.1.as_dict()?;
    let xobjects = budget
        .resolve(&pdf.document, resources.get(b"XObject")?)?
        .1
        .as_dict()?;
    if xobjects.len() != 1 {
        return Err(Error::Refused(
            "image donor must expose exactly one image XObject",
        ));
    }
    let stream = budget
        .resolve(
            &pdf.document,
            xobjects
                .iter()
                .next()
                .ok_or(Error::Invalid("image donor empty"))?
                .1,
        )?
        .1
        .as_stream()?;
    allow(
        &stream.dict,
        &[
            b"Type",
            b"Subtype",
            b"Width",
            b"Height",
            b"ColorSpace",
            b"BitsPerComponent",
            b"Interpolate",
            b"SMask",
            b"Filter",
            b"DecodeParms",
            b"Intent",
            b"Length",
        ],
    )?;
    if stream.dict.get(b"Type")?.as_name()? != b"XObject"
        || stream.dict.get(b"Subtype")?.as_name()? != b"Image"
        || integer(&stream.dict, b"BitsPerComponent")? != 8
    {
        return Err(Error::Refused(
            "image asset requires an eight-bit Image XObject",
        ));
    }
    let width = integer(&stream.dict, b"Width")?;
    let height = integer(&stream.dict, b"Height")?;
    if width <= 0
        || height <= 0
        || width > 8192
        || height > 8192
        || width.saturating_mul(height) > 8 * 1024 * 1024
    {
        return Err(Error::Limit(
            "image exceeds 8192 per side or 8388608 pixels",
        ));
    }
    if let Ok(value) = stream.dict.get(b"Interpolate")
        && !matches!(value, Object::Boolean(false))
    {
        return Err(Error::Refused("image interpolation tier is unsupported"));
    }
    let mut profile = None;
    let channels = match stream.dict.get(b"ColorSpace")? {
        Object::Name(name) if name == b"DeviceRGB" => 3,
        Object::Name(name) if name == b"DeviceGray" => 1,
        Object::Array(values) if values.len() == 2 && values[0].as_name()? == b"ICCBased" => {
            let icc = budget.resolve(&pdf.document, &values[1])?.1.as_stream()?;
            allow(&icc.dict, &[b"N", b"Alternate", b"Filter", b"Length"])?;
            if icc.content.len() > 1024 * 1024 {
                return Err(Error::Limit("image ICC encoded profile exceeds 1 MiB"));
            }
            match icc.dict.get(b"Filter") {
                Err(_) => {}
                Ok(Object::Name(name)) if name == b"FlateDecode" => {}
                _ => return Err(Error::Refused("image ICC filter tier is unsupported")),
            }
            if integer(&icc.dict, b"N")? != 3
                || icc.dict.get(b"Alternate")?.as_name()? != b"DeviceRGB"
            {
                return Err(Error::Refused(
                    "image ICC profile requires three RGB components",
                ));
            }
            let bytes = icc.decompressed_content_with_limit(1024 * 1024)?;
            if bytes.len() < 128
                || bytes.get(36..40) != Some(b"acsp")
                || bytes.get(16..20) != Some(b"RGB ")
                || u32::from_be_bytes(
                    bytes[..4]
                        .try_into()
                        .map_err(|_| Error::Invalid("ICC header"))?,
                ) as usize
                    != bytes.len()
            {
                return Err(Error::Invalid("image ICC profile header is invalid"));
            }
            profile = Some(Stream::new(
                dictionary! {"N"=>3,"Alternate"=>"DeviceRGB"},
                bytes,
            ));
            3
        }
        _ => return Err(Error::Refused("image asset color space is unsupported")),
    };
    if let Ok(value) = stream.dict.get(b"Intent")
        && !matches!(value,Object::Name(name) if matches!(name.as_slice(),b"Perceptual"|b"RelativeColorimetric"|b"Saturation"|b"AbsoluteColorimetric"))
    {
        return Err(Error::Refused("image rendering intent is unsupported"));
    }
    let dct = matches!(stream.dict.get(b"Filter"),Ok(Object::Name(name)) if name==b"DCTDecode");
    if let Ok(value) = stream.dict.get(b"DecodeParms") {
        let dict = value.as_dict()?;
        allow(dict, &[b"ColorTransform"])?;
        if !dct || !matches!(integer(dict, b"ColorTransform")?, 0 | 1) {
            return Err(Error::Refused("image decode parameters are unsupported"));
        }
    }
    samples(stream, width, height, channels, true)?;
    let mask = if let Ok(value) = stream.dict.get(b"SMask") {
        if dct {
            return Err(Error::Refused("JPEG donor soft masks are unsupported"));
        }
        let mask = budget.resolve(&pdf.document, value)?.1.as_stream()?;
        allow(
            &mask.dict,
            &[
                b"Type",
                b"Subtype",
                b"Width",
                b"Height",
                b"ColorSpace",
                b"BitsPerComponent",
                b"Interpolate",
                b"Filter",
                b"Length",
            ],
        )?;
        if mask.dict.get(b"Type")?.as_name()? != b"XObject"
            || mask.dict.get(b"Subtype")?.as_name()? != b"Image"
            || integer(&mask.dict, b"Width")? != width
            || integer(&mask.dict, b"Height")? != height
            || integer(&mask.dict, b"BitsPerComponent")? != 8
            || mask.dict.get(b"ColorSpace")?.as_name()? != b"DeviceGray"
        {
            return Err(Error::Refused(
                "image soft mask geometry or samples are unsupported",
            ));
        }
        if let Ok(value) = mask.dict.get(b"Interpolate")
            && !matches!(value, Object::Boolean(false))
        {
            return Err(Error::Refused("image mask interpolation is unsupported"));
        }
        samples(mask, width, height, 1, false)?;
        Some(mask.clone())
    } else {
        None
    };
    Ok(Asset {
        image: stream.clone(),
        mask,
        profile,
    })
}
fn real(value: f64) -> Object {
    Object::Real(value as f32)
}
impl PdfDocument {
    /// Insert the sole image XObject from a decoded/validated image PDF (such as
    /// pdf-convert::images_to_pdf). Only image/soft-mask/ICC data are imported.
    /// This method validates the PDF asset graph but does not decode JPEG input.
    pub fn add_image_overlay(
        &mut self,
        placement: &ImageOverlay,
        image_pdf: &PdfDocument,
    ) -> Result<()> {
        self.ensure_editable()?;
        self.validate_selection(&[placement.page])?;
        let [x0, y0, x1, y1] = placement.rectangle;
        if placement
            .rectangle
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
            || x0 >= x1
            || y0 >= y1
            || !matches!(placement.rotation, 0 | 90 | 180 | 270)
        {
            return Err(Error::Invalid(
                "image overlay rectangle or rotation is invalid",
            ));
        }
        let info = self.info()?;
        let page_id = self.pages[placement.page as usize - 1];
        for bounds in [
            info.pages[placement.page as usize - 1].media_box,
            info.pages[placement.page as usize - 1].crop_box,
        ] {
            if x0 < f64::from(bounds[0])
                || y0 < f64::from(bounds[1])
                || x1 > f64::from(bounds[2])
                || y1 > f64::from(bounds[3])
            {
                return Err(Error::Refused(
                    "image overlay rectangle must be within the visible page",
                ));
            }
        }
        let mut budget = Budget::new();
        let catalog = budget
            .resolve(&self.document, self.document.trailer.get(b"Root")?)?
            .1
            .as_dict()?;
        if catalog.has(b"StructTreeRoot")
            || self
                .document
                .get_object(page_id)?
                .as_dict()?
                .has(b"StructParents")
        {
            return Err(Error::Refused(
                "image overlays on tagged pages need semantic tagging support",
            ));
        }
        let mut ids = Vec::new();
        let mut source = Vec::new();
        if let Ok(value) = self
            .document
            .get_object(page_id)?
            .as_dict()?
            .get(b"Contents")
        {
            overlay::contents(
                &self.document,
                value,
                &mut budget,
                &mut BTreeSet::new(),
                &mut ids,
                &mut source,
                0,
            )?;
        }
        if overlay::balanced(&source)?.saturating_add(7) > 100_000 {
            return Err(Error::Limit("image overlay exceeds 100000 operators"));
        }
        let resources = crate::structure::inherited_raw(&self.document, page_id, b"Resources")?
            .map(|value| {
                budget
                    .resolve(&self.document, value)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        if let Some(resources) = resources {
            for (key, value) in resources.iter() {
                budget.report(key.len())?;
                appearance::copied(value, 0, &mut budget)?;
            }
        }
        let xobjects = resources
            .and_then(|r| r.get(b"XObject").ok())
            .map(|value| {
                budget
                    .resolve(&self.document, value)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        if let Some(values) = xobjects {
            for (key, value) in values.iter() {
                budget.report(key.len())?;
                appearance::copied(value, 0, &mut budget)?;
            }
        }
        let mut name = b"NativeImageOverlay".to_vec();
        for index in 0..=100_000 {
            if !xobjects.is_some_and(|values| values.has(&name)) {
                break;
            }
            if index == 100_000 {
                return Err(Error::Limit("image overlay resource-name budget"));
            }
            name = format!("NativeImageOverlay{index}").into_bytes();
        }
        if self.document.objects.len().saturating_add(5) > crate::MAX_OBJECTS
            || self.document.max_id as usize + 5 >= crate::MAX_OBJECTS
        {
            return Err(Error::Limit("image overlay object budget"));
        }
        let asset = image_asset(image_pdf)?;
        let mut resources = resources.cloned().unwrap_or_default();
        let mut xobjects = xobjects.cloned().unwrap_or_default();
        let mut candidate = self.document.clone();
        crate::require_pdf_1_4(&mut candidate);
        let mut image = asset.image;
        image.dict.set("Length", image.content.len() as i64);
        if let Some(mut mask) = asset.mask {
            mask.dict.set("Length", mask.content.len() as i64);
            image.dict.set("SMask", candidate.add_object(mask));
        }
        if let Some(profile) = asset.profile {
            let profile = candidate.add_object(profile);
            image.dict.set(
                "ColorSpace",
                vec![
                    Object::Name(b"ICCBased".to_vec()),
                    Object::Reference(profile),
                ],
            );
        }
        let image = candidate.add_object(image);
        xobjects.set(name.clone(), image);
        resources.set("XObject", xobjects);
        let w = x1 - x0;
        let h = y1 - y0;
        let matrix = match placement.rotation {
            0 => [w, 0.0, 0.0, h, x0, y0],
            90 => [0.0, h, -w, 0.0, x1, y0],
            180 => [-w, 0.0, 0.0, -h, x1, y1],
            _ => [0.0, -h, w, 0.0, x0, y1],
        };
        let prefix = candidate.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
        let tail = Content {
            operations: vec![
                Operation::new("Q", vec![]),
                Operation::new("n", vec![]),
                Operation::new("q", vec![]),
                Operation::new("cm", matrix.into_iter().map(real).collect()),
                Operation::new("Do", vec![Object::Name(name)]),
                Operation::new("Q", vec![]),
            ],
        }
        .encode()?;
        let suffix = candidate.add_object(Stream::new(Dictionary::new(), tail));
        let mut contents = vec![Object::Reference(prefix)];
        contents.extend(ids.into_iter().map(Object::Reference));
        contents.push(Object::Reference(suffix));
        let page = candidate.get_object_mut(page_id)?.as_dict_mut()?;
        page.set("Resources", resources);
        page.set("Contents", contents);
        let rebuilt = Self::from_document(candidate, false)?;
        if rebuilt.info()?.pages != info.pages {
            return Err(Error::Invalid("image overlay changed page geometry"));
        }
        rebuilt.verified_bytes()?;
        *self = rebuilt;
        Ok(())
    }
}
