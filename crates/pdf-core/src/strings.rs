use crate::{Error, Result};
use lopdf::Object;

/// Callers bound the encoded input and decoded result for their own report.
pub(crate) fn decode(object: &Object) -> Result<String> {
    let bytes = object.as_str()?;
    if let Some(bytes) = bytes.strip_prefix(b"\xFE\xFF") {
        if bytes.len() % 2 != 0 {
            return Err(Error::Invalid("odd-length UTF-16 PDF text string"));
        }
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16(&units)
            .map_err(|_| Error::Invalid("invalid UTF-16 PDF text string"));
    }
    if let Some(bytes) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| Error::Invalid("invalid UTF-8 PDF text string"));
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return Err(Error::Refused("UTF-16LE PDF text strings are unsupported"));
    }
    let decoded = lopdf::decode_text_string(object)?;
    // Defined PDFDocEncoding bytes each produce one scalar; lopdf drops undefined slots.
    if decoded.chars().count() != bytes.len() {
        return Err(Error::Refused("undefined PDFDocEncoding character"));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::StringFormat;

    fn object(bytes: &[u8]) -> Object {
        Object::String(bytes.to_vec(), StringFormat::Literal)
    }

    #[test]
    fn undefined_pdfdoc_slots_cannot_silently_change_identifiers_or_values() -> Result<()> {
        for slot in [0, 0x7f, 0x9f, 0xad] {
            assert!(matches!(
                decode(&object(&[b'A', slot, b'B'])),
                Err(Error::Refused(_))
            ));
        }
        assert_eq!(decode(&object(b"caf\xe9\x18"))?, "café˘");
        Ok(())
    }

    #[test]
    fn unicode_boms_preserve_supplementary_text_without_replacement() -> Result<()> {
        let text = "café 中文 😀";
        let mut bytes = b"\xfe\xff".to_vec();
        bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(decode(&object(&bytes))?, text);
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend(text.as_bytes());
        assert_eq!(decode(&object(&bytes))?, text);
        Ok(())
    }

    #[test]
    fn malformed_boms_and_surrogates_fail_instead_of_padding_or_replacing() {
        for bytes in [
            b"\xfe\xff\0".as_slice(),
            b"\xfe\xff\xd8\0",
            b"\xef\xbb\xbf\xff",
            b"\xff\xfeA\0",
        ] {
            assert!(decode(&object(bytes)).is_err());
        }
    }
}
