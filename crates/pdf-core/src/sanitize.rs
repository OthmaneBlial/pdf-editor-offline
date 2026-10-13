//! Targeted full-rewrite sanitization; not content redaction or a privacy certificate.
use crate::{Error, MAX_INPUT_BYTES, MAX_STREAM_BYTES, PdfDocument, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, content::Content};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};
const MAX_VALUES: usize = 2_000_000;
const MAX_FINDINGS: usize = 20_000;
const LIMITATIONS: &[&str] = &[
    "This removes specified PDF metadata, file associations and actions, not arbitrary private data.",
    "Visible page text, images, embedded font data and unrelated annotation contents are preserved.",
    "Image/stream-internal metadata, duplicated strings, steganography and undisclosed custom payloads are not scrubbed.",
    "Document identifiers and unrelated custom metadata are retained.",
    "Form values, hidden page/layer content, thumbnails and application-specific data are not removed.",
    "Visible attachment removal supports balanced vector appearances or standard omitted-AP icons; other appearances are refused.",
    "This is not secure text/image redaction, filesystem erasure, or proof that no private data remains.",
];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttachmentIcon {
    Graph,
    PushPin,
    Paperclip,
    Tag,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovedAttachmentAnnotation {
    pub page: u32,
    pub rectangle: [f32; 4],
    /// Screen visibility permitted by flags; not proof of painted pixels or print visibility.
    pub visible: bool,
    pub predefined_icon: Option<AttachmentIcon>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizationReport {
    pub info_links_removed: usize,
    pub xmp_links_removed: usize,
    pub associated_file_links_removed: usize,
    pub embedded_file_name_trees_removed: usize,
    pub javascript_name_trees_removed: usize,
    pub collections_removed: usize,
    pub action_links_removed: usize,
    pub automatic_triggers_removed: usize,
    pub attachment_annotations_removed: Vec<RemovedAttachmentAnnotation>,
    pub unreachable_objects_removed: usize,
    pub transport_containers_removed: usize,
    pub full_rewrite: bool,
    pub limitations: Vec<String>,
}
impl SanitizationReport {
    fn new() -> Self {
        Self {
            info_links_removed: 0,
            xmp_links_removed: 0,
            associated_file_links_removed: 0,
            embedded_file_name_trees_removed: 0,
            javascript_name_trees_removed: 0,
            collections_removed: 0,
            action_links_removed: 0,
            automatic_triggers_removed: 0,
            attachment_annotations_removed: Vec::new(),
            unreachable_objects_removed: 0,
            transport_containers_removed: 0,
            full_rewrite: true,
            limitations: LIMITATIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        }
    }
}
#[derive(Default)]
struct Budget {
    values: usize,
    bytes: usize,
}
impl Budget {
    fn queue(&self, pending: usize, additional: usize) -> Result<()> {
        if self
            .values
            .checked_add(pending)
            .and_then(|n| n.checked_add(additional))
            .is_none_or(|n| n > MAX_VALUES)
        {
            return Err(Error::Limit(
                "sanitization graph queue exceeds two million values",
            ));
        }
        Ok(())
    }
    fn step(&mut self, depth: usize, count: usize) -> Result<()> {
        self.values = self
            .values
            .checked_add(count)
            .ok_or(Error::Limit("sanitization work overflow"))?;
        if depth > 128 || self.values > MAX_VALUES {
            return Err(Error::Limit("sanitization exceeds nesting/work budget"));
        }
        Ok(())
    }
    fn payload(&mut self, count: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or(Error::Limit("sanitization byte budget overflow"))?;
        if self.bytes > MAX_INPUT_BYTES {
            return Err(Error::Limit(
                "sanitization logical/copy payload exceeds 128 MiB",
            ));
        }
        Ok(())
    }
    fn resolve<'a>(
        &mut self,
        document: &'a Document,
        mut object: &'a Object,
    ) -> Result<(Option<ObjectId>, &'a Object)> {
        let mut seen = HashSet::new();
        let mut id = None;
        while let Object::Reference(next) = object {
            self.step(seen.len() + 1, 1)?;
            if !seen.insert(*next) {
                return Err(Error::Invalid("cyclic sanitization reference"));
            }
            id = Some(*next);
            object = document
                .objects
                .get(next)
                .ok_or(Error::Invalid("missing sanitization reference"))?;
        }
        Ok((id, object))
    }
}
fn name<'a>(
    document: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
    budget: &mut Budget,
) -> Result<Option<&'a [u8]>> {
    dict.get(key)
        .ok()
        .map(|value| {
            budget
                .resolve(document, value)
                .map(|(_, value)| value.as_name().ok())
        })
        .transpose()
        .map(Option::flatten)
}
fn action(document: &Document, dict: &Dictionary, budget: &mut Budget) -> Result<bool> {
    let kind = name(document, dict, b"Type", budget)?;
    Ok(kind == Some(b"Action")
        || (kind.is_none()
            && matches!(
                name(document, dict, b"S", budget)?,
                Some(
                    b"JavaScript"
                        | b"GoTo"
                        | b"GoToR"
                        | b"GoToE"
                        | b"Launch"
                        | b"URI"
                        | b"SubmitForm"
                        | b"ResetForm"
                        | b"ImportData"
                        | b"Hide"
                        | b"Named"
                        | b"SetOCGState"
                        | b"Rendition"
                        | b"Trans"
                        | b"GoTo3DView"
                        | b"Thread"
                        | b"Sound"
                        | b"Movie"
                )
            )))
}
fn carrier(object: &Object) -> bool {
    matches!(object.type_name(), Ok(b"XRef" | b"ObjStm"))
}
fn address(dict: &Dictionary) -> usize {
    dict as *const Dictionary as usize
}
#[derive(Default)]
struct Roles {
    named_maps: HashSet<usize>,
    name_tables: HashSet<usize>,
    content_streams: BTreeSet<ObjectId>,
}
impl Roles {
    fn register_map(
        &mut self,
        document: &Document,
        value: &Object,
        budget: &mut Budget,
        kind: &[u8],
    ) -> Result<()> {
        if let Object::Dictionary(dict) = budget.resolve(document, value)?.1 {
            if !self.named_maps.insert(address(dict)) {
                return Ok(());
            }
            if matches!(
                name(document, dict, b"Type", budget)?,
                Some(
                    b"Catalog"
                        | b"Page"
                        | b"Pages"
                        | b"Annot"
                        | b"StructTreeRoot"
                        | b"StructElem"
                        | b"Action"
                        | b"Metadata"
                        | b"EmbeddedFile"
                        | b"Filespec"
                        | b"Font"
                        | b"FontDescriptor"
                        | b"ExtGState"
                        | b"XObject"
                        | b"OCG"
                        | b"OCMD"
                )
            ) {
                return Err(Error::Refused(
                    "a resource namespace has a conflicting document-structure type",
                ));
            }
            budget.step(1, dict.len())?;
            for (_, value) in dict.iter() {
                let value = budget.resolve(document, value)?.1;
                let valid = match (kind, value) {
                    (_, Object::Null) => true,
                    (b"ColorSpace", Object::Name(_)) => true,
                    (b"ColorSpace", Object::Array(values)) => values
                        .first()
                        .is_some_and(|first| matches!(first, Object::Name(_))),
                    (b"XObject" | b"CharProcs", Object::Stream(_)) => true,
                    (b"Font" | b"ExtGState" | b"Properties", Object::Dictionary(_)) => true,
                    (b"Pattern" | b"Shading", Object::Dictionary(_) | Object::Stream(_)) => true,
                    _ => false,
                };
                if !valid {
                    return Err(Error::Refused(
                        "unsupported or conflicting resource namespace values",
                    ));
                }
            }
        }
        Ok(())
    }
    fn content(&mut self, document: &Document, value: &Object, budget: &mut Budget) -> Result<()> {
        let mut pending = vec![(value, 1)];
        let mut arrays = HashSet::new();
        while let Some((value, depth)) = pending.pop() {
            budget.step(depth, 1)?;
            let (id, value) = budget.resolve(document, value)?;
            match value {
                Object::Stream(_) => {
                    self.content_streams.insert(
                        id.ok_or(Error::Refused("direct content streams are unsupported"))?,
                    );
                }
                Object::Array(values) => {
                    if id.is_some_and(|id| !arrays.insert(id)) {
                        return Err(Error::Invalid("cyclic or repeated content array"));
                    }
                    budget.step(depth, values.len())?;
                    budget.queue(pending.len(), values.len())?;
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Object::Null => {}
                _ => return Err(Error::Invalid("invalid content stream association")),
            }
        }
        Ok(())
    }
    fn inspect(
        &mut self,
        document: &Document,
        dict: &Dictionary,
        budget: &mut Budget,
    ) -> Result<()> {
        if let Ok(resources) = dict.get(b"Resources")
            && let Object::Dictionary(resources) = budget.resolve(document, resources)?.1
        {
            for key in [
                b"Font".as_slice(),
                b"XObject",
                b"ExtGState",
                b"ColorSpace",
                b"Pattern",
                b"Shading",
                b"Properties",
            ] {
                if let Ok(value) = resources.get(key) {
                    self.register_map(document, value, budget, key)?;
                }
            }
        }
        if name(document, dict, b"Type", budget)? == Some(b"Catalog")
            && let Ok(value) = dict.get(b"Names")
            && let Object::Dictionary(names) = budget.resolve(document, value)?.1
        {
            self.name_tables.insert(address(names));
        }
        if name(document, dict, b"Subtype", budget)? == Some(b"Type3")
            && let Ok(value) = dict.get(b"CharProcs")
        {
            self.register_map(document, value, budget, b"CharProcs")?;
            if let Object::Dictionary(map) = budget.resolve(document, value)?.1 {
                for (_, value) in map.iter() {
                    self.content(document, value, budget)?;
                }
            }
        }
        if let Ok(value) = dict.get(b"AP")
            && let Object::Dictionary(map) = budget.resolve(document, value)?.1
        {
            for key in [b"N".as_slice(), b"R", b"D"] {
                if let Ok(value) = map.get(key) {
                    match budget.resolve(document, value)?.1 {
                        Object::Dictionary(states) => {
                            self.named_maps.insert(address(states));
                            for (_, value) in states.iter() {
                                self.content(document, value, budget)?;
                            }
                        }
                        Object::Stream(_) => self.content(document, value, budget)?,
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}
fn preflight(document: &Document) -> Result<Roles> {
    let mut roles = Roles::default();
    let mut budget = Budget::default();
    let mut pending = document
        .objects
        .iter()
        .map(|(id, value)| (value, 1, Some(*id)))
        .collect::<Vec<_>>();
    while let Some((value, depth, id)) = pending.pop() {
        budget.step(depth, 1)?;
        match value {
            Object::String(bytes, _) | Object::Name(bytes) => budget.payload(bytes.len())?,
            Object::Array(values) => {
                budget.step(depth, values.len())?;
                budget.queue(pending.len(), values.len())?;
                pending.extend(values.iter().map(|v| (v, depth + 1, None)));
            }
            Object::Dictionary(dict) => {
                roles.inspect(document, dict, &mut budget)?;
                budget.queue(pending.len(), dict.len())?;
                for (key, value) in dict.iter() {
                    budget.payload(key.len())?;
                    pending.push((value, depth + 1, None));
                }
            }
            Object::Stream(stream) => {
                budget.payload(stream.content.len())?;
                roles.inspect(document, &stream.dict, &mut budget)?;
                if name(document, &stream.dict, b"Subtype", &mut budget)? == Some(b"Form")
                    || stream
                        .dict
                        .get(b"PatternType")
                        .and_then(Object::as_i64)
                        .ok()
                        == Some(1)
                {
                    roles
                        .content_streams
                        .insert(id.ok_or(Error::Refused("direct form streams are unsupported"))?);
                }
                for (key, value) in stream.dict.iter() {
                    budget.payload(key.len())?;
                    pending.push((value, depth + 1, None));
                }
            }
            _ => {}
        }
        if pending.len() + budget.values > MAX_VALUES {
            return Err(Error::Limit(
                "sanitization graph queue exceeds two million values",
            ));
        }
    }
    for (_, value) in document.objects.iter() {
        if let Object::Dictionary(dict) = value
            && name(document, dict, b"Type", &mut budget)? == Some(b"Page")
            && let Ok(contents) = dict.get(b"Contents")
        {
            roles.content(document, contents, &mut budget)?;
        }
    }
    // A dictionary cannot simultaneously be a protected resource namespace and
    // an actual document structure. Otherwise reserved-key protection could hide
    // a targeted link on the Catalog, page tree, or catalog Names dictionary.
    let catalog = budget
        .resolve(document, document.trailer.get(b"Root")?)?
        .1
        .as_dict()?;
    let mut structures = HashSet::from([address(catalog)]);
    if let Ok(form) = catalog.get(b"AcroForm") {
        structures.insert(address(budget.resolve(document, form)?.1.as_dict()?));
    }
    let mut visited = BTreeSet::new();
    // All callers operate on the core's already validated page tree.
    for (_, page) in document.get_pages() {
        let reference = Object::Reference(page);
        let mut value = &reference;
        for depth in 1..=128 {
            budget.step(depth, 1)?;
            let (id, object) = budget.resolve(document, value)?;
            if let Some(id) = id
                && !visited.insert(id)
            {
                break;
            }
            let dictionary = object.as_dict()?;
            structures.insert(address(dictionary));
            let Ok(parent) = dictionary.get(b"Parent") else {
                break;
            };
            if depth == 128 {
                return Err(Error::Limit("sanitization page ancestors exceed 128"));
            }
            value = parent;
        }
    }
    if roles
        .named_maps
        .iter()
        .any(|map| structures.contains(map) || roles.name_tables.contains(map))
    {
        return Err(Error::Refused(
            "a resource namespace aliases an actual document structure",
        ));
    }
    Ok(roles)
}

struct Plan<'a> {
    document: &'a Document,
    roles: Roles,
    targets: BTreeSet<ObjectId>,
    budget: Budget,
    copies: Budget,
    report: SanitizationReport,
    pages: BTreeMap<ObjectId, u32>,
}
impl Plan<'_> {
    fn private(&mut self, value: &Object) -> Result<()> {
        let mut pending = vec![(value, 1)];
        let mut seen = HashSet::new();
        while let Some((value, depth)) = pending.pop() {
            self.budget.step(depth, 1)?;
            match value {
                Object::Reference(id) => {
                    self.targets.insert(*id);
                    if seen.insert(*id) {
                        pending.push((
                            self.document
                                .objects
                                .get(id)
                                .ok_or(Error::Invalid("missing private payload reference"))?,
                            depth + 1,
                        ));
                    }
                }
                Object::Array(values) => {
                    self.budget.step(depth, values.len())?;
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Object::Dictionary(dict) => {
                    let is_action = action(self.document, dict, &mut self.budget)?;
                    let subtype = name(self.document, dict, b"S", &mut self.budget)?;
                    for (key, value) in dict.iter() {
                        let public = is_action
                            && (key == b"Type"
                                || key == b"S"
                                || (key == b"D" && subtype == Some(b"GoTo"))
                                || (key == b"State" && subtype == Some(b"SetOCGState"))
                                || (key == b"T" && subtype == Some(b"Hide"))
                                || (key == b"Fields"
                                    && matches!(subtype, Some(b"SubmitForm" | b"ResetForm"))));
                        if !public {
                            pending.push((value, depth + 1));
                        }
                    }
                }
                Object::Stream(stream) => {
                    for (key, value) in stream.dict.iter() {
                        if ![b"Type".as_slice(), b"Subtype", b"Length", b"Filter", b"DL"]
                            .contains(&key.as_slice())
                        {
                            pending.push((value, depth + 1));
                        }
                    }
                }
                _ => {}
            }
            if pending.len() + self.budget.values > MAX_VALUES {
                return Err(Error::Limit(
                    "private payload queue exceeds sanitization budget",
                ));
            }
        }
        Ok(())
    }
    fn clone_value(&mut self, value: &Object) -> Result<Object> {
        let mut pending = vec![(value, 1)];
        while let Some((value, depth)) = pending.pop() {
            self.copies.step(depth, 1)?;
            match value {
                Object::Name(bytes) | Object::String(bytes, _) => {
                    self.copies.payload(bytes.len())?
                }
                Object::Array(values) => {
                    self.copies.step(depth, values.len())?;
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Object::Dictionary(dict) => {
                    for (key, value) in dict.iter() {
                        self.copies.payload(key.len())?;
                        pending.push((value, depth + 1));
                    }
                }
                Object::Stream(_) => {
                    return Err(Error::Refused("direct annotation streams are unsupported"));
                }
                _ => {}
            }
        }
        Ok(value.clone())
    }
    fn attachments(&mut self, page: ObjectId, value: &Object) -> Result<Option<Object>> {
        let (_, value) = self.budget.resolve(self.document, value)?;
        let values = value.as_array()?;
        self.copies.step(1, values.len())?;
        let mut result = Vec::with_capacity(values.len());
        let mut removed = false;
        for value in values {
            let (id, object) = self.budget.resolve(self.document, value)?;
            let dict = object.as_dict()?;
            if name(self.document, dict, b"Subtype", &mut self.budget)? != Some(b"FileAttachment") {
                let mut retained = self.clone_value(value)?;
                self.rewrite(value, &mut retained, 2)?;
                result.push(retained);
                continue;
            }
            if [
                b"StructParent".as_slice(),
                b"Popup",
                b"IRT",
                b"RT",
                b"Parent",
                b"FT",
            ]
            .iter()
            .any(|key| dict.has(key))
            {
                return Err(Error::Refused(
                    "linked, tagged or field attachment annotations are unsupported",
                ));
            }
            let flags = dict
                .get(b"F")
                .ok()
                .map(|value| {
                    self.budget
                        .resolve(self.document, value)
                        .and_then(|(_, value)| Ok(value.as_i64()?))
                })
                .transpose()?
                .unwrap_or(0);
            if !(0..=u32::MAX as i64).contains(&flags) {
                return Err(Error::Invalid("invalid attachment annotation flags"));
            }
            if flags & (64 | 128 | 512) != 0 {
                return Err(Error::Refused("read-only or locked attachment annotation"));
            }
            let visible = flags & (1 | 2 | 32) == 0;
            if visible && flags & (8 | 16) != 0 {
                return Err(Error::Refused(
                    "NoZoom/NoRotate attachment footprint cannot be proved",
                ));
            }
            let values = self
                .budget
                .resolve(self.document, dict.get(b"Rect")?)?
                .1
                .as_array()?;
            if values.len() != 4 {
                return Err(Error::Invalid(
                    "attachment rectangle must have four coordinates",
                ));
            }
            let mut rectangle = [0.0; 4];
            for (index, value) in values.iter().enumerate() {
                rectangle[index] = self.budget.resolve(self.document, value)?.1.as_float()?;
            }
            if rectangle.iter().any(|value| !value.is_finite())
                || rectangle[0] >= rectangle[2]
                || rectangle[1] >= rectangle[3]
            {
                return Err(Error::Invalid("invalid attachment annotation rectangle"));
            }
            let predefined_icon = if let Ok(ap) = dict.get(b"AP") {
                self.appearance_targets(ap)?;
                let ap = self.budget.resolve(self.document, ap)?.1.as_dict()?;
                let normal = self
                    .budget
                    .resolve(self.document, ap.get(b"N")?)?
                    .1
                    .as_stream()?;
                if name(self.document, &normal.dict, b"Subtype", &mut self.budget)? != Some(b"Form")
                {
                    return Err(Error::Refused(
                        "attachment appearance must be a Form XObject",
                    ));
                }
                if visible {
                    self.vector_appearance(normal)?;
                }
                let bbox = self
                    .budget
                    .resolve(self.document, normal.dict.get(b"BBox")?)?
                    .1
                    .as_array()?;
                if bbox.len() != 4
                    || bbox
                        .iter()
                        .any(|value| value.as_float().map_or(true, |value| !value.is_finite()))
                    || bbox[0].as_float()? >= bbox[2].as_float()?
                    || bbox[1].as_float()? >= bbox[3].as_float()?
                {
                    return Err(Error::Refused(
                        "attachment appearance bounding box is unsupported",
                    ));
                }
                if let Ok(matrix) = normal.dict.get(b"Matrix") {
                    let matrix = self.budget.resolve(self.document, matrix)?.1.as_array()?;
                    if matrix.len() != 6
                        || matrix
                            .iter()
                            .any(|value| value.as_float().map_or(true, |value| !value.is_finite()))
                        || matrix
                            .iter()
                            .map(Object::as_float)
                            .collect::<std::result::Result<Vec<_>, _>>()?
                            != [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                    {
                        return Err(Error::Refused(
                            "attachment appearance matrix is unsupported",
                        ));
                    }
                }
                None
            } else {
                Some(self.default_icon(dict)?)
            };
            let page_number = self
                .pages
                .get(&page)
                .copied()
                .ok_or(Error::Invalid("attachment page not in tree"))?;
            if self.report.attachment_annotations_removed.len() >= MAX_FINDINGS {
                return Err(Error::Limit("too many attachment annotation removals"));
            }
            self.report
                .attachment_annotations_removed
                .push(RemovedAttachmentAnnotation {
                    page: page_number,
                    rectangle,
                    visible,
                    predefined_icon,
                });
            if let Some(id) = id {
                self.targets.insert(id);
            }
            for (key, value) in dict.iter() {
                if ![
                    b"Type".as_slice(),
                    b"Subtype",
                    b"Rect",
                    b"F",
                    b"P",
                    b"AP",
                    b"AS",
                ]
                .contains(&key.as_slice())
                {
                    self.private(value)?;
                }
            }
            // Missing file specifications are malformed, even when an icon exists.
            dict.get(b"FS")?;
            removed = true;
        }
        Ok(removed.then_some(Object::Array(result)))
    }
    fn appearance_targets(&mut self, value: &Object) -> Result<()> {
        let (id, value) = self.budget.resolve(self.document, value)?;
        if let Some(id) = id {
            self.targets.insert(id);
        }
        let map = value.as_dict()?;
        for key in [b"N".as_slice(), b"R", b"D"] {
            if let Ok(value) = map.get(key) {
                let (id, value) = self.budget.resolve(self.document, value)?;
                if let Some(id) = id {
                    self.targets.insert(id);
                }
                match value {
                    Object::Stream(_) => {}
                    Object::Dictionary(states) => {
                        self.budget.step(2, states.len())?;
                        for (_, value) in states.iter() {
                            let (id, value) = self.budget.resolve(self.document, value)?;
                            value.as_stream()?;
                            if let Some(id) = id {
                                self.targets.insert(id);
                            }
                        }
                    }
                    _ => return Err(Error::Refused("unsupported attachment appearance")),
                }
            }
        }
        Ok(())
    }
    fn default_icon(&mut self, dict: &Dictionary) -> Result<AttachmentIcon> {
        // ISO 32000-1 Table 184 specifies these names and PushPin as the default.
        // Poppler 26.10 builds a 24x24 form and maps its BBox to the annotation Rect.
        // This does not depend on our renderer having builtin icon display support.
        let icon = match name(self.document, dict, b"Name", &mut self.budget)? {
            Some(b"Graph") => AttachmentIcon::Graph,
            Some(b"PushPin") => AttachmentIcon::PushPin,
            None if !dict.has(b"Name") => AttachmentIcon::PushPin,
            Some(b"Paperclip") => AttachmentIcon::Paperclip,
            Some(b"Tag") => AttachmentIcon::Tag,
            _ => return Err(Error::Refused("unsupported predefined attachment icon")),
        };
        if let Ok(color) = dict.get(b"C") {
            let values = self.budget.resolve(self.document, color)?.1.as_array()?;
            self.budget.step(1, values.len())?;
            if ![0, 1, 3, 4].contains(&values.len()) {
                return Err(Error::Refused("unsupported attachment icon color"));
            }
            for value in values {
                let value = self.budget.resolve(self.document, value)?.1.as_float()?;
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(Error::Refused("invalid attachment icon color"));
                }
            }
        }
        if let Ok(opacity) = dict.get(b"CA") {
            let opacity = self.budget.resolve(self.document, opacity)?.1.as_float()?;
            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                return Err(Error::Refused("invalid attachment icon opacity"));
            }
        }
        Ok(icon)
    }
    fn vector_appearance(&mut self, stream: &lopdf::Stream) -> Result<()> {
        let plain = stream.get_plain_content_with_limit(MAX_STREAM_BYTES)?;
        self.budget.payload(plain.len())?;
        let content = Content::decode(&plain)?;
        let mut graphics = 0usize;
        for operation in content.operations {
            self.budget.step(1, 1)?;
            if ![
                "q", "Q", "cm", "w", "J", "j", "M", "d", "i", "m", "l", "c", "v", "y", "h", "re",
                "S", "s", "f", "F", "f*", "B", "B*", "b", "b*", "n", "W", "W*", "g", "G", "rg",
                "RG", "k", "K",
            ]
            .contains(&operation.operator.as_str())
            {
                return Err(Error::Refused(
                    "attachment appearance uses unsupported non-vector operators",
                ));
            }
            let expected = match operation.operator.as_str() {
                "cm" | "c" => 6,
                "v" | "y" | "re" | "k" | "K" => 4,
                "rg" | "RG" => 3,
                "m" | "l" | "d" => 2,
                "w" | "J" | "j" | "M" | "i" | "g" | "G" => 1,
                _ => 0,
            };
            if operation.operands.len() != expected
                || operation.operands.iter().enumerate().any(|(index, value)| {
                    if operation.operator == "d" && index == 0 {
                        value.as_array().map_or(true, |items| {
                            items.iter().any(|value| {
                                value.as_float().map_or(true, |n| !n.is_finite() || n < 0.0)
                            })
                        })
                    } else {
                        value.as_float().map_or(true, |n| !n.is_finite())
                    }
                })
            {
                return Err(Error::Refused("malformed attachment appearance operands"));
            }
            match operation.operator.as_str() {
                "q" => {
                    graphics += 1;
                    if graphics > 128 {
                        return Err(Error::Limit("attachment graphics nesting exceeds 128"));
                    }
                }
                "Q" => {
                    graphics = graphics
                        .checked_sub(1)
                        .ok_or(Error::Refused("unbalanced attachment graphics state"))?;
                }
                _ => {}
            }
            let mut values = operation
                .operands
                .iter()
                .map(|value| (value, 1))
                .collect::<Vec<_>>();
            while let Some((value, depth)) = values.pop() {
                self.budget.step(depth, 1)?;
                match value {
                    Object::Integer(_) => {}
                    Object::Real(number) if number.is_finite() => {}
                    Object::Array(items) => {
                        self.budget.step(depth, items.len())?;
                        values.extend(items.iter().map(|value| (value, depth + 1)));
                    }
                    _ => {
                        return Err(Error::Refused(
                            "attachment appearance has unsupported operands",
                        ));
                    }
                }
            }
        }
        if graphics != 0 {
            return Err(Error::Refused("unbalanced attachment graphics state"));
        }
        Ok(())
    }
    fn rewrite(&mut self, original: &Object, output: &mut Object, depth: usize) -> Result<()> {
        self.budget.step(depth, 1)?;
        match (original, output) {
            (Object::Dictionary(before), Object::Dictionary(after)) => {
                self.dictionary(before, after, depth, false, None)
            }
            (Object::Stream(before), Object::Stream(after)) => {
                self.dictionary(&before.dict, &mut after.dict, depth, false, None)
            }
            (Object::Array(before), Object::Array(after)) => {
                for (a, b) in before.iter().zip(after) {
                    self.rewrite(a, b, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn dictionary(
        &mut self,
        before: &Dictionary,
        after: &mut Dictionary,
        depth: usize,
        trailer: bool,
        page: Option<ObjectId>,
    ) -> Result<()> {
        let named = self.roles.named_maps.contains(&address(before));
        let name_table = self.roles.name_tables.contains(&address(before));
        let catalog = name(self.document, before, b"Type", &mut self.budget)? == Some(b"Catalog");
        for (key, value) in before.iter() {
            let mut remove = false;
            if !named {
                match key.as_slice() {
                    b"Info" if trailer || catalog => {
                        self.private(value)?;
                        self.report.info_links_removed += 1;
                        remove = true;
                    }
                    b"Metadata" => {
                        self.private(value)?;
                        self.report.xmp_links_removed += 1;
                        remove = true;
                    }
                    b"AF" | b"MCAF" => {
                        self.private(value)?;
                        self.report.associated_file_links_removed += 1;
                        remove = true;
                    }
                    b"EmbeddedFiles" if name_table => {
                        self.private(value)?;
                        self.report.embedded_file_name_trees_removed += 1;
                        remove = true;
                    }
                    b"JavaScript" if name_table => {
                        self.private(value)?;
                        self.report.javascript_name_trees_removed += 1;
                        remove = true;
                    }
                    b"Collection" if catalog => {
                        self.private(value)?;
                        self.report.collections_removed += 1;
                        remove = true;
                    }
                    b"AA" | b"OpenAction" => {
                        if key == b"OpenAction"
                            && let (_, Object::Array(_)) =
                                self.budget.resolve(self.document, value)?
                        {
                            if let Object::Reference(id) = value {
                                self.targets.insert(*id);
                            }
                        } else {
                            self.private(value)?;
                        }
                        self.report.automatic_triggers_removed += 1;
                        remove = true;
                    }
                    b"A" => {
                        if let Object::Dictionary(action_dict) =
                            self.budget.resolve(self.document, value)?.1
                            && (action(self.document, action_dict, &mut self.budget)?
                                || (name(self.document, action_dict, b"Type", &mut self.budget)?
                                    .is_none()
                                    && name(self.document, action_dict, b"S", &mut self.budget)?
                                        .is_some()))
                        {
                            self.private(value)?;
                            self.report.action_links_removed += 1;
                            remove = true;
                        }
                    }
                    b"Annots" if page.is_some() => {
                        if let Some(value) = self.attachments(
                            page.ok_or(Error::Invalid("missing page context"))?,
                            value,
                        )? {
                            after.set(key.clone(), value);
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            if remove {
                after.remove(key);
            } else if let Ok(output) = after.get_mut(key) {
                self.rewrite(value, output, depth + 1)?;
            }
        }
        Ok(())
    }
}

fn target_dictionary(
    document: &Document,
    dict: &Dictionary,
    roles: &Roles,
    budget: &mut Budget,
) -> Result<bool> {
    if roles.named_maps.contains(&address(dict)) {
        return Ok(false);
    }
    Ok(matches!(
        name(document, dict, b"Type", budget)?,
        Some(b"Metadata" | b"EmbeddedFile")
    ) || dict.has(b"EF")
        || action(document, dict, budget)?
        || name(document, dict, b"Subtype", budget)? == Some(b"FileAttachment"))
}
fn prove(document: &Document) -> Result<()> {
    let roles = preflight(document)?;
    let mut budget = Budget::default();
    if document.trailer.has(b"Info") {
        return Err(Error::Verification("Info metadata remains"));
    }
    let mut pending = document
        .objects
        .values()
        .map(|value| (value, 1))
        .collect::<Vec<_>>();
    while let Some((value, depth)) = pending.pop() {
        budget.step(depth, 1)?;
        let dict = match value {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            Object::Array(values) => {
                budget.step(depth, values.len())?;
                pending.extend(values.iter().map(|value| (value, depth + 1)));
                None
            }
            _ => None,
        };
        if let Some(dict) = dict {
            if !roles.named_maps.contains(&address(dict)) {
                if target_dictionary(document, dict, &roles, &mut budget)?
                    || (name(document, dict, b"Type", &mut budget)? == Some(b"Catalog")
                        && (dict.has(b"Info") || dict.has(b"Collection")))
                    || [
                        b"Metadata".as_slice(),
                        b"AF",
                        b"MCAF",
                        b"AA",
                        b"OpenAction",
                        b"JS",
                    ]
                    .iter()
                    .any(|key| dict.has(key))
                    || (roles.name_tables.contains(&address(dict))
                        && (dict.has(b"EmbeddedFiles") || dict.has(b"JavaScript")))
                {
                    return Err(Error::Refused(
                        "targeted data remains through an unsupported association",
                    ));
                }
                if [b"XFA".as_slice(), b"RichMediaContent", b"RichMediaSettings"]
                    .iter()
                    .any(|key| dict.has(key))
                {
                    return Err(Error::Refused(
                        "XFA and rich media need a separate sanitization implementation",
                    ));
                }
            }
            budget.step(depth, dict.len())?;
            pending.extend(dict.iter().map(|(_, value)| (value, depth + 1)));
        }
    }
    let mut decoded = 0usize;
    let mut work = Budget::default();
    for id in roles.content_streams {
        let stream = document.get_object(id)?.as_stream()?;
        let plain =
            stream.get_plain_content_with_limit(MAX_STREAM_BYTES.min(MAX_INPUT_BYTES - decoded))?;
        decoded = decoded
            .checked_add(plain.len())
            .ok_or(Error::Limit("sanitization content byte overflow"))?;
        if decoded > MAX_INPUT_BYTES {
            return Err(Error::Limit("sanitization decoded content exceeds 128 MiB"));
        }
        let content = Content::decode(&plain)?;
        for operation in content.operations {
            work.step(1, 1)?;
            let mut values = operation
                .operands
                .iter()
                .map(|value| (value, 1))
                .collect::<Vec<_>>();
            while let Some((value, depth)) = values.pop() {
                work.step(depth, 1)?;
                match value {
                    Object::Reference(_) => {
                        return Err(Error::Refused(
                            "indirect content operands prevent safe garbage collection",
                        ));
                    }
                    Object::Dictionary(dict) => {
                        if [
                            b"AF".as_slice(),
                            b"MCAF",
                            b"JS",
                            b"AA",
                            b"OpenAction",
                            b"EF",
                            b"Metadata",
                        ]
                        .iter()
                        .any(|key| dict.has(key))
                            || action(document, dict, &mut work)?
                        {
                            return Err(Error::Refused(
                                "inline marked-content targets need explicit stream rewriting",
                            ));
                        }
                        work.step(depth, dict.len())?;
                        values.extend(dict.iter().map(|(_, value)| (value, depth + 1)));
                    }
                    Object::Array(items) => {
                        work.step(depth, items.len())?;
                        values.extend(items.iter().map(|value| (value, depth + 1)));
                    }
                    Object::Stream(_) => {
                        return Err(Error::Refused("inline stream operand is unsupported"));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
impl PdfDocument {
    /// Remove specified standard structures into a separate, fully verified document.
    /// Source objects are not modified; unidentified shared target references are refused.
    pub fn sanitized_copy(&self) -> Result<(PdfDocument, SanitizationReport)> {
        self.ensure_editable()?;
        let before = self.info()?.pages;
        let roles = preflight(&self.document)?;
        let mut plan = Plan {
            document: &self.document,
            roles,
            targets: BTreeSet::new(),
            budget: Budget::default(),
            copies: Budget::default(),
            report: SanitizationReport::new(),
            pages: self
                .pages
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, index as u32 + 1))
                .collect(),
        };
        for (id, object) in &self.document.objects {
            let dict = match object {
                Object::Dictionary(dict) => Some(dict),
                Object::Stream(stream) => Some(&stream.dict),
                _ => None,
            };
            if let Some(dict) = dict
                && target_dictionary(&self.document, dict, &plan.roles, &mut plan.budget)?
            {
                plan.targets.insert(*id);
            }
        }
        plan.budget = Budget::default();
        let mut candidate = self.clone();
        for (id, original) in &self.document.objects {
            if carrier(original) {
                continue;
            }
            let output = candidate
                .document
                .objects
                .get_mut(id)
                .ok_or(Error::Invalid("candidate object missing"))?;
            let page = plan.pages.contains_key(id).then_some(*id);
            if let (Object::Dictionary(before), Object::Dictionary(after)) =
                (original, &mut *output)
            {
                plan.dictionary(before, after, 1, false, page)?;
            } else {
                plan.rewrite(original, output, 1)?;
            }
        }
        plan.dictionary(
            &self.document.trailer,
            &mut candidate.document.trailer,
            1,
            true,
            None,
        )?;
        for key in [
            b"Prev".as_slice(),
            b"XRefStm",
            b"Size",
            b"Type",
            b"W",
            b"Index",
            b"Length",
            b"Filter",
            b"DecodeParms",
        ] {
            candidate.document.trailer.remove(key);
        }
        let reachable = crate::inspect::reachable_objects(&candidate.document)?;
        if plan.targets.iter().any(|id| reachable.contains(id)) {
            return Err(Error::Refused(
                "shared private target is still referenced outside sanitized structures",
            ));
        }
        let carriers = candidate
            .document
            .objects
            .iter()
            .filter_map(|(id, object)| carrier(object).then_some(*id))
            .collect::<BTreeSet<_>>();
        if carriers.iter().any(|id| reachable.contains(id)) {
            return Err(Error::Refused(
                "logical reference to an old transport container prevents sanitization",
            ));
        }
        plan.report.transport_containers_removed = carriers.len();
        plan.report.unreachable_objects_removed =
            candidate.document.objects.len() - reachable.len();
        candidate
            .document
            .objects
            .retain(|id, _| reachable.contains(id));
        candidate.document.max_id = candidate
            .document
            .objects
            .last_key_value()
            .map_or(0, |(id, _)| id.0);
        if candidate.info()?.pages != before || candidate.pages != self.pages {
            return Err(Error::Verification(
                "sanitization changed page geometry or identity",
            ));
        }
        prove(&candidate.document)?;
        let bytes = candidate.verified_bytes()?;
        let reopened = PdfDocument::from_bytes(&bytes, None)?;
        prove(&reopened.document)?;
        Ok((candidate, plan.report))
    }
    /// Publish a verified new copy without overwriting any existing path.
    pub fn sanitize_new(&self, output: impl AsRef<Path>) -> Result<SanitizationReport> {
        let (candidate, report) = self.sanitized_copy()?;
        candidate.save_new(output)?;
        Ok(report)
    }
}
