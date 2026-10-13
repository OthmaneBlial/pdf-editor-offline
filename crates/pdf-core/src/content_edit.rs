//! A deliberately narrow existing-operator editor; this is not redaction.
use crate::{Error, MAX_OBJECTS, MAX_STREAM_BYTES, PdfDocument, Result, structure};
use lopdf::content::Content;
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RUNS: usize = 10_000;
const MAX_TEXT: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct TextState {
    font: Option<ContentTextEncoding>,
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
    /// Printable ASCII only. Other encodings are deliberately not guessed.
    pub text: Option<String>,
    pub writable: bool,
    pub refusal_reason: Option<String>,
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
    /// Shared core/UI validation. StandardEncoding quote codes are deliberately outside this ASCII tier.
    pub fn validate_replacement(&self, value: &str) -> Result<()> {
        let expected = self
            .text
            .as_deref()
            .filter(|_| self.writable)
            .ok_or(Error::Refused(
                "text run is outside the supported editing tier",
            ))?;
        if !self
            .encoding
            .is_some_and(|encoding| encoding.accepts_ascii(value))
            || expected.len() != value.len()
            || expected
                .bytes()
                .zip(value.bytes())
                .any(|(old, new)| (old == b' ') != (new == b' '))
        {
            return Err(Error::Refused(
                "replacement must preserve ASCII length/space positions; StandardEncoding codes39/96 are unsupported",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextChange {
    pub identity: ContentTextIdentity,
    pub expected: String,
    pub replacement: String,
}

fn ascii(bytes: &[u8]) -> bool {
    bytes.len() <= 65_536 && bytes.iter().all(|byte| (32..=126).contains(byte))
}

fn single_content(pdf: &PdfDocument, page: u32) -> Result<(ObjectId, ObjectId, Vec<u8>, Content)> {
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
    let content = Content::decode(&bytes)?;
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
    Ok((page_id, stream_id, bytes, content))
}

fn courier(
    resources: Option<&Dictionary>,
    pdf: &PdfDocument,
    name: &[u8],
) -> Result<Option<ContentTextEncoding>> {
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
    let supported = font.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font")
        && font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type1")
        && font.iter().all(|(key, _)| {
            matches!(
                key.as_slice(),
                b"Type" | b"Subtype" | b"BaseFont" | b"Encoding" | b"Name"
            )
        })
        && matches!(
            font.get(b"BaseFont").and_then(Object::as_name).ok(),
            Some(b"Courier" | b"Courier-Bold" | b"Courier-Oblique" | b"Courier-BoldOblique")
        )
        && ![
            b"FontDescriptor".as_slice(),
            b"Widths",
            b"FirstChar",
            b"LastChar",
            b"ToUnicode",
        ]
        .iter()
        .any(|key| font.has(key));
    Ok(if supported { encoding } else { None })
}

fn number(object: &Object) -> bool {
    matches!(object, Object::Integer(_))
        || matches!(object, Object::Real(value) if value.is_finite())
}

impl PdfDocument {
    /// Inventory direct Tj runs. Form XObjects, TJ arrays, and annotation text are outside this tier.
    pub fn content_text_runs(&self, page: u32) -> Result<Vec<ContentTextRun>> {
        let (page_id, stream_id, bytes, content) = single_content(self, page)?;
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
        let mut runs = Vec::new();
        let mut text_bytes = 0usize;
        let mut block = None;
        for (index, op) in content.operations.iter().enumerate() {
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
                        && state.font.is_some_and(|encoding| {
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
                        "requires one final unambiguous ASCII Tj, explicit Tf/Tm/Tr 0 and standard Courier; StandardEncoding quote codes39/96 are unsupported",
                    )
                } else {
                    None
                };
                runs.push(ContentTextRun {
                    encoding: if supported && !marked {
                        block.as_ref().and_then(|state| state.font)
                    } else {
                        None
                    },
                    identity: ContentTextIdentity {
                        page_object: page_id,
                        stream_object: stream_id,
                        stream_digest: digest,
                        operation: index as u32,
                    },
                    text,
                    writable: reason.is_none(),
                    refusal_reason: reason.map(str::to_owned),
                });
            }
            if op.operator == "ET" && (block.take().is_none() || !op.operands.is_empty()) {
                return Err(Error::Invalid("unbalanced or malformed text object"));
            }
            if op.operator != "BT"
                && let Some(state) = &mut block
            {
                match (op.operator.as_str(), op.operands.as_slice()) {
                    ("Tf", [Object::Name(name), size]) if number(size) => {
                        state.font = courier(resources, self, name)?
                    }
                    ("Tm", values) if values.len() == 6 && values.iter().all(number) => {
                        state.positioned = true
                    }
                    ("Tr", [Object::Integer(0)]) => state.render = true,
                    ("Tc" | "Tw" | "Tz" | "Ts", [value]) if number(value) => {}
                    _ => state.valid = false,
                }
            }
        }
        if block.is_some() {
            return Err(Error::Invalid("unterminated text object"));
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
        let (page_id, stream_id, _, mut content) = single_content(self, page)?;
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
            run.validate_replacement(&change.replacement)?;
            let operand = &mut content.operations[change.identity.operation as usize].operands[0];
            *operand = Object::string_literal(change.replacement.as_bytes().to_vec());
        }
        let encoded = content.encode()?;
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
        candidate.verified_bytes()?;
        *self = candidate;
        Ok(())
    }
}
