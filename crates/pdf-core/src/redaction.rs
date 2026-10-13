//! Permanent removal for an intentionally small, fully inspected plaintext profile.
use crate::{Error, PdfDocument, Result};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod profile;
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
        }
        .pages()?;
        let mut report=TextRedactionReport {
            changed_text_shows:0, matched_targets:0, affected_pages:Vec::new(),
            full_rewrite:true, profile:"ascii-standard-type1-uniform-background-v2".into(),
            cleanup, rewritten_stream_orphans_removed:0,content_comments_removed:0,
            limitations:vec![
                "Exact ASCII target glyphs are removed; their advance is retained as a TJ adjustment, preserving unmatched glyph positions.".into(),
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
