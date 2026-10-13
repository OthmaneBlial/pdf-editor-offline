//! Budgeted text decoding with the existing lopdf mapping semantics.

use super::{Error, Result};
use lopdf::Encoding;

fn append_scalar(output: &mut String, value: char, limit: usize) -> Result<()> {
    if value.len_utf8() > limit.saturating_sub(output.len()) {
        return Err(Error::Limit("extracted text exceeds 128 MiB"));
    }
    output.push(value);
    Ok(())
}

// The same decoder as lopdf, with fixed storage and output checked before
// appending. Keep BOM sniffing and malformed-input replacement semantics.
struct Utf16Output {
    decoder: encoding_rs::Decoder,
}
impl Default for Utf16Output {
    fn default() -> Self {
        Self {
            decoder: encoding_rs::UTF_16BE.new_decoder(),
        }
    }
}
impl Utf16Output {
    fn feed(&mut self, bytes: &[u8], output: &mut String, limit: usize, last: bool) -> Result<()> {
        let mut read = 0;
        loop {
            let mut buffer = [0u8; 64];
            let (result, consumed, written, _) =
                self.decoder
                    .decode_to_utf8(&bytes[read..], &mut buffer, last);
            read += consumed;
            if written > limit.saturating_sub(output.len()) {
                return Err(Error::Limit("extracted text exceeds 128 MiB"));
            }
            output.push_str(
                std::str::from_utf8(&buffer[..written])
                    .map_err(|_| Error::Invalid("decoder returned invalid UTF-8"))?,
            );
            if result == encoding_rs::CoderResult::InputEmpty {
                return Ok(());
            }
            if consumed == 0 && written == 0 {
                return Err(Error::Invalid("text decoder made no progress"));
            }
        }
    }
    fn push(&mut self, unit: u16, output: &mut String, limit: usize) -> Result<()> {
        self.feed(&unit.to_be_bytes(), output, limit, false)
    }
    fn finish(&mut self, output: &mut String, limit: usize) -> Result<()> {
        self.feed(&[], output, limit, true)
    }
}

pub(super) fn append_encoded(
    encoding: &Encoding<'_>,
    bytes: &[u8],
    output: &mut String,
    limit: usize,
) -> Result<()> {
    match encoding {
        Encoding::OneByteEncoding(map) => {
            for byte in bytes {
                if let Some(glyph) = map.get(usize::from(*byte)).copied().flatten() {
                    for scalar in char::decode_utf16([glyph.utf16_code_unit()]).flatten() {
                        append_scalar(output, scalar, limit)?;
                    }
                }
            }
            Ok(())
        }
        Encoding::Differences(differences) => {
            for byte in bytes {
                if let Some(glyph) = differences.map.get(byte) {
                    for scalar in char::decode_utf16([glyph.utf16_code_unit()]).flatten() {
                        append_scalar(output, scalar, limit)?;
                    }
                } else {
                    append_encoded(&differences.base, &[*byte], output, limit)?;
                }
            }
            Ok(())
        }
        Encoding::UnicodeMapEncoding(map) => {
            let mut decoder = Utf16Output::default();
            let mut code = 0u32;
            let mut length = 0u8;
            let mut emit = |units: Vec<u16>| -> Result<()> {
                // The PDF CMap parser bounds each destination to 256 units.
                // This single mapping is bounded; never collect all expansions.
                if units.is_empty() || units.len() > 256 {
                    return Err(Error::Invalid("unbounded or empty Unicode mapping"));
                }
                for unit in units {
                    decoder.push(unit, output, limit)?;
                }
                Ok(())
            };
            for byte in bytes {
                if length == 4 {
                    emit(
                        map.get_checked(code, length)
                            .map_err(|_| Error::Invalid("undefined Unicode range mapping"))?
                            .unwrap_or_else(|| vec![0xfffd]),
                    )?;
                    code = 0;
                    length = 0;
                }
                code = code * 256 + u32::from(*byte);
                length += 1;
                if let Some(units) = map
                    .get_checked(code, length)
                    .map_err(|_| Error::Invalid("undefined Unicode range mapping"))?
                {
                    emit(units)?;
                    code = 0;
                    length = 0;
                }
            }
            if length > 0 {
                emit(
                    map.get_checked(code, length)
                        .map_err(|_| Error::Invalid("undefined Unicode range mapping"))?
                        .unwrap_or_else(|| vec![0xfffd]),
                )?;
            }
            decoder.finish(output, limit)
        }
        Encoding::SimpleEncoding(b"UniGB-UCS2-H" | b"UniGB-UTF16-H") => {
            let mut decoder = Utf16Output::default();
            decoder.feed(bytes, output, limit, true)
        }
        _ => {
            // Remaining supported simple mapping produces one scalar per byte.
            // Calling the existing mapping for each byte retains its undefined
            // slot/error behavior without a whole-input temporary allocation.
            encoding.bytes_to_string(&[])?;
            for byte in bytes {
                let value = encoding.bytes_to_string(&[*byte])?;
                for scalar in value.chars() {
                    append_scalar(output, scalar, limit)?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod bounded_encoding_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::MAX_STREAM_BYTES;
    use lopdf::{Dictionary, Document, Object, Stream};
    fn unicode(entries: &[(String, String)]) -> Encoding<'static> {
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /SyntheticBoundedUnicode def\n/CMapType 2 def\n4 begincodespacerange\n<00> <FF>\n<0000> <FFFF>\n<000000> <FFFFFF>\n<00000000> <FFFFFFFF>\nendcodespacerange\n",
        );
        cmap.push_str(&format!("{} beginbfchar\n", entries.len()));
        for (source, target) in entries {
            cmap.push_str(&format!("<{source}> <{target}>\n"));
        }
        cmap.push_str(
            "endbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n",
        );
        let mut doc = Document::with_version("1.4");
        let id = doc.add_object(Stream::new(Dictionary::new(), cmap.into_bytes()));
        let mut font = Dictionary::new();
        font.set("Type", "Font");
        font.set("ToUnicode", id);
        let Encoding::UnicodeMapEncoding(map) = font
            .get_font_encoding_with_limit(&doc, MAX_STREAM_BYTES)
            .unwrap()
        else {
            panic!("fixture needs Unicode map");
        };
        Encoding::UnicodeMapEncoding(map)
    }
    fn equivalent(encoding: &Encoding<'_>, input: &[u8]) {
        let expected = encoding.bytes_to_string(input).unwrap();
        let mut actual = String::new();
        append_encoded(encoding, input, &mut actual, 16 * 1024).unwrap();
        assert_eq!(actual, expected, "{input:?}");
    }
    #[test]
    fn expansion_limit_is_checked_incrementally_with_a_small_valid_map() {
        let map = unicode(&[("0001".into(), "0041".repeat(256))]);
        let input = [0, 1].repeat(32);
        let mut success = String::new();
        append_encoded(&map, &input, &mut success, 8192).unwrap();
        assert_eq!(success, "A".repeat(8192));
        let mut refused = String::from("prefix");
        assert!(matches!(
            append_encoded(&map, &input, &mut refused, 1024),
            Err(Error::Limit(_))
        ));
        assert_eq!(refused.len(), 1024);
        assert!(refused.starts_with("prefix"));
    }
    #[test]
    fn unicode_variable_codes_bom_surrogates_and_missing_codes_match_existing_semantics() {
        let map = unicode(&[
            ("01".into(), "0041".into()),
            ("0203".into(), "D83D".into()),
            ("040506".into(), "DE00".into()),
            ("0708090A".into(), "0065D800".into()),
            ("0B".into(), "0042".into()),
        ]);
        for input in [
            &[1][..],
            &[2, 3, 4, 5, 6],
            &[7, 8, 9, 10, 11],
            &[0xff, 0xee, 0xdd, 0xcc, 0xbb],
            &[2],
            &[],
        ] {
            equivalent(&map, input);
        }
        for units in ["FEFF0041", "FFFE4100", "D8000041DC00", "D83DDE00"] {
            equivalent(&unicode(&[("01".into(), units.into())]), &[1]);
        }
        for name in [b"UniGB-UCS2-H".as_slice(), b"UniGB-UTF16-H".as_slice()] {
            let encoding = Encoding::SimpleEncoding(name);
            for input in [
                &[0xfe, 0xff, 0, 65][..],
                &[0xff, 0xfe, 65, 0],
                &[0xd8, 0x3d, 0xde, 0],
                &[0xd8, 0, 0, 65, 0xdc, 0],
                &[0xd8, 0, 1],
                &[0],
                &[],
            ] {
                equivalent(&encoding, input);
            }
        }
    }
    #[test]
    fn single_byte_and_differences_keep_undefined_slots_and_errors() {
        let bytes = (0..=255).collect::<Vec<u8>>();
        equivalent(&Encoding::SimpleEncoding(b"WinAnsiEncoding"), &bytes);
        let font = lopdf::dictionary! {"Type"=>"Font", "Encoding"=>lopdf::dictionary! {"BaseEncoding"=>"WinAnsiEncoding", "Differences"=>vec![65.into(),Object::Name(b"fi".to_vec()),Object::Name(b"unknown".to_vec())]}};
        equivalent(
            &font
                .get_font_encoding_with_limit(&Document::new(), MAX_STREAM_BYTES)
                .unwrap(),
            &bytes,
        );
        for input in [&[][..], &[1]] {
            assert!(
                append_encoded(
                    &Encoding::SimpleEncoding(b"Unsupported"),
                    input,
                    &mut String::new(),
                    1024
                )
                .is_err()
            );
        }
    }
    #[test]
    fn utf16_streaming_matches_upstream_at_bom_and_malformed_boundaries() {
        let encoding = Encoding::SimpleEncoding(b"UniGB-UTF16-H");
        let units: [u16; 10] = [
            0, 65, 0xfeff, 0xfffe, 0xd800, 0xdbff, 0xdc00, 0xdfff, 0xd83d, 0xde00,
        ];
        for a in units {
            for b in units {
                for c in units {
                    let bytes = [a, b, c]
                        .into_iter()
                        .flat_map(u16::to_be_bytes)
                        .collect::<Vec<_>>();
                    for count in [3, 4, 5, 6] {
                        equivalent(&encoding, &bytes[..count]);
                    }
                }
            }
        }
    }
    #[test]
    fn existing_utf8_bom_sniffing_is_preserved_even_for_declared_utf16() {
        equivalent(
            &Encoding::SimpleEncoding(b"UniGB-UTF16-H"),
            &[0xef, 0xbb, 0xbf, 65],
        );
        equivalent(&unicode(&[("01".into(), "EFBBBF41".into())]), &[1]);
    }
    #[test]
    fn exact_utf8_output_boundaries_accept_complete_scalars_without_spare_capacity() {
        for text in ["A", "é", "中", "😀"] {
            let bytes = text
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            let encoding = Encoding::SimpleEncoding(b"UniGB-UTF16-H");
            let mut exact = String::new();
            append_encoded(&encoding, &bytes, &mut exact, text.len()).unwrap();
            assert_eq!(exact, text);
            let mut refused = String::new();
            assert!(append_encoded(&encoding, &bytes, &mut refused, text.len() - 1).is_err());
            assert!(refused.len() < text.len());
        }
    }
    #[test]
    fn checked_lookup_distinguishes_missing_codes_from_empty_programmatic_destinations() {
        let Encoding::UnicodeMapEncoding(mut map) = unicode(&[("01".into(), "0041".into())]) else {
            unreachable!()
        };
        assert_eq!(map.get_checked(1, 1).unwrap(), Some(vec![65]));
        assert_eq!(map.get_checked(2, 1).unwrap(), None);
        map.put_char(1, 1, vec![]);
        assert!(map.get_checked(1, 1).is_err());
        assert_eq!(map.get(1, 1), None);
        assert!(map.get_checked(1, 0).is_err());
        assert!(map.get_checked(1, 5).is_err());
    }
    #[test]
    fn original_origins_keep_duplicate_sequences_and_reverse_lookup_consistent() {
        let Encoding::UnicodeMapEncoding(map) = unicode(&[
            ("00".into(), "00660069".into()),
            ("01".into(), "00660069".into()),
        ]) else {
            unreachable!()
        };
        assert_eq!(map.get_checked(0, 1).unwrap(), Some(vec![102, 105]));
        assert_eq!(map.get_checked(1, 1).unwrap(), Some(vec![102, 105]));
        let sources = map
            .get_source_codes_for_unicode(&[102, 105])
            .unwrap()
            .iter()
            .map(|entry| entry.source_code)
            .collect::<Vec<_>>();
        assert_eq!(sources, vec![0, 1]);
    }
}
