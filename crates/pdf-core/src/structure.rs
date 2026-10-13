use crate::{Error, MAX_PAGES, Result};
use lopdf::{Document, Object, ObjectId};
use std::collections::BTreeSet;

pub(crate) fn validate_pages(document: &Document) -> Result<Vec<ObjectId>> {
    enum Step {
        Enter(ObjectId, Option<ObjectId>, usize),
        Exit(ObjectId, usize),
    }
    let root = document.catalog()?.get(b"Pages")?.as_reference()?;
    let mut pending = vec![Step::Enter(root, None, 0)];
    let mut seen = BTreeSet::new();
    let mut pages = Vec::new();
    while let Some(step) = pending.pop() {
        let (id, parent, depth) = match step {
            Step::Enter(id, parent, depth) => (id, parent, depth),
            Step::Exit(id, start) => {
                let declared = document.get_dictionary(id)?.get(b"Count")?.as_i64()?;
                if declared != (pages.len() - start) as i64 {
                    return Err(Error::Invalid(
                        "page tree count disagrees with descendant leaves",
                    ));
                }
                continue;
            }
        };
        if depth > 128 {
            return Err(Error::Limit("page tree exceeds 128 levels"));
        }
        if !seen.insert(id) {
            return Err(Error::Invalid("cyclic or shared page tree node"));
        }
        let dict = document.get_dictionary(id)?;
        if parent.is_none() && dict.has(b"Parent") {
            return Err(Error::Invalid("root page tree node must not have a parent"));
        }
        if let Some(parent) = parent
            && dict.get(b"Parent")?.as_reference()? != parent
        {
            return Err(Error::Invalid("page parent disagrees with page tree"));
        }
        match dict.get(b"Type")?.as_name()? {
            b"Page" => {
                if parent.is_none() {
                    return Err(Error::Invalid("catalog Pages must reference a Pages node"));
                }
                if pages.len() == MAX_PAGES {
                    return Err(Error::Limit("more than 20000 pages"));
                }
                pages.push(id);
            }
            b"Pages" => {
                if dict.get(b"Count")?.as_i64()? < 0 {
                    return Err(Error::Invalid("page tree count is negative"));
                }
                let kids = dict.get(b"Kids")?.as_array()?;
                if seen.len() + pending.len() + kids.len() > MAX_PAGES * 4 {
                    return Err(Error::Limit("too many page tree nodes"));
                }
                pending.push(Step::Exit(id, pages.len()));
                for kid in kids.iter().rev() {
                    pending.push(Step::Enter(kid.as_reference()?, Some(id), depth + 1));
                }
            }
            _ => return Err(Error::Invalid("unexpected page tree node type")),
        }
    }
    if pages.is_empty() {
        return Err(Error::Invalid("document contains no pages"));
    }
    Ok(pages)
}

/// Inspect direct nested values without following reference cycles or decoding stream contents.
pub(crate) fn signature_fields(document: &Document) -> Result<usize> {
    const MAX_VALUES: usize = 2_000_000;
    let mut pending = document
        .objects
        .values()
        .map(|object| (object, 0))
        .collect::<Vec<_>>();
    if pending.len().saturating_add(document.trailer.len()) + 1 > MAX_VALUES {
        return Err(Error::Limit("more than 2000000 structural object values"));
    }
    pending.extend(document.trailer.iter().map(|(_, value)| (value, 1)));
    let mut visited = 1;
    let mut count = usize::from(is_signature(document, &document.trailer)?);
    while let Some((object, depth)) = pending.pop() {
        if depth > 128 {
            return Err(Error::Limit("structural object nesting exceeds 128 levels"));
        }
        visited += 1;
        let children = match object {
            Object::Array(values) => values.len(),
            Object::Dictionary(dict) => dict.len(),
            Object::Stream(stream) => stream.dict.len(),
            _ => 0,
        };
        if visited + pending.len() + children > MAX_VALUES {
            return Err(Error::Limit("more than 2000000 structural object values"));
        }
        match object {
            Object::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Object::Dictionary(dict) => {
                count += usize::from(is_signature(document, dict)?);
                pending.extend(dict.iter().map(|(_, value)| (value, depth + 1)));
            }
            Object::Stream(stream) => {
                count += usize::from(is_signature(document, &stream.dict)?);
                pending.extend(stream.dict.iter().map(|(_, value)| (value, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(count)
}

fn is_signature(document: &Document, dictionary: &lopdf::Dictionary) -> Result<bool> {
    for key in [b"FT".as_slice(), b"Type"] {
        if let Ok(value) = dictionary.get(key)
            && document
                .dereference(value)?
                .1
                .as_name()
                .is_ok_and(|name| name == b"Sig")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn inherited<'a>(
    document: &'a Document,
    page: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>> {
    Ok(match inherited_raw(document, page, key)? {
        Some(value) => Some(document.dereference(value)?.1),
        None => None,
    })
}

/// Preserve indirect sharing when materializing inherited attributes during tree edits.
pub(crate) fn inherited_raw<'a>(
    document: &'a Document,
    page: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>> {
    inherited_raw_with_work(document, page, key, || Ok(()))
}

pub(crate) fn inherited_raw_with_work<'a>(
    document: &'a Document,
    mut page: ObjectId,
    key: &[u8],
    mut visit: impl FnMut() -> Result<()>,
) -> Result<Option<&'a Object>> {
    for _ in 0..=128 {
        visit()?;
        let dict = document.get_dictionary(page)?;
        if let Ok(value) = dict.get(key) {
            return Ok(Some(value));
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
