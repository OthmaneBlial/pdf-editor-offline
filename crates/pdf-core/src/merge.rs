use crate::{Error, MAX_INPUT_BYTES, MAX_OBJECTS, MAX_PAGES, PdfDocument, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, dictionary, xref::XrefType};
use std::collections::{BTreeMap, BTreeSet};

impl PdfDocument {
    /// Beta: append simple documents with matching inert metadata. Object IDs change.
    /// Unsupported document structures and metadata conflicts fail without modifying either input.
    pub fn append(&mut self, other: &Self) -> Result<()> {
        self.ensure_editable()?;
        other.ensure_editable()?;
        let carriers = [source_gate(self)?, source_gate(other)?];
        if literal_info(&self.document, self.document.catalog()?, b"Info")?
            != literal_info(&other.document, other.document.catalog()?, b"Info")?
            || literal_info(&self.document, &self.document.trailer, b"Info")?
                != literal_info(&other.document, &other.document.trailer, b"Info")?
        {
            return Err(Error::Refused(
                "merge requires matching inert catalog and trailer metadata",
            ));
        }
        if self.page_count() + other.page_count() > MAX_PAGES {
            return Err(Error::Limit("merged document exceeds 20000 pages"));
        }
        if self.document.objects.len() + other.document.objects.len() + 1 > MAX_OBJECTS {
            return Err(Error::Limit("merged document exceeds one million objects"));
        }
        let mut values = 0;
        let mut bytes = 0;
        for (source, excluded) in [
            (&self.document, &carriers[0]),
            (&other.document, &carriers[1]),
        ] {
            check_graph(source, excluded, &mut values, &mut bytes)?;
        }
        let version = effective_version(&self.document)?.max(effective_version(&other.document)?);
        let mut output = Document::with_version(format!("{}.{}", version.0, version.1));
        output.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
        let mut maps = Vec::new();
        let mut next = 1u32;
        for (source, excluded) in [
            (&self.document, &carriers[0]),
            (&other.document, &carriers[1]),
        ] {
            let mut map = BTreeMap::new();
            for id in source.objects.keys().filter(|id| !excluded.contains(id)) {
                map.insert(*id, (next, id.1));
                next = next
                    .checked_add(1)
                    .ok_or(Error::Limit("PDF object identifiers are exhausted"))?;
            }
            for (id, object) in source
                .objects
                .iter()
                .filter(|(id, _)| !excluded.contains(id))
            {
                let mut object = object.clone();
                rewrite(&mut object, &map)?;
                output.objects.insert(map[id], object);
            }
            maps.push(map);
        }
        let primary_catalog = self.document.trailer.get(b"Root")?.as_reference()?;
        let roots = [
            maps[0][&self.document.catalog()?.get(b"Pages")?.as_reference()?],
            maps[1][&other.document.catalog()?.get(b"Pages")?.as_reference()?],
        ];
        let new_root = (next, 0);
        output.objects.insert(
            new_root,
            dictionary! { "Type" => "Pages",
            "Kids" => roots.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => (self.page_count() + other.page_count()) as i64 }
            .into(),
        );
        output.max_id = next;
        for root in roots {
            output.get_dictionary_mut(root)?.set("Parent", new_root);
        }
        let catalog = maps[0][&primary_catalog];
        output.get_dictionary_mut(catalog)?.set("Pages", new_root);
        if output.get_dictionary(catalog)?.has(b"Version") {
            let version = Object::Name(output.version.as_bytes().to_vec());
            output.get_dictionary_mut(catalog)?.set("Version", version);
        }
        output.trailer.set("Root", catalog);
        if let Ok(info) = self.document.trailer.get(b"Info") {
            let mut info = info.clone();
            rewrite(&mut info, &maps[0])?;
            output.trailer.set("Info", info);
        }
        let candidate = Self::from_document(output, false)?;
        *self = candidate;
        Ok(())
    }
}

fn literal_info<'a>(
    document: &'a Document,
    owner: &'a Dictionary,
    key: &[u8],
) -> Result<Option<&'a Dictionary>> {
    let Ok(value) = owner.get(key) else {
        return Ok(None);
    };
    let dictionary = document.dereference(value)?.1.as_dict()?;
    if dictionary
        .iter()
        .any(|(_, value)| !matches!(value, Object::String(_, _)))
    {
        return Err(Error::Refused(
            "merge metadata must contain inert direct string values",
        ));
    }
    Ok(Some(dictionary))
}

fn effective_version(document: &Document) -> Result<(u8, u8)> {
    let parse = |value: &[u8]| match value {
        [b'1', b'.', minor @ b'0'..=b'7'] => Ok((1, *minor - b'0')),
        b"2.0" => Ok((2, 0)),
        _ => Err(Error::Refused(
            "unsupported PDF document or catalog version",
        )),
    };
    let mut version = parse(document.version.as_bytes())?;
    if let Ok(value) = document.catalog()?.get(b"Version") {
        version = version.max(parse(document.dereference(value)?.1.as_name()?)?);
    }
    Ok(version)
}

fn source_gate(source: &PdfDocument) -> Result<BTreeSet<ObjectId>> {
    let document = &source.document;
    let catalog_keys = [b"Type".as_slice(), b"Pages", b"Version", b"Info"];
    if document
        .catalog()?
        .iter()
        .any(|(key, _)| !catalog_keys.contains(&key.as_slice()))
    {
        return Err(Error::Refused(
            "merge supports simple catalogs only; document structures need explicit remapping",
        ));
    }
    literal_info(document, document.catalog()?, b"Info")?;
    literal_info(document, &document.trailer, b"Info")?;
    let trailer_keys = [
        b"Root".as_slice(),
        b"Size",
        b"Info",
        b"ID",
        b"Prev",
        b"XRefStm",
        b"Type",
        b"W",
        b"Index",
        b"Length",
        b"Filter",
        b"DecodeParms",
    ];
    if document
        .trailer
        .iter()
        .any(|(key, _)| !trailer_keys.contains(&key.as_slice()))
    {
        return Err(Error::Refused(
            "custom trailer semantics are not supported by merge",
        ));
    }
    let xref_trailer = document
        .trailer
        .get(b"Type")
        .ok()
        .map(|value| {
            document
                .dereference(value)
                .and_then(|(_, value)| value.as_name())
        })
        .transpose()?;
    if xref_trailer.is_some_and(|name| name != b"XRef")
        || (xref_trailer.is_none()
            && [
                b"W".as_slice(),
                b"Index",
                b"Length",
                b"Filter",
                b"DecodeParms",
            ]
            .iter()
            .any(|key| document.trailer.has(key)))
    {
        return Err(Error::Refused(
            "non-xref trailer fields cannot be discarded by merge",
        ));
    }
    if let Ok(ids) = document.trailer.get(b"ID") {
        let ids = document.dereference(ids)?.1.as_array()?;
        if ids.len() != 2
            || ids
                .iter()
                .any(|value| !matches!(value, Object::String(_, _)))
        {
            return Err(Error::Refused("unsupported trailer identity format"));
        }
    }
    for id in &source.pages {
        if [
            b"Annots".as_slice(),
            b"AA",
            b"StructParents",
            b"B",
            b"PresSteps",
            b"PieceInfo",
            b"TemplateInstantiated",
        ]
        .iter()
        .any(|key| document.get_dictionary(*id).is_ok_and(|page| page.has(key)))
        {
            return Err(Error::Refused(
                "page annotations, actions, tags or templates need explicit merge support",
            ));
        }
    }
    let mut carriers = BTreeSet::new();
    for (id, object) in &document.objects {
        let dictionary = match object {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        if let Some(dictionary) = dictionary {
            let kind = dictionary
                .get(b"Type")
                .ok()
                .map(|value| {
                    document
                        .dereference(value)
                        .and_then(|(_, value)| value.as_name())
                })
                .transpose()?;
            if kind == Some(b"ObjStm")
                || kind == Some(b"Linearized")
                || dictionary.has(b"Linearized")
            {
                return Err(Error::Refused(
                    "object streams and linearization need validated merge normalization",
                ));
            }
            if kind == Some(b"XRef") {
                carriers.insert(*id);
            }
        }
    }
    Ok(carriers)
}

fn check_graph(
    document: &Document,
    carriers: &BTreeSet<ObjectId>,
    values: &mut usize,
    bytes: &mut usize,
) -> Result<()> {
    let mut pending = document
        .objects
        .iter()
        .filter(|(id, _)| !carriers.contains(id))
        .map(|(_, object)| (object, 0))
        .collect::<Vec<_>>();
    for (key, value) in document.trailer.iter() {
        *bytes += key.len();
        pending.push((value, 0));
    }
    while let Some((object, depth)) = pending.pop() {
        *values += 1;
        if depth > 128 || *values + pending.len() > 2_000_000 {
            return Err(Error::Limit("merge structural value budget exceeded"));
        }
        let dictionary = match object {
            Object::Reference(id) => {
                if carriers.contains(id) {
                    return Err(Error::Refused(
                        "a logical object references an old xref carrier",
                    ));
                }
                if !document.objects.contains_key(id) {
                    return Err(Error::Refused(
                        "merge input contains an unresolved object reference",
                    ));
                }
                None
            }
            Object::Array(array) => {
                pending.extend(array.iter().map(|value| (value, depth + 1)));
                None
            }
            Object::Dictionary(dictionary) => Some(dictionary),
            Object::Stream(stream) => {
                *bytes += stream.content.len();
                Some(&stream.dict)
            }
            Object::Name(name) | Object::String(name, _) => {
                *bytes += name.len();
                None
            }
            Object::Real(value) if !value.is_finite() => {
                return Err(Error::Invalid("non-finite merge object"));
            }
            _ => None,
        };
        if let Some(dictionary) = dictionary {
            for (key, value) in dictionary.iter() {
                *bytes += key.len();
                pending.push((value, depth + 1));
            }
        }
        if *bytes > MAX_INPUT_BYTES || *values + pending.len() > 2_000_000 {
            return Err(Error::Limit(
                "merge aggregate payload or structural value budget exceeded",
            ));
        }
    }
    Ok(())
}

fn rewrite(object: &mut Object, map: &BTreeMap<ObjectId, ObjectId>) -> Result<()> {
    let mut pending = vec![(object, 0)];
    while let Some((object, depth)) = pending.pop() {
        if depth > 128 {
            return Err(Error::Limit("merge reference rewrite nesting exceeded"));
        }
        match object {
            Object::Reference(id) => {
                *id = *map
                    .get(id)
                    .ok_or(Error::Refused("merge cannot remap an unresolved object"))?
            }
            Object::Array(values) => {
                pending.extend(values.iter_mut().map(|value| (value, depth + 1)))
            }
            Object::Dictionary(dictionary) => {
                pending.extend(dictionary.iter_mut().map(|(_, value)| (value, depth + 1)))
            }
            Object::Stream(stream) => {
                pending.extend(stream.dict.iter_mut().map(|(_, value)| (value, depth + 1)))
            }
            _ => {}
        }
    }
    Ok(())
}
