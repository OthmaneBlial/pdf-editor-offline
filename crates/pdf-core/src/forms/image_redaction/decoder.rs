//! Fail-closed privacy samples; renderer recovery is deliberately not reused.
use super::*;
use lopdf::dictionary;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Raster {
    pub id: ObjectId,
    pub width: i64,
    pub height: i64,
    pub gray: bool,
    pub data: Vec<u8>,
    pub mask: Option<ObjectId>,
    interpolate: Option<bool>,
    intent: Option<Vec<u8>>,
}
impl Raster {
    pub fn canonical(&self) -> Stream {
        let mut dict = lopdf::dictionary! {
            "Type"=>"XObject", "Subtype"=>"Image", "Width"=>self.width,
            "Height"=>self.height, "BitsPerComponent"=>8,
            "ColorSpace"=>if self.gray { "DeviceGray" } else { "DeviceRGB" },
        };
        if let Some(mask) = self.mask {
            dict.set("SMask", mask);
        }
        if let Some(value) = self.interpolate {
            dict.set("Interpolate", value);
        }
        if let Some(value) = &self.intent {
            dict.set("Intent", Object::Name(value.clone()));
        }
        Stream::new(dict, self.data.clone())
    }
}
fn value<'a>(
    document: &'a lopdf::Document,
    dict: &'a Dictionary,
    key: &[u8],
    budget: &mut Budget,
) -> Result<&'a Object> {
    Ok(budget.resolve(document, dict.get(key)?)?.1)
}
pub(super) fn decode(
    pdf: &PdfDocument,
    id: ObjectId,
    rasters: &mut BTreeMap<ObjectId, Raster>,
    budget: &mut Budget,
    decoded: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > 1 {
        return Err(Error::Refused("nested privacy image masks are unsupported"));
    }
    if rasters.contains_key(&id) {
        return Ok(());
    }
    let stream = pdf.document.get_object(id)?.as_stream()?;
    if value(&pdf.document, &stream.dict, b"Length", budget)?.as_i64()?
        != stream.content.len() as i64
    {
        return Err(Error::Refused(
            "privacy image declared transport length mismatch",
        ));
    }
    if stream.dict.iter().any(|(key, _)| {
        ![
            b"Type".as_slice(),
            b"Subtype",
            b"Width",
            b"Height",
            b"BitsPerComponent",
            b"ColorSpace",
            b"Length",
            b"Filter",
            b"DecodeParms",
            b"Decode",
            b"SMask",
            b"Mask",
            b"ImageMask",
            b"Interpolate",
            b"Intent",
            b"Metadata",
        ]
        .contains(&key.as_slice())
    }) {
        return Err(Error::Refused(
            "privacy image has unsupported dictionary carriers",
        ));
    }
    if value(&pdf.document, &stream.dict, b"Subtype", budget)?.as_name()? != b"Image"
        || value(&pdf.document, &stream.dict, b"BitsPerComponent", budget)?.as_i64()? != 8
    {
        return Err(Error::Refused(
            "privacy decoding requires eight-bit image samples",
        ));
    }
    if stream.dict.has(b"Type")
        && value(&pdf.document, &stream.dict, b"Type", budget)?.as_name()? != b"XObject"
    {
        return Err(Error::Refused("invalid privacy image type"));
    }
    let gray = match value(&pdf.document, &stream.dict, b"ColorSpace", budget)?.as_name()? {
        b"DeviceGray" => true,
        b"DeviceRGB" if depth == 0 => false,
        _ => {
            return Err(Error::Refused(
                "privacy decoding requires DeviceGray or DeviceRGB",
            ));
        }
    };
    let width = value(&pdf.document, &stream.dict, b"Width", budget)?.as_i64()?;
    let height = value(&pdf.document, &stream.dict, b"Height", budget)?.as_i64()?;
    if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
        return Err(Error::Limit("privacy image dimensions exceed 8192"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .filter(|count| *count <= 8_000_000)
        .ok_or(Error::Limit("privacy image exceeds eight million pixels"))?;
    let expected = pixels
        .checked_mul(if gray { 1 } else { 3 })
        .ok_or(Error::Limit("privacy image sample multiplication"))?;
    *decoded = decoded.saturating_add(expected);
    if *decoded > 64 * 1024 * 1024 {
        return Err(Error::Limit("privacy decoded image samples exceed 64 MiB"));
    }
    if let Ok(filter) = stream.dict.get(b"Filter")
        && budget.resolve(&pdf.document, filter)?.1.as_name()? != b"FlateDecode"
    {
        return Err(Error::Refused(
            "privacy image codec requires plain or Flate samples",
        ));
    }
    if let Ok(params) = stream.dict.get(b"DecodeParms")
        && !matches!(budget.resolve(&pdf.document, params)?.1, Object::Null)
    {
        return Err(Error::Refused(
            "privacy image predictors and decode parameters are unsupported",
        ));
    }
    if let Ok(mask) = stream.dict.get(b"Mask")
        && !matches!(budget.resolve(&pdf.document, mask)?.1, Object::Null)
    {
        return Err(Error::Refused(
            "privacy explicit/color-key masks are unsupported",
        ));
    }
    if let Ok(mask) = stream.dict.get(b"ImageMask")
        && budget.resolve(&pdf.document, mask)?.1.as_bool()?
    {
        return Err(Error::Refused(
            "privacy stencil masks require a separate decoder",
        ));
    }
    if let Ok(decode) = stream.dict.get(b"Decode") {
        let values = budget.resolve(&pdf.document, decode)?.1.as_array()?;
        if values.len() != if gray { 2 } else { 6 } {
            return Err(Error::Refused("privacy Decode array length"));
        }
        budget.work(values.len())?;
        for (index, value) in values.iter().enumerate() {
            if budget.resolve(&pdf.document, value)?.1.as_float()? != (index % 2) as f32 {
                return Err(Error::Refused("privacy nonidentity Decode array"));
            }
        }
    }
    let interpolate = stream
        .dict
        .get(b"Interpolate")
        .ok()
        .map(|v| Ok::<_, Error>(budget.resolve(&pdf.document, v)?.1.as_bool()?))
        .transpose()?;
    let intent = stream
        .dict
        .get(b"Intent")
        .ok()
        .map(|v| {
            let name = budget.resolve(&pdf.document, v)?.1.as_name()?;
            if !matches!(
                name,
                b"AbsoluteColorimetric" | b"RelativeColorimetric" | b"Saturation" | b"Perceptual"
            ) {
                return Err(Error::Refused("privacy image rendering intent"));
            }
            Ok(name.to_vec())
        })
        .transpose()?;
    let mask = stream
        .dict
        .get(b"SMask")
        .ok()
        .map(|v| {
            let (id, object) = budget.resolve(&pdf.document, v)?;
            if matches!(object, Object::Null) {
                return Ok(None);
            }
            if depth != 0 {
                return Err(Error::Refused("privacy soft mask has its own mask"));
            }
            Ok(Some(id.ok_or(Error::Refused(
                "privacy image mask must be indirect",
            ))?))
        })
        .transpose()?
        .flatten();
    if let Some(mask) = mask {
        if mask == id {
            return Err(Error::Refused("cyclic privacy image mask"));
        }
        decode(pdf, mask, rasters, budget, decoded, depth + 1)?;
        let raster = &rasters[&mask];
        if !raster.gray || raster.width != width || raster.height != height || raster.mask.is_some()
        {
            return Err(Error::Refused(
                "privacy soft mask dimensions or structure mismatch",
            ));
        }
    }
    // Check exact length, including excess raw bytes. A recoverable renderer
    // truncating or padding sample data cannot establish this privacy condition.
    let data = stream.get_plain_content_with_limit(expected)?;
    if data.len() != expected {
        return Err(Error::Refused("privacy image sample length mismatch"));
    }
    rasters.insert(
        id,
        Raster {
            id,
            width,
            height,
            gray,
            data,
            mask,
            interpolate,
            intent,
        },
    );
    Ok(())
}
