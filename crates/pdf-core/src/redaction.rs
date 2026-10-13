//! Permanent removal for an intentionally small, fully inspected plaintext profile.
use crate::{Error, PdfDocument, Result};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod profile;
mod rewrite;
mod verification;
use profile::Profile;
use rewrite::plan;
pub use verification::verify_redaction_bytes;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRedactionRequest {
    pub targets: Vec<String>,
}
impl std::fmt::Debug for TextRedactionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRedactionRequest")
            .field("targets", &"[REDACTED]")
            .field("count", &self.targets.len())
            .finish()
    }
}
impl TextRedactionRequest {
    pub fn validate(&self) -> Result<()> {
        targets(&self.targets).map(|_| ())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRedactionReport {
    pub changed_text_shows: usize,
    pub matched_targets: usize,
    pub affected_pages: Vec<u32>,
    pub full_rewrite: bool,
    pub profile: String,
    pub limitations: Vec<String>,
    pub cleanup: crate::SanitizationReport,
    pub rewritten_stream_orphans_removed: usize,
    pub content_comments_removed: usize,
}
struct Needle<'a> {
    bytes: &'a [u8],
    failure: Vec<usize>,
}
#[derive(Default)]
struct Search {
    work: usize,
    matches: usize,
    covered: usize,
}
impl Search {
    fn find(
        &mut self,
        bytes: &[u8],
        needles: &[Needle<'_>],
        first_only: bool,
    ) -> Result<Vec<std::ops::Range<usize>>> {
        let mut result = Vec::new();
        for needle in needles {
            // KMP is linear even for long repetitive prefixes. Refuse aggregate
            // search work before traversal, not after a pathological comparison.
            self.work = self.work.saturating_add(bytes.len().saturating_mul(2));
            if self.work > 256 * 1024 * 1024 {
                return Err(Error::Limit(
                    "redaction aggregate search exceeds 256 MiB comparison budget",
                ));
            }
            let mut matched = 0;
            for (index, byte) in bytes.iter().enumerate() {
                while matched > 0 && needle.bytes[matched] != *byte {
                    matched = needle.failure[matched - 1];
                }
                if needle.bytes[matched] == *byte {
                    matched += 1;
                }
                if matched == needle.bytes.len() {
                    self.matches += 1;
                    self.covered = self.covered.saturating_add(matched);
                    if self.covered > crate::MAX_INPUT_BYTES {
                        return Err(Error::Limit(
                            "redaction overlapping matches exceed 128 MiB work",
                        ));
                    }
                    if self.matches > 100_000 {
                        return Err(Error::Limit("redaction exceeds 100000 target occurrences"));
                    }
                    result.push(index + 1 - matched..index + 1);
                    if first_only {
                        return Ok(result);
                    }
                    matched = needle.failure[matched - 1];
                }
            }
        }
        Ok(result)
    }
    fn contains(&mut self, bytes: &[u8], needles: &[Needle<'_>]) -> Result<bool> {
        Ok(!self.find(bytes, needles, true)?.is_empty())
    }
}
fn targets(values: &[String]) -> Result<Vec<Needle<'_>>> {
    if values.is_empty()
        || values.len() > 100
        || values.iter().map(String::len).sum::<usize>() > 65536
    {
        return Err(Error::Limit(
            "redaction requires 1..100 targets, at most 64 KiB total",
        ));
    }
    if values
        .iter()
        .any(|s| s.is_empty() || s.len() > 4096 || !s.bytes().all(|b| (32..=126).contains(&b)))
    {
        return Err(Error::Refused(
            "redaction targets must be nonempty printable ASCII, at most 4096 bytes",
        ));
    }
    if values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(Error::Refused("redaction targets must be unique"));
    }
    Ok(values
        .iter()
        .map(|s| {
            let bytes = s.as_bytes();
            let mut failure = vec![0; bytes.len()];
            let mut matched = 0;
            for index in 1..bytes.len() {
                while matched > 0 && bytes[index] != bytes[matched] {
                    matched = failure[matched - 1];
                }
                if bytes[index] == bytes[matched] {
                    matched += 1;
                }
                failure[index] = matched;
            }
            Needle { bytes, failure }
        })
        .collect())
}
type Font = (String, bool);
type Fonts = BTreeMap<Vec<u8>, Font>;
/// Image-asset removal has no visual-keyword claim. Validate the remaining
/// standard-font text grammar while admitting only known numeric graphics ops.
pub(crate) fn verify_image_program(bytes: &[u8], fonts: &Fonts, media_box: [f32; 4]) -> Result<()> {
    let (program, spans) = crate::content_spans::decode_spanned(bytes)?;
    let mut text_only = bytes.to_vec();
    for (operation, span) in program.operations.iter().zip(&spans) {
        let arity = match operation.operator.as_str() {
            "cm" | "c" => Some(6),
            "m" | "l" => Some(2),
            "v" | "y" | "re" | "K" | "k" => Some(4),
            "RG" | "rg" => Some(3),
            "w" | "J" | "j" | "M" | "G" | "g" => Some(1),
            "h" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" | "W" | "W*" => {
                Some(0)
            }
            _ => None,
        };
        if let Some(arity) = arity {
            if operation.operands.len() != arity
                || operation
                    .operands
                    .iter()
                    .any(|v| !v.as_float().is_ok_and(f32::is_finite))
            {
                return Err(Error::Refused(
                    "invalid privacy image numeric graphics operator",
                ));
            }
            text_only[span.full.clone()].fill(b' ');
        } else if operation.operator == "Do" {
            if operation.operands.len() != 1 || !matches!(operation.operands[0], Object::Name(_)) {
                return Err(Error::Refused("invalid privacy image Do operand"));
            }
            text_only[span.full.clone()].fill(b' ');
        }
    }
    plan(&text_only, fonts, media_box, &[], &mut Search::default())?;
    Ok(())
}
/// Actual canonical bytes plus decoded names/strings/content operands, not a
/// substring-only inference for arbitrary opaque or visual encodings.
pub(crate) fn verify_payload_absence(
    document: &lopdf::Document,
    bytes: &[u8],
    patterns: &[&[u8]],
) -> Result<()> {
    let total = patterns.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value.len())
            .ok_or(Error::Limit("image payload proof bytes"))
    })?;
    if total > 32 * 1024 * 1024 {
        return Err(Error::Limit("image payload proof material exceeds 32 MiB"));
    }
    let mut needles = Vec::new();
    for pattern in patterns {
        if pattern.is_empty() {
            return Err(Error::Invalid("empty image payload proof"));
        }
        let mut failure = Vec::new();
        failure
            .try_reserve_exact(pattern.len())
            .map_err(|_| Error::Limit("image payload proof allocation"))?;
        failure.resize(pattern.len(), 0);
        let mut matched = 0;
        for index in 1..pattern.len() {
            while matched > 0 && pattern[matched] != pattern[index] {
                matched = failure[matched - 1];
            }
            if pattern[matched] == pattern[index] {
                matched += 1;
            }
            failure[index] = matched;
        }
        needles.push(Needle {
            bytes: pattern,
            failure,
        });
    }
    let mut search = Search::default();
    if search.contains(bytes, &needles)? {
        return Err(Error::Verification(
            "selected image payload bytes remain in output",
        ));
    }
    if document.objects.len() > 2_000_000 {
        return Err(Error::Limit("image payload verification object budget"));
    }
    let mut pending = Vec::new();
    pending
        .try_reserve(document.objects.len())
        .map_err(|_| Error::Limit("image payload verification allocation"))?;
    pending.extend(document.objects.values().map(|v| (v, 0usize)));
    let mut work = 0usize;
    while let Some((value, depth)) = pending.pop() {
        work += 1;
        if work > 2_000_000 || depth > 128 {
            return Err(Error::Limit("image payload verification graph budget"));
        }
        match value {
            Object::String(value, _) | Object::Name(value)
                if search.contains(value, &needles)? =>
            {
                return Err(Error::Verification(
                    "selected image payload survives in a logical value",
                ));
            }
            Object::Array(values) => {
                if pending
                    .len()
                    .saturating_add(values.len())
                    .saturating_add(work)
                    > 2_000_000
                {
                    return Err(Error::Limit("image payload array budget"));
                }
                pending
                    .try_reserve(values.len())
                    .map_err(|_| Error::Limit("image payload verification allocation"))?;
                pending.extend(values.iter().map(|v| (v, depth + 1)));
            }
            Object::Dictionary(dict) => {
                if pending
                    .len()
                    .saturating_add(dict.len())
                    .saturating_add(work)
                    > 2_000_000
                {
                    return Err(Error::Limit("image payload dictionary budget"));
                }
                pending
                    .try_reserve(dict.len())
                    .map_err(|_| Error::Limit("image payload verification allocation"))?;
                for (key, value) in dict {
                    if search.contains(key, &needles)? {
                        return Err(Error::Verification(
                            "selected image payload survives in a dictionary key",
                        ));
                    }
                    pending.push((value, depth + 1));
                }
            }
            Object::Stream(stream) => {
                if stream.dict.has(b"Filter") || stream.dict.has(b"DecodeParms") {
                    return Err(Error::Refused("opaque image verification transport"));
                }
                if pending
                    .len()
                    .saturating_add(stream.dict.len())
                    .saturating_add(work)
                    > 2_000_000
                {
                    return Err(Error::Limit("image payload stream dictionary budget"));
                }
                pending
                    .try_reserve(stream.dict.len())
                    .map_err(|_| Error::Limit("image payload verification allocation"))?;
                for (key, value) in &stream.dict {
                    if search.contains(key, &needles)? {
                        return Err(Error::Verification(
                            "selected payload survives in a stream key",
                        ));
                    }
                    pending.push((value, depth + 1));
                }
                if stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|v| v.as_name().ok())
                    != Some(b"Image")
                {
                    let (content, _) = crate::content_spans::decode_spanned(&stream.content)?;
                    for operation in content.operations {
                        if work
                            .saturating_add(operation.operands.len())
                            .saturating_add(pending.len())
                            > 2_000_000
                        {
                            return Err(Error::Limit("image payload operand budget"));
                        }
                        let mut operands = Vec::new();
                        operands
                            .try_reserve(operation.operands.len())
                            .map_err(|_| Error::Limit("image payload verification allocation"))?;
                        operands.extend(operation.operands.iter().map(|v| (v, 0usize)));
                        while let Some((value, level)) = operands.pop() {
                            work += 1;
                            if work > 2_000_000 || level > 128 {
                                return Err(Error::Limit("image payload operand budget"));
                            }
                            match value {
                                Object::String(value, _) | Object::Name(value)
                                    if search.contains(value, &needles)? =>
                                {
                                    return Err(Error::Verification(
                                        "selected payload survives in decoded page operands",
                                    ));
                                }
                                Object::Array(values) => {
                                    if work
                                        .saturating_add(pending.len())
                                        .saturating_add(operands.len())
                                        .saturating_add(values.len())
                                        > 2_000_000
                                    {
                                        return Err(Error::Limit(
                                            "image payload operand array budget",
                                        ));
                                    }
                                    operands.try_reserve(values.len()).map_err(|_| {
                                        Error::Limit("image payload verification allocation")
                                    })?;
                                    operands.extend(values.iter().map(|v| (v, level + 1)))
                                }
                                Object::Dictionary(_)
                                | Object::Stream(_)
                                | Object::Reference(_) => {
                                    return Err(Error::Refused(
                                        "opaque page operand in image verification",
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
impl PdfDocument {
    /// Remove exact target glyphs into a verified separate document, compensating
    /// their standard-font advances so retained glyph positions remain unchanged.
    /// Supports only the reported strict plaintext profile; never overwrites source.
    pub fn redact_text_copy(
        &self,
        values: &[String],
    ) -> Result<(PdfDocument, TextRedactionReport)> {
        let needles = targets(values)?;
        self.ensure_editable()?;
        if self.document.objects.len().saturating_add(self.pages.len()) > crate::MAX_OBJECTS {
            return Err(Error::Limit("redaction page rewrite object budget"));
        }
        let (mut candidate, cleanup) = self.sanitized_copy()?;
        candidate.document.reference_table.cross_reference_type =
            lopdf::xref::XrefType::CrossReferenceTable;
        candidate.document.trailer.remove(b"ID");
        let pages = Profile {
            pdf: &candidate,
            seen: BTreeSet::new(),
            work: 0,
            decoded: 0,
            images: None,
        }
        .pages()?;
        let mut report=TextRedactionReport {
            changed_text_shows:0, matched_targets:0, affected_pages:Vec::new(),
            full_rewrite:true, profile:"ascii-standard-type1-text-state-v3".into(),
            cleanup, rewritten_stream_orphans_removed:0,content_comments_removed:0,
            limitations:vec![
                "Exact ASCII target glyphs are removed; their advance is retained as a TJ adjustment, preserving unmatched glyph positions.".into(),
                "Inherited font/size, character and word spacing, horizontal scale, rise, line moves and filled/invisible text are supported; glyph advances restore across outer q/Q. Heuristic extraction reading order may change.".into(),
                "Only printable ASCII, unembedded standard Type1 fonts and the explicitly inspected semantic-text/uniform-background profile are supported; opaque, image, form, annotation and custom carriers refuse.".into(),
                "Arbitrary painted paths, transforms and clipping refuse; this is not proof against every visual or steganographic encoding. Broader vector/image redaction remains unsupported.".into(),
                "This proves absence of the supplied exact targets within this supported output profile, not absence of other private information or erasure of source, backups, recovery journals or filesystem copies.".into(),
            ],
        };
        let mut search = Search::default();
        let mut output_bytes = 0usize;
        for (index, (bytes, fonts)) in pages.iter().enumerate() {
            let (bytes, comments) = crate::content_spans::clear_content_comments(bytes)?;
            let media_box = crate::structure::page_box(
                &candidate.document,
                candidate.pages[index],
                b"MediaBox",
            )?;
            let (output, count, matches) = plan(&bytes, fonts, media_box, &needles, &mut search)?;
            output_bytes = output_bytes.saturating_add(output.len());
            if output_bytes > crate::MAX_INPUT_BYTES {
                return Err(Error::Limit(
                    "redaction aggregate expanded content exceeds 128 MiB",
                ));
            }
            report.content_comments_removed += comments;
            {
                if candidate.document.objects.len() >= crate::MAX_OBJECTS
                    || candidate.document.max_id as usize >= crate::MAX_OBJECTS
                {
                    return Err(Error::Limit("redaction object allocation"));
                }
                let id = candidate
                    .document
                    .add_object(Stream::new(Dictionary::new(), output));
                candidate
                    .document
                    .get_object_mut(candidate.pages[index])?
                    .as_dict_mut()?
                    .set("Contents", id);
                report.changed_text_shows += count;
                report.matched_targets += matches;
                if count != 0 || comments != 0 {
                    report.affected_pages.push(index as u32 + 1);
                }
            }
        }
        if report.changed_text_shows == 0 {
            return Err(Error::Refused(
                "no target occurs in supported page text objects",
            ));
        }
        // Reuse the existing sanitizer's bounded reachable-graph/full-rewrite proof
        // after page rewrites: the original text streams must not survive as orphans.
        let (candidate, second_cleanup) = candidate.sanitized_copy()?;
        report.rewritten_stream_orphans_removed = second_cleanup.unreachable_objects_removed;
        candidate.verify_redaction(values)?;
        Ok((candidate, report))
    }
}
