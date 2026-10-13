//! Direct page image instances. Replacement changes one Do operand, never paints a cover.
use super::{Budget, appearance, image_overlay};
use crate::{Error, PdfDocument, Result};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageSelection {
    pub page: u32,
    /// One-based operator ordinal in the concatenated page content program.
    pub operator: u32,
    pub image_id: ObjectId,
    pub resource_name: Vec<u8>,
    /// SHA-256 of bounded canonical serialization; invalidated by any document edit.
    pub source_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageInstance {
    pub selection: ImageSelection,
    pub matrix: [f64; 6],
    /// Nominal unrotated PDF-space corners; existing clipping/opacity still apply.
    pub corners: [[f64; 2]; 4],
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub replaceable: bool,
    pub refusal_reason: Option<String>,
}
struct Program {
    bytes: Vec<u8>,
    spans: Vec<crate::content_spans::OperatorSpan>,
    resources: Dictionary,
    xobjects: Dictionary,
    images: Vec<ImageInstance>,
}
fn collect(
    pdf: &PdfDocument,
    value: &Object,
    budget: &mut Budget,
    active: &mut BTreeSet<ObjectId>,
    bytes: &mut Vec<u8>,
    depth: usize,
) -> Result<()> {
    if depth >= 128 {
        return Err(Error::Limit("image content nesting exceeds 128"));
    }
    budget.work(1)?;
    match value {
        Object::Reference(id) => {
            if !active.insert(*id) {
                return Err(Error::Invalid("cyclic image page contents"));
            }
            let value = pdf
                .document
                .objects
                .get(id)
                .ok_or(Error::Invalid("missing image content reference"))?;
            if let Object::Stream(stream) = value {
                if stream.dict.iter().any(|(key, _)| {
                    !matches!(key.as_slice(), b"Length" | b"Filter" | b"DecodeParms")
                }) {
                    return Err(Error::Refused(
                        "image replacement content stream carries unsupported semantics",
                    ));
                }
                let remaining = crate::MAX_STREAM_BYTES.saturating_sub(bytes.len());
                let decoded = stream.decompressed_content_with_limit(remaining)?;
                if decoded.len() > remaining {
                    return Err(Error::Limit("image content exceeds 32 MiB"));
                }
                bytes.extend_from_slice(&decoded);
            } else {
                collect(pdf, value, budget, active, bytes, depth + 1)?;
            }
            active.remove(id);
        }
        Object::Array(values) => {
            budget.work(values.len())?;
            for value in values {
                collect(pdf, value, budget, active, bytes, depth + 1)?;
            }
        }
        _ => {
            return Err(Error::Refused(
                "image replacement requires indirect page content streams",
            ));
        }
    }
    Ok(())
}
fn operands(values: &[Object], budget: &mut Budget) -> Result<()> {
    budget.work(values.len().saturating_add(1))?;
    let mut pending = values.iter().map(|v| (v, 0usize)).collect::<Vec<_>>();
    while let Some((value, depth)) = pending.pop() {
        if depth >= 128 {
            return Err(Error::Limit("image content operand nesting exceeds 128"));
        }
        match value {
            Object::Array(values) => {
                budget.work(values.len())?;
                pending.extend(values.iter().map(|v| (v, depth + 1)));
            }
            Object::Dictionary(dict) => {
                budget.work(dict.len())?;
                pending.extend(dict.iter().map(|(_, v)| (v, depth + 1)));
            }
            Object::Real(n) if !n.is_finite() => {
                return Err(Error::Invalid("image content contains nonfinite numbers"));
            }
            Object::Reference(_) | Object::Stream(_) => {
                return Err(Error::Refused(
                    "image content operands cannot contain indirect objects or streams",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
fn number(value: &Object, raw: &[u8]) -> Result<f64> {
    match value {
        Object::Integer(_) | Object::Real(_) => {}
        _ => return Err(Error::Invalid("image graphics matrix requires numbers")),
    };
    let n = std::str::from_utf8(raw)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or(Error::Invalid("image matrix number token is invalid"))?;
    if !n.is_finite() || n.abs() > 1_000_000.0 {
        return Err(Error::Refused(
            "image graphics matrix is nonfinite or outside supported range",
        ));
    }
    Ok(n)
}
fn compose(a: [f64; 6], b: [f64; 6]) -> Result<[f64; 6]> {
    let m = [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ];
    if m.iter().any(|n| !n.is_finite() || n.abs() > 1_000_000.0) {
        return Err(Error::Refused(
            "composed image matrix exceeds supported range",
        ));
    }
    Ok(m)
}
fn program(pdf: &PdfDocument, page: u32) -> Result<Program> {
    pdf.validate_selection(&[page])?;
    let page_id = pdf.pages[page as usize - 1];
    let mut budget = Budget::new();
    let catalog = budget
        .resolve(&pdf.document, pdf.document.trailer.get(b"Root")?)?
        .1
        .as_dict()?;
    let tagged = catalog.has(b"StructTreeRoot")
        || pdf
            .document
            .get_object(page_id)?
            .as_dict()?
            .has(b"StructParents");
    let readonly = if pdf.encrypted_input {
        Some("Decrypted PDFs are read-only")
    } else if pdf.info()?.signature_fields > 0 {
        Some("Signed PDFs are read-only")
    } else if tagged {
        Some("Tagged images require semantic structure support")
    } else {
        None
    };
    let resources = crate::structure::inherited_raw(&pdf.document, page_id, b"Resources")?
        .map(|v| {
            budget
                .resolve(&pdf.document, v)
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
                .resolve(&pdf.document, v)
                .and_then(|(_, v)| Ok(v.as_dict()?))
        })
        .transpose()?;
    if let Some(values) = xobjects {
        for (key, value) in values.iter() {
            budget.report(key.len())?;
            appearance::copied(value, 0, &mut budget)?;
        }
    }
    let mut bytes = Vec::new();
    if let Ok(contents) = pdf
        .document
        .get_object(page_id)?
        .as_dict()?
        .get(b"Contents")
    {
        collect(
            pdf,
            contents,
            &mut budget,
            &mut BTreeSet::new(),
            &mut bytes,
            0,
        )?;
    }
    let (content, spans) = crate::content_spans::decode_spanned(&bytes)?;
    if content.operations.len() > 100_000 {
        return Err(Error::Limit("image page exceeds 100000 operators"));
    }
    let source = crate::save::serialize_bounded(&pdf.document, crate::MAX_INPUT_BYTES)?;
    let source_hash = Sha256::digest(&source)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    drop(source);
    let mut matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut stack = Vec::new();
    let mut images = Vec::new();
    let mut text = false;
    let mut marked = 0usize;
    for (index, op) in content.operations.iter().enumerate() {
        operands(&op.operands, &mut budget)?;
        match op.operator.as_str() {
            "q" if op.operands.is_empty() => {
                if stack.len() >= 32 {
                    return Err(Error::Limit("image graphics nesting exceeds 32"));
                }
                stack.push(matrix);
            }
            "Q" if op.operands.is_empty() => {
                matrix = stack
                    .pop()
                    .ok_or(Error::Invalid("image graphics stack underflow"))?;
            }
            "cm" if op.operands.len() == 6 => {
                let mut m = [0.0; 6];
                for (dst, (value, range)) in m
                    .iter_mut()
                    .zip(op.operands.iter().zip(&spans[index].operands))
                {
                    *dst = number(value, &bytes[range.clone()])?;
                }
                matrix = compose(matrix, m)?;
            }
            "q" | "Q" | "cm" => return Err(Error::Invalid("malformed image graphics operator")),
            "BT" if !text && op.operands.is_empty() => text = true,
            "ET" if text && op.operands.is_empty() => text = false,
            "BT" | "ET" => return Err(Error::Invalid("malformed image text nesting")),
            "BI" | "ID" | "EI" => {
                return Err(Error::Refused("inline image replacement is unsupported"));
            }
            "BDC" => {
                return Err(Error::Refused(
                    "image page property-marked content is unsupported",
                ));
            }
            "BMC" if op.operands.len() == 1 && matches!(&op.operands[0], Object::Name(_)) => {
                marked += 1;
                if marked > 128 {
                    return Err(Error::Limit("image marked nesting exceeds 128"));
                }
            }
            "EMC" if op.operands.is_empty() => {
                marked = marked
                    .checked_sub(1)
                    .ok_or(Error::Invalid("image marked nesting underflow"))?;
            }
            "BMC" | "EMC" => return Err(Error::Invalid("malformed image marked content")),
            "Do" => {
                if text || op.operands.len() != 1 {
                    return Err(Error::Invalid("image Do must have one name outside text"));
                }
                let name = op.operands[0].as_name()?;
                if name.len() > 4096 {
                    return Err(Error::Limit("image resource name exceeds 4096 bytes"));
                }
                let entry = xobjects
                    .ok_or(Error::Invalid("image XObject resources missing"))?
                    .get(name)?;
                let (id, value) = budget.resolve(&pdf.document, entry)?;
                let stream = value.as_stream()?;
                if stream.dict.get(b"Subtype")?.as_name()? != b"Image" {
                    return Err(Error::Refused(
                        "nested Form XObjects need recursive image instance support",
                    ));
                }
                let id = id.ok_or(Error::Refused("image XObject must be indirect"))?;
                let width = stream.dict.get(b"Width")?.as_i64()?;
                let height = stream.dict.get(b"Height")?.as_i64()?;
                if width <= 0 || height <= 0 || width > u32::MAX as i64 || height > u32::MAX as i64
                {
                    return Err(Error::Invalid("image pixel dimensions invalid"));
                }
                let reason = readonly.or_else(|| {
                    if matrix[0] * matrix[3] - matrix[1] * matrix[2] == 0.0 {
                        Some("Image placement is singular")
                    } else if [b"OC".as_slice(), b"StructParent", b"Alternates", b"OPI"]
                        .iter()
                        .any(|key| stream.dict.has(key))
                    {
                        Some("Image carries unsupported structure or alternate semantics")
                    } else {
                        None
                    }
                });
                if images.len() >= 10_000 {
                    return Err(Error::Limit("image report exceeds 10000 instances"));
                }
                budget.report(name.len().saturating_mul(4).saturating_add(1024))?;
                images.push(ImageInstance {
                    selection: ImageSelection {
                        page,
                        operator: index as u32 + 1,
                        image_id: id,
                        resource_name: name.to_vec(),
                        source_hash: source_hash.clone(),
                    },
                    matrix,
                    corners: [
                        [matrix[4], matrix[5]],
                        [matrix[0] + matrix[4], matrix[1] + matrix[5]],
                        [
                            matrix[0] + matrix[2] + matrix[4],
                            matrix[1] + matrix[3] + matrix[5],
                        ],
                        [matrix[2] + matrix[4], matrix[3] + matrix[5]],
                    ],
                    pixel_width: width as u32,
                    pixel_height: height as u32,
                    replaceable: reason.is_none(),
                    refusal_reason: reason.map(str::to_owned),
                });
            }
            _ => {}
        }
    }
    if !stack.is_empty() || text || marked != 0 {
        return Err(Error::Invalid("unbalanced image page content state"));
    }
    Ok(Program {
        bytes,
        spans,
        resources: resources.cloned().unwrap_or_default(),
        xobjects: xobjects.cloned().unwrap_or_default(),
        images,
    })
}
impl PdfDocument {
    /// Inventory direct page Image Do occurrences; nominal transforms are not visible-pixel bounds.
    pub fn list_image_instances(&self, page: u32) -> Result<Vec<ImageInstance>> {
        Ok(program(self, page)?.images)
    }
    /// Import a validated donor image and change exactly one selected page Do. Old objects
    /// remain retained, so this is ordinary editing, never secure erasure or sanitization.
    pub fn replace_image_instance(
        &mut self,
        selection: &ImageSelection,
        image_pdf: &PdfDocument,
    ) -> Result<()> {
        self.ensure_editable()?;
        if selection.operator == 0
            || selection.operator > 100_000
            || selection.resource_name.len() > 4096
            || selection.source_hash.len() != 64
            || !selection.source_hash.bytes().all(|b| b.is_ascii_hexdigit())
            || selection.image_id.0 == 0
        {
            return Err(Error::Invalid("image selection identity is malformed"));
        }
        let mut plan = program(self, selection.page)?;
        let selected = plan
            .images
            .iter()
            .find(|image| image.selection == *selection)
            .ok_or(Error::Refused(
                "image selection is stale or belongs to another source",
            ))?;
        if !selected.replaceable {
            return Err(Error::Refused("selected image is structurally read-only"));
        }
        if self.document.objects.len().saturating_add(4) > crate::MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(4) >= crate::MAX_OBJECTS
        {
            return Err(Error::Limit("image replacement object budget"));
        }
        let asset = image_overlay::image_asset(image_pdf)?;
        let mut name = b"NativeImageReplacement".to_vec();
        for i in 0..=100_000 {
            if !plan.xobjects.has(&name) {
                break;
            }
            if i == 100_000 {
                return Err(Error::Limit("image replacement resource name budget"));
            }
            name = format!("NativeImageReplacement{i}").into_bytes();
        }
        let span = plan
            .spans
            .get(selection.operator as usize - 1)
            .ok_or(Error::Invalid("selected image operator span missing"))?;
        if &plan.bytes[span.operator.clone()] != b"Do" || span.operands.len() != 1 {
            return Err(Error::Refused("selected image lexical span is ambiguous"));
        }
        let range = span.operands[0].clone();
        if range.start < span.full.start || range.end > span.full.end {
            return Err(Error::Refused("image operand exceeds its operator span"));
        }
        let output_len = plan
            .bytes
            .len()
            .saturating_sub(range.len())
            .saturating_add(name.len() + 1);
        if output_len > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("rewritten image content exceeds 32 MiB"));
        }
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(output_len)
            .map_err(|_| Error::Limit("cannot allocate bounded image replacement content"))?;
        encoded.extend_from_slice(&plan.bytes[..range.start]);
        encoded.push(b'/');
        encoded.extend_from_slice(&name);
        encoded.extend_from_slice(&plan.bytes[range.end..]);
        if encoded.len() > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("rewritten image content exceeds 32 MiB"));
        }
        let original_info = self.info()?;
        let mut candidate = self.document.clone();
        crate::require_pdf_1_4(&mut candidate);
        let image = image_overlay::import_asset(&mut candidate, asset);
        plan.xobjects.set(name, image);
        plan.resources.set("XObject", plan.xobjects);
        let content = candidate.add_object(Stream::new(Dictionary::new(), encoded));
        let page = candidate
            .get_object_mut(self.pages[selection.page as usize - 1])?
            .as_dict_mut()?;
        page.set("Resources", plan.resources);
        page.set("Contents", content);
        let rebuilt = Self::from_document(candidate, false)?;
        if rebuilt.info()?.pages != original_info.pages {
            return Err(Error::Verification(
                "image replacement changed page geometry",
            ));
        }
        rebuilt.verified_bytes()?;
        *self = rebuilt;
        Ok(())
    }
}
