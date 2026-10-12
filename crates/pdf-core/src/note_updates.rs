use crate::{
    Error, MAX_NOTE_AUTHOR_BYTES, MAX_NOTE_CONTENT_BYTES, MAX_OBJECTS, Note, PdfDocument, Result,
    notes::{annotation, annotation_entries, resolve, unicode_string},
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};

impl PdfDocument {
    /// Edit the selected physical /Annots occurrence by copy on write; other aliases survive.
    /// Rect/appearance remain unchanged. Popup/reply/action/tag/name relationships are refused.
    pub fn edit_note(
        &mut self,
        page: u32,
        annotation_index: u32,
        contents: Option<&str>,
        author: Option<&str>,
    ) -> Result<Note> {
        self.ensure_editable()?;
        if contents.is_none() && author.is_none() {
            return Err(Error::Refused("select note contents or author to edit"));
        }
        for (value, limit) in [
            (contents, MAX_NOTE_CONTENT_BYTES),
            (author, MAX_NOTE_AUTHOR_BYTES),
        ] {
            if let Some(value) = value {
                if value.len() > limit {
                    return Err(Error::Limit(
                        "note contents exceed 64 KiB or author exceeds 1 KiB",
                    ));
                }
                if value.contains('\0') {
                    return Err(Error::Refused("note text must not contain NUL"));
                }
            }
        }
        let (mut note, dictionary, entries) = selected_note(self, page, annotation_index)?;
        validate_copy(entries, Some(dictionary))?;
        validate_note_schema(&self.document, &self.pages, dictionary, note.flags)?;
        ensure_unlinked(
            &self.document,
            &self.pages,
            note.object_id,
            self.pages[page as usize - 1],
        )?;
        if self.document.objects.len().saturating_add(1) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(1) > MAX_OBJECTS
        {
            return Err(Error::Limit(
                "PDF object capacity exhausted while editing note",
            ));
        }
        let mut dictionary = dictionary.clone();
        if let Some(contents) = contents {
            dictionary.set("Contents", unicode_string(contents));
            note.contents = contents.to_owned();
        }
        if let Some(author) = author {
            dictionary.set("T", unicode_string(author));
            note.author = author.to_owned();
        }
        dictionary.set("P", self.pages[page as usize - 1]);
        validate_copy(entries, Some(&dictionary))?;
        let mut entries = entries.to_vec();
        let mut candidate = self.clone();
        let id = candidate.document.add_object(dictionary);
        entries[annotation_index as usize - 1] = id.into();
        candidate
            .document
            .get_dictionary_mut(candidate.pages[page as usize - 1])?
            .set("Annots", entries);
        let candidate = Self::from_document(candidate.document, candidate.encrypted_input)?;
        note.object_id = Some(id);
        *self = candidate;
        Ok(note)
    }

    /// Remove one physical occurrence. Old objects/values remain; this is not secure erasure.
    pub fn remove_note(&mut self, page: u32, annotation_index: u32) -> Result<Note> {
        self.ensure_editable()?;
        let (note, dictionary, entries) = selected_note(self, page, annotation_index)?;
        validate_copy(entries, None)?;
        validate_note_schema(&self.document, &self.pages, dictionary, note.flags)?;
        ensure_unlinked(
            &self.document,
            &self.pages,
            note.object_id,
            self.pages[page as usize - 1],
        )?;
        let mut entries = entries.to_vec();
        entries.remove(annotation_index as usize - 1);
        let mut candidate = self.clone();
        candidate
            .document
            .get_dictionary_mut(candidate.pages[page as usize - 1])?
            .set("Annots", entries);
        let candidate = Self::from_document(candidate.document, candidate.encrypted_input)?;
        *self = candidate;
        Ok(note)
    }
}

fn selected_note(
    document: &PdfDocument,
    page: u32,
    index: u32,
) -> Result<(Note, &Dictionary, &[Object])> {
    document.validate_selection(&[page])?;
    let entries = annotation_entries(&document.document, document.pages[page as usize - 1])?;
    let entry = entries
        .get(
            index
                .checked_sub(1)
                .ok_or(Error::Pages("annotation index must be one-based"))? as usize,
        )
        .ok_or(Error::Pages("annotation index outside page Annots array"))?;
    let (_, dictionary) = annotation(&document.document, entry)?;
    let note = document
        .list_notes(&[page])?
        .into_iter()
        .find(|note| note.annotation_index == index)
        .ok_or(Error::Refused(
            "selected annotation is not a sticky Text note",
        ))?;
    Ok((note, dictionary, entries))
}

fn validate_note_schema(
    document: &Document,
    pages: &[ObjectId],
    dictionary: &Dictionary,
    flags: i64,
) -> Result<()> {
    if flags & (64 | 128 | 512) != 0 {
        return Err(Error::Refused(
            "read-only or locked notes cannot be edited or removed",
        ));
    }
    for (key, _) in dictionary.iter() {
        if !matches!(
            key.as_slice(),
            b"Type"
                | b"Subtype"
                | b"P"
                | b"Rect"
                | b"Contents"
                | b"T"
                | b"F"
                | b"C"
                | b"Name"
                | b"Open"
                | b"AP"
                | b"AS"
                | b"M"
                | b"CA"
                | b"Border"
                | b"Lang"
                | b"OC"
        ) {
            return Err(Error::Refused(
                "popup/reply/action/tag/unique-name/custom note structures are unsupported",
            ));
        }
    }
    if let Ok(value) = dictionary.get(b"Type")
        && resolve(document, value)?.1.as_name()? != b"Annot"
    {
        return Err(Error::Refused("note Type must be Annot"));
    }
    if let Ok(value) = dictionary.get(b"P") {
        let (id, _) = resolve(document, value)?;
        if !id.is_some_and(|id| pages.contains(&id)) {
            return Err(Error::Refused("note P must reference a live page"));
        }
    }
    Ok(())
}

fn validate_copy(entries: &[Object], dictionary: Option<&Dictionary>) -> Result<()> {
    const MAX_VALUES: usize = 100_000;
    const MAX_PAYLOAD: usize = 1024 * 1024;
    if entries
        .len()
        .saturating_add(dictionary.map_or(0, Dictionary::len))
        > MAX_VALUES
    {
        return Err(Error::Limit(
            "note update copies more than 100000 direct values",
        ));
    }
    let mut pending = entries
        .iter()
        .map(|value| (value, 0usize))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    let mut payload = dictionary.map_or(0, |dict| {
        dict.iter().map(|(key, _)| key.len()).sum::<usize>()
    });
    if payload > MAX_PAYLOAD {
        return Err(Error::Limit(
            "note update direct string/name/key payload exceeds 1 MiB",
        ));
    }
    if let Some(dict) = dictionary {
        pending.extend(dict.iter().map(|(_, value)| (value, 0usize)));
    }
    while let Some((object, depth)) = pending.pop() {
        visited += 1;
        if depth > 128 || visited > MAX_VALUES {
            return Err(Error::Limit(
                "note update direct graph exceeds structural budget",
            ));
        }
        let children = match object {
            Object::Array(values) => values.len(),
            Object::Dictionary(dict) => dict.len(),
            _ => 0,
        };
        if visited
            .saturating_add(pending.len())
            .saturating_add(children)
            > MAX_VALUES
        {
            return Err(Error::Limit(
                "note update copies more than 100000 direct values",
            ));
        }
        match object {
            Object::String(bytes, _) | Object::Name(bytes) => {
                payload = payload.saturating_add(bytes.len())
            }
            Object::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Object::Dictionary(dict) => {
                payload =
                    payload.saturating_add(dict.iter().map(|(key, _)| key.len()).sum::<usize>());
                if payload > MAX_PAYLOAD {
                    return Err(Error::Limit(
                        "note update direct string/name/key payload exceeds 1 MiB",
                    ));
                }
                pending.extend(dict.iter().map(|(_, value)| (value, depth + 1)));
            }
            Object::Stream(_) => {
                return Err(Error::Refused(
                    "direct streams in annotation arrays are unsupported",
                ));
            }
            _ => {}
        }
        if payload > MAX_PAYLOAD {
            return Err(Error::Limit(
                "note update direct string/name/key payload exceeds 1 MiB",
            ));
        }
    }
    Ok(())
}

fn ensure_unlinked(
    document: &Document,
    pages: &[ObjectId],
    target: Option<ObjectId>,
    selected_page: ObjectId,
) -> Result<()> {
    // COW retains old objects. Their unreachable references are harmless history;
    // live links, including an external alias that keeps an old container alive, are not.
    let reachable = crate::inspect::reachable_objects(document)?;
    let mut reverse = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
    for (id, object) in &document.objects {
        if reachable.contains(id)
            && let Object::Reference(to) = object
        {
            reverse.entry(*to).or_default().push(*id);
        }
    }
    let note_aliases = alias_closure(&reverse, &target.into_iter().collect::<Vec<_>>())?;
    let mut targets = note_aliases.clone();
    let page_ids = pages.iter().copied().collect::<BTreeSet<_>>();
    let mut arrays = BTreeSet::new();
    let mut occurrences = 0usize;
    for page in pages {
        let entries = annotation_entries(document, *page)?;
        occurrences = occurrences.saturating_add(entries.len());
        if occurrences > 100_000 {
            return Err(Error::Limit(
                "note reference analysis exceeds 100000 page annotation occurrences",
            ));
        }
        for entry in entries {
            annotation(document, entry)?;
        }
        let dictionary = document.get_dictionary(*page)?;
        if let Ok(value) = dictionary.get(b"Annots")
            && let Some(id) = resolve(document, value)?.0
        {
            arrays.insert(id);
            if (target.is_none() && *page==selected_page) || entries.iter().any(|entry|matches!(entry,Object::Reference(reference) if note_aliases.contains(reference))) {
                targets.insert(id);
                if targets.len()>10_000 {return Err(Error::Limit("note incoming alias closure exceeds 10000 objects"));}
            }
        }
    }
    let aliases = alias_closure(&reverse, &targets.into_iter().collect::<Vec<_>>())?;
    let mut visited = 0usize;
    for (id, object) in &document.objects {
        if !reachable.contains(id) {
            continue;
        }
        if aliases.contains(id) && matches!(object, Object::Reference(_)) {
            continue;
        }
        if page_ids.contains(id) {
            let dictionary = object.as_dict()?;
            for (key, value) in dictionary.iter() {
                if key == b"Annots" {
                    scan_entries(
                        annotation_entries(document, *id)?,
                        &note_aliases,
                        &aliases,
                        &mut visited,
                    )?;
                } else {
                    scan(value, &aliases, &mut visited)?;
                }
            }
        } else if arrays.contains(id) {
            scan_entries(object.as_array()?, &note_aliases, &aliases, &mut visited)?;
        } else {
            scan(object, &aliases, &mut visited)?;
        }
    }
    for (_, value) in document.trailer.iter() {
        scan(value, &aliases, &mut visited)?;
    }
    Ok(())
}

fn alias_closure(
    reverse: &BTreeMap<ObjectId, Vec<ObjectId>>,
    targets: &[ObjectId],
) -> Result<BTreeSet<ObjectId>> {
    if targets.len() > 10_000 {
        return Err(Error::Limit(
            "note incoming alias closure exceeds 10000 objects",
        ));
    }
    let mut aliases = targets.iter().copied().collect::<BTreeSet<_>>();
    let mut pending = aliases.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let next = reverse.get(&id).map_or(&[][..], Vec::as_slice);
        for next in next {
            if aliases.insert(*next) {
                if aliases.len() > 10_000 {
                    return Err(Error::Limit(
                        "note incoming alias closure exceeds 10000 objects",
                    ));
                }
                pending.push(*next);
            }
        }
    }
    Ok(aliases)
}

fn scan_entries(
    entries: &[Object],
    note_aliases: &BTreeSet<ObjectId>,
    aliases: &BTreeSet<ObjectId>,
    visited: &mut usize,
) -> Result<()> {
    for entry in entries {
        if matches!(entry,Object::Reference(id) if note_aliases.contains(id)) {
            continue;
        }
        scan(entry, aliases, visited)?;
    }
    Ok(())
}
fn scan(root: &Object, aliases: &BTreeSet<ObjectId>, visited: &mut usize) -> Result<()> {
    let mut pending = vec![(root, 0usize)];
    while let Some((object, depth)) = pending.pop() {
        *visited += 1;
        if depth > 128 || *visited > 2_000_000 {
            return Err(Error::Limit(
                "note incoming reference analysis exceeds structural budget",
            ));
        }
        let children = match object {
            Object::Array(values) => values.len(),
            Object::Dictionary(dict) => dict.len(),
            Object::Stream(stream) => stream.dict.len(),
            _ => 0,
        };
        if visited
            .saturating_add(pending.len())
            .saturating_add(children)
            > 2_000_000
        {
            return Err(Error::Limit(
                "note incoming reference analysis exceeds structural budget",
            ));
        }
        match object {
            Object::Reference(id) if aliases.contains(id) => {
                return Err(Error::Refused(
                    "note has references outside supported page annotation occurrences",
                ));
            }
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
    Ok(())
}
