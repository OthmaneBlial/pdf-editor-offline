use crate::cmap_section::{CMapParseError, CMapSection, CodeLen, SourceCode};
use crate::parser::cmap_parser::parse;

use log::error;
use rangemap::RangeInclusiveMap;
use std::collections::HashMap;
use thiserror::Error;

/// Unicode Cmap is implemented by 4 maps.
/// Each map contains a mappings from source codes to unicode values for a different length of codes.
/// Codes vary from 1 byte to 4 bytes so they are always in limits of u32.
/// However to map a code to a unicode value an additional knowledge about the number of bytes is needed,
/// as 2 byte code <0000> shouldn't be matched with a single byte <00> even though they have the same integer value.
#[derive(Debug, Default)]
pub struct ToUnicodeCMap {
    pub bf_ranges: [RangeInclusiveMap<SourceCode, BfRangeTarget>; 4],
    // RangeInclusiveMap may merge/split keys; destinations remain relative to
    // the originally declared range, not the resulting storage key.
    origins: Box<[RangeInclusiveMap<SourceCode, (SourceCode, SourceCode)>; 4]>,
    reverse_map: Option<HashMap<Vec<u16>, Vec<ReverseCMapEntry>>>,
}
/// Represents the information needed to map a Unicode sequence back to a source code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReverseCMapEntry {
    pub source_code: SourceCode,
    pub code_len: CodeLen,
    // Optionally, add priority if multiple source codes map to the same Unicode sequence
    // pub priority: u8,
}

#[derive(Debug, Error)]
pub enum UnicodeCMapError {
    #[error("could not parse ToUnicode CMap: {0:#?}")]
    Parse(CMapParseError),
    #[error("invalid code range")]
    InvalidCodeRange,
}

impl From<CMapParseError> for UnicodeCMapError {
    fn from(err: CMapParseError) -> Self {
        UnicodeCMapError::Parse(err)
    }
}

impl ToUnicodeCMap {
    const REPLACEMENT_CHAR: u16 = 0xfffd;

    pub fn new() -> ToUnicodeCMap {
        ToUnicodeCMap {
            bf_ranges: [(); 4].map(|_| RangeInclusiveMap::new()),
            origins: Box::new([(); 4].map(|_| RangeInclusiveMap::new())),
            reverse_map: None,
        }
    }

    pub(crate) fn parse(stream_content: Vec<u8>) -> Result<ToUnicodeCMap, UnicodeCMapError> {
        let cmap_sections = parse(&stream_content[..])?;
        Self::from_sections(cmap_sections)
    }

    fn from_sections(cmap_sections: Vec<CMapSection>) -> Result<ToUnicodeCMap, UnicodeCMapError> {
        let mut cmap = Self::new();
        for section in cmap_sections {
            match section {
                CMapSection::CsRange(_) => (), // currently no additional validation is implemented for code ranges
                CMapSection::BfChar(char_mappings) => {
                    for ((code, code_len), dst) in char_mappings {
                        if dst.is_empty() || dst.len() > 256 {
                            return Err(UnicodeCMapError::InvalidCodeRange);
                        }
                        cmap.put_char(code, code_len, dst);
                    }
                }
                CMapSection::BfRange(range_mappings) => {
                    for ((start, end, code_len), dst_vec) in range_mappings {
                        if end < start {
                            return Err(UnicodeCMapError::InvalidCodeRange);
                        }
                        let target = match dst_vec.len() {
                            1 if dst_vec[0].len() == 1 => BfRangeTarget::UTF16CodePoint {
                                offset: u32::wrapping_sub(dst_vec[0][0] as u32, start),
                            },
                            1 => BfRangeTarget::HexString(dst_vec[0].clone()),
                            0 => return Err(UnicodeCMapError::InvalidCodeRange),
                            _ => BfRangeTarget::ArrayOfHexStrings(dst_vec),
                        };
                        validate_target(start, end, &target)?;
                        cmap.put(start, end, code_len, target);
                    }
                }
            }
        }

        let mut rev_map = HashMap::new();

        for code_len_idx in 0..cmap.bf_ranges.len() {
            let code_len = (code_len_idx + 1) as u8;
            for (range, _) in cmap.bf_ranges[code_len_idx].iter() {
                for src_code in range.clone() {
                    let unicode_sequence = cmap.get_checked(src_code, code_len)?;

                    if let Some(uni_seq) = unicode_sequence
                        && !uni_seq.is_empty()
                    {
                        rev_map.entry(uni_seq).or_insert_with(Vec::new).push(ReverseCMapEntry {
                            source_code: src_code,
                            code_len,
                        });
                    }
                }
            }
        }
        cmap.reverse_map = Some(rev_map);

        Ok(cmap)
    }

    pub fn get(&self, code: SourceCode, code_len: CodeLen) -> Option<Vec<u16>> {
        if code_len > 4 || code_len == 0 {
            error!("Code lenght should be between l and 4 bytes, got {code_len}");
            return None;
        }
        let bf_ranges_map = &self.bf_ranges[(code_len - 1) as usize];
        bf_ranges_map.get_key_value(&code).and_then(|(range, target)| {
            let start = self.origins[(code_len - 1) as usize]
                .get(&code)
                .map_or(*range.start(), |origin| origin.0);
            lookup_target(code, start, target)
        })
    }

    /// Checked lookup for callers that must distinguish undefined CMap ranges
    /// from an unmapped code. ISO 32000-1 9.10.3 increments only the last byte.
    pub fn get_checked(&self, code: SourceCode, code_len: CodeLen) -> Result<Option<Vec<u16>>, UnicodeCMapError> {
        if !(1..=4).contains(&code_len) {
            return Err(UnicodeCMapError::InvalidCodeRange);
        }
        let Some((range, target)) = self.bf_ranges[(code_len - 1) as usize].get_key_value(&code) else {
            return Ok(None);
        };
        let (start, end) = self.origins[(code_len - 1) as usize]
            .get(&code)
            .copied()
            .unwrap_or((*range.start(), *range.end()));
        validate_target(start, end, target)?;
        if let BfRangeTarget::ArrayOfHexStrings(values) = target {
            let value = values
                .get(code.checked_sub(start).ok_or(UnicodeCMapError::InvalidCodeRange)? as usize)
                .ok_or(UnicodeCMapError::InvalidCodeRange)?;
            if value.is_empty() || value.len() > 256 {
                return Err(UnicodeCMapError::InvalidCodeRange);
            }
        }
        lookup_target(code, start, target)
            .map(Some)
            .ok_or(UnicodeCMapError::InvalidCodeRange)
    }

    pub fn get_or_replacement_char(&self, code: SourceCode, code_len: CodeLen) -> Vec<u16> {
        self.get(code, code_len)
            .unwrap_or(vec![ToUnicodeCMap::REPLACEMENT_CHAR])
    }

    pub fn put(&mut self, src_code_lo: SourceCode, src_code_hi: SourceCode, code_len: CodeLen, target: BfRangeTarget) {
        if code_len > 4 || code_len == 0 {
            error!("Code lenght should be between l and 4 bytes, got {code_len}, ignoring");
            return;
        }
        self.bf_ranges[(code_len - 1) as usize].insert(src_code_lo..=src_code_hi, target);
        self.origins[(code_len - 1) as usize].insert(src_code_lo..=src_code_hi, (src_code_lo, src_code_hi));
    }

    pub fn put_char(&mut self, code: SourceCode, code_len: CodeLen, dst: Vec<u16>) {
        let target = if dst.len() == 1 {
            BfRangeTarget::UTF16CodePoint {
                offset: u32::wrapping_sub(dst[0] as u32, code),
            }
        } else {
            BfRangeTarget::HexString(dst)
        };
        self.put(code, code, code_len, target)
    }

    /// Gets the source code(s) for a given Unicode sequence.
    /// Prioritizes shorter byte sequences if multiple mappings exist.
    pub fn get_source_codes_for_unicode(&self, unicode_sequence: &[u16]) -> Option<&[ReverseCMapEntry]> {
        if let Some(map) = &self.reverse_map {
            // TODO: Add prioritization logic if needed (e.g., prefer shorter code_len)
            map.get(unicode_sequence).map(|v| v.as_slice())
        } else {
            None
        }
    }
}

fn lookup_target(code: SourceCode, start: SourceCode, target: &BfRangeTarget) -> Option<Vec<u16>> {
    let delta = code.checked_sub(start)?;
    match target {
        BfRangeTarget::HexString(units) => {
            let mut result = units.clone();
            let last = result.last_mut()?;
            *last = last.checked_add(u16::try_from(delta).ok()?)?;
            Some(result)
        }
        BfRangeTarget::UTF16CodePoint { offset } => Some(vec![code.wrapping_add(*offset) as u16]),
        BfRangeTarget::ArrayOfHexStrings(values) => Some(
            values
                .get(delta as usize)
                .cloned()
                .unwrap_or_else(|| vec![ToUnicodeCMap::REPLACEMENT_CHAR]),
        ),
    }
}

fn validate_target(start: SourceCode, end: SourceCode, target: &BfRangeTarget) -> Result<(), UnicodeCMapError> {
    let delta = end.checked_sub(start).ok_or(UnicodeCMapError::InvalidCodeRange)?;
    let last = match target {
        BfRangeTarget::UTF16CodePoint { offset } => start.wrapping_add(*offset) as u16,
        BfRangeTarget::HexString(units) => {
            if units.len() > 256 {
                return Err(UnicodeCMapError::InvalidCodeRange);
            }
            *units.last().ok_or(UnicodeCMapError::InvalidCodeRange)?
        }
        BfRangeTarget::ArrayOfHexStrings(values) => {
            return if values.len() as u64 == u64::from(delta) + 1 {
                Ok(())
            } else {
                Err(UnicodeCMapError::InvalidCodeRange)
            };
        }
    };
    // The standard explicitly makes carrying past the last byte undefined.
    if delta > u32::from(255 - (last & 255)) {
        return Err(UnicodeCMapError::InvalidCodeRange);
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BfRangeTarget {
    // UTF16-BE encoding is used
    HexString(Vec<u16>),
    // don't store the actual codepoint but rather an offset to the src_code_lo
    // so that consecutive ranges can be mapped to the same value in the range map
    UTF16CodePoint { offset: u32 },
    ArrayOfHexStrings(Vec<Vec<u16>>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_char_can_be_retrieved() {
        let mut cmap = ToUnicodeCMap::new();
        let char_code = 0x01;
        let char_value = vec![0x1234];
        cmap.put_char(char_code, 2, char_value.clone());

        assert_eq!(cmap.get(char_code, 2), Some(char_value))
    }

    #[test]
    fn char_can_be_retrieved_only_by_appropriate_len() {
        let mut cmap = ToUnicodeCMap::new();
        let char_code = 0x1;
        let code_len = 4;
        let char_value = vec![0x1234];
        cmap.put_char(char_code, code_len, char_value.clone());

        for i in 1..=3 {
            assert_eq!(cmap.get(char_code, i), None);
        }

        assert_eq!(cmap.get(char_code, code_len), Some(char_value));
    }

    #[test]
    fn wrong_code_len_does_not_panic() {
        let mut cmap = ToUnicodeCMap::new();
        let char_code = 0x1;
        let char_value = vec![0x1234];

        cmap.put_char(char_code, 5, char_value.clone());
        cmap.put_char(char_code, 0, char_value.clone());
    }

    #[test]
    fn array_of_hex_strings_out_of_bounds_returns_replacement() {
        // Simulate a malformed CMap bfrange where the array has fewer entries
        // than the declared source code range covers.
        let mut cmap = ToUnicodeCMap::new();
        let array = BfRangeTarget::ArrayOfHexStrings(vec![
            vec![0x0041], // 'A' — for code 0x10
            vec![0x0042], // 'B' — for code 0x11
        ]);
        // Range 0x10..=0x14 but only 2 entries in array (needs 5)
        cmap.put(0x10, 0x14, 2, array);

        // In-bounds lookups work
        assert_eq!(cmap.get(0x10, 2), Some(vec![0x0041]));
        assert_eq!(cmap.get(0x11, 2), Some(vec![0x0042]));

        // Out-of-bounds lookups return replacement char instead of panicking
        assert_eq!(cmap.get(0x12, 2), Some(vec![ToUnicodeCMap::REPLACEMENT_CHAR]));
        assert_eq!(cmap.get(0x13, 2), Some(vec![ToUnicodeCMap::REPLACEMENT_CHAR]));
        assert_eq!(cmap.get(0x14, 2), Some(vec![ToUnicodeCMap::REPLACEMENT_CHAR]));
    }
    #[test]
    fn checked_scalar_lookup_refuses_undefined_carry_without_panicking_or_wrapping() {
        let mut cmap = ToUnicodeCMap::new();
        cmap.put(0, 1, 2, BfRangeTarget::HexString(vec![0x0041, 0xffff]));
        assert!(cmap.get_checked(0, 2).is_err());
        assert!(cmap.get_checked(1, 2).is_err());
        assert_eq!(cmap.get(1, 2), None);
        cmap.put(0, 1, 2, BfRangeTarget::HexString(vec![0x0041, 0x00fe]));
        assert_eq!(cmap.get_checked(1, 2).unwrap(), Some(vec![0x0041, 0x00ff]));
        assert_eq!(cmap.get_checked(2, 2).unwrap(), None);
    }
    #[test]
    fn checked_lookup_handles_empty_programmatic_maps_and_valid_large_arrays() {
        let mut cmap = ToUnicodeCMap::new();
        cmap.put_char(1, 2, vec![]);
        assert!(cmap.get_checked(1, 2).is_err());
        assert_eq!(cmap.get(1, 2), None);
        cmap.put(0, 256, 2, BfRangeTarget::ArrayOfHexStrings(vec![vec![0xffff]; 257]));
        assert_eq!(cmap.get_checked(256, 2).unwrap(), Some(vec![0xffff]));
        assert!(cmap.get_checked(0, 0).is_err());
        assert!(cmap.get_checked(0, 5).is_err());
    }
}
