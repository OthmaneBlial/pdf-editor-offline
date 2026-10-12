use crate::{Error, MAX_PAGES, Result};
use lopdf::{Document, Object, ObjectId};
use std::collections::BTreeSet;

pub(crate) fn validate_pages(document: &Document) -> Result<Vec<ObjectId>> {
    let root = document.catalog()?.get(b"Pages")?.as_reference()?;
    let mut pending = vec![(root, None, 0)];
    let mut seen = BTreeSet::new();
    let mut pages = Vec::new();
    while let Some((id, parent, depth)) = pending.pop() {
        if depth > 128 {
            return Err(Error::Limit("page tree exceeds 128 levels"));
        }
        if !seen.insert(id) {
            return Err(Error::Invalid("cyclic or shared page tree node"));
        }
        let dict = document.get_dictionary(id)?;
        if let Some(parent) = parent
            && dict.get(b"Parent")?.as_reference()? != parent
        {
            return Err(Error::Invalid("page parent disagrees with page tree"));
        }
        match dict.get(b"Type")?.as_name()? {
            b"Page" => {
                if pages.len() == MAX_PAGES {
                    return Err(Error::Limit("more than 20000 pages"));
                }
                pages.push(id);
            }
            b"Pages" => {
                let kids = dict.get(b"Kids")?.as_array()?;
                if seen.len() + pending.len() + kids.len() > MAX_PAGES * 4 {
                    return Err(Error::Limit("too many page tree nodes"));
                }
                for kid in kids.iter().rev() {
                    pending.push((kid.as_reference()?, Some(id), depth + 1));
                }
            }
            _ => return Err(Error::Invalid("unexpected page tree node type")),
        }
    }
    if pages.is_empty() {
        return Err(Error::Invalid("document contains no pages"));
    }
    let declared = document.get_dictionary(root)?.get(b"Count")?.as_i64()?;
    if declared != pages.len() as i64 {
        return Err(Error::Invalid("page tree count disagrees with leaves"));
    }
    Ok(pages)
}

pub(crate) fn inherited<'a>(
    document: &'a Document,
    mut page: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>> {
    for _ in 0..=128 {
        let dict = document.get_dictionary(page)?;
        if let Ok(value) = dict.get(key) {
            return Ok(Some(document.dereference(value)?.1));
        }
        match dict.get(b"Parent") {
            Ok(parent) => page = parent.as_reference()?,
            Err(_) => return Ok(None),
        }
    }
    Err(Error::Limit("page inheritance exceeds 128 levels"))
}

pub(crate) fn page_box(document: &Document, page: ObjectId, key: &[u8]) -> Result<[f32; 4]> {
    let array = inherited(document, page, key)?
        .ok_or(Error::Invalid("missing page box"))?
        .as_array()?;
    if array.len() != 4 {
        return Err(Error::Invalid("page box must contain four numbers"));
    }
    let mut result = [0.0; 4];
    for (index, value) in array.iter().enumerate() {
        result[index] = value.as_float()?;
    }
    if result
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
        || result[2] <= result[0]
        || result[3] <= result[1]
    {
        return Err(Error::Invalid("non-finite, inverted or oversized page box"));
    }
    Ok(result)
}
