//! Reopened original-content fonts are checked against the actual embedding
//! backend. A resource name or marker is never an eligibility proof.
use super::appearance::fonts::FontPlan;
use crate::{Error, MAX_STREAM_BYTES, Result};
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(crate) struct ContentFontPlan<'a>(FontPlan<'a>);
impl<'a> ContentFontPlan<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self(FontPlan::new(bytes)?))
    }
    pub fn codes(&mut self, text: &str) -> Result<(Vec<u8>, Vec<f64>)> {
        self.0.direct_codes(text)
    }
    pub fn show(&mut self, text: &str) -> Result<(Object, Vec<f64>, Vec<f64>)> {
        match self.codes(text) {
            Ok((codes, widths)) => Ok((
                Object::String(codes, lopdf::StringFormat::Hexadecimal),
                widths.clone(),
                widths,
            )),
            Err(Error::Refused(_)) => self.0.positioned_show(text),
            Err(error) => Err(error),
        }
    }
    pub fn embed(&self, document: &mut Document) -> Result<ObjectId> {
        let existing = self.0.existing_program(document)?;
        self.0.embed(document, existing)
    }
}

#[derive(Clone)]
pub(crate) struct ReopenedFont {
    pub program: Arc<[u8]>,
    pub program_fingerprint: [u8; 32],
    pub text: String,
    pub widths: Vec<f64>,
    pub advances: Vec<f64>,
    pub name: String,
}

#[derive(Default)]
pub(crate) struct FontReadContext {
    streams: BTreeMap<ObjectId, Arc<[u8]>>,
    bytes: usize,
    work: usize,
    text_bytes: usize,
    results: BTreeMap<(ObjectId, [u8; 32]), ReopenedFont>,
}
impl FontReadContext {
    fn stream(&mut self, document: &Document, id: ObjectId, limit: usize) -> Result<Arc<[u8]>> {
        if let Some(bytes) = self.streams.get(&id) {
            if bytes.len() > limit {
                return Err(Error::Limit("cached direct font stream budget"));
            }
            return Ok(Arc::clone(bytes));
        }
        let bytes = document
            .get_object(id)?
            .as_stream()?
            .get_plain_content_with_limit(limit.min(MAX_STREAM_BYTES - self.bytes))?;
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(Error::Limit("direct font stream budget overflow"))?;
        let bytes: Arc<[u8]> = bytes.into();
        self.streams.insert(id, Arc::clone(&bytes));
        Ok(bytes)
    }
}

pub(crate) fn candidate(
    document: &Document,
    resources: Option<&Dictionary>,
    name: &[u8],
) -> Result<Option<ObjectId>> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let fonts = resources.get_deref(b"Font", document)?.as_dict()?;
    let edge = fonts.get(name)?;
    let Ok(id) = edge.as_reference() else {
        return Ok(None);
    };
    let root = document.get_dictionary(id)?;
    if root.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type0")
        || root.get(b"Encoding").and_then(Object::as_name).ok() != Some(b"Identity-H")
    {
        return Ok(None);
    }
    let Some(descendant) = root
        .get(b"DescendantFonts")
        .and_then(Object::as_array)
        .ok()
        .filter(|values| values.len() == 1)
        .and_then(|values| values[0].as_reference().ok())
    else {
        return Ok(None);
    };
    let descendant = document.get_dictionary(descendant)?;
    let Some(descriptor) = descendant
        .get(b"FontDescriptor")
        .and_then(Object::as_reference)
        .ok()
    else {
        return Ok(None);
    };
    Ok(document
        .get_dictionary(descriptor)?
        .has(b"FontFile2")
        .then_some(id))
}

#[cfg(test)]
fn reopen(document: &Document, font: ObjectId, codes: &[u8]) -> Result<ReopenedFont> {
    reopen_with_context(
        document,
        font,
        &Object::String(codes.to_vec(), lopdf::StringFormat::Hexadecimal),
        &mut FontReadContext::default(),
    )
}
pub(crate) fn reopen_show(
    document: &Document,
    font: ObjectId,
    show: &Object,
) -> Result<ReopenedFont> {
    reopen_with_context(document, font, show, &mut FontReadContext::default())
}
pub(crate) fn reopen_with_context(
    document: &Document,
    font: ObjectId,
    show: &Object,
    context: &mut FontReadContext,
) -> Result<ReopenedFont> {
    let mut codes = Vec::new();
    match show {
        Object::String(bytes, _) => codes.extend_from_slice(bytes),
        Object::Array(items) if items.len() <= 131_069 => {
            for item in items {
                match item {
                    Object::String(bytes, _) if bytes.len() == 2 => codes.extend_from_slice(bytes),
                    Object::Integer(_) | Object::Real(_) => (),
                    _ => return Err(Error::Refused("unsupported positioned show structure")),
                }
            }
        }
        _ => return Err(Error::Refused("unsupported embedded show operand")),
    }
    if codes.len() > 131_068 || !codes.len().is_multiple_of(2) {
        return Err(Error::Limit("direct font code budget"));
    }
    let encoded_show = lopdf::content::Content {
        operations: vec![lopdf::content::Operation::new("TJ", vec![show.clone()])],
    }
    .encode()?;
    let key = (font, Sha256::digest(&encoded_show).into());
    if let Some(result) = context.results.get(&key) {
        return Ok(result.clone());
    }
    let root = document.get_dictionary(font)?;
    let descendants = root.get(b"DescendantFonts")?.as_array()?;
    let [descendant] = descendants.as_slice() else {
        return Err(Error::Refused("direct font requires one descendant"));
    };
    let descriptor = descendant
        .as_reference()
        .and_then(|id| document.get_dictionary(id))?
        .get(b"FontDescriptor")?
        .as_reference()?;
    let program_id = document
        .get_dictionary(descriptor)?
        .get(b"FontFile2")?
        .as_reference()?;
    let program = context.stream(document, program_id, 16 * 1024 * 1024)?;
    let unicode_id = root.get(b"ToUnicode")?.as_reference()?;
    let map_bytes = context.stream(document, unicode_id, MAX_STREAM_BYTES)?;
    let mut unicode = Dictionary::new();
    unicode.set("Type", "Font");
    unicode.set("ToUnicode", unicode_id);
    let text = if codes.is_empty() {
        String::new()
    } else {
        let Encoding::UnicodeMapEncoding(map) =
            unicode.get_font_encoding_with_limit(document, map_bytes.len())?
        else {
            return Err(Error::Refused("direct font needs explicit Unicode map"));
        };
        decode_cids(&codes, 4 * 1024 * 1024 - context.text_bytes, |code| {
            map.get(u32::from(code), 2)
        })?
    };
    context.text_bytes = context
        .text_bytes
        .checked_add(text.len())
        .ok_or(Error::Limit("direct font text budget overflow"))?;
    if context.text_bytes > 4 * 1024 * 1024 {
        return Err(Error::Limit("direct font decoding exceeds4MiBtext"));
    }
    let mut plan = ContentFontPlan::new(&program)?;
    let (expected, widths, advances) = plan.show(&text)?;
    if !same_show(show, &expected) {
        return Err(Error::Refused(
            "direct font codes do not match the validated logical text",
        ));
    }
    let mut canonical = Document::with_version("1.4");
    let expected = plan.embed(&mut canonical)?;
    let mut budget = GraphBudget {
        work: context.work,
        active: BTreeSet::new(),
        stream_id: None,
        context,
    };
    let checked = compare(
        document,
        &Object::Reference(font),
        &canonical,
        &Object::Reference(expected),
        0,
        &mut budget,
    );
    budget.context.work = budget.work;
    checked?;
    let name = root.get(b"BaseFont")?.as_name()?;
    let name = String::from_utf8(name.to_vec())
        .map_err(|_| Error::Refused("unsupported direct font name"))?;
    let result = ReopenedFont {
        program_fingerprint: Sha256::digest(&program).into(),
        program,
        text,
        widths,
        advances,
        name,
    };
    context.results.insert(key, result.clone());
    Ok(result)
}

fn same_show(actual: &Object, expected: &Object) -> bool {
    match (actual, expected) {
        (Object::String(a, _), Object::String(b, _)) => a == b,
        (Object::Array(a), Object::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_show(a, b))
        }
        (Object::Real(a), Object::Real(b)) => a.is_finite() && a == b,
        (Object::Integer(a), Object::Real(b)) | (Object::Real(b), Object::Integer(a)) => {
            b.is_finite() && *a as f64 == f64::from(*b)
        }
        (Object::Integer(a), Object::Integer(b)) => a == b,
        _ => false,
    }
}

fn decode_cids(
    codes: &[u8],
    remaining: usize,
    mut mapping: impl FnMut(u16) -> Option<Vec<u16>>,
) -> Result<String> {
    let mut text = String::new();
    for code in codes.chunks_exact(2) {
        let units = mapping(u16::from_be_bytes([code[0], code[1]]))
            .ok_or(Error::Refused("missing direct font Unicode mapping"))?;
        // One UTF-16 unit needs at least one UTF-8 byte. Reject amplification
        // before converting/append; conversion itself is bounded by this limit.
        if units.len() > remaining.saturating_sub(text.len()) {
            return Err(Error::Limit("direct font Unicode expansion budget"));
        }
        let value = String::from_utf16(&units)
            .map_err(|_| Error::Refused("invalid direct font Unicode mapping"))?;
        if value.len() > remaining.saturating_sub(text.len()) {
            return Err(Error::Limit("direct font Unicode expansion budget"));
        }
        text.push_str(&value);
    }
    Ok(text)
}

struct GraphBudget<'a> {
    work: usize,
    active: BTreeSet<(ObjectId, ObjectId)>,
    stream_id: Option<ObjectId>,
    context: &'a mut FontReadContext,
}
fn compare(
    a_doc: &Document,
    a: &Object,
    b_doc: &Document,
    b: &Object,
    depth: usize,
    budget: &mut GraphBudget<'_>,
) -> Result<()> {
    budget.work += 1;
    if depth > 32 || budget.work > 2_000_000 {
        return Err(Error::Limit("direct font graph traversal budget"));
    }
    match (a, b) {
        (Object::Reference(a), Object::Reference(b)) => {
            if !budget.active.insert((*a, *b)) {
                return Err(Error::Invalid("cyclic direct font graph"));
            }
            let old_id = budget.stream_id.replace(*a);
            let result = compare(
                a_doc,
                a_doc.get_object(*a)?,
                b_doc,
                b_doc.get_object(*b)?,
                depth + 1,
                budget,
            );
            budget.active.remove(&(*a, *b));
            budget.stream_id = old_id;
            result
        }
        (Object::Array(a), Object::Array(b)) if a.len() == b.len() => {
            for (a, b) in a.iter().zip(b) {
                compare(a_doc, a, b_doc, b, depth + 1, budget)?;
            }
            Ok(())
        }
        (Object::Dictionary(a), Object::Dictionary(b)) if a.len() == b.len() => {
            for (key, a) in a.iter() {
                compare(a_doc, a, b_doc, b.get(key)?, depth + 1, budget)?;
            }
            Ok(())
        }
        (Object::Stream(a), Object::Stream(b)) => {
            if a.dict.len() > b.dict.len().saturating_add(3) {
                return Err(Error::Refused("direct font stream dictionary differs"));
            }
            let id = budget.stream_id.ok_or(Error::Refused(
                "direct font needs indirect program/map streams",
            ))?;
            let a_bytes = budget.context.stream(a_doc, id, MAX_STREAM_BYTES)?;
            let b_bytes = b.get_plain_content_with_limit(MAX_STREAM_BYTES)?;
            if a_bytes.as_ref() != b_bytes {
                return Err(Error::Refused(
                    "direct font stream does not match validated embedding",
                ));
            }
            let transport = |key: &[u8]| matches!(key, b"Length" | b"Filter" | b"DecodeParms");
            if a.dict.iter().filter(|(key, _)| !transport(key)).count()
                != b.dict.iter().filter(|(key, _)| !transport(key)).count()
            {
                return Err(Error::Refused("direct font stream dictionary differs"));
            }
            for (key, value) in a.dict.iter().filter(|(key, _)| !transport(key)) {
                compare(a_doc, value, b_doc, b.dict.get(key)?, depth + 1, budget)?;
            }
            Ok(())
        }
        (Object::Integer(a), Object::Real(b)) if *a as f64 == f64::from(*b) => Ok(()),
        (Object::Real(a), Object::Integer(b)) if f64::from(*a) == *b as f64 => Ok(()),
        _ if a == b => Ok(()),
        _ => Err(Error::Refused(
            "direct font graph does not match validated embedding",
        )),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    const FONT: &[u8] = include_bytes!("../../tests/fixtures/fonts/amiri/Amiri-Regular.ttf");
    #[test]
    fn single_show_nominal_font_reopens_from_actual_program_and_graph() {
        for text in ["Original", "TEST", "é", ""] {
            let mut plan = ContentFontPlan::new(FONT).unwrap();
            let (codes, widths) = plan.codes(text).unwrap();
            let mut doc = Document::with_version("1.4");
            let id = plan.embed(&mut doc).unwrap();
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();
            let reopened = Document::load_mem(&bytes).unwrap();
            let read = reopen(&reopened, id, &codes).unwrap();
            assert_eq!(read.text, text);
            assert_eq!(read.widths, widths);
            assert_eq!(read.program.as_ref(), FONT);
            assert_eq!(read.name, "Amiri-Regular");
        }
    }
    #[test]
    fn graph_marker_and_unicode_cannot_launder_wrong_metrics() {
        let mut plan = ContentFontPlan::new(FONT).unwrap();
        let (codes, _) = plan.codes("TEST").unwrap();
        let mut doc = Document::with_version("1.4");
        let id = plan.embed(&mut doc).unwrap();
        let descendant = doc
            .get_dictionary(id)
            .unwrap()
            .get(b"DescendantFonts")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        doc.get_dictionary_mut(descendant).unwrap().set("DW", 600);
        assert!(reopen(&doc, id, &codes).is_err());
    }
    #[test]
    fn cached_font_streams_share_bytes_and_aggregate_decode_bound_before_growth() {
        let mut doc = Document::with_version("1.4");
        let mut context = FontReadContext::default();
        let one = doc.add_object(lopdf::Stream::new(
            lopdf::dictionary! {},
            vec![b'A'; 16 * 1024 * 1024],
        ));
        let first = context.stream(&doc, one, 16 * 1024 * 1024).unwrap();
        let second = context.stream(&doc, one, 16 * 1024 * 1024).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(context.bytes, 16 * 1024 * 1024);
        let two = doc.add_object(lopdf::Stream::new(
            lopdf::dictionary! {},
            vec![b'B'; 16 * 1024 * 1024],
        ));
        context.stream(&doc, two, 16 * 1024 * 1024).unwrap();
        let three = doc.add_object(lopdf::Stream::new(lopdf::dictionary! {}, vec![b'C'; 1024]));
        assert!(context.stream(&doc, three, 16 * 1024 * 1024).is_err());
        assert_eq!(context.bytes, MAX_STREAM_BYTES);
    }
    #[test]
    fn reordered_positioned_and_uncovered_glyphs_are_not_silently_substituted() {
        for value in ["مرحبا", "Ω", "\u{200f}", "\n"] {
            assert!(
                ContentFontPlan::new(FONT).unwrap().codes(value).is_err(),
                "{value:?}"
            );
        }
    }
    #[test]
    fn unicode_amplification_is_rejected_before_decoding_the_whole_operand() {
        let mut calls = 0;
        assert!(
            decode_cids(&[0, 1, 0, 1, 0, 1], 1024, |_| {
                calls += 1;
                Some(vec![65; 768])
            })
            .is_err()
        );
        assert_eq!(calls, 2);
        assert!(decode_cids(&[0, 1], 8, |_| Some(vec![0xd800])).is_err());
        assert!(decode_cids(&[0, 1], 8, |_| None).is_err());
        assert_eq!(
            decode_cids(&[0, 1], 4, |_| Some(vec![0xd83d, 0xde00])).unwrap(),
            "😀"
        );
    }
    #[test]
    fn horizontal_shaping_reopens_exact_positioned_show_without_nominal_fallback() {
        let font = include_bytes!(
            "../../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"
        );
        let mut positioned = 0;
        for text in ["AV", "To", "ffi", "office", "WAVE"] {
            let mut plan = ContentFontPlan::new(font).unwrap();
            let (show, widths, advances) = plan.show(text).unwrap();
            if let Object::Array(items) = &show {
                assert!(items.iter().any(|item| matches!(item, Object::Real(_))));
                positioned += 1;
            }
            let mut doc = Document::with_version("1.4");
            let id = plan.embed(&mut doc).unwrap();
            let read = reopen_show(&doc, id, &show).unwrap();
            assert_eq!(read.text, text);
            assert_eq!(read.widths, widths);
            assert_eq!(read.advances, advances);
            if let Object::Array(mut items) = show {
                let item = items
                    .iter_mut()
                    .find(|item| matches!(item, Object::Real(_)))
                    .unwrap();
                *item = Object::Real(item.as_float().unwrap() + 1.);
                assert!(reopen_show(&doc, id, &Object::Array(items)).is_err());
            }
        }
        assert!(
            positioned > 0,
            "the fixture must exercise actual kerning or ligature positioning"
        );
    }
}
