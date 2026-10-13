//! New text over existing page artwork; never edits or removes existing text.
use super::{
    Budget,
    appearance::{
        self, LineLayout, Style,
        fonts::FontPlan,
        multiline::{self, BoxLayout},
    },
};
use crate::{Error, PdfDocument, Result};
use lopdf::{
    Dictionary, Object, ObjectId, Stream,
    content::{Content, Operation},
    dictionary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextOverlay {
    /// One-based page; coordinates are unrotated PDF user-space coordinates.
    pub page: u32,
    pub rectangle: [f64; 4],
    pub value: String,
    pub size: f64,
    pub color: [f64; 3],
    pub alignment: TextAlignment,
}
fn real(n: f64) -> Object {
    Object::Real(n as f32)
}
fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

pub(super) fn contents(
    document: &lopdf::Document,
    value: &Object,
    budget: &mut Budget,
    active: &mut BTreeSet<ObjectId>,
    ids: &mut Vec<ObjectId>,
    bytes: &mut Vec<u8>,
    depth: usize,
) -> Result<()> {
    if depth >= 128 {
        return Err(Error::Limit("overlay content nesting exceeds 128"));
    }
    budget.work(1)?;
    if let Object::Reference(id) = value {
        if !active.insert(*id) {
            return Err(Error::Invalid("cyclic page contents"));
        }
        let object = document.get_object(*id)?;
        if let Object::Stream(stream) = object {
            if ids.len() >= 10_000 {
                return Err(Error::Limit("overlay page content work budget"));
            }
            let remaining = crate::MAX_STREAM_BYTES.saturating_sub(bytes.len());
            let decoded = stream.decompressed_content_with_limit(remaining)?;
            if decoded.len().saturating_add(1) > remaining {
                return Err(Error::Limit("overlay page content exceeds 32 MiB"));
            }
            bytes.extend(decoded);
            bytes.push(b'\n');
            ids.push(*id);
        } else {
            contents(document, object, budget, active, ids, bytes, depth + 1)?;
        }
        active.remove(id);
    } else if let Object::Array(values) = value {
        for object in values {
            contents(document, object, budget, active, ids, bytes, depth + 1)?;
        }
    } else {
        return Err(Error::Refused("overlay requires indirect content streams"));
    }
    Ok(())
}
pub(super) fn balanced(bytes: &[u8]) -> Result<usize> {
    let content = Content::decode_strict(bytes)?;
    if content.operations.len() > 100_000 {
        return Err(Error::Limit("overlay source exceeds 100000 operators"));
    }
    let count = content.operations.len();
    let mut graphics = 0usize;
    let mut marked = 0usize;
    let mut text = false;
    for operation in content.operations {
        match operation.operator.as_str() {
            "q" => {
                graphics += 1;
                // The new page wrapper consumes one of the text reader's 32
                // graphics-state levels; do not publish an unreadable candidate.
                if graphics > 31 {
                    return Err(Error::Limit("overlay source graphics nesting"));
                }
            }
            "Q" => {
                graphics = graphics.checked_sub(1).ok_or(Error::Refused(
                    "overlay cannot isolate unbalanced page graphics",
                ))?;
            }
            "BT" if !text => text = true,
            "ET" if text => text = false,
            "BT" | "ET" => {
                return Err(Error::Refused(
                    "overlay cannot isolate malformed text objects",
                ));
            }
            "BMC" | "BDC" => {
                marked += 1;
                if marked > 128 {
                    return Err(Error::Limit("overlay source marked nesting"));
                }
            }
            "EMC" => {
                marked = marked.checked_sub(1).ok_or(Error::Refused(
                    "overlay source marked content is unbalanced",
                ))?
            }
            _ => {}
        }
    }
    if graphics != 0 || marked != 0 || text {
        return Err(Error::Refused(
            "overlay requires balanced source graphics and text",
        ));
    }
    Ok(count)
}

struct OverlayPlan {
    page_id: ObjectId,
    ids: Vec<ObjectId>,
    resources: Dictionary,
    xobjects: Dictionary,
    name: Vec<u8>,
    encoded: Vec<u8>,
    width: f64,
    height: f64,
    matrix: [f64; 6],
}
impl PdfDocument {
    /// Add a top-aligned Unicode overlay using an explicitly chosen covering font.
    /// Exact line breaks are preserved; wrapping is bounded, overflow is refused.
    pub fn add_text_overlay(&mut self, overlay: &TextOverlay, font_bytes: &[u8]) -> Result<()> {
        self.add_text_overlay_batch(&[(overlay.clone(), 0)], font_bytes)
    }
    pub(super) fn add_text_overlay_batch(
        &mut self,
        overlays: &[(TextOverlay, u16)],
        font_bytes: &[u8],
    ) -> Result<()> {
        let info = self.info()?;
        if self.encrypted_input || info.signature_fields != 0 {
            return Err(Error::Refused(
                "signed or decrypted documents are read-only",
            ));
        }
        if overlays.is_empty() || overlays.len() > crate::MAX_PAGES {
            return Err(Error::Limit("overlay batch page count is invalid"));
        }
        let extra = overlays
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(12))
            .ok_or(Error::Limit("overlay object budget"))?;
        if self.document.objects.len().saturating_add(extra) > crate::MAX_OBJECTS
            || self.document.max_id as usize + extra >= crate::MAX_OBJECTS
        {
            return Err(Error::Limit("overlay object allocation budget"));
        }
        let mut font = FontPlan::new(font_bytes)?;
        let mut budget = Budget::new();
        let mut plans = Vec::with_capacity(overlays.len());
        let mut seen = BTreeSet::new();
        let mut stream_bytes = 0usize;
        for (overlay, rotation) in overlays {
            if !seen.insert(overlay.page) {
                return Err(Error::Invalid("overlay batch contains duplicate pages"));
            }
            let plan = self.plan_text_overlay(overlay, *rotation, &info, &mut font, &mut budget)?;
            stream_bytes = stream_bytes
                .checked_add(plan.encoded.len())
                .ok_or(Error::Limit("overlay batch stream budget"))?;
            if stream_bytes > crate::MAX_STREAM_BYTES {
                return Err(Error::Limit("overlay batch streams exceed 32 MiB"));
            }
            plans.push(plan);
        }
        let existing = font.existing_program(&self.document)?;
        let mut candidate = self.document.clone();
        crate::require_pdf_1_4(&mut candidate);
        let font_id = font.embed(&mut candidate, existing)?;
        for mut plan in plans {
            let form = candidate.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![real(0.0),real(0.0),real(plan.width),real(plan.height)],"Resources"=>dictionary!{"Font"=>dictionary!{"OverlayFont"=>font_id}}}, plan.encoded));
            plan.xobjects.set(plan.name.clone(), form);
            plan.resources.set("XObject", plan.xobjects);
            let prefix = candidate.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
            let suffix = candidate.add_object(Stream::new(
                Dictionary::new(),
                Content {
                    operations: vec![
                        op("n", vec![]),
                        op("Q", vec![]),
                        op("q", vec![]),
                        op("cm", plan.matrix.into_iter().map(real).collect()),
                        op("Do", vec![Object::Name(plan.name)]),
                        op("Q", vec![]),
                    ],
                }
                .encode()?,
            ));
            let mut contents = vec![Object::Reference(prefix)];
            contents.extend(plan.ids.into_iter().map(Object::Reference));
            contents.push(Object::Reference(suffix));
            let page = candidate.get_object_mut(plan.page_id)?.as_dict_mut()?;
            page.set("Resources", plan.resources);
            page.set("Contents", contents);
        }
        let rebuilt = Self::from_document(candidate, false)?;
        if rebuilt.info()?.pages != info.pages {
            return Err(Error::Invalid("text overlay changed page geometry"));
        }
        rebuilt.verified_bytes()?;
        *self = rebuilt;
        Ok(())
    }
    fn plan_text_overlay(
        &self,
        overlay: &TextOverlay,
        rotation: u16,
        info: &crate::DocumentInfo,
        font: &mut FontPlan<'_>,
        budget: &mut Budget,
    ) -> Result<OverlayPlan> {
        if !matches!(rotation, 0 | 90 | 180 | 270) {
            return Err(Error::Invalid("overlay painting rotation is invalid"));
        }
        self.validate_selection(&[overlay.page])?;
        if overlay.value.is_empty() {
            return Err(Error::Invalid("text overlay value is empty"));
        }
        if overlay.value.len() > 64 * 1024 {
            return Err(Error::Limit("text overlay value exceeds 64 KiB"));
        }
        if !overlay.size.is_finite()
            || !(1.0..=1024.0).contains(&overlay.size)
            || overlay
                .color
                .iter()
                .any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
        {
            return Err(Error::Invalid("overlay size or RGB color is invalid"));
        }
        let [x0, y0, x1, y1] = overlay.rectangle;
        if overlay
            .rectangle
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
            || x0 >= x1
            || y0 >= y1
        {
            return Err(Error::Invalid("overlay rectangle is invalid"));
        }
        let page_id = self.pages[overlay.page as usize - 1];
        let page = &info.pages[overlay.page as usize - 1];
        for bounds in [page.media_box, page.crop_box] {
            if x0 < f64::from(bounds[0])
                || y0 < f64::from(bounds[1])
                || x1 > f64::from(bounds[2])
                || y1 > f64::from(bounds[3])
            {
                return Err(Error::Refused(
                    "overlay rectangle must be within the visible page",
                ));
            }
        }
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
                "text overlays on tagged pages need semantic tagging support",
            ));
        }
        let mut source = Vec::new();
        let mut ids = Vec::new();
        if let Ok(value) = self
            .document
            .get_object(page_id)?
            .as_dict()?
            .get(b"Contents")
        {
            contents(
                &self.document,
                value,
                budget,
                &mut BTreeSet::new(),
                &mut ids,
                &mut source,
                0,
            )?;
        }
        budget.appearance_bytes = budget
            .appearance_bytes
            .checked_add(source.len())
            .ok_or(Error::Limit("overlay batch source byte budget"))?;
        if budget.appearance_bytes > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("overlay batch decoded source exceeds 32 MiB"));
        }
        let original_operators = balanced(&source)?;
        budget.work(original_operators)?;
        let resources = crate::structure::inherited_raw(&self.document, page_id, b"Resources")?
            .map(|v| {
                budget
                    .resolve(&self.document, v)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        if let Some(resources) = resources {
            for (key, value) in resources.iter() {
                budget.report(key.len())?;
                appearance::copied(value, 0, budget)?;
            }
        }
        let xobjects = resources
            .and_then(|r| r.get(b"XObject").ok())
            .map(|v| {
                budget
                    .resolve(&self.document, v)
                    .and_then(|(_, v)| Ok(v.as_dict()?))
            })
            .transpose()?;
        if let Some(values) = xobjects {
            for (key, value) in values.iter() {
                budget.report(key.len())?;
                appearance::copied(value, 0, budget)?;
            }
        }
        let mut name = b"NativeTextOverlay".to_vec();
        for index in 0..=100_000 {
            if !xobjects.is_some_and(|v| v.has(&name)) {
                break;
            }
            if index == 100_000 {
                return Err(Error::Limit("overlay resource name budget"));
            }
            name = format!("NativeTextOverlay{index}").into_bytes();
        }
        let (width, height) = if matches!(rotation, 90 | 270) {
            (y1 - y0, x1 - x0)
        } else {
            (x1 - x0, y1 - y0)
        };
        if width as f32 <= 0.0
            || height as f32 <= 0.0
            || x0 as f32 >= x1 as f32
            || y0 as f32 >= y1 as f32
        {
            return Err(Error::Refused(
                "overlay geometry collapses at PDF number precision",
            ));
        }
        let layout = multiline::layout_box(
            font,
            &overlay.value,
            &BoxLayout {
                width,
                height,
                padding: 0.0,
                size: overlay.size,
                alignment: match overlay.alignment {
                    TextAlignment::Left => 0,
                    TextAlignment::Center => 1,
                    TextAlignment::Right => 2,
                },
            },
            &mut 0,
        )?;
        let line = font.shape("")?;
        let style = Style {
            size: overlay.size,
            color: op("rg", overlay.color.into_iter().map(real).collect()),
        };
        let mut operations = Vec::new();
        for (glyphs, logical) in &layout.runs {
            let logical_text = &overlay.value[logical.clone()];
            let painted_text =
                logical_text.trim_end_matches(['\r', '\n', '\u{85}', '\u{2028}', '\u{2029}']);
            operations.extend(appearance::text_operations(
                LineLayout {
                    line: &line,
                    value: painted_text,
                    style: &style,
                    q: 0,
                    rectangle: [0.0, 0.0, width, height],
                },
                b"OverlayFont",
                layout.size,
                &layout.glyphs[glyphs.clone()],
                0.0,
                height,
            )?);
            let separator = &logical_text[painted_text.len()..];
            if !separator.is_empty() {
                // Logical separators carry no painted glyph. Keep them exact in
                // a separate semantic span; including them in the painted row
                // lets some readers apply bidi across rows or duplicate breaks.
                operations.extend([
                    op("BDC",vec![Object::Name(b"Span".to_vec()),Object::Dictionary(dictionary!{"ActualText"=>appearance::fonts::unicode_string(separator)})]),
                    op("EMC",vec![]),
                ]);
            }
        }
        if original_operators
            .saturating_add(operations.len())
            .saturating_add(7)
            > 100_000
        {
            return Err(Error::Limit("text overlay exceeds 100000 operators"));
        }
        budget.work(operations.len())?;
        let encoded = Content { operations }.encode()?;
        if encoded.len() > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("text overlay stream exceeds 32 MiB"));
        }
        let matrix = match rotation {
            0 => [1.0, 0.0, 0.0, 1.0, x0, y0],
            90 => [0.0, 1.0, -1.0, 0.0, x1, y0],
            180 => [-1.0, 0.0, 0.0, -1.0, x1, y1],
            _ => [0.0, -1.0, 1.0, 0.0, x0, y1],
        };
        Ok(OverlayPlan {
            page_id,
            ids,
            resources: resources.cloned().unwrap_or_default(),
            xobjects: xobjects.cloned().unwrap_or_default(),
            name,
            encoded,
            width,
            height,
            matrix,
        })
    }
}
