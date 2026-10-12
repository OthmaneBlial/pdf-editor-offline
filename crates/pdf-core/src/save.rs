use crate::{Error, Result};
use lopdf::{Document, Object, ObjectId};
use std::collections::BTreeSet;
use std::io::{self, Write};

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(length) = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= self.limit)
        else {
            self.exceeded = true;
            return Err(io::Error::other("output exceeds serialization byte limit"));
        };
        if length > self.bytes.capacity() {
            let capacity = length
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn serialize_bounded(document: &Document, limit: usize) -> Result<Vec<u8>> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    };
    let result = document.clone().save_to(&mut writer);
    if writer.exceeded {
        return Err(Error::Limit("output exceeds 128 MiB"));
    }
    result?;
    Ok(writer.bytes)
}

fn carrier(object: &Object) -> bool {
    matches!(object.type_name(), Ok(b"ObjStm" | b"XRef"))
}

fn references_carrier(object: &Object, carriers: &BTreeSet<ObjectId>) -> bool {
    let mut pending = vec![object];
    while let Some(object) = pending.pop() {
        match object {
            Object::Reference(id) if carriers.contains(id) => return true,
            Object::Array(values) => pending.extend(values),
            Object::Dictionary(dictionary) => {
                pending.extend(dictionary.iter().map(|(_, value)| value))
            }
            Object::Stream(stream) => pending.extend(stream.dict.iter().map(|(_, value)| value)),
            _ => {}
        }
    }
    false
}

fn equivalent(expected: &Object, actual: &Object) -> bool {
    if let (Object::Stream(expected), Object::Stream(actual)) = (expected, actual) {
        // Stream length references and compression hints are writer details, not document content.
        expected.content == actual.content
            && expected
                .dict
                .iter()
                .filter(|(key, _)| key.as_slice() != b"Length")
                .count()
                == actual
                    .dict
                    .iter()
                    .filter(|(key, _)| key.as_slice() != b"Length")
                    .count()
            && expected
                .dict
                .iter()
                .filter(|(key, _)| key.as_slice() != b"Length")
                .all(|(key, value)| {
                    actual
                        .dict
                        .get(key)
                        .is_ok_and(|other| equivalent(value, other))
                })
    } else {
        match (expected, actual) {
            (Object::Integer(integer), Object::Real(real))
            | (Object::Real(real), Object::Integer(integer)) => {
                real.fract() == 0.0 && *real as i128 == i128::from(*integer)
            }
            (Object::String(expected, _), Object::String(actual, _)) => expected == actual,
            (Object::Array(expected), Object::Array(actual)) => {
                expected.len() == actual.len()
                    && expected
                        .iter()
                        .zip(actual)
                        .all(|(left, right)| equivalent(left, right))
            }
            (Object::Dictionary(expected), Object::Dictionary(actual)) => {
                expected.len() == actual.len()
                    && expected.iter().all(|(key, value)| {
                        actual.get(key).is_ok_and(|other| equivalent(value, other))
                    })
            }
            _ => expected == actual,
        }
    }
}

pub(crate) fn verify_objects(expected: &Document, actual: &Document) -> Result<()> {
    let carriers = expected
        .objects
        .iter()
        .filter(|(_, object)| carrier(object))
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    let logical_count = expected.objects.len() - carriers.len();
    if actual
        .objects
        .values()
        .filter(|object| !carrier(object))
        .count()
        != logical_count
    {
        return Err(Error::Verification(
            "logical object count changed during serialization",
        ));
    }
    for (id, object) in expected
        .objects
        .iter()
        .filter(|(id, _)| !carriers.contains(id))
    {
        if references_carrier(object, &carriers) {
            return Err(Error::Refused(
                "a logical object references an old xref/object-stream container",
            ));
        }
        if !actual
            .objects
            .get(id)
            .is_some_and(|other| equivalent(object, other))
        {
            return Err(Error::Verification(
                "logical object changed during serialization",
            ));
        }
    }
    let structural = [
        b"Size".as_slice(),
        b"Prev",
        b"XRefStm",
        b"Type",
        b"W",
        b"Index",
        b"Length",
        b"Filter",
        b"DecodeParms",
    ];
    for (key, value) in expected
        .trailer
        .iter()
        .filter(|(key, _)| !structural.contains(&key.as_slice()))
    {
        if !actual.trailer.get(key).is_ok_and(|other| value == other) {
            return Err(Error::Verification(
                "trailer document reference changed during serialization",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn bounded_writer_rejects_before_growth_and_retains_existing_bytes() {
        let mut writer = BoundedWriter {
            bytes: Vec::new(),
            limit: 9,
            exceeded: false,
        };
        writer.write_all(b"12345678").unwrap();
        let capacity = writer.bytes.capacity();
        assert!(writer.write_all(b"90").is_err());
        assert_eq!(writer.bytes, b"12345678");
        assert_eq!(writer.bytes.capacity(), capacity);
        assert!(capacity <= 9);
        assert!(writer.exceeded);
    }

    #[test]
    fn serialization_limit_is_a_typed_resource_error() {
        assert!(matches!(
            serialize_bounded(&Document::new(), 8),
            Err(Error::Limit(_))
        ));
    }

    #[test]
    fn verification_detects_content_changes_even_when_object_count_is_unchanged() {
        let mut expected = Document::new();
        let id = expected.add_object(dictionary! { "Important" => 1 });
        let mut changed = expected.clone();
        changed
            .objects
            .insert(id, dictionary! { "Important" => 2 }.into());
        assert!(verify_objects(&expected, &changed).is_err());
        assert!(verify_objects(&expected, &expected).is_ok());
    }

    #[test]
    fn numeric_serialization_normalization_does_not_hide_precision_loss() {
        assert!(equivalent(&Object::Real(300.0), &Object::Integer(300)));
        assert!(!equivalent(&Object::Real(300.5), &Object::Integer(300)));
        assert!(!equivalent(
            &Object::Integer((1_i64 << 60) + 1),
            &Object::Real((1_i64 << 60) as f32)
        ));
    }

    #[test]
    fn references_to_discarded_structural_containers_are_refused() {
        let mut expected = Document::new();
        let container = expected.add_object(dictionary! { "Type" => "ObjStm" });
        expected.add_object(dictionary! { "Payload" => container });
        let mut actual = expected.clone();
        actual.objects.remove(&container);
        assert!(matches!(
            verify_objects(&expected, &actual),
            Err(Error::Refused(_))
        ));
    }
}
