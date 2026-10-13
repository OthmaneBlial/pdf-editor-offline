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

fn contents(
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
fn balanced(bytes: &[u8]) -> Result<usize> {
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

impl PdfDocument {
    /// Add a top-aligned Unicode overlay using an explicitly chosen covering font.
    /// Exact line breaks are preserved; wrapping is bounded, overflow is refused.
    pub fn add_text_overlay(&mut self, overlay: &TextOverlay, font_bytes: &[u8]) -> Result<()> {
        if self.encrypted_input || self.info()?.signature_fields != 0 {
            return Err(Error::Refused(
                "signed or decrypted documents are read-only",
            ));
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
        let info = self.info()?;
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
                &mut budget,
                &mut BTreeSet::new(),
                &mut ids,
                &mut source,
                0,
            )?;
        }
        let original_operators = balanced(&source)?;
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
                appearance::copied(value, 0, &mut budget)?;
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
                appearance::copied(value, 0, &mut budget)?;
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
        let mut font = FontPlan::new(font_bytes)?;
        let layout = multiline::layout_box(
            &mut font,
            &overlay.value,
            &BoxLayout {
                width: x1 - x0,
                height: y1 - y0,
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
                    rectangle: [0.0, 0.0, x1 - x0, y1 - y0],
                },
                b"OverlayFont",
                layout.size,
                &layout.glyphs[glyphs.clone()],
                0.0,
                y1 - y0,
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
        let encoded = Content { operations }.encode()?;
        if encoded.len() > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("text overlay stream exceeds 32 MiB"));
        }
        let extra = 12;
        if self.document.objects.len().saturating_add(extra) > crate::MAX_OBJECTS
            || self.document.max_id as usize + extra >= crate::MAX_OBJECTS
        {
            return Err(Error::Limit("overlay object allocation budget"));
        }
        let existing = font.existing_program(&self.document)?;
        let mut new_resources = resources.cloned().unwrap_or_default();
        let mut new_xobjects = xobjects.cloned().unwrap_or_default();
        let mut candidate = self.document.clone();
        crate::require_pdf_1_4(&mut candidate);
        let font_id = font.embed(&mut candidate, existing)?;
        let form=candidate.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![real(0.0),real(0.0),real(x1-x0),real(y1-y0)],"Resources"=>dictionary!{"Font"=>dictionary!{"OverlayFont"=>font_id}}},encoded));
        new_xobjects.set(name.clone(), form);
        new_resources.set("XObject", new_xobjects);
        let prefix = candidate.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
        let tail = Content {
            operations: vec![
                op("Q", vec![]),
                op("n", vec![]),
                op("q", vec![]),
                op(
                    "cm",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), real(x0), real(y0)],
                ),
                op("Do", vec![Object::Name(name)]),
                op("Q", vec![]),
            ],
        }
        .encode()?;
        let suffix = candidate.add_object(Stream::new(Dictionary::new(), tail));
        let mut new_contents = vec![Object::Reference(prefix)];
        new_contents.extend(ids.into_iter().map(Object::Reference));
        new_contents.push(Object::Reference(suffix));
        let page = candidate.get_object_mut(page_id)?.as_dict_mut()?;
        page.set("Resources", new_resources);
        page.set("Contents", new_contents);
        let rebuilt = Self::from_document(candidate, false)?;
        if rebuilt.info()?.pages != info.pages {
            return Err(Error::Invalid("text overlay changed page geometry"));
        }
        rebuilt.verified_bytes()?;
        *self = rebuilt;
        Ok(())
    }
}
