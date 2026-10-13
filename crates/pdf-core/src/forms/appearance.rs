//! Single-line text with an explicitly supplied editable TrueType or CID CFF1 font.
//! No viewer regeneration, missing-glyph substitution, truncation or font fallback.
#[path = "fonts.rs"]
pub(super) mod fonts;
use super::{Budget, FormField, FormFieldKind, MAX_FIELDS, MAX_REPORT_BYTES, inherited, numeric};
use crate::{Error, PdfDocument, Result};
use fonts::{FontPlan, Line, unicode_string};
use lopdf::{
    Dictionary, Object, ObjectId, Stream,
    content::{Content, Operation},
    dictionary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextChange {
    pub name: String,
    pub value: String,
}

pub(super) struct Style {
    pub size: f64,
    pub color: Operation,
}
pub(super) struct WidgetPlan {
    pub field: ObjectId,
    pub widget: ObjectId,
    stream: ObjectId,
    pub content: Content<Vec<Operation>>,
    pub text_start: usize,
    pub text_end: usize,
    resources: Dictionary,
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub q: i64,
    pub style: Style,
    pub line: Line,
    pub value: String,
}

fn number(object: &Object) -> Result<f64> {
    let value = match object {
        Object::Integer(n) => *n as f64,
        Object::Real(n) => f64::from(*n),
        _ => return Err(Error::Invalid("text appearance number is not numeric")),
    };
    if !value.is_finite() {
        return Err(Error::Invalid("text appearance number is not finite"));
    }
    Ok(value)
}
fn style(bytes: &[u8]) -> Result<Style> {
    if bytes.len() > 4096 {
        return Err(Error::Limit("default appearance exceeds 4096 bytes"));
    }
    let content = Content::decode_strict(bytes)?;
    let mut size = None;
    let mut color = Operation::new("g", vec![0.into()]);
    for operation in content.operations {
        match operation.operator.as_str() {
            "Tf" if size.is_none() && operation.operands.len() == 2 => {
                operation.operands[0].as_name()?;
                let value = number(&operation.operands[1])?;
                if !(0.0..=1024.0).contains(&value) {
                    return Err(Error::Refused(
                        "default font size exceeds the supported range",
                    ));
                }
                size = Some(value);
            }
            "g" | "rg" | "k" => {
                let expected = match operation.operator.as_str() {
                    "g" => 1,
                    "rg" => 3,
                    _ => 4,
                };
                if operation.operands.len() != expected
                    || operation
                        .operands
                        .iter()
                        .any(|n| number(n).map_or(true, |n| !(0.0..=1.0).contains(&n)))
                {
                    return Err(Error::Refused("invalid default appearance color"));
                }
                color = operation;
            }
            _ => {
                return Err(Error::Refused(
                    "default appearance text-state operators need a later layout implementation",
                ));
            }
        }
    }
    Ok(Style {
        size: size.ok_or(Error::Invalid("default appearance has no font selection"))?,
        color,
    })
}

/// Bound copied inline dictionaries before cloning shared resources. Indirect
/// resources remain references; their streams are never copied by this walk.
fn copied(object: &Object, depth: usize, budget: &mut Budget) -> Result<()> {
    if depth >= 128 {
        return Err(Error::Limit("text resources exceed 128 levels"));
    }
    budget.work(1)?;
    match object {
        Object::Array(values) => {
            for value in values {
                copied(value, depth + 1, budget)?;
            }
        }
        Object::Dictionary(values) => {
            for (key, value) in values.iter() {
                budget.report(key.len())?;
                copied(value, depth + 1, budget)?;
            }
        }
        Object::Name(bytes) | Object::String(bytes, _) => budget.report(bytes.len())?,
        Object::Stream(_) => return Err(Error::Refused("inline resource streams are unsupported")),
        _ => {}
    }
    Ok(())
}

fn text_region(content: &Content<Vec<Operation>>) -> Result<(usize, usize)> {
    if content.operations.len() > 100_000 {
        return Err(Error::Limit("appearance exceeds 100000 operators"));
    }
    let mut marked = Vec::new();
    let mut tx = None;
    let mut text = None;
    let mut result = None;
    let mut graphics = 0usize;
    let mut actual_span = None;
    let mut actual_end = None;
    for (index, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "cm" => {
                return Err(Error::Refused(
                    "transformed appearance artwork needs a later text layout implementation",
                ));
            }
            "q" => {
                graphics += 1;
                if graphics > 32 {
                    return Err(Error::Limit("appearance graphics nesting"));
                }
            }
            "Q" => {
                graphics = graphics
                    .checked_sub(1)
                    .ok_or(Error::Invalid("unbalanced appearance graphics state"))?;
            }
            "BMC" | "BDC" => {
                if marked.len() >= 32 {
                    return Err(Error::Limit("appearance marked-content nesting"));
                }
                let tag = operation
                    .operands
                    .first()
                    .ok_or(Error::Invalid("missing marked-content tag"))?
                    .as_name()?;
                let is_tx = tag == b"Tx";
                if is_tx {
                    if !marked.is_empty() || tx.is_some() || operation.operator != "BMC" {
                        return Err(Error::Refused(
                            "ambiguous or nested variable-text appearance",
                        ));
                    }
                    tx = Some(index);
                }
                let actual = if operation.operator == "BDC" {
                    operation
                        .operands
                        .get(1)
                        .ok_or(Error::Invalid("missing marked-content properties"))?
                        .as_dict()
                        .map_err(|_| {
                            Error::Refused(
                                "named appearance properties need a later text implementation",
                            )
                        })?
                        .has(b"ActualText")
                } else {
                    false
                };
                marked.push((is_tx, actual, index));
            }
            "EMC" => {
                let (is_tx, actual, start) = marked
                    .pop()
                    .ok_or(Error::Invalid("unbalanced appearance marked content"))?;
                if actual && actual_span == Some(start) {
                    actual_end = Some(index);
                }
                if is_tx && (text.is_some() || result.is_none()) {
                    return Err(Error::Refused(
                        "variable-text appearance needs one complete text object",
                    ));
                }
            }
            "BT" => {
                if text.is_some() || !marked.iter().any(|(tx, _, _)| *tx) || result.is_some() {
                    return Err(Error::Refused(
                        "appearance has unsupported extra text objects",
                    ));
                }
                let actual: Vec<_> = marked.iter().filter(|(_, actual, _)| *actual).collect();
                if actual.len() > 1 {
                    return Err(Error::Refused(
                        "nested ActualText appearance semantics are unsupported",
                    ));
                }
                actual_span = actual.first().map(|(_, _, start)| *start);
                text = Some(index);
            }
            "ET" => {
                let start = text
                    .take()
                    .ok_or(Error::Invalid("unbalanced appearance text object"))?;
                result = Some((start, index));
            }
            _ => {}
        }
    }
    if !marked.is_empty() || text.is_some() || graphics != 0 {
        return Err(Error::Invalid("unbalanced appearance content"));
    }
    let (mut start, mut end) = result.ok_or(Error::Refused(
        "appearance has no balanced variable-text object",
    ))?;
    if let Some(span) = actual_span {
        let close = actual_end.ok_or(Error::Invalid("unclosed ActualText appearance"))?;
        if span + 1 != start || close != end + 1 {
            return Err(Error::Refused(
                "ActualText span contains unsupported surrounding artwork",
            ));
        }
        start = span;
        end = close;
        if start >= 4
            && end + 1 < content.operations.len()
            && content.operations[start - 4].operator == "q"
            && content.operations[start - 3].operator == "re"
            && content.operations[start - 2].operator == "W"
            && content.operations[start - 1].operator == "n"
            && content.operations[end + 1].operator == "Q"
        {
            start -= 4;
            end += 1;
        }
    }
    replacement_boundaries(content, start, end)?;
    Ok((start, end))
}

/// Choice variable content includes row highlights as well as text. Preserve
/// everything outside the single top-level Tx region, and validate its balance.
fn choice_region(content: &Content<Vec<Operation>>) -> Result<(usize, usize)> {
    if content.operations.len() > 100_000 {
        return Err(Error::Limit("choice appearance exceeds 100000 operators"));
    }
    let mut marked = Vec::new();
    let mut graphics = 0usize;
    let mut text = false;
    let mut region = None;
    let mut tx = None;
    let mut saw_text = false;
    for (index, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "cm" => return Err(Error::Refused("transformed choice artwork is unsupported")),
            "q" => {
                graphics += 1;
                if graphics > 32 {
                    return Err(Error::Limit("choice graphics nesting"));
                }
            }
            "Q" => {
                graphics = graphics
                    .checked_sub(1)
                    .ok_or(Error::Invalid("unbalanced choice graphics"))?;
            }
            "BMC" | "BDC" => {
                if marked.len() >= 32 {
                    return Err(Error::Limit("choice marked nesting"));
                }
                let tag = operation
                    .operands
                    .first()
                    .ok_or(Error::Invalid("missing choice marked tag"))?
                    .as_name()?;
                let is_tx = tag == b"Tx";
                if is_tx {
                    if !marked.is_empty()
                        || tx.is_some()
                        || region.is_some()
                        || operation.operator != "BMC"
                    {
                        return Err(Error::Refused("ambiguous choice Tx region"));
                    }
                    tx = Some(index);
                }
                if operation.operator == "BDC" {
                    operation
                        .operands
                        .get(1)
                        .ok_or(Error::Invalid("missing choice marked properties"))?
                        .as_dict()?;
                }
                marked.push(is_tx);
            }
            "EMC" => {
                let is_tx = marked
                    .pop()
                    .ok_or(Error::Invalid("unbalanced choice marked content"))?;
                if is_tx {
                    if text {
                        return Err(Error::Invalid("choice text crosses Tx boundary"));
                    }
                    region = Some((
                        tx.take().ok_or(Error::Invalid("missing choice Tx start"))? + 1,
                        index - 1,
                    ));
                }
            }
            "BT" => {
                if text || tx.is_none() {
                    return Err(Error::Refused("choice text must be inside one Tx region"));
                }
                text = true;
                saw_text = true;
            }
            "ET" => {
                if !text {
                    return Err(Error::Invalid("unbalanced choice text"));
                }
                text = false;
            }
            _ => (),
        }
    }
    if text || graphics != 0 || !marked.is_empty() {
        return Err(Error::Invalid("unbalanced choice appearance"));
    }
    if !saw_text {
        return Err(Error::Refused("choice appearance has no text region"));
    }
    let (start, end) = region.ok_or(Error::Refused("choice appearance has no Tx region"))?;
    replacement_boundaries(content, start, end)?;
    Ok((start, end))
}

fn clipping(content: &Content<Vec<Operation>>, interior: [f64; 4]) -> Result<()> {
    let mut clip: Option<[f64; 4]> = None;
    let mut stack = Vec::new();
    for (index, op) in content.operations.iter().enumerate() {
        match op.operator.as_str() {
            "BT" => break,
            "q" => stack.push(clip),
            "Q" => {
                clip = stack
                    .pop()
                    .ok_or(Error::Invalid("unbalanced text clipping state"))?
            }
            "W" | "W*" => {
                let rect = index
                    .checked_sub(1)
                    .and_then(|index| content.operations.get(index))
                    .filter(|op| op.operator == "re" && op.operands.len() == 4)
                    .ok_or(Error::Refused(
                        "nonrectangular text clipping is unsupported",
                    ))?;
                let x = number(&rect.operands[0])?;
                let y = number(&rect.operands[1])?;
                let x1 = x + number(&rect.operands[2])?;
                let y1 = y + number(&rect.operands[3])?;
                let bounds = [x.min(x1), y.min(y1), x.max(x1), y.max(y1)];
                clip = Some(if let Some(old) = clip {
                    [
                        old[0].max(bounds[0]),
                        old[1].max(bounds[1]),
                        old[2].min(bounds[2]),
                        old[3].min(bounds[3]),
                    ]
                } else {
                    bounds
                });
            }
            _ => {}
        }
    }
    if let Some(clip) = clip
        && (clip[0] > interior[0] + 0.001
            || clip[1] > interior[1] + 0.001
            || clip[2] < interior[2] - 0.001
            || clip[3] < interior[3] - 0.001)
    {
        return Err(Error::Refused(
            "existing clipping does not contain the text layout interior",
        ));
    }
    Ok(())
}

/// PDF paths are not saved by q/Q. A replacement's clip must not consume
/// surrounding artwork, or remove graphics saves/restores owned by that artwork.
fn replacement_boundaries(
    content: &Content<Vec<Operation>>,
    start: usize,
    end: usize,
) -> Result<()> {
    let mut graphics = 0usize;
    let mut entry = 0usize;
    let mut path = false;
    let mut isolated =
        content.operations[start].operator == "q" && content.operations[end].operator == "Q";
    for (index, operation) in content.operations.iter().enumerate().take(end + 1) {
        if index == start {
            entry = graphics;
            if path {
                return Err(Error::Refused(
                    "appearance path crosses the text replacement boundary",
                ));
            }
        }
        match operation.operator.as_str() {
            "q" => graphics += 1,
            "Q" => {
                graphics = graphics
                    .checked_sub(1)
                    .ok_or(Error::Invalid("unbalanced appearance graphics state"))?
            }
            "m" | "l" | "c" | "v" | "y" | "h" | "re" => path = true,
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => path = false,
            _ => (),
        }
        if index >= start && graphics < entry {
            return Err(Error::Refused(
                "appearance graphics state crosses the text replacement boundary",
            ));
        }
        if index >= start && index < end && graphics == entry {
            isolated = false;
        }
    }
    if graphics != entry || path {
        return Err(Error::Refused(
            "appearance graphics or path crosses the text replacement boundary",
        ));
    }
    // Generated text restores its own graphics/text state. Original text may
    // instead leave colors, line styles, clipping or font settings for later
    // artwork. Do not silently change those consumers: require an original
    // enclosing restore before anything besides structural delimiters.
    if !isolated {
        for operation in content.operations.iter().skip(end + 1) {
            match operation.operator.as_str() {
                "BMC" | "BDC" | "EMC" => (),
                "q" => graphics += 1,
                "Q" => {
                    graphics = graphics
                        .checked_sub(1)
                        .ok_or(Error::Invalid("unbalanced appearance graphics state"))?;
                    if graphics < entry {
                        break;
                    }
                }
                _ => {
                    return Err(Error::Refused(
                        "surrounding appearance depends on replaced text state",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn text_eligibility(
    pdf: &PdfDocument,
    field: &FormField,
    budget: &mut Budget,
) -> Result<()> {
    if field.kind != FormFieldKind::Text || field.widgets.is_empty() || field.name.is_empty() {
        return Err(Error::Refused(
            "an unambiguous visible text field is required",
        ));
    }
    if field.flags & (1 | 1 << 12 | 1 << 13 | 1 << 20 | 1 << 24 | 1 << 25) != 0 {
        return Err(Error::Refused(
            "read-only, multiline, password, file-select, comb and rich text fields need separate implementations",
        ));
    }
    for key in [b"AA".as_slice(), b"A", b"RV", b"DS"] {
        if inherited(&pdf.document, field.object_id, key, budget)?.is_some() {
            return Err(Error::Refused(
                "text field actions or rich text are unsupported",
            ));
        }
    }
    if let Some(max) = inherited(&pdf.document, field.object_id, b"MaxLen", budget)?
        && max.as_i64()? < 0
    {
        return Err(Error::Invalid("negative text character limit"));
    }
    for widget in &field.widgets {
        widget_plan(
            pdf,
            widget.object_id,
            field.object_id,
            "",
            Line {
                glyphs: vec![],
                bounds: [0., 0., 0., 1.],
            },
            budget,
        )?;
    }
    Ok(())
}

pub(super) fn widget_plan(
    pdf: &PdfDocument,
    widget: ObjectId,
    field: ObjectId,
    value: &str,
    line: Line,
    budget: &mut Budget,
) -> Result<WidgetPlan> {
    let document = &pdf.document;
    let dict = document.get_dictionary(widget)?;
    if dict.has(b"A") || dict.has(b"AA") || dict.has(b"StructParent") {
        return Err(Error::Refused(
            "text widget actions or tagged structure are unsupported",
        ));
    }
    let flags = dict
        .get(b"F")
        .ok()
        .map(|v| {
            budget
                .resolve(document, v)
                .and_then(|(_, v)| Ok(v.as_i64()?))
        })
        .transpose()?
        .unwrap_or(0);
    if flags & (128 | 512) != 0 {
        return Err(Error::Refused("text widget is locked"));
    }
    let rect = numeric::<4>(document, dict.get(b"Rect")?, budget)?;
    let width = f64::from(rect[2] - rect[0]);
    let height = f64::from(rect[3] - rect[1]);
    if width <= 0.0 || height <= 0.0 {
        return Err(Error::Invalid("text widget rectangle is empty"));
    }
    let ap = budget.resolve(document, dict.get(b"AP")?)?.1.as_dict()?;
    let normal = ap.get(b"N")?;
    super::appearance(document, normal, budget)?;
    let (id, object) = budget.resolve(document, normal)?;
    let stream = id.ok_or(Error::Refused("text appearance must be indirect"))?;
    let appearance = object.as_stream()?;
    for (key, value) in appearance.dict.iter() {
        budget.report(key.len())?;
        copied(value, 0, budget)?;
    }
    let bbox = numeric::<4>(document, appearance.dict.get(b"BBox")?, budget)?;
    let rotation = if let Ok(mk) = dict.get(b"MK") {
        let mk = budget.resolve(document, mk)?.1.as_dict()?;
        mk.get(b"R")
            .ok()
            .map(|r| {
                budget
                    .resolve(document, r)
                    .and_then(|(_, r)| Ok(r.as_i64()?))
            })
            .transpose()?
            .unwrap_or(0)
            .rem_euclid(360)
    } else {
        0
    };
    let (layout_w, layout_h, matrix) = match rotation {
        0 => (width, height, [1., 0., 0., 1., 0., 0.]),
        90 => (height, width, [0., 1., -1., 0., width, 0.]),
        180 => (width, height, [-1., 0., 0., -1., width, height]),
        270 => (height, width, [0., -1., 1., 0., 0., height]),
        _ => return Err(Error::Refused("text widget rotation must be a quadrant")),
    };
    let actual_matrix = appearance
        .dict
        .get(b"Matrix")
        .ok()
        .map(|m| numeric::<6>(document, m, budget))
        .transpose()?
        .unwrap_or([1., 0., 0., 1., 0., 0.]);
    if bbox
        .iter()
        .map(|n| f64::from(*n))
        .zip([0., 0., layout_w, layout_h])
        .any(|(a, b)| (a - b).abs() > 0.001)
        || actual_matrix
            .iter()
            .map(|n| f64::from(*n))
            .zip(matrix)
            .any(|(a, b)| (a - b).abs() > 0.001)
    {
        return Err(Error::Refused(
            "text appearance bounds or matrix do not match the widget layout",
        ));
    }
    let catalog = budget
        .resolve(document, document.trailer.get(b"Root")?)?
        .1
        .as_dict()?;
    let form = budget
        .resolve(document, catalog.get(b"AcroForm")?)?
        .1
        .as_dict()?;
    let da = inherited(document, field, b"DA", budget)?
        .or(form
            .get(b"DA")
            .ok()
            .map(|v| budget.resolve(document, v).map(|(_, v)| v))
            .transpose()?)
        .ok_or(Error::Refused("text field has no default appearance"))?;
    let style = style(da.as_str()?)?;
    let q = inherited(document, field, b"Q", budget)?
        .map(Object::as_i64)
        .transpose()?
        .unwrap_or(0);
    if !(0..=2).contains(&q) {
        return Err(Error::Invalid("invalid text alignment"));
    }
    let mut border = 0.0;
    if let Ok(bs) = dict.get(b"BS") {
        let bs = budget.resolve(document, bs)?.1.as_dict()?;
        border = bs
            .get(b"W")
            .ok()
            .map(|v| budget.resolve(document, v).and_then(|(_, v)| number(v)))
            .transpose()?
            .unwrap_or(1.0);
    } else if let Ok(values) = dict.get(b"Border") {
        let values = budget.resolve(document, values)?.1.as_array()?;
        border = number(
            values
                .get(2)
                .ok_or(Error::Invalid("invalid widget border"))?,
        )?;
    }
    if border < 0.0 || !border.is_finite() {
        return Err(Error::Invalid("invalid text border width"));
    }
    let padding = 2.0 + border;
    if layout_w <= 2.0 * padding || layout_h <= 2.0 * padding {
        return Err(Error::Refused("text widget has no usable interior"));
    }
    let resources = appearance
        .dict
        .get(b"Resources")
        .ok()
        .map(|v| {
            budget
                .resolve(document, v)
                .and_then(|(_, v)| Ok(v.as_dict()?))
        })
        .transpose()?
        .ok_or(Error::Refused("text appearance has no resources"))?;
    for (key, value) in resources.iter() {
        budget.report(key.len())?;
        copied(value, 0, budget)?;
    }
    let original_fonts = resources.get_deref(b"Font", document)?.as_dict()?;
    for (key, value) in original_fonts.iter() {
        budget.report(key.len())?;
        copied(value, 0, budget)?;
    }
    let bytes = appearance.get_plain_content_with_limit(4 * 1024 * 1024)?;
    budget.report(bytes.len())?;
    let content = Content::decode_strict(&bytes)?;
    let choice = inherited(document, field, b"FT", budget)?
        .is_some_and(|kind| kind.as_name().ok() == Some(b"Ch"));
    let (text_start, text_end) = if choice {
        choice_region(&content)?
    } else {
        text_region(&content)?
    };
    let clipping_content = choice.then(|| Content {
        operations: content.operations[..text_start].to_vec(),
    });
    clipping(
        clipping_content.as_ref().unwrap_or(&content),
        [padding, padding, layout_w - padding, layout_h - padding],
    )?;
    Ok(WidgetPlan {
        field,
        widget,
        stream,
        content,
        text_start,
        text_end,
        resources: resources.clone(),
        width: layout_w,
        height: layout_h,
        padding,
        q,
        style,
        line,
        value: value.to_owned(),
    })
}

fn operation(name: &str, values: impl IntoIterator<Item = Object>) -> Operation {
    Operation::new(name, values.into_iter().collect())
}
fn real(value: f64) -> Object {
    Object::Real(value as f32)
}
pub(super) struct LineLayout<'a> {
    pub line: &'a Line,
    pub value: &'a str,
    pub style: &'a Style,
    pub q: i64,
    /// Local x, y, width and height of this line's clipping rectangle.
    pub rectangle: [f64; 4],
}
pub(super) fn generated(plan: &WidgetPlan, font_name: &[u8]) -> Result<Vec<Operation>> {
    generated_line(
        LineLayout {
            line: &plan.line,
            value: &plan.value,
            style: &plan.style,
            q: plan.q,
            rectangle: [
                plan.padding,
                plan.padding,
                plan.width - 2.0 * plan.padding,
                plan.height - 2.0 * plan.padding,
            ],
        },
        font_name,
    )
}
pub(super) fn generated_line(layout: LineLayout<'_>, font_name: &[u8]) -> Result<Vec<Operation>> {
    let bounds = layout.line.bounds;
    let inner_w = layout.rectangle[2];
    let inner_h = layout.rectangle[3];
    let extent_w = bounds[2] - bounds[0];
    let extent_h = bounds[3] - bounds[1];
    let size = if layout.style.size == 0.0 {
        (inner_h / extent_h)
            .min(if extent_w > 0.0 {
                inner_w / extent_w
            } else {
                1024.0
            })
            .min(1024.0)
    } else {
        layout.style.size
    };
    if !size.is_finite()
        || size < 1.0
        || extent_w * size > inner_w + 0.001
        || extent_h * size > inner_h + 0.001
    {
        return Err(Error::Refused(
            "text does not fit the widget without overflow",
        ));
    }
    let spare = inner_w - extent_w * size;
    let x = layout.rectangle[0] - bounds[0] * size
        + match layout.q {
            1 => spare / 2.0,
            2 => spare,
            _ => 0.0,
        };
    let y = layout.rectangle[1] + (inner_h - extent_h * size) / 2.0 - bounds[1] * size;
    let mut ops = vec![
        operation("q", []),
        operation(
            "re",
            [
                real(layout.rectangle[0]),
                real(layout.rectangle[1]),
                real(inner_w),
                real(inner_h),
            ],
        ),
        operation("W", []),
        operation("n", []),
        operation(
            "BDC",
            [
                Object::Name(b"Span".to_vec()),
                Object::Dictionary(dictionary! {"ActualText"=>unicode_string(layout.value)}),
            ],
        ),
        operation("BT", []),
        operation("Tc", [0.into()]),
        operation("Tw", [0.into()]),
        operation("Tz", [100.into()]),
        operation("Tr", [0.into()]),
        operation("Ts", [0.into()]),
        layout.style.color.clone(),
        operation("Tf", [Object::Name(font_name.to_vec()), real(size)]),
    ];
    for glyph in &layout.line.glyphs {
        ops.push(operation(
            "Tr",
            [if glyph.invisible { 3.into() } else { 0.into() }],
        ));
        ops.push(operation(
            "Tm",
            [
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                real(x + glyph.x * size),
                real(y + glyph.y * size),
            ],
        ));
        ops.push(operation(
            "Tj",
            [Object::String(
                glyph.cid.to_be_bytes().to_vec(),
                lopdf::StringFormat::Hexadecimal,
            )],
        ));
    }
    ops.extend([
        operation("ET", []),
        operation("EMC", []),
        operation("Q", []),
    ]);
    Ok(ops)
}

/// Choice row replacement covers the glyph run, leaving BT/ET separators real.
pub(super) fn generated_row(layout: LineLayout<'_>, font_name: &[u8]) -> Result<Vec<Operation>> {
    let mut ops = generated_line(layout, font_name)?;
    let start = ops
        .iter()
        .position(|op| op.operator == "BDC")
        .ok_or(Error::Invalid("generated row has no logical span"))?;
    let span = ops.remove(start);
    let end = ops
        .iter()
        .position(|op| op.operator == "EMC")
        .ok_or(Error::Invalid("generated row has no logical span end"))?;
    let close = ops.remove(end);
    let text_end = ops
        .iter()
        .position(|op| op.operator == "ET")
        .ok_or(Error::Invalid("generated row has no text end"))?;
    ops.insert(text_end, close);
    let font = ops
        .iter()
        .position(|op| op.operator == "Tf")
        .ok_or(Error::Invalid("generated row has no font"))?;
    ops.insert(font + 1, span);
    Ok(ops)
}

impl PdfDocument {
    /// Fill single-line text using the caller's explicitly approved complete
    /// TrueType or static CID-keyed CFF1 font. The caller must have permission to embed the font; its
    /// editable-embedding flags are validated. Color/bitmap/SVG fonts and unsupported
    /// layout or CFF matrices are refused; non-whitespace text requires visible outlines.
    /// Appearances whose surrounding artwork depends on removed text state are refused.
    pub fn fill_text(&mut self, changes: &[TextChange], font_bytes: &[u8]) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_FIELDS {
            return Err(Error::Refused("select between one and 10000 text fields"));
        }
        let fields = super::discover(self)?;
        let mut budget = Budget::new();
        let catalog = budget
            .resolve(&self.document, self.document.trailer.get(b"Root")?)?
            .1
            .as_dict()?;
        let form = budget
            .resolve(&self.document, catalog.get(b"AcroForm")?)?
            .1
            .as_dict()?;
        if super::form_readonly(self, &self.document, form, catalog, &mut budget)?.is_some() {
            return Err(Error::Refused(
                "form calculations, actions or viewer-generated appearances are unsupported",
            ));
        }
        let mut font = FontPlan::new(font_bytes)?;
        let mut names = BTreeSet::new();
        let mut plans = Vec::new();
        let mut updates = Vec::new();
        let mut glyph_count = 0usize;
        for change in changes {
            budget.report(change.name.len() + change.value.len())?;
            if !names.insert(change.name.as_str()) {
                return Err(Error::Refused("duplicate field in text transaction"));
            }
            let mut matches = fields.iter().filter(|field| field.name == change.name);
            let field = matches
                .next()
                .ok_or(Error::Refused("text field name not found"))?;
            if matches.next().is_some()
                || field.kind != FormFieldKind::Text
                || field.widgets.is_empty()
            {
                return Err(Error::Refused(
                    "an unambiguous visible text field is required",
                ));
            }
            if !field.writable_text {
                return Err(Error::Refused(
                    "text field is not writable in the supported appearance tier",
                ));
            }
            text_eligibility(self, field, &mut budget)?;
            if let Some(max) = inherited(&self.document, field.object_id, b"MaxLen", &mut budget)? {
                let max = max.as_i64()?;
                if max < 0 || change.value.chars().count() as u64 > max as u64 {
                    return Err(Error::Refused("text exceeds the field's character limit"));
                }
            }
            for widget in &field.widgets {
                let line = font.shape(&change.value)?;
                glyph_count = glyph_count
                    .checked_add(line.glyphs.len())
                    .ok_or(Error::Limit("text planning overflow"))?;
                if glyph_count > 100_000 {
                    return Err(Error::Limit("text transaction exceeds 100000 drawn glyphs"));
                }
                plans.push(widget_plan(
                    self,
                    widget.object_id,
                    field.object_id,
                    &change.value,
                    line,
                    &mut budget,
                )?);
            }
            updates.push((field.object_id, change.value.clone()));
        }
        let patches = updates
            .into_iter()
            .map(|(id, value)| FieldPatch {
                id,
                values: vec![("V", Some(unicode_string(&value)))],
            })
            .collect();
        publish_plans(self, font, plans, patches, &mut budget, generated)
    }
}

pub(super) struct FieldPatch {
    pub id: ObjectId,
    pub values: Vec<(&'static str, Option<Object>)>,
}

/// Plan, encode and validate the entire candidate before publishing any mutation.
pub(super) fn publish_plans(
    pdf: &mut PdfDocument,
    font: FontPlan<'_>,
    plans: Vec<WidgetPlan>,
    patches: Vec<FieldPatch>,
    budget: &mut Budget,
    replacement: impl Fn(&WidgetPlan, &[u8]) -> Result<Vec<Operation>>,
) -> Result<()> {
    let catalog = budget
        .resolve(&pdf.document, pdf.document.trailer.get(b"Root")?)?
        .1
        .as_dict()?;
    let form = budget
        .resolve(&pdf.document, catalog.get(b"AcroForm")?)?
        .1
        .as_dict()?;
    let mut font_name = b"NativeFormText".to_vec();
    let mut suffix = 0usize;
    let dr = form
        .get(b"DR")
        .ok()
        .map(|v| {
            budget
                .resolve(&pdf.document, v)
                .and_then(|(_, v)| Ok(v.as_dict()?))
        })
        .transpose()?;
    let dr_fonts = dr
        .and_then(|dr| dr.get(b"Font").ok())
        .map(|v| {
            budget
                .resolve(&pdf.document, v)
                .and_then(|(_, v)| Ok(v.as_dict()?))
        })
        .transpose()?;
    while dr_fonts.is_some_and(|fonts| fonts.has(&font_name))
        || plans.iter().any(|plan| {
            plan.resources
                .get_deref(b"Font", &pdf.document)
                .and_then(Object::as_dict)
                .is_ok_and(|fonts| fonts.has(&font_name))
        })
    {
        suffix += 1;
        if suffix > 100_000 {
            return Err(Error::Limit("font resource name budget"));
        }
        font_name = format!("NativeFormText{suffix}").into_bytes();
    }
    let mut encoded = Vec::new();
    let mut bytes = 0usize;
    for plan in &plans {
        let replacement = replacement(plan, &font_name)?;
        if plan.content.operations.len() - (plan.text_end - plan.text_start + 1) + replacement.len()
            > 100_000
        {
            return Err(Error::Limit(
                "generated text appearance exceeds 100000 operators",
            ));
        }
        let mut content = plan.content.clone();
        content
            .operations
            .splice(plan.text_start..=plan.text_end, replacement);
        let output = content.encode()?;
        bytes = bytes
            .checked_add(output.len())
            .ok_or(Error::Limit("generated appearances overflow"))?;
        if bytes > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("generated appearances exceed 32 MiB"));
        }
        encoded.push(output);
    }
    let extra = plans.len() + 6;
    if pdf.document.objects.len().saturating_add(extra) > crate::MAX_OBJECTS
        || pdf.document.max_id as usize + extra >= crate::MAX_OBJECTS
    {
        return Err(Error::Limit(
            "text transaction exceeds object allocation budget",
        ));
    }
    for (key, value) in form.iter() {
        budget.report(key.len())?;
        copied(value, 0, budget)?;
    }
    if let Some(dr) = dr {
        for (key, value) in dr.iter() {
            budget.report(key.len())?;
            copied(value, 0, budget)?;
        }
    }
    if let Some(fonts) = dr_fonts {
        for (key, value) in fonts.iter() {
            budget.report(key.len())?;
            copied(value, 0, budget)?;
        }
    }
    let default_appearances = patches
        .iter()
        .map(|patch| {
            let id = &patch.id;
            let plan = plans
                .iter()
                .find(|plan| plan.field == *id)
                .ok_or(Error::Invalid("text field has no planned widget"))?;
            let content = Content {
                operations: vec![
                    operation(
                        "Tf",
                        [Object::Name(font_name.clone()), real(plan.style.size)],
                    ),
                    plan.style.color.clone(),
                ],
            };
            Ok((*id, content.encode()?))
        })
        .collect::<Result<Vec<_>>>()?;
    let existing_font = font.existing_program(&pdf.document)?;
    let mut candidate = pdf.clone();
    let font_id = font.embed(&mut candidate.document, existing_font)?;
    let mut resources = dr.cloned().unwrap_or_default();
    let mut fonts = dr_fonts.cloned().unwrap_or_default();
    fonts.set(font_name.clone(), font_id);
    resources.set("Font", fonts);
    let mut updated_form = form.clone();
    updated_form.set("DR", resources);
    let (form_id, _) = budget.resolve(&pdf.document, catalog.get(b"AcroForm")?)?;
    if let Some(form_id) = form_id {
        *candidate.document.get_dictionary_mut(form_id)? = updated_form;
    } else {
        let catalog_id = budget
            .resolve(&pdf.document, pdf.document.trailer.get(b"Root")?)?
            .0
            .ok_or(Error::Invalid("direct catalog is unsupported"))?;
        candidate
            .document
            .get_dictionary_mut(catalog_id)?
            .set("AcroForm", updated_form);
    }
    for (plan, content) in plans.into_iter().zip(encoded) {
        let mut resources = plan.resources;
        let mut fonts = resources
            .get_deref(b"Font", &pdf.document)
            .and_then(Object::as_dict)?
            .clone();
        fonts.set(font_name.clone(), font_id);
        resources.set("Font", fonts);
        let mut dict = pdf
            .document
            .get_object(plan.stream)?
            .as_stream()?
            .dict
            .clone();
        dict.remove(b"Filter");
        dict.remove(b"DecodeParms");
        dict.set("Resources", resources);
        let stream = candidate.document.add_object(Stream::new(dict, content));
        let old = pdf.document.get_dictionary(plan.widget)?;
        let mut ap = budget
            .resolve(&pdf.document, old.get(b"AP")?)?
            .1
            .as_dict()?
            .clone();
        ap.set("N", stream);
        candidate
            .document
            .get_dictionary_mut(plan.widget)?
            .set("AP", ap);
    }
    for patch in patches {
        let field = candidate.document.get_dictionary_mut(patch.id)?;
        for (key, value) in patch.values {
            if let Some(value) = value {
                field.set(key, value);
            } else {
                field.remove(key.as_bytes());
            }
        }
    }
    for (id, da) in default_appearances {
        candidate
            .document
            .get_dictionary_mut(id)?
            .set("DA", Object::String(da, lopdf::StringFormat::Literal));
    }
    let rebuilt = PdfDocument::from_document(candidate.document, candidate.encrypted_input)?;
    if rebuilt.pages != pdf.pages {
        return Err(Error::Invalid("text transaction changed page identities"));
    }
    if bytes > MAX_REPORT_BYTES * 8 {
        return Err(Error::Limit("text appearance byte budget"));
    }
    rebuilt.to_bytes()?;
    *pdf = rebuilt;
    Ok(())
}
