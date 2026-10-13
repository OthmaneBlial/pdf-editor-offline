//! Whole selected image assets; no partial pixels or visual keyword certificate.
use super::{Budget, ImageSelection, image_instances};
use crate::{Error, PdfDocument, Result, SanitizationReport};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod decoder;
use decoder::Raster;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovedImageDraw {
    pub page: u32,
    pub operator: u32,
    /// Nominal placement, not a claimed reader-independent painted footprint.
    pub corners: [[f64; 2]; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRedactionReport {
    pub profile: String,
    pub removed_draws: Vec<RemovedImageDraw>,
    pub removed_image_objects: usize,
    pub removed_mask_objects: usize,
    pub resource_links_removed: usize,
    pub rewritten_orphans_removed: usize,
    pub content_comments_removed: usize,
    pub full_rewrite: bool,
    pub cleanup: SanitizationReport,
    pub limitations: Vec<String>,
}
fn selected(selection: &ImageSelection) -> (u32, u32) {
    (selection.page, selection.operator)
}
fn collect_targets(
    selections: &[ImageSelection],
    rasters: &BTreeMap<ObjectId, Raster>,
) -> BTreeSet<ObjectId> {
    let mut ids = BTreeSet::new();
    for selection in selections {
        ids.insert(selection.image_id);
        if let Some(mask) = rasters[&selection.image_id].mask {
            ids.insert(mask);
        }
    }
    ids
}
fn remove_resources(
    document: &mut lopdf::Document,
    targets: &BTreeSet<ObjectId>,
    budget: &mut Budget,
) -> Result<usize> {
    budget.work(document.objects.len())?;
    let owners = document
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let dict = object.as_dict().ok()?;
            matches!(dict.get(b"Type").ok()?.as_name().ok()?, b"Page" | b"Pages").then_some(*id)
        })
        .collect::<Vec<_>>();
    let mut edited = BTreeSet::new();
    let mut links = 0;
    for owner in owners {
        let page = document.get_object(owner)?.as_dict()?;
        let Ok(resources) = page.get(b"Resources") else {
            continue;
        };
        let (resource_id, resources) = budget.resolve(document, resources)?;
        let resources = resources.as_dict()?;
        let Ok(objects) = resources.get(b"XObject") else {
            continue;
        };
        let (objects_id, objects) = budget.resolve(document, objects)?;
        let objects = objects.as_dict()?;
        // Shared dictionaries are edited once, on the private candidate only.
        // Final closed-graph verification refuses any unsupported external role.
        let place = if let Some(id) = objects_id {
            (0u8, id)
        } else if let Some(id) = resource_id {
            (1u8, id)
        } else {
            (2u8, owner)
        };
        if !edited.insert(place) {
            continue;
        }
        budget.work(objects.len())?;
        let mut names = Vec::new();
        for (name, value) in objects {
            let (id, _) = budget.resolve(document, value)?;
            if id.is_some_and(|id| targets.contains(&id)) {
                if name.len() > 4096 {
                    return Err(Error::Limit("image redaction resource name"));
                }
                budget.report(name.len())?;
                names.push(name.clone());
            }
        }
        if names.is_empty() {
            continue;
        }
        let objects = match place.0 {
            0 => document.get_object_mut(place.1)?.as_dict_mut()?,
            1 => document
                .get_object_mut(place.1)?
                .as_dict_mut()?
                .get_mut(b"XObject")?
                .as_dict_mut()?,
            _ => document
                .get_object_mut(owner)?
                .as_dict_mut()?
                .get_mut(b"Resources")?
                .as_dict_mut()?
                .get_mut(b"XObject")?
                .as_dict_mut()?,
        };
        for name in names {
            objects.remove(&name);
            links += 1;
        }
    }
    Ok(links)
}
fn target_absence(
    pdf: &PdfDocument,
    bytes: &[u8],
    targets: &BTreeSet<ObjectId>,
    patterns: &[&[u8]],
) -> Result<()> {
    if targets
        .iter()
        .any(|id| pdf.document.objects.contains_key(id))
    {
        return Err(Error::Verification(
            "selected image or mask object survived the full rewrite",
        ));
    }
    crate::redaction::verify_payload_absence(&pdf.document, bytes, patterns)
}
impl PdfDocument {
    /// Verify supplied output bytes against the deterministic bounded removal of
    /// these source-bound draws. This is not an arbitrary-PDF privacy verifier.
    pub fn verify_image_redaction_bytes(
        &self,
        selections: &[ImageSelection],
        output: &[u8],
    ) -> Result<()> {
        if output.is_empty() || output.len() > crate::MAX_INPUT_BYTES {
            return Err(Error::Limit("image redaction verification output bytes"));
        }
        let (expected, _) = self.redact_images_copy(selections)?;
        if expected.verified_bytes()? != output {
            return Err(Error::Verification(
                "image removal bytes differ from the verified source-bound rewrite",
            ));
        }
        Ok(())
    }
    /// Permanently remove complete selected Image Do assets into a verified new
    /// document. All uses of shared target images/masks must be included. Original
    /// files/history remain; this does not certify arbitrary visual keyword absence.
    pub fn redact_images_copy(
        &self,
        selections: &[ImageSelection],
    ) -> Result<(Self, ImageRedactionReport)> {
        self.ensure_editable()?;
        if selections.is_empty() || selections.len() > 10_000 {
            return Err(Error::Limit(
                "image redaction requires one to 10000 selected draws",
            ));
        }
        let original_info = self.info()?;
        let source = crate::save::serialize_bounded(&self.document, crate::MAX_INPUT_BYTES)?;
        let hash = Sha256::digest(&source)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        drop(source);
        let mut chosen = BTreeMap::new();
        for selection in selections {
            if selection.source_hash != hash
                || selection.operator == 0
                || selection.operator > 100_000
                || selection.image_id.0 == 0
                || selection.resource_name.is_empty()
                || selection.resource_name.len() > 4096
                || chosen.insert(selected(selection), selection).is_some()
            {
                return Err(Error::Refused(
                    "image selection is stale, duplicate or malformed",
                ));
            }
            self.validate_selection(&[selection.page])?;
        }
        let mut rasters = BTreeMap::new();
        let mut budget = Budget::new();
        let mut decoded = 0;
        let mut content_bytes = 0usize;
        let mut draws = Vec::new();
        let mut other_images = BTreeSet::new();
        let mut found = BTreeSet::new();
        let mut contents = BTreeMap::new();
        let mut images_count = 0usize;
        let mut comments = 0usize;
        // One source hash and no retained per-page resource clones. Total retained
        // decoded content and privacy sample buffers are separately capped.
        for page in 1..=self.page_count() as u32 {
            let program = image_instances::program_with_hash(self, page, &hash, None, &mut budget)?;
            content_bytes = content_bytes.saturating_add(program.bytes.len());
            if content_bytes > crate::MAX_INPUT_BYTES {
                return Err(Error::Limit("image redaction content exceeds 128 MiB"));
            }
            images_count = images_count.saturating_add(program.images.len());
            if images_count > 10_000 {
                return Err(Error::Limit("whole-document image inventory exceeds 10000"));
            }
            let mut output = program.bytes;
            for image in &program.images {
                decoder::decode(
                    self,
                    image.selection.image_id,
                    &mut rasters,
                    &mut budget,
                    &mut decoded,
                    0,
                )?;
                if let Some(selection) = chosen.get(&selected(&image.selection)) {
                    if **selection != image.selection || !image.replaceable {
                        return Err(Error::Refused(
                            "selected image is stale or structurally read-only",
                        ));
                    }
                    found.insert(selected(selection));
                    let span = &program.spans[selection.operator as usize - 1];
                    if &output[span.operator.clone()] != b"Do" || span.operands.len() != 1 {
                        return Err(Error::Refused("selected image lexical span is ambiguous"));
                    }
                    output[span.full.clone()].fill(b' ');
                    draws.push(RemovedImageDraw {
                        page,
                        operator: selection.operator,
                        corners: image.corners,
                    });
                } else {
                    other_images.insert(image.selection.image_id);
                }
            }
            let (output, count) = crate::content_spans::clear_content_comments(&output)?;
            comments += count;
            contents.insert(page, output);
        }
        if found.len() != chosen.len() {
            return Err(Error::Refused(
                "selected image occurrence does not belong to this source",
            ));
        }
        let targets = collect_targets(selections, &rasters);
        if other_images.iter().any(|id| {
            targets.contains(id) || rasters[id].mask.is_some_and(|mask| targets.contains(&mask))
        }) {
            return Err(Error::Refused(
                "surviving shared image or mask use must be explicitly included",
            ));
        }
        // A separately stored duplicate must not be mistaken for removed data.
        // This conservative check can also refuse coincidentally identical samples.
        for id in &other_images {
            if targets
                .iter()
                .any(|target| rasters[id].data == rasters[target].data)
            {
                return Err(Error::Refused(
                    "duplicate selected image samples survive in another image",
                ));
            }
        }
        if self.document.objects.len().saturating_add(self.pages.len()) >= crate::MAX_OBJECTS {
            return Err(Error::Limit("image redaction page rewrite object budget"));
        }
        let mut patterns = Vec::new();
        let mut proof_bytes = 0usize;
        for id in &targets {
            let samples = rasters[id].data.as_slice();
            let encoded = self
                .document
                .get_object(*id)?
                .as_stream()?
                .content
                .as_slice();
            for value in [samples, encoded] {
                if patterns.contains(&value) {
                    continue;
                }
                proof_bytes = proof_bytes.saturating_add(value.len());
                if proof_bytes > 32 * 1024 * 1024 {
                    return Err(Error::Limit("selected image proof material exceeds 32 MiB"));
                }
                patterns.push(value);
            }
        }
        let (mut candidate, cleanup) = self.sanitized_copy()?;
        candidate.document.reference_table.cross_reference_type =
            lopdf::xref::XrefType::CrossReferenceTable;
        candidate.document.trailer.remove(b"ID");
        let links = remove_resources(&mut candidate.document, &targets, &mut budget)?;
        for (id, raster) in &rasters {
            if !targets.contains(id) {
                candidate
                    .document
                    .objects
                    .insert(*id, Object::Stream(raster.canonical()));
            }
        }
        for (page, bytes) in contents {
            if candidate.document.objects.len() >= crate::MAX_OBJECTS
                || candidate.document.max_id as usize >= crate::MAX_OBJECTS
            {
                return Err(Error::Limit("image redaction content allocation"));
            }
            let id = candidate
                .document
                .add_object(Stream::new(Dictionary::new(), bytes));
            candidate
                .document
                .get_object_mut(candidate.pages[page as usize - 1])?
                .as_dict_mut()?
                .set("Contents", id);
        }
        let (candidate, second) = candidate.sanitized_copy()?;
        let bytes = candidate.verified_bytes()?;
        let reopened = Self::from_bytes(&bytes, None)?;
        if reopened.verified_bytes()? != bytes {
            return Err(Error::Verification(
                "image redaction rewrite is not canonical",
            ));
        }
        if crate::inspect::reachable_objects(&reopened.document)?.len()
            != reopened.document.objects.len()
        {
            return Err(Error::Verification(
                "image redaction retained unreachable objects",
            ));
        }
        if reopened.info()?.pages != original_info.pages
            || reopened.extract_text(&(1..=self.page_count() as u32).collect::<Vec<_>>())?
                != self.extract_text(&(1..=self.page_count() as u32).collect::<Vec<_>>())?
        {
            return Err(Error::Verification(
                "image redaction changed page geometry or text",
            ));
        }
        // Fresh decoder instances validate returned bytes; cached source samples
        // cannot be treated as proof of a recoverable output decoder result.
        let mut remaining = BTreeMap::new();
        let mut output_budget = Budget::new();
        let mut output_decoded = 0;
        for id in &other_images {
            decoder::decode(
                &reopened,
                *id,
                &mut remaining,
                &mut output_budget,
                &mut output_decoded,
                0,
            )?;
        }
        for (id, actual) in &remaining {
            if rasters.get(id) != Some(actual) {
                return Err(Error::Verification(
                    "surviving privacy image samples changed",
                ));
            }
        }
        let approved = remaining.keys().copied().collect::<BTreeSet<_>>();
        let pages = crate::redaction::profile::Profile {
            pdf: &reopened,
            seen: BTreeSet::new(),
            work: 0,
            decoded: 0,
            images: Some(&approved),
        }
        .pages()?;
        for (index, (bytes, fonts)) in pages.iter().enumerate() {
            crate::redaction::verify_image_program(
                bytes,
                fonts,
                original_info.pages[index].media_box,
            )?;
        }
        target_absence(&reopened, &bytes, &targets, &patterns)?;
        let roots = selections
            .iter()
            .map(|s| s.image_id)
            .collect::<BTreeSet<_>>();
        Ok((reopened, ImageRedactionReport {
            profile: "whole-image-rgb-gray8-plain-flate-smask-v1".into(),
            removed_draws: draws, removed_image_objects: roots.len(),
            removed_mask_objects: targets.difference(&roots).count(),
            resource_links_removed: links, rewritten_orphans_removed: second.unreachable_objects_removed,
            content_comments_removed: comments, full_rewrite: true, cleanup,
            limitations: vec![
                "Whole selected image draws, resources and masks are removed; surviving shared/duplicate samples refuse unless included.".into(),
                "Only exact eight-bit DeviceRGB/DeviceGray plain/Flate samples and matching grayscale soft masks are privacy decoded; no renderer recovery/defaults are trusted.".into(),
                "This is not partial-pixel redaction or verification of arbitrary visual keywords, vector copies, steganography or all private information.".into(),
                "Original files, undo history, recovery journals and filesystem copies remain. Nominal corners do not claim exact painted footprints.".into(),
            ],
        }))
    }
}
