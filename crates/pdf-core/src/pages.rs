use crate::{Error, MAX_INPUT_BYTES, MAX_OBJECTS, MAX_PAGES, PdfDocument, Result, structure};
use lopdf::{Object, ObjectId, dictionary};
use std::collections::BTreeSet;

impl PdfDocument {
    /// Reorder every page exactly once. Page IDs, annotations and catalog structures remain intact.
    pub fn reorder(&mut self, order: &[u32]) -> Result<()> {
        self.ensure_editable()?;
        self.unique_selection(order)?;
        if order.len() != self.page_count() {
            return Err(Error::Pages("reorder requires a complete page permutation"));
        }
        self.ensure_no_page_labels()?;
        let ids = order
            .iter()
            .map(|number| self.pages[*number as usize - 1])
            .collect();
        let mut candidate = self.clone();
        candidate.rebuild_pages(ids)?;
        *self = candidate;
        Ok(())
    }

    pub fn reverse(&mut self) -> Result<()> {
        let order = self.all_pages().into_iter().rev().collect::<Vec<_>>();
        self.reorder(&order)
    }

    /// Change CropBox only; underlying content is retained and remains recoverable.
    pub fn crop(&mut self, pages: &[u32], rectangle: [f32; 4]) -> Result<()> {
        self.ensure_editable()?;
        self.unique_selection(pages)?;
        if rectangle.iter().any(|value| !value.is_finite())
            || rectangle[2] <= rectangle[0]
            || rectangle[3] <= rectangle[1]
        {
            return Err(Error::Pages("crop must be a finite, nonempty rectangle"));
        }
        let mut candidate = self.clone();
        for number in pages {
            let id = self.pages[*number as usize - 1];
            let media = structure::page_box(&self.document, id, b"MediaBox")?;
            if rectangle[0] < media[0]
                || rectangle[1] < media[1]
                || rectangle[2] > media[2]
                || rectangle[3] > media[3]
            {
                return Err(Error::Pages("crop rectangle lies outside the media box"));
            }
            candidate.document.get_dictionary_mut(id)?.set(
                "CropBox",
                Object::Array(rectangle.into_iter().map(Object::Real).collect()),
            );
        }
        candidate.info()?;
        *self = candidate;
        Ok(())
    }

    /// Insert before a one-based position; count + 1 appends. Dimensions are PDF points.
    pub fn insert_blank(&mut self, position: u32, width: f32, height: f32) -> Result<()> {
        self.ensure_editable()?;
        self.ensure_no_page_labels()?;
        if position == 0 || position as usize > self.page_count() + 1 {
            return Err(Error::Pages("insert position outside document"));
        }
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || width > 1_000_000.0
            || height > 1_000_000.0
        {
            return Err(Error::Pages(
                "blank page dimensions must be finite positive points",
            ));
        }
        if self.page_count() >= MAX_PAGES {
            return Err(Error::Limit("more than 20000 pages"));
        }
        if self.document.max_id as usize >= MAX_OBJECTS {
            return Err(Error::Limit("PDF object identifiers are exhausted"));
        }
        if self.document.objects.len() >= MAX_OBJECTS {
            return Err(Error::Limit("too many PDF objects"));
        }
        let mut candidate = self.clone();
        let page = candidate.document.add_object(dictionary! {
            "Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
            "CropBox" => vec![0.into(), 0.into(), width.into(), height.into()],
            "Resources" => dictionary! {}, "Rotate" => 0,
        });
        let mut ids = self.pages.clone();
        ids.insert(position as usize - 1, page);
        candidate.rebuild_pages(ids)?;
        *self = candidate;
        Ok(())
    }

    /// Add one independent page dictionary after each selected original, in document order.
    /// Content and resource objects remain shared. Advanced page/catalog structures are refused.
    pub fn duplicate_pages(&mut self, pages: &[u32]) -> Result<()> {
        self.ensure_editable()?;
        self.unique_selection(pages)?;
        self.ensure_subset_safe()?;
        if self.page_count().saturating_add(pages.len()) > MAX_PAGES {
            return Err(Error::Limit("more than 20000 pages"));
        }
        if self.document.objects.len().saturating_add(pages.len()) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(pages.len()) >= MAX_OBJECTS
        {
            return Err(Error::Limit("page duplication exceeds PDF object limits"));
        }
        self.ensure_duplication_safe()?;
        let selected = pages.iter().copied().collect::<BTreeSet<_>>();
        let mut copied_bytes = 0;
        let mut copied_values = 0;
        for number in &selected {
            duplication_footprint(
                self.document
                    .get_dictionary(self.pages[*number as usize - 1])?,
                &mut copied_bytes,
                &mut copied_values,
                true,
            )?;
        }
        let mut candidate = self.clone();
        // Resolve inheritance and move direct resource dictionaries once, before copying leaves.
        candidate.rebuild_pages(self.pages.clone())?;
        if candidate.document.objects.len().saturating_add(pages.len()) > MAX_OBJECTS
            || (candidate.document.max_id as usize).saturating_add(pages.len()) >= MAX_OBJECTS
        {
            return Err(Error::Limit(
                "normalized page duplication exceeds PDF object limits",
            ));
        }
        let mut copied_bytes = 0;
        let mut copied_values = 0;
        for number in &selected {
            let page = candidate
                .document
                .get_dictionary(candidate.pages[*number as usize - 1])?;
            duplication_footprint(page, &mut copied_bytes, &mut copied_values, false)?;
        }
        let mut ids = Vec::with_capacity(self.pages.len() + selected.len());
        for (index, id) in self.pages.iter().copied().enumerate() {
            ids.push(id);
            if selected.contains(&(index as u32 + 1)) {
                let page = candidate.document.get_dictionary(id)?.clone();
                ids.push(candidate.document.add_object(page));
            }
        }
        candidate.rebuild_pages(ids)?;
        candidate.info()?;
        *self = candidate;
        Ok(())
    }

    fn ensure_duplication_safe(&self) -> Result<()> {
        let allowed = [
            b"Type".as_slice(),
            b"Parent",
            b"Resources",
            b"MediaBox",
            b"CropBox",
            b"Rotate",
            b"Contents",
            b"BleedBox",
            b"TrimBox",
            b"ArtBox",
            b"UserUnit",
            b"Group",
            b"Metadata",
            b"Thumb",
            b"LastModified",
        ];
        let leaves = self.pages.iter().copied().collect::<BTreeSet<_>>();
        let mut tree_nodes = BTreeSet::new();
        for id in &self.pages {
            // Aliased leaf objects are outside this guarded tier.
            let page = self
                .document
                .objects
                .get(id)
                .ok_or(lopdf::Error::ObjectNotFound(*id))?
                .as_dict()
                .map_err(|_| Error::Refused("aliased page leaves cannot be duplicated"))?;
            if page
                .iter()
                .any(|(key, _)| !allowed.contains(&key.as_slice()))
            {
                return Err(Error::Refused(
                    "unsupported page dictionary fields cannot be duplicated",
                ));
            }
            let mut parent = page.get(b"Parent")?.as_reference()?;
            for depth in 0..=128 {
                let node = self
                    .document
                    .objects
                    .get(&parent)
                    .ok_or(lopdf::Error::ObjectNotFound(parent))?
                    .as_dict()
                    .map_err(|_| Error::Refused("aliased page tree nodes cannot be duplicated"))?;
                tree_nodes.insert(parent);
                match node.get(b"Parent") {
                    Ok(next) if depth < 128 => parent = next.as_reference()?,
                    Ok(_) => return Err(Error::Limit("page inheritance exceeds 128 levels")),
                    Err(_) => break,
                }
            }
        }
        for (id, object) in &self.document.objects {
            if tree_nodes.contains(id)
                || object.as_dict().is_ok_and(|dict| {
                    dict.get(b"Type")
                        .and_then(Object::as_name)
                        .is_ok_and(|name| name == b"Pages")
                })
            {
                for (key, value) in object.as_dict()? {
                    if key.as_slice() != b"Kids" && references_any(value, &leaves)? {
                        return Err(Error::Refused(
                            "page references require remapping before duplication",
                        ));
                    }
                }
            } else if references_any(object, &leaves)? {
                return Err(Error::Refused(
                    "page references require remapping before duplication",
                ));
            }
        }
        for (_, value) in &self.document.trailer {
            if references_any(value, &leaves)? {
                return Err(Error::Refused(
                    "trailer page references require remapping before duplication",
                ));
            }
        }
        Ok(())
    }

    /// Remove pages from the page tree. This is not secure deletion of their content.
    pub fn delete_pages(&mut self, pages: &[u32]) -> Result<()> {
        self.ensure_editable()?;
        self.unique_selection(pages)?;
        if pages.len() == self.page_count() {
            return Err(Error::Pages("cannot delete every page"));
        }
        let excluded = pages.iter().copied().collect::<BTreeSet<_>>();
        let order = self
            .all_pages()
            .into_iter()
            .filter(|page| !excluded.contains(page))
            .collect::<Vec<_>>();
        let replacement = self.extract_pages(&order)?;
        *self = replacement;
        Ok(())
    }

    /// Extract selected pages in caller order, refusing structures not safely remapped yet.
    /// Unreachable objects may remain in the copy; extraction is not a privacy operation.
    pub fn extract_pages(&self, pages: &[u32]) -> Result<Self> {
        self.ensure_editable()?;
        self.unique_selection(pages)?;
        self.ensure_subset_safe()?;
        let ids = pages
            .iter()
            .map(|number| self.pages[*number as usize - 1])
            .collect::<Vec<_>>();
        let kept = ids.iter().copied().collect::<BTreeSet<_>>();
        let removed = self
            .pages
            .iter()
            .copied()
            .filter(|id| !kept.contains(id))
            .collect::<BTreeSet<_>>();
        for (id, object) in &self.document.objects {
            if removed.contains(id) {
                continue;
            }
            if object.as_dict().is_ok_and(|dict| {
                dict.get(b"Type")
                    .and_then(Object::as_name)
                    .is_ok_and(|name| name == b"Pages")
            }) {
                continue;
            }
            if references_any(object, &removed)? {
                return Err(Error::Refused(
                    "an object references a removed page; remapping is not supported",
                ));
            }
        }
        let mut candidate = self.clone();
        candidate.rebuild_pages(ids)?;
        candidate.info()?;
        Ok(candidate)
    }

    fn unique_selection(&self, pages: &[u32]) -> Result<()> {
        self.validate_selection(pages)?;
        if pages.iter().copied().collect::<BTreeSet<_>>().len() != pages.len() {
            return Err(Error::Pages("page selection contains duplicates"));
        }
        Ok(())
    }

    fn ensure_no_page_labels(&self) -> Result<()> {
        if self.document.catalog()?.has(b"PageLabels") {
            return Err(Error::Refused(
                "page labels require remapping before changing page order",
            ));
        }
        Ok(())
    }

    fn ensure_subset_safe(&self) -> Result<()> {
        let allowed = [
            b"Type".as_slice(),
            b"Pages",
            b"Version",
            b"Metadata",
            b"PageLayout",
            b"PageMode",
            b"Lang",
            b"Info",
        ];
        if self
            .document
            .catalog()?
            .iter()
            .any(|(key, _)| !allowed.contains(&key.as_slice()))
        {
            return Err(Error::Refused(
                "catalog structures require remapping before page subsets or duplication",
            ));
        }
        if let Ok(info) = self.document.catalog()?.get(b"Info") {
            // MuPDF 1.28 stores producer metadata here; allow only inert string fields.
            let (_, info) = self.document.dereference(info)?;
            if info
                .as_dict()?
                .iter()
                .any(|(_, value)| !matches!(value, Object::String(_, _)))
            {
                return Err(Error::Refused(
                    "non-string catalog Info requires remapping before page subsets or duplication",
                ));
            }
        }
        for id in &self.pages {
            let page = self.document.get_dictionary(*id)?;
            if [
                b"Annots".as_slice(),
                b"AA",
                b"StructParents",
                b"B",
                b"PresSteps",
                b"PieceInfo",
            ]
            .iter()
            .any(|key| page.has(key))
            {
                return Err(Error::Refused(
                    "page annotations, actions or structure require remapping",
                ));
            }
        }
        Ok(())
    }

    fn rebuild_pages(&mut self, ids: Vec<ObjectId>) -> Result<()> {
        if self.document.objects.len() > MAX_OBJECTS {
            return Err(Error::Limit("too many PDF objects"));
        }
        let root = self.document.catalog()?.get(b"Pages")?.as_reference()?;
        let mut nodes = BTreeSet::new();
        for leaf in &ids {
            let mut id = *leaf;
            for depth in 0..=128 {
                let object = self
                    .document
                    .objects
                    .get(&id)
                    .ok_or(lopdf::Error::ObjectNotFound(id))?;
                let canonical = self.document.dereference(object)?.0.unwrap_or(id);
                if !nodes.insert(canonical) {
                    break;
                }
                let node = self.document.get_dictionary(canonical)?;
                match node.get(b"Parent") {
                    Ok(parent) if depth < 128 => id = parent.as_reference()?,
                    Ok(_) => return Err(Error::Limit("page inheritance exceeds 128 levels")),
                    Err(_) => break,
                }
            }
        }
        let mut direct_resources = Vec::new();
        for id in nodes {
            if let Ok(resources) = self.document.get_dictionary(id)?.get(b"Resources") {
                self.document.dereference(resources)?.1.as_dict()?;
                if matches!(resources, Object::Dictionary(_)) {
                    direct_resources.push(id);
                }
            }
        }
        if self.document.objects.len() + direct_resources.len() > MAX_OBJECTS {
            return Err(Error::Limit("too many PDF objects"));
        }
        if self.document.max_id as usize + direct_resources.len() > MAX_OBJECTS {
            return Err(Error::Limit("PDF object identifiers are exhausted"));
        }
        // Move shared ancestor dictionaries once, before materialization could copy them per leaf.
        for id in direct_resources {
            let resources = self
                .document
                .get_dictionary_mut(id)?
                .remove(b"Resources")
                .ok_or(Error::Invalid(
                    "page resources disappeared during normalization",
                ))?;
            let resource_id = self.document.add_object(resources);
            self.document
                .get_dictionary_mut(id)?
                .set("Resources", resource_id);
        }
        // Inheritance must be resolved against the old tree before any parent is changed.
        let materialized = ids
            .iter()
            .map(|id| {
                let mut values = Vec::new();
                for key in [b"Resources".as_slice(), b"MediaBox", b"CropBox", b"Rotate"] {
                    if let Some(value) = structure::inherited_raw(&self.document, *id, key)? {
                        values.push((key.to_vec(), value.clone()));
                    }
                }
                Ok((*id, values))
            })
            .collect::<Result<Vec<_>>>()?;
        for (id, values) in materialized {
            let page = self.document.get_dictionary_mut(id)?;
            for (key, value) in values {
                page.set(key, value);
            }
            page.set("Parent", root);
        }
        let tree = self.document.get_dictionary_mut(root)?;
        tree.set(
            "Kids",
            Object::Array(ids.iter().copied().map(Object::Reference).collect()),
        );
        tree.set("Count", ids.len() as i64);
        self.pages = structure::validate_pages(&self.document)?;
        self.info()?;
        Ok(())
    }
}

fn references_any(object: &Object, targets: &BTreeSet<ObjectId>) -> Result<bool> {
    let mut pending = vec![(object, 0)];
    while let Some((object, depth)) = pending.pop() {
        if depth > 128 {
            return Err(Error::Limit("object nesting exceeds 128 levels"));
        }
        let child_count = match object {
            Object::Array(values) => values.len(),
            Object::Dictionary(dict) => dict.len(),
            Object::Stream(stream) => stream.dict.len(),
            _ => 0,
        };
        if pending.len().saturating_add(child_count) > MAX_OBJECTS {
            return Err(Error::Limit("too many nested object values"));
        }
        match object {
            Object::Reference(id) if targets.contains(id) => return Ok(true),
            Object::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Object::Dictionary(dict) => {
                pending.extend(dict.iter().map(|(_, value)| (value, depth + 1)))
            }
            Object::Stream(stream) => {
                pending.extend(stream.dict.iter().map(|(_, value)| (value, depth + 1)))
            }
            _ => {}
        }
    }
    Ok(false)
}

// Charge copied inline values before dictionary cloning. Indirect targets are shared,
// not traversed or copied; direct streams are outside the supported page tier.
fn duplication_footprint(
    page: &lopdf::Dictionary,
    bytes: &mut usize,
    values: &mut usize,
    skip_resources: bool,
) -> Result<()> {
    let mut pending = Vec::new();
    for (key, value) in page {
        if skip_resources && key.as_slice() == b"Resources" {
            continue;
        }
        *bytes = bytes.saturating_add(key.len());
        pending.push((value, 0usize));
    }
    while let Some((value, depth)) = pending.pop() {
        *values = values.saturating_add(1);
        if depth > 128 || values.saturating_add(pending.len()) > MAX_OBJECTS {
            return Err(Error::Limit("duplicated page inline value budget exceeded"));
        }
        match value {
            Object::Array(items) => {
                if values
                    .saturating_add(pending.len())
                    .saturating_add(items.len())
                    > MAX_OBJECTS
                {
                    return Err(Error::Limit("duplicated page inline value budget exceeded"));
                }
                pending.extend(items.iter().map(|value| (value, depth + 1)));
            }
            Object::Dictionary(dictionary) => {
                if values
                    .saturating_add(pending.len())
                    .saturating_add(dictionary.len())
                    > MAX_OBJECTS
                {
                    return Err(Error::Limit("duplicated page inline value budget exceeded"));
                }
                for (key, value) in dictionary {
                    *bytes = bytes.saturating_add(key.len());
                    pending.push((value, depth + 1));
                }
            }
            Object::String(data, _) | Object::Name(data) => {
                *bytes = bytes.saturating_add(data.len())
            }
            Object::Stream(_) => {
                return Err(Error::Refused(
                    "inline streams cannot be duplicated; use indirect streams",
                ));
            }
            _ => {}
        }
        if *bytes > MAX_INPUT_BYTES {
            return Err(Error::Limit("duplicated page inline bytes exceed 128 MiB"));
        }
    }
    Ok(())
}
