use crate::{Error, MAX_OBJECTS, PdfDocument, Result, structure};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use serde::Serialize;
use std::collections::BTreeSet;

pub const MAX_NOTE_CONTENT_BYTES: usize = 64 * 1024;
pub const MAX_NOTE_AUTHOR_BYTES: usize = 1024;
const MAX_ANNOTATIONS_PER_PAGE: usize = 10_000;
const MAX_LIST_ANNOTATIONS: usize = 100_000;
const MAX_LIST_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// A PDF Text annotation. Rectangle coordinates are unrotated PDF user-space points.
/// Direct annotations have no object ID. Contents are comment metadata, not page text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub page: u32,
    /// One-based physical occurrence in this page's /Annots array; shifts after removal.
    pub annotation_index: u32,
    pub object_id: Option<ObjectId>,
    pub rectangle: [f32; 4],
    pub contents: String,
    pub author: String,
    pub flags: i64,
}

impl PdfDocument {
    /// Add a sticky Text note with an explicit vector appearance, preserving old annotations.
    pub fn add_note(
        &mut self,
        page: u32,
        rectangle: [f32; 4],
        contents: &str,
        author: &str,
    ) -> Result<Note> {
        self.ensure_editable()?;
        self.validate_selection(&[page])?;
        validate_rectangle(rectangle)?;
        if contents.len() > MAX_NOTE_CONTENT_BYTES || author.len() > MAX_NOTE_AUTHOR_BYTES {
            return Err(Error::Limit(
                "note contents exceed 64 KiB or author exceeds 1 KiB",
            ));
        }
        let page_id = self.pages[page as usize - 1];
        let media = structure::page_box(&self.document, page_id, b"MediaBox")?;
        let crop = if structure::inherited_raw(&self.document, page_id, b"CropBox")?.is_some() {
            structure::page_box(&self.document, page_id, b"CropBox")?
        } else {
            media
        };
        if rectangle[0] < media[0].max(crop[0])
            || rectangle[1] < media[1].max(crop[1])
            || rectangle[2] > media[2].min(crop[2])
            || rectangle[3] > media[3].min(crop[3])
        {
            return Err(Error::Pages(
                "note rectangle lies outside the visible page box",
            ));
        }
        let entries = annotation_entries(&self.document, page_id)?;
        if entries.len() >= MAX_ANNOTATIONS_PER_PAGE {
            return Err(Error::Limit("more than 10000 annotations on a page"));
        }
        // Check references before cloning; do not interpret or rewrite existing annotations.
        for entry in entries {
            annotation(&self.document, entry)?;
        }
        if self.document.objects.len().saturating_add(2) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(2) > MAX_OBJECTS
        {
            return Err(Error::Limit(
                "PDF object capacity exhausted while adding note",
            ));
        }
        let mut entries = entries.to_vec();
        let mut candidate = self.clone();
        let appearance = candidate.document.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "FormType" => 1,
                "BBox" => vec![0.into(), 0.into(), 24.into(), 24.into()],
                "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()],
                "Resources" => dictionary! {},
            },
            // Fixed, self-contained vector icon; comment Unicode needs no embedded font.
            b"q 1 0.88 0.25 rg 0.2 0.18 0.1 RG 1 w 2 2 m 22 2 l 22 16 l 16 22 l 2 22 l h B 1 0.96 0.66 rg 16 22 m 16 16 l 22 16 l h B 0.35 0.3 0.12 RG 5 12 m 18 12 l S 5 8 m 15 8 l S Q".to_vec(),
        ));
        let id = candidate.document.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Text", "P" => page_id,
            "Rect" => Object::Array(rectangle.into_iter().map(Object::Real).collect()),
            "Contents" => unicode_string(contents), "T" => unicode_string(author),
            "F" => 4, "Name" => "Note", "Open" => false,
            "C" => vec![1.into(), Object::Real(0.88), Object::Real(0.25)],
            "AP" => dictionary! {"N" => appearance},
        });
        entries.push(id.into());
        let annotation_index = entries.len() as u32;
        // Replacing this page's value leaves any shared indirect array unchanged.
        candidate
            .document
            .get_dictionary_mut(page_id)?
            .set("Annots", entries);
        let note = Note {
            page,
            annotation_index,
            object_id: Some(id),
            rectangle,
            contents: contents.to_owned(),
            author: author.to_owned(),
            flags: 4,
        };
        *self = candidate;
        Ok(note)
    }

    /// List every Text annotation on selected pages, including hidden notes; flags are retained.
    /// Read-only: signed inputs and successfully decrypted inputs may be inspected.
    pub fn list_notes(&self, pages: &[u32]) -> Result<Vec<Note>> {
        self.validate_selection(pages)?;
        let mut notes = Vec::new();
        let mut seen_pages = BTreeSet::new();
        let mut visited = 0usize;
        let mut text_bytes = 0usize;
        for page in pages
            .iter()
            .copied()
            .filter(|page| seen_pages.insert(*page))
        {
            for (index, entry) in annotation_entries(&self.document, self.pages[page as usize - 1])?
                .iter()
                .enumerate()
            {
                visited += 1;
                if visited > MAX_LIST_ANNOTATIONS {
                    return Err(Error::Limit("more than 100000 selected annotations"));
                }
                let (id, dictionary) = annotation(&self.document, entry)?;
                if resolve(&self.document, dictionary.get(b"Subtype")?)?
                    .1
                    .as_name()?
                    != b"Text"
                {
                    continue;
                }
                let rect = resolve(&self.document, dictionary.get(b"Rect")?)?
                    .1
                    .as_array()?;
                if rect.len() != 4 {
                    return Err(Error::Invalid("note Rect needs four coordinates"));
                }
                let mut rectangle = [0.0; 4];
                for (target, value) in rectangle.iter_mut().zip(rect) {
                    *target = resolve(&self.document, value)?.1.as_float()?;
                }
                validate_rectangle(rectangle)?;
                let contents = read_string(
                    &self.document,
                    dictionary,
                    b"Contents",
                    MAX_NOTE_CONTENT_BYTES,
                )?;
                let author = read_string(&self.document, dictionary, b"T", MAX_NOTE_AUTHOR_BYTES)?;
                text_bytes = text_bytes
                    .saturating_add(contents.len())
                    .saturating_add(author.len());
                if text_bytes > MAX_LIST_TEXT_BYTES {
                    return Err(Error::Limit("listed note text exceeds 16 MiB"));
                }
                let flags = if dictionary.has(b"F") {
                    resolve(&self.document, dictionary.get(b"F")?)?.1.as_i64()?
                } else {
                    0
                };
                if flags < 0 || flags > u32::MAX as i64 {
                    return Err(Error::Invalid("invalid annotation flags"));
                }
                notes.push(Note {
                    page,
                    annotation_index: index as u32 + 1,
                    object_id: id,
                    rectangle,
                    contents,
                    author,
                    flags,
                });
            }
        }
        Ok(notes)
    }
}

fn validate_rectangle(rectangle: [f32; 4]) -> Result<()> {
    if rectangle.iter().any(|value| !value.is_finite())
        || rectangle[2] <= rectangle[0]
        || rectangle[3] <= rectangle[1]
        || !(rectangle[2] - rectangle[0]).is_finite()
        || !(rectangle[3] - rectangle[1]).is_finite()
    {
        return Err(Error::Pages("note rectangle must be finite and nonempty"));
    }
    Ok(())
}

pub(crate) fn unicode_string(text: &str) -> Object {
    let mut encoded = Vec::new();
    lopdf::encode_utf16_be(text, &mut encoded);
    Object::String(encoded, StringFormat::Hexadecimal)
}

pub(crate) fn resolve<'a>(
    document: &'a Document,
    mut object: &'a Object,
) -> Result<(Option<ObjectId>, &'a Object)> {
    let mut seen = BTreeSet::new();
    let mut id = None;
    while let Object::Reference(reference) = object {
        if !seen.insert(*reference) {
            return Err(Error::Invalid("cyclic annotation reference"));
        }
        if seen.len() > 128 {
            return Err(Error::Limit(
                "annotation reference chain exceeds 128 levels",
            ));
        }
        id = Some(*reference);
        // Access the map directly: lopdf get_object automatically follows references.
        object = document
            .objects
            .get(reference)
            .ok_or(Error::Invalid("missing annotation reference"))?;
    }
    Ok((id, object))
}

pub(crate) fn annotation_entries(document: &Document, page: ObjectId) -> Result<&[Object]> {
    let dictionary = document.get_dictionary(page)?;
    if !dictionary.has(b"Annots") {
        return Ok(&[]);
    }
    let entries = resolve(document, dictionary.get(b"Annots")?)?
        .1
        .as_array()?;
    if entries.len() > MAX_ANNOTATIONS_PER_PAGE {
        return Err(Error::Limit("more than 10000 annotations on a page"));
    }
    Ok(entries)
}

pub(crate) fn annotation<'a>(
    document: &'a Document,
    entry: &'a Object,
) -> Result<(Option<ObjectId>, &'a Dictionary)> {
    let (id, value) = resolve(document, entry)?;
    let dictionary = value.as_dict()?;
    resolve(document, dictionary.get(b"Subtype")?)?
        .1
        .as_name()?;
    Ok((id, dictionary))
}

fn read_string(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limit: usize,
) -> Result<String> {
    if !dictionary.has(key) {
        return Ok(String::new());
    }
    let value = resolve(document, dictionary.get(key)?)?.1;
    let bytes = value.as_str()?;
    if bytes.len() > limit.saturating_mul(2).saturating_add(2) {
        return Err(Error::Limit("encoded note string exceeds limit"));
    }
    // lopdf supports PDFDocEncoding/UTF-16, but pads odd UTF-16 tails and retains UTF-8 BOM.
    if bytes.starts_with(b"\xFE\xFF") && bytes.len() % 2 != 0 {
        return Err(Error::Invalid("odd-length UTF-16 note string"));
    }
    let text = if let Some(bytes) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        std::str::from_utf8(bytes)
            .map_err(|_| Error::Invalid("invalid UTF-8 note string"))?
            .to_owned()
    } else {
        lopdf::decode_text_string(value)?
    };
    if text.len() > limit {
        return Err(Error::Limit("decoded note string exceeds limit"));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Result<PdfDocument> {
        let mut document = Document::with_version("1.7");
        let pages = document.new_object_id();
        let page = document.add_object(dictionary! {"Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()]});
        document.objects.insert(
            pages,
            dictionary! {"Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()]}.into(),
        );
        let catalog = document.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages});
        document.trailer.set("Root", catalog);
        PdfDocument::from_document(document, false)
    }

    #[test]
    fn object_id_capacity_and_page_annotation_limits_are_transactional() -> Result<()> {
        let mut document = document()?;
        document.document.max_id = MAX_OBJECTS as u32 - 1;
        let before = document.document.objects.clone();
        assert!(matches!(
            document.add_note(1, [10.0, 10.0, 34.0, 34.0], "x", ""),
            Err(Error::Limit(_))
        ));
        assert_eq!(document.document.objects, before);
        let mut document = self::document()?;
        document.document.max_id = MAX_OBJECTS as u32 - 2;
        let note = document.add_note(1, [10.0, 10.0, 34.0, 34.0], "x", "")?;
        assert_eq!(note.object_id, Some((MAX_OBJECTS as u32, 0)));
        assert_eq!(document.document.max_id, MAX_OBJECTS as u32);
        let mut document = self::document()?;
        let page = document.pages[0];
        document.document.get_dictionary_mut(page)?.set(
            "Annots",
            vec![Object::Dictionary(dictionary! {"Subtype"=>"Link"}); MAX_ANNOTATIONS_PER_PAGE],
        );
        let before = document.document.objects.clone();
        assert!(matches!(
            document.add_note(1, [10.0, 10.0, 34.0, 34.0], "x", ""),
            Err(Error::Limit(_))
        ));
        assert_eq!(document.document.objects, before);
        Ok(())
    }

    #[test]
    fn aggregate_note_contents_are_bounded_even_when_shared() -> Result<()> {
        let mut document = document()?;
        let contents = document
            .document
            .add_object(unicode_string(&"x".repeat(MAX_NOTE_CONTENT_BYTES)));
        let note = document.document.add_object(dictionary! {"Subtype"=>"Text", "Rect"=>vec![10.into(),10.into(),34.into(),34.into()], "Contents"=>contents});
        document
            .document
            .get_dictionary_mut(document.pages[0])?
            .set(
                "Annots",
                vec![Object::Reference(note); MAX_LIST_TEXT_BYTES / MAX_NOTE_CONTENT_BYTES + 1],
            );
        assert!(matches!(document.list_notes(&[1]), Err(Error::Limit(_))));
        Ok(())
    }

    #[test]
    fn reference_depth_and_total_annotation_walk_are_bounded() -> Result<()> {
        let mut document = document()?;
        let mut entry = document
            .document
            .add_object(dictionary! {"Subtype"=>"Link"});
        for _ in 0..128 {
            entry = document.document.add_object(Object::Reference(entry));
        }
        document
            .document
            .get_dictionary_mut(document.pages[0])?
            .set("Annots", vec![Object::Reference(entry)]);
        assert!(matches!(document.list_notes(&[1]), Err(Error::Limit(_))));

        let mut document = self::document()?;
        let parent = document.document.catalog()?.get(b"Pages")?.as_reference()?;
        let entry = document
            .document
            .add_object(dictionary! {"Subtype"=>"Link"});
        let entries = document.document.add_object(Object::Array(vec![
            Object::Reference(entry);
            MAX_ANNOTATIONS_PER_PAGE
        ]));
        let mut pages = Vec::new();
        for _ in 0..11 {
            pages.push(document.document.add_object(dictionary! {"Type"=>"Page", "Parent"=>parent, "MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()], "Annots"=>entries}));
        }
        let root = document.document.get_dictionary_mut(parent)?;
        root.set(
            "Kids",
            pages.into_iter().map(Object::Reference).collect::<Vec<_>>(),
        );
        root.set("Count", 11);
        let document = PdfDocument::from_document(document.document, false)?;
        assert!(matches!(
            document.list_notes(&document.all_pages()),
            Err(Error::Limit(_))
        ));
        Ok(())
    }
}
