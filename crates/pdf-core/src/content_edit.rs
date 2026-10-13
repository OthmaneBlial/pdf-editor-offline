//! A deliberately narrow existing-operator editor; this is not redaction.
use crate::{Error, MAX_OBJECTS, MAX_STREAM_BYTES, PdfDocument, Result, structure};
use lopdf::content::Content;
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const MAX_RUNS: usize = 10_000;
const MAX_TEXT: usize = 4 * 1024 * 1024;
pub(crate) mod layout;
mod properties;
use layout::GraphicsTextState;
pub use layout::{ContentTextLayout, ContentTextPosition};
pub use properties::ContentTextPropertyChange;

#[derive(Default)]
struct TextState {
    font: Option<(ContentTextFont, ContentTextEncoding)>,
    zero_character_spacing: bool,
    size: Option<f64>,
    matrix: Option<[f64; 6]>,
    render: bool,
    positioned: bool,
    valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextIdentity {
    pub page_object: ObjectId,
    pub stream_object: ObjectId,
    pub stream_digest: [u8; 32],
    pub operation: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextRun {
    pub identity: ContentTextIdentity,
    pub encoding: Option<ContentTextEncoding>,
    pub standard_font: Option<ContentTextFont>,
    /// Different glyph counts require an explicit `0 Tc` in this text object.
    pub allows_variable_length: bool,
    pub layout: Option<ContentTextLayout>,
    /// Printable ASCII only. Other encodings are deliberately not guessed.
    pub text: Option<String>,
    pub writable: bool,
    pub refusal_reason: Option<String>,
    pub properties_writable: bool,
    pub properties_refusal_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTextFont {
    Courier,
    CourierBold,
    CourierOblique,
    CourierBoldOblique,
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    TimesRoman,
    TimesBold,
    TimesItalic,
    TimesBoldItalic,
}
impl ContentTextFont {
    pub fn pdf_name(self) -> &'static str {
        match self {
            Self::Courier => "Courier",
            Self::CourierBold => "Courier-Bold",
            Self::CourierOblique => "Courier-Oblique",
            Self::CourierBoldOblique => "Courier-BoldOblique",
            Self::Helvetica => "Helvetica",
            Self::HelveticaBold => "Helvetica-Bold",
            Self::HelveticaOblique => "Helvetica-Oblique",
            Self::HelveticaBoldOblique => "Helvetica-BoldOblique",
            Self::TimesRoman => "Times-Roman",
            Self::TimesBold => "Times-Bold",
            Self::TimesItalic => "Times-Italic",
            Self::TimesBoldItalic => "Times-BoldItalic",
        }
    }
    fn from_name(name: &[u8]) -> Option<Self> {
        [
            Self::Courier,
            Self::CourierBold,
            Self::CourierOblique,
            Self::CourierBoldOblique,
            Self::Helvetica,
            Self::HelveticaBold,
            Self::HelveticaOblique,
            Self::HelveticaBoldOblique,
            Self::TimesRoman,
            Self::TimesBold,
            Self::TimesItalic,
            Self::TimesBoldItalic,
        ]
        .into_iter()
        .find(|font| font.pdf_name().as_bytes() == name)
    }
    fn width(self, value: &str, encoding: ContentTextEncoding) -> Result<u64> {
        value.bytes().try_fold(0u64, |sum, code| {
            let width = self.code_width(code, encoding)?;
            sum.checked_add(u64::from(width))
                .ok_or(Error::Limit("standard text width overflow"))
        })
    }
    fn code_width(self, code: u8, encoding: ContentTextEncoding) -> Result<u16> {
        pdf_render::standard_ascii_width(
            self.pdf_name(),
            code,
            matches!(encoding, ContentTextEncoding::WinAnsi),
        )
        .ok_or(Error::Refused("standard font metric is unavailable"))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTextEncoding {
    Standard,
    WinAnsi,
}
impl ContentTextEncoding {
    pub fn accepts_ascii(self, value: &str) -> bool {
        ascii(value.as_bytes())
            && (!matches!(self, Self::Standard)
                || !value.bytes().any(|byte| matches!(byte, 39 | 96)))
    }
}
impl ContentTextRun {
    /// Shared core/UI validation preserving nominal word starts and final advance.
    pub fn validate_replacement(&self, value: &str) -> Result<()> {
        self.validate_replacement_with_policy(value, ContentTextLayoutPolicy::PreserveWordAdvances)
    }
    pub fn validate_replacement_with_policy(
        &self,
        value: &str,
        policy: ContentTextLayoutPolicy,
    ) -> Result<()> {
        let expected = self
            .text
            .as_deref()
            .filter(|_| self.writable)
            .ok_or(Error::Refused(
                "text run is outside the supported editing tier",
            ))?;
        let encoding = self
            .encoding
            .filter(|encoding| encoding.accepts_ascii(value))
            .ok_or(Error::Refused(
                "replacement requires unambiguous printable ASCII in the original encoding",
            ))?;
        let font = self
            .standard_font
            .ok_or(Error::Refused("standard font metrics are unavailable"))?;
        if matches!(policy, ContentTextLayoutPolicy::AllowWidthChange) {
            self.replacement_position(value)?;
            return Ok(());
        }
        let old_words = expected.split(' ');
        let new_words = value.split(' ');
        if old_words.clone().count() != new_words.clone().count() {
            return Err(Error::Refused(
                "replacement must preserve spaces and every word's nominal advance",
            ));
        }
        for (old, new) in old_words.zip(new_words) {
            if old.len() != new.len() && !self.allows_variable_length {
                return Err(Error::Refused(
                    "variable glyph counts require explicit zero character spacing",
                ));
            }
            if font.width(old, encoding)? != font.width(new, encoding)? {
                return Err(Error::Refused(
                    "replacement changes a word's nominal advance",
                ));
            }
        }
        Ok(())
    }
    /// Nominal content-space baseline endpoints, not a glyph-ink clipping box.
    pub fn replacement_position(&self, value: &str) -> Result<ContentTextPosition> {
        let encoding = self
            .encoding
            .filter(|encoding| encoding.accepts_ascii(value))
            .ok_or(Error::Refused(
                "replacement requires unambiguous printable ASCII",
            ))?;
        let font = self
            .standard_font
            .ok_or(Error::Refused("standard font metrics are unavailable"))?;
        self.layout
            .as_ref()
            .ok_or(Error::Refused("bounded text layout is unavailable"))?
            .position(font, encoding, value)
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTextLayoutPolicy {
    #[default]
    PreserveWordAdvances,
    AllowWidthChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextChange {
    pub identity: ContentTextIdentity,
    pub expected: String,
    pub replacement: String,
    #[serde(default)]
    pub layout_policy: ContentTextLayoutPolicy,
}

fn ascii(bytes: &[u8]) -> bool {
    bytes.len() <= 65_536 && bytes.iter().all(|byte| (32..=126).contains(byte))
}

struct PageContent {
    page_id: ObjectId,
    stream_id: ObjectId,
    bytes: Vec<u8>,
    content: Content,
    spans: Vec<crate::content_spans::OperatorSpan>,
}
fn single_content(pdf: &PdfDocument, page: u32) -> Result<PageContent> {
    let page_id = *pdf
        .pages
        .get(
            page.checked_sub(1)
                .ok_or(Error::Pages("page is one-based"))? as usize,
        )
        .ok_or(Error::Pages(
            "content inventory page is outside the document",
        ))?;
    let stream_id = pdf
        .document
        .get_dictionary(page_id)?
        .get(b"Contents")?
        .as_reference()
        .map_err(|_| Error::Refused("content editing requires one indirect page content stream"))?;
    let stream = pdf.document.get_object(stream_id)?.as_stream()?;
    if stream.dict.has(b"F") || stream.dict.has(b"FFilter") || stream.dict.has(b"FDecodeParms") {
        return Err(Error::Refused("external content streams cannot be edited"));
    }
    let bytes = stream.decompressed_content_with_limit(MAX_STREAM_BYTES)?;
    let (content, spans) = crate::content_spans::decode_spanned(&bytes)?;
    if content.operations.len() > 100_000 {
        return Err(Error::Limit("content inventory exceeds 100000 operators"));
    }
    if content
        .operations
        .iter()
        .any(|op| matches!(op.operator.as_str(), "BI" | "ID" | "EI"))
    {
        return Err(Error::Refused(
            "inline image streams cannot be rewritten by the text editor",
        ));
    }
    Ok(PageContent {
        page_id,
        stream_id,
        bytes,
        content,
        spans,
    })
}

fn standard_font(
    resources: Option<&Dictionary>,
    pdf: &PdfDocument,
    name: &[u8],
) -> Result<Option<(ContentTextFont, ContentTextEncoding)>> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let fonts = resources.get_deref(b"Font", &pdf.document)?.as_dict()?;
    let font = fonts.get_deref(name, &pdf.document)?.as_dict()?;
    let encoding = if !font.has(b"Encoding") {
        Some(ContentTextEncoding::Standard)
    } else {
        match font.get(b"Encoding").and_then(Object::as_name).ok() {
            Some(b"StandardEncoding") => Some(ContentTextEncoding::Standard),
            Some(b"WinAnsiEncoding") => Some(ContentTextEncoding::WinAnsi),
            _ => None,
        }
    };
    let family = font
        .get(b"BaseFont")
        .and_then(Object::as_name)
        .ok()
        .and_then(ContentTextFont::from_name);
    let supported = font.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font")
        && font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type1")
        && font.iter().all(|(key, _)| {
            matches!(
                key.as_slice(),
                b"Type" | b"Subtype" | b"BaseFont" | b"Encoding" | b"Name"
            )
        })
        && family.is_some()
        && ![
            b"FontDescriptor".as_slice(),
            b"Widths",
            b"FirstChar",
            b"LastChar",
            b"ToUnicode",
        ]
        .iter()
        .any(|key| font.has(key));
    Ok(if supported {
        family.zip(encoding)
    } else {
        None
    })
}

fn number(object: &Object) -> bool {
    matches!(object, Object::Integer(_))
        || matches!(object, Object::Real(value) if value.is_finite())
}

fn splice_content(bytes: &[u8], mut changes: Vec<(Range<usize>, Vec<u8>)>) -> Result<Vec<u8>> {
    changes.sort_by_key(|(range, _)| range.start);
    let mut previous = 0usize;
    let mut size = bytes.len();
    for (range, replacement) in &changes {
        if range.start < previous || range.end < range.start || range.end > bytes.len() {
            return Err(Error::Invalid("overlapping or invalid content spans"));
        }
        previous = range.end;
        size = size
            .checked_sub(range.len())
            .and_then(|size| size.checked_add(replacement.len()))
            .ok_or(Error::Limit("edited content size overflow"))?;
    }
    if size > MAX_STREAM_BYTES {
        return Err(Error::Limit("edited content exceeds 32 MiB"));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(size)
        .map_err(|_| Error::Limit("edited content allocation"))?;
    let mut previous = 0;
    for (range, replacement) in changes {
        output.extend_from_slice(&bytes[previous..range.start]);
        output.extend_from_slice(&replacement);
        previous = range.end;
    }
    output.extend_from_slice(&bytes[previous..]);
    Ok(output)
}

// This tier has already validated printable ASCII. Escape only PDF literal-string
// delimiters/backslashes; no parser or general-purpose object writer is duplicated.
fn ascii_operand(value: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.len() * 2 + 2);
    bytes.push(b'(');
    for byte in value.bytes() {
        if matches!(byte, b'(' | b')' | b'\\') {
            bytes.push(b'\\');
        }
        bytes.push(byte);
    }
    bytes.push(b')');
    bytes
}

impl PdfDocument {
    /// Inventory direct Tj runs. Form XObjects, TJ arrays, and annotation text are outside this tier.
    pub fn content_text_runs(&self, page: u32) -> Result<Vec<ContentTextRun>> {
        let PageContent {
            page_id,
            stream_id,
            bytes,
            content,
            ..
        } = single_content(self, page)?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let resources = structure::inherited(&self.document, page_id, b"Resources")?
            .map(Object::as_dict)
            .transpose()?;
        let marked = self.document.catalog()?.has(b"StructTreeRoot")
            || self.document.catalog()?.has(b"MarkInfo")
            || self.document.get_dictionary(page_id)?.has(b"StructParents")
            || content
                .operations
                .iter()
                .any(|op| matches!(op.operator.as_str(), "BMC" | "BDC" | "EMC"));
        let read_only = match self.ensure_editable() {
            Ok(()) => false,
            Err(Error::Refused(_)) => true,
            Err(error) => return Err(error),
        };
        let property_context_reason = match self.content_property_context(page_id, &content) {
            Ok(()) => None,
            Err(Error::Refused(reason)) => Some(reason),
            Err(error) => return Err(error),
        };
        let mut runs = Vec::new();
        let mut text_bytes = 0usize;
        let mut block = None;
        let mut graphics = GraphicsTextState::default();
        let mut stack = Vec::new();
        for (index, op) in content.operations.iter().enumerate() {
            if block.is_none()
                && matches!(
                    op.operator.as_str(),
                    "Tj" | "TJ" | "'" | "\"" | "Tm" | "Td" | "TD" | "T*"
                )
            {
                return Err(Error::Invalid(
                    "text positioning or showing outside a text object",
                ));
            }
            match op.operator.as_str() {
                "q" if op.operands.is_empty() => {
                    if stack.len() >= 1000 {
                        return Err(Error::Limit("content graphics-state nesting exceeds1000"));
                    }
                    stack.push(graphics.clone());
                }
                "Q" if op.operands.is_empty() => {
                    graphics = stack
                        .pop()
                        .ok_or(Error::Invalid("unbalanced content graphics state"))?
                }
                "q" | "Q" => return Err(Error::Invalid("malformed content graphics state")),
                "cm" | "Tc" | "Tw" | "Tz" | "Ts" => graphics.update(&op.operator, &op.operands)?,
                _ => {}
            }
            if op.operator == "BT" {
                if block.is_some() || !op.operands.is_empty() {
                    return Err(Error::Invalid("nested or malformed text object"));
                }
                block = Some(TextState {
                    valid: true,
                    ..TextState::default()
                });
            }
            if op.operator == "Tj" {
                if runs.len() >= MAX_RUNS {
                    return Err(Error::Limit("more than 10000 direct text runs"));
                }
                let mut text = match op.operands.as_slice() {
                    [Object::String(value, _)] if ascii(value) => Some(
                        String::from_utf8(value.clone())
                            .map_err(|_| Error::Invalid("ASCII text decoding failed"))?,
                    ),
                    _ => None,
                };
                text_bytes = text_bytes.saturating_add(text.as_ref().map_or(0, String::len));
                if text_bytes > MAX_TEXT {
                    return Err(Error::Limit("text inventory exceeds 4 MiB"));
                }
                let supported = block.as_ref().is_some_and(|state| {
                    state.valid
                        && state.font.is_some_and(|(_, encoding)| {
                            text.as_deref()
                                .is_some_and(|text| encoding.accepts_ascii(text))
                        })
                        && state.render
                        && state.positioned
                }) && content
                    .operations
                    .get(index + 1)
                    .is_some_and(|op| op.operator == "ET" && op.operands.is_empty())
                    && text.is_some();
                // Raw bytes in unsupported fonts or ActualText spans are not a
                // trustworthy Unicode inventory; expose no guessed string.
                if !supported || marked {
                    text = None;
                }
                let reason = if read_only {
                    Some("signed or encrypted document is read-only")
                } else if marked {
                    Some("marked content and ActualText are not editable in this tier")
                } else if !supported {
                    Some(
                        "requires one final unambiguous ASCII Tj, explicit Tf/Tm/Tr 0 and standard Courier/Helvetica/Times without metric overrides; StandardEncoding quote codes39/96 are unsupported",
                    )
                } else {
                    None
                };
                let mut run = ContentTextRun {
                    layout: if supported && !marked {
                        block
                            .as_ref()
                            .and_then(|state| Some(graphics.layout(state.size?, state.matrix?)))
                            .filter(|layout| {
                                let Some((font, encoding)) =
                                    block.as_ref().and_then(|state| state.font)
                                else {
                                    return false;
                                };
                                let Some(text) = text.as_deref() else {
                                    return false;
                                };
                                layout.position(font, encoding, text).is_ok()
                            })
                    } else {
                        None
                    },
                    encoding: if supported && !marked {
                        block
                            .as_ref()
                            .and_then(|state| state.font.map(|(_, encoding)| encoding))
                    } else {
                        None
                    },
                    standard_font: if supported && !marked {
                        block
                            .as_ref()
                            .and_then(|state| state.font.map(|(font, _)| font))
                    } else {
                        None
                    },
                    allows_variable_length: supported
                        && !marked
                        && block
                            .as_ref()
                            .is_some_and(|state| state.zero_character_spacing),
                    identity: ContentTextIdentity {
                        page_object: page_id,
                        stream_object: stream_id,
                        stream_digest: digest,
                        operation: index as u32,
                    },
                    text,
                    writable: reason.is_none(),
                    refusal_reason: reason.map(str::to_owned),
                    properties_writable: false,
                    properties_refusal_reason: None,
                };
                run.properties_refusal_reason = run
                    .refusal_reason
                    .clone()
                    .or_else(|| property_context_reason.map(str::to_owned))
                    .or_else(|| {
                        run.layout
                            .is_none()
                            .then(|| "bounded text layout is unavailable".to_owned())
                    });
                run.properties_writable = run.properties_refusal_reason.is_none();
                runs.push(run);
            }
            if op.operator == "ET" && (block.take().is_none() || !op.operands.is_empty()) {
                return Err(Error::Invalid("unbalanced or malformed text object"));
            }
            if op.operator != "BT"
                && let Some(state) = &mut block
            {
                match (op.operator.as_str(), op.operands.as_slice()) {
                    ("Tf", [Object::Name(name), size]) if number(size) => {
                        state.font = standard_font(resources, self, name)?;
                        state.size = Some(layout::numeric(size)?);
                    }
                    ("Tm", values) if values.len() == 6 && values.iter().all(number) => {
                        state.positioned = true;
                        state.matrix = Some(layout::matrix(values)?);
                    }
                    ("Tr", [Object::Integer(0)]) => state.render = true,
                    ("Tc", [value]) if number(value) => {
                        state.zero_character_spacing = value.as_float()? == 0.0
                    }
                    ("Tw" | "Tz" | "Ts", [value]) if number(value) => {}
                    ("g", [value]) if number(value) => {}
                    ("rg", values) if values.len() == 3 && values.iter().all(number) => {}
                    _ => state.valid = false,
                }
            }
        }
        if block.is_some() {
            return Err(Error::Invalid("unterminated text object"));
        }
        if !stack.is_empty() {
            return Err(Error::Invalid("unterminated content graphics state"));
        }
        Ok(runs)
    }

    /// Replace original string operands transactionally, retaining old orphan objects (not secure erasure).
    pub fn replace_content_text(&mut self, page: u32, changes: &[ContentTextChange]) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_RUNS {
            return Err(Error::Invalid("provide 1 through 10000 text replacements"));
        }
        let runs = self
            .content_text_runs(page)?
            .into_iter()
            .map(|run| (run.identity.operation, run))
            .collect::<BTreeMap<_, _>>();
        let PageContent {
            page_id,
            stream_id,
            bytes,
            spans,
            ..
        } = single_content(self, page)?;
        let mut replacements = Vec::new();
        let mut selected = BTreeSet::new();
        let mut total = 0usize;
        for change in changes {
            total = total
                .saturating_add(change.expected.len())
                .saturating_add(change.replacement.len());
            if total > MAX_TEXT {
                return Err(Error::Limit("text changes exceed 4 MiB"));
            }
            if !selected.insert(change.identity.operation) {
                return Err(Error::Invalid("duplicate text replacement target"));
            }
            let run = runs
                .get(&change.identity.operation)
                .filter(|run| run.identity == change.identity)
                .ok_or(Error::Refused(
                    "text inventory is stale or targets a different page",
                ))?;
            if !run.writable {
                return Err(Error::Refused(
                    "text run is outside the supported editing tier",
                ));
            }
            if run.text.as_deref() != Some(change.expected.as_str()) {
                return Err(Error::Refused("text expected value changed"));
            }
            run.validate_replacement_with_policy(&change.replacement, change.layout_policy)?;
            replacements.push((
                spans[change.identity.operation as usize].operands[0].clone(),
                ascii_operand(&change.replacement),
            ));
        }
        self.publish_content_bytes(page_id, stream_id, splice_content(&bytes, replacements)?)
    }

    fn publish_content_bytes(
        &mut self,
        page_id: ObjectId,
        stream_id: ObjectId,
        encoded: Vec<u8>,
    ) -> Result<()> {
        let (content, _) = crate::content_spans::decode_spanned(&encoded)?;
        if content.operations.len() > 100_000 {
            return Err(Error::Limit("edited content exceeds 100000 operators"));
        }
        if encoded.len() > MAX_STREAM_BYTES {
            return Err(Error::Limit("edited content exceeds 32 MiB"));
        }
        if self.document.objects.len() >= MAX_OBJECTS
            || self.document.max_id as usize >= MAX_OBJECTS
        {
            return Err(Error::Limit("text replacement object budget"));
        }
        let mut candidate = self.clone();
        let mut dictionary = self
            .document
            .get_object(stream_id)?
            .as_stream()?
            .dict
            .clone();
        for key in [b"Filter".as_slice(), b"DecodeParms", b"Length"] {
            dictionary.remove(key);
        }
        let id = candidate
            .document
            .add_object(Stream::new(dictionary, encoded));
        candidate
            .document
            .get_dictionary_mut(page_id)?
            .set("Contents", id);
        candidate.info()?;
        let page = candidate
            .pages
            .iter()
            .position(|id| *id == page_id)
            .ok_or(Error::Invalid("edited page disappeared"))?;
        candidate.content_text_runs(page as u32 + 1)?;
        candidate.verified_bytes()?;
        *self = candidate;
        Ok(())
    }
}
