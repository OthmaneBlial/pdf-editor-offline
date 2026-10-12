use crate::{Error, MAX_OBJECTS, PdfDocument, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_METADATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_INFO_ENTRIES: usize = 10_000;
const MAX_INFO_COPY_BYTES: usize = 1024 * 1024;
const MAX_INFO_COPY_VALUES: usize = 10_000;

/// Standard text fields in the trailer /Info dictionary. Dates and XMP are separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataKey {
    Title,
    Author,
    Subject,
    Keywords,
    Creator,
    Producer,
}

impl MetadataKey {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "title" => Ok(Self::Title),
            "author" => Ok(Self::Author),
            "subject" => Ok(Self::Subject),
            "keywords" => Ok(Self::Keywords),
            "creator" => Ok(Self::Creator),
            "producer" => Ok(Self::Producer),
            _ => Err(Error::Refused("unsupported standard metadata key")),
        }
    }

    fn pdf_name(self) -> &'static [u8] {
        match self {
            Self::Title => b"Title",
            Self::Author => b"Author",
            Self::Subject => b"Subject",
            Self::Keywords => b"Keywords",
            Self::Creator => b"Creator",
            Self::Producer => b"Producer",
        }
    }
}

const KEYS: [MetadataKey; 6] = [
    MetadataKey::Title,
    MetadataKey::Author,
    MetadataKey::Subject,
    MetadataKey::Keywords,
    MetadataKey::Creator,
    MetadataKey::Producer,
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentMetadata {
    pub fields: BTreeMap<MetadataKey, String>,
    pub info_present: bool,
    /// Presence of the active catalog's /Metadata key; its payload is never changed here.
    pub xmp_present: bool,
    /// Nonstandard catalog /Info is preserved independently from trailer /Info.
    pub catalog_info_present: bool,
    pub xmp_synchronized: bool,
    pub secure_erasure: bool,
}

impl PdfDocument {
    /// Read the six standard trailer /Info fields, including on read-only documents.
    pub fn metadata(&self) -> Result<DocumentMetadata> {
        let info = info_dictionary(&self.document)?;
        let mut fields = BTreeMap::new();
        if let Some(info) = info {
            for key in KEYS {
                if let Ok(value) = info.get(key.pdf_name()) {
                    let value = resolve(&self.document, value)?;
                    if matches!(value, Object::Null) {
                        continue;
                    }
                    fields.insert(key, decode(value)?);
                }
            }
        }
        let catalog = self.document.catalog()?;
        Ok(DocumentMetadata {
            fields,
            info_present: info.is_some(),
            xmp_present: catalog.has(b"Metadata"),
            catalog_info_present: catalog.has(b"Info"),
            xmp_synchronized: false,
            secure_erasure: false,
        })
    }

    /// Atomically set/remove typed standard fields. `None` removes a key; `Some("")`
    /// retains an explicitly empty string. Custom entries, dates, XMP and aliases survive.
    pub fn update_metadata(
        &mut self,
        changes: &[(MetadataKey, Option<&str>)],
    ) -> Result<DocumentMetadata> {
        self.ensure_editable()?;
        if changes.is_empty() {
            return Err(Error::Refused("select at least one metadata change"));
        }
        let mut selected = BTreeSet::new();
        for (key, value) in changes {
            if !selected.insert(*key) {
                return Err(Error::Refused("duplicate metadata change key"));
            }
            if let Some(value) = value {
                if value.len() > MAX_METADATA_VALUE_BYTES {
                    return Err(Error::Limit("metadata value exceeds 64 KiB of UTF-8"));
                }
                if value.contains('\0') {
                    return Err(Error::Refused("metadata value must not contain NUL"));
                }
            }
        }
        // Validate all supported existing values before replacing any of them.
        self.metadata()?;
        let original = info_dictionary(&self.document)?;
        if let Some(info) = original {
            validate_copy_budget(info)?;
        }
        if original.is_none() && changes.iter().all(|(_, value)| value.is_none()) {
            return self.metadata();
        }
        if self.document.objects.len().saturating_add(1) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(1) > MAX_OBJECTS
        {
            return Err(Error::Limit(
                "PDF object capacity exhausted while editing metadata",
            ));
        }
        let mut info = original.cloned().unwrap_or_default();
        for (key, value) in changes {
            match value {
                Some(value) => {
                    let mut bytes = Vec::new();
                    lopdf::encode_utf16_be(value, &mut bytes);
                    info.set(
                        key.pdf_name(),
                        Object::String(bytes, StringFormat::Hexadecimal),
                    );
                }
                None => {
                    info.remove(key.pdf_name());
                }
            }
        }
        if info.len() > MAX_INFO_ENTRIES {
            return Err(Error::Limit(
                "metadata Info dictionary exceeds 10000 entries",
            ));
        }
        validate_copy_budget(&info)?;
        let mut candidate = self.clone();
        // Copy on write: /Info may be referenced by another logical object.
        // Old dictionaries/strings remain in the graph; removal is not secure erasure.
        let id = candidate.document.add_object(info);
        candidate.document.trailer.set("Info", id);
        let candidate = Self::from_document(candidate.document, candidate.encrypted_input)?;
        let result = candidate.metadata()?;
        *self = candidate;
        Ok(result)
    }
}

fn info_dictionary(document: &Document) -> Result<Option<&Dictionary>> {
    let Ok(object) = document.trailer.get(b"Info") else {
        return Ok(None);
    };
    let object = resolve(document, object)?;
    if matches!(object, Object::Null) {
        return Ok(None);
    }
    let info = object.as_dict()?;
    if info.len() > MAX_INFO_ENTRIES {
        return Err(Error::Limit(
            "metadata Info dictionary exceeds 10000 entries",
        ));
    }
    Ok(Some(info))
}

fn validate_copy_budget(info: &Dictionary) -> Result<()> {
    // Count only the direct graph that Dictionary::clone copies; references remain shared.
    if info.len() > MAX_INFO_COPY_VALUES {
        return Err(Error::Limit("metadata copy exceeds 10000 direct values"));
    }
    let mut bytes = info.iter().map(|(key, _)| key.len()).sum::<usize>();
    if bytes > MAX_INFO_COPY_BYTES {
        return Err(Error::Limit(
            "direct Info metadata string/name/key payload exceeds 1 MiB",
        ));
    }
    let mut pending = info
        .iter()
        .map(|(_, value)| (value, 0usize))
        .collect::<Vec<_>>();
    let mut count = 0usize;
    while let Some((object, depth)) = pending.pop() {
        count += 1;
        if depth > 128 || count > MAX_INFO_COPY_VALUES {
            return Err(Error::Limit(
                "metadata direct graph exceeds structural budget",
            ));
        }
        match object {
            Object::String(value, _) | Object::Name(value) => {
                bytes = bytes.saturating_add(value.len())
            }
            Object::Array(values) => {
                if count
                    .saturating_add(pending.len())
                    .saturating_add(values.len())
                    > MAX_INFO_COPY_VALUES
                {
                    return Err(Error::Limit("metadata copy exceeds 10000 direct values"));
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)))
            }
            Object::Dictionary(dict) => {
                if count
                    .saturating_add(pending.len())
                    .saturating_add(dict.len())
                    > MAX_INFO_COPY_VALUES
                {
                    return Err(Error::Limit("metadata copy exceeds 10000 direct values"));
                }
                bytes = bytes.saturating_add(dict.iter().map(|(key, _)| key.len()).sum::<usize>());
                if bytes > MAX_INFO_COPY_BYTES {
                    return Err(Error::Limit(
                        "direct Info metadata string/name/key payload exceeds 1 MiB",
                    ));
                }
                pending.extend(dict.iter().map(|(_, value)| (value, depth + 1)));
            }
            Object::Stream(_) => {
                return Err(Error::Refused(
                    "direct streams in Info metadata are unsupported",
                ));
            }
            _ => {}
        }
        if bytes > MAX_INFO_COPY_BYTES {
            return Err(Error::Limit(
                "direct Info metadata string/name/key payload exceeds 1 MiB",
            ));
        }
    }
    Ok(())
}

fn resolve<'a>(document: &'a Document, mut object: &'a Object) -> Result<&'a Object> {
    let mut visited = BTreeSet::<ObjectId>::new();
    while let Object::Reference(id) = object {
        if !visited.insert(*id) {
            return Err(Error::Invalid("cyclic metadata reference"));
        }
        if visited.len() > 128 {
            return Err(Error::Limit("metadata reference chain exceeds 128 levels"));
        }
        object = document
            .objects
            .get(id)
            .ok_or(Error::Invalid("missing metadata reference"))?;
    }
    Ok(object)
}

fn decode(object: &Object) -> Result<String> {
    let bytes = object.as_str()?;
    if bytes.len() > MAX_METADATA_VALUE_BYTES * 2 + 2 {
        return Err(Error::Limit("encoded metadata value exceeds limit"));
    }
    let value = if let Some(bytes) = bytes.strip_prefix(b"\xFE\xFF") {
        if bytes.len() % 2 != 0 {
            return Err(Error::Invalid("odd-length UTF-16 metadata string"));
        }
        let units = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).map_err(|_| Error::Invalid("invalid UTF-16 metadata string"))?
    } else if let Some(bytes) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        std::str::from_utf8(bytes)
            .map_err(|_| Error::Invalid("invalid UTF-8 metadata string"))?
            .to_owned()
    } else if bytes.starts_with(b"\xFF\xFE") {
        return Err(Error::Refused("UTF-16LE metadata strings are unsupported"));
    } else {
        lopdf::decode_text_string(object)?
    };
    if value.len() > MAX_METADATA_VALUE_BYTES {
        return Err(Error::Limit("decoded metadata value exceeds 64 KiB"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    #[test]
    fn exhausted_object_capacity_is_transactional() -> Result<()> {
        let mut document = Document::with_version("1.7");
        let pages = document.new_object_id();
        let page=document.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()]});
        document.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
        );
        let root = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        document.trailer.set("Root", root);
        let mut document = PdfDocument::from_document(document, false)?;
        document.document.max_id = MAX_OBJECTS as u32;
        let before = document.clone();
        assert!(matches!(
            document.update_metadata(&[(MetadataKey::Title, Some("new"))]),
            Err(Error::Limit(_))
        ));
        assert_eq!(document.document.objects, before.document.objects);
        assert_eq!(document.document.trailer, before.document.trailer);
        assert_eq!(document.document.max_id, before.document.max_id);
        Ok(())
    }
}
