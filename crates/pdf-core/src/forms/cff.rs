//! Static CID-keyed CFF1 embedding. Character codes and CIDs are distinct.
//! Adobe Technical Note 5176 §18 and ISO 32000-1 §§9.7/9.9 define the mapping.
use crate::{Error, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    raw::{
        TableProvider,
        ps::cff::{CffFontRef, charset::CharsetKind, dict},
    },
};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Write,
};

pub(super) struct CffPlan<'a> {
    program: &'a [u8],
    registry: Vec<u8>,
    ordering: Vec<u8>,
    supplement: i32,
    glyph_cids: Vec<u16>,
}

fn name(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes
            .iter()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_' | b'.'))
    {
        return Err(Error::Refused("CFF registry or ordering is unsupported"));
    }
    Ok(bytes.to_vec())
}
fn mapping(charset: &skrifa::raw::ps::cff::charset::Charset<'_>, count: u32) -> Result<Vec<u16>> {
    if count == 0 || count > 65535 || !matches!(charset.kind(), CharsetKind::Custom(_)) {
        return Err(Error::Refused("CID CFF needs a bounded custom charset"));
    }
    fn ranges(mut ranges: impl Iterator<Item = (u16, u16)>, count: u32) -> Result<()> {
        let mut covered = 1u32;
        while covered < count {
            let (first, left) = ranges
                .next()
                .ok_or(Error::Refused("truncated CFF charset range"))?;
            if first.checked_add(left).is_none() {
                return Err(Error::Refused("CFF charset CID range overflows"));
            }
            covered = covered
                .checked_add(u32::from(left) + 1)
                .ok_or(Error::Refused("CFF charset range overflow"))?;
        }
        if covered != count {
            return Err(Error::Refused("CFF charset range exceeds glyph count"));
        }
        Ok(())
    }
    match charset.kind() {
        CharsetKind::Custom(skrifa::raw::ps::cff::charset::CustomCharset::Format1(format)) => {
            ranges(
                format
                    .ranges()
                    .iter()
                    .map(|range| (range.first(), u16::from(range.n_left()))),
                count,
            )?
        }
        CharsetKind::Custom(skrifa::raw::ps::cff::charset::CustomCharset::Format2(format)) => {
            ranges(
                format
                    .ranges()
                    .iter()
                    .map(|range| (range.first(), range.n_left())),
                count,
            )?
        }
        _ => {}
    }
    let mut cids = Vec::with_capacity(count as usize);
    let mut seen = HashSet::new();
    // The iterator traverses ranges once; repeated string_id lookups can be quadratic.
    for (gid, cid) in charset.iter().take(count as usize + 1) {
        let cid = cid.to_u16();
        if gid.to_u32() != cids.len() as u32 || !seen.insert(cid) || (gid.to_u32() != 0 && cid == 0)
        {
            return Err(Error::Refused(
                "CFF charset has duplicate or inconsistent CIDs",
            ));
        }
        cids.push(cid);
    }
    if cids.len() != count as usize || cids.first() != Some(&0) {
        return Err(Error::Refused("CFF charset does not cover every glyph"));
    }
    Ok(cids)
}
fn fd_selection(data: &[u8], glyphs: u32, dictionaries: u16) -> Result<()> {
    let invalid = || Error::Refused("invalid CFF FDSelect ranges");
    match data.first().copied() {
        Some(0) => {
            let selectors = data.get(1..1 + glyphs as usize).ok_or_else(invalid)?;
            if selectors
                .iter()
                .any(|index| u16::from(*index) >= dictionaries)
            {
                return Err(invalid());
            }
        }
        Some(3) => {
            let count = data.get(1..3).ok_or_else(invalid)?;
            let count = usize::from(u16::from_be_bytes([count[0], count[1]]));
            if count == 0 || count > glyphs as usize {
                return Err(invalid());
            }
            let ranges = data.get(3..3 + count * 3).ok_or_else(invalid)?;
            let mut previous = None;
            for range in ranges.chunks_exact(3) {
                let first = u32::from(u16::from_be_bytes([range[0], range[1]]));
                if (previous.is_none() && first != 0)
                    || previous.is_some_and(|last| first <= last)
                    || first >= glyphs
                    || u16::from(range[2]) >= dictionaries
                {
                    return Err(invalid());
                }
                previous = Some(first);
            }
            let end = data.get(3 + count * 3..5 + count * 3).ok_or_else(invalid)?;
            if u32::from(u16::from_be_bytes([end[0], end[1]])) != glyphs {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

fn charge(work: &mut usize, count: usize) -> Result<()> {
    *work = work
        .checked_add(count)
        .ok_or(Error::Limit("CFF validation work overflow"))?;
    if *work > 2_000_000 {
        return Err(Error::Limit("CFF validation exceeds two million entries"));
    }
    Ok(())
}

fn valid_index(index: &skrifa::raw::ps::cff::index::Index<'_>, work: &mut usize) -> Result<()> {
    charge(work, index.count() as usize)?;
    for item in 0..index.count() {
        index
            .get(item as usize)
            .map_err(|_| Error::Refused("invalid CFF subroutine index"))?;
    }
    Ok(())
}

fn canonical_matrix(matrix: &skrifa::raw::ps::transform::ScaledFontMatrix, units: u16) -> bool {
    let values = matrix.matrix.elements();
    units != 0
        && matrix.scale > 0
        && [1usize, 2, 4, 5]
            .into_iter()
            .all(|index| values[index].to_bits() == 0)
        && [0usize, 3].into_iter().all(|index| {
            i64::from(values[index].to_bits()) * i64::from(units) == i64::from(matrix.scale) * 65536
        })
}
impl<'a> CffPlan<'a> {
    pub fn new(face: &FontRef<'a>) -> Result<Self> {
        if face.cff2().is_ok() || face.fvar().is_ok() || face.glyf().is_ok() {
            return Err(Error::Refused("only static CID-keyed CFF1 is supported"));
        }
        let table = face
            .cff()
            .map_err(|_| Error::Refused("font has no valid CFF1 table"))?;
        let program = table.offset_data().as_bytes();
        if program.len() > 16 * 1024 * 1024 {
            return Err(Error::Limit("CFF program exceeds 16 MiB"));
        }
        if table.names().count() != 1
            || table.top_dicts().count() != 1
            || table.header().major() != 1
        {
            return Err(Error::Refused("CFF must contain exactly one CFF1 font"));
        }
        let postscript = face
            .localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
            .next()
            .ok_or(Error::Refused("CFF font PostScript name is missing"))?
            .to_string();
        if table.name(0) != Some(postscript.as_bytes()) {
            return Err(Error::Refused("CFF and OpenType PostScript names disagree"));
        }
        let mut ros = None;
        let mut required = HashSet::new();
        let mut cid_count = 8720;
        let mut fd_offset = None;
        let mut fd_array = None;
        let mut work = 0;
        let mut top_matrix = None;
        for entry in dict::entries(
            table
                .top_dicts()
                .get(0)
                .map_err(|_| Error::Refused("invalid CFF top dictionary"))?,
            None,
        ) {
            let entry = entry.map_err(|_| Error::Refused("malformed CFF top dictionary"))?;
            charge(&mut work, 1)?;
            let key = match &entry {
                dict::Entry::Ros { .. } => Some(0),
                dict::Entry::Charset(_) => Some(1),
                dict::Entry::CharstringsOffset(_) => Some(2),
                dict::Entry::FdArrayOffset(_) => Some(3),
                dict::Entry::FdSelectOffset(_) => Some(4),
                dict::Entry::CidCount(_) => Some(5),
                dict::Entry::FontMatrix(_) => Some(6),
                _ => None,
            };
            if let Some(key) = key
                && !required.insert(key)
            {
                return Err(Error::Refused("duplicate CFF structural operator"));
            }
            match entry {
                dict::Entry::Ros {
                    registry,
                    ordering,
                    supplement,
                } => {
                    let value = supplement.to_f64();
                    if value < 0.0 || value.fract() != 0.0 {
                        return Err(Error::Refused("invalid CFF ROS supplement"));
                    }
                    ros = Some((
                        name(
                            table
                                .string(registry)
                                .ok_or(Error::Refused("missing CFF registry"))?,
                        )?,
                        name(
                            table
                                .string(ordering)
                                .ok_or(Error::Refused("missing CFF ordering"))?,
                        )?,
                        value as i32,
                    ));
                }
                dict::Entry::CidCount(count) => cid_count = count,
                dict::Entry::FontMatrix(matrix) => top_matrix = Some(matrix),
                dict::Entry::FdSelectOffset(offset) => fd_offset = Some(offset),
                dict::Entry::FdArrayOffset(offset) => fd_array = Some(offset),
                dict::Entry::PostScript(_)
                | dict::Entry::SyntheticBase(_)
                | dict::Entry::VariationStoreOffset(_)
                | dict::Entry::Encoding(_) => {
                    return Err(Error::Refused(
                        "unsupported executable, synthetic or encoded CFF font",
                    ));
                }
                dict::Entry::CharstringType(kind) if kind != 2 => {
                    return Err(Error::Refused("unsupported CFF charstring type"));
                }
                dict::Entry::CidFontType(kind) if kind != 0 => {
                    return Err(Error::Refused("unsupported CFF CID font type"));
                }
                _ => {}
            }
        }
        if !(0..5).all(|key| required.contains(&key)) {
            return Err(Error::Refused("CFF CID structural operators are missing"));
        }
        let (registry, ordering, supplement) = ros.ok_or(Error::Refused("CFF ROS is missing"))?;
        let units = face
            .head()
            .map_err(|_| Error::Refused("CFF font units are missing"))?
            .units_per_em();
        if top_matrix
            .as_ref()
            .is_some_and(|matrix| !canonical_matrix(matrix, units))
            || (top_matrix.is_none() && units != 1000)
        {
            return Err(Error::Refused(
                "CFF font matrix does not match its layout units",
            ));
        }
        let font = CffFontRef::new(program, 0, None)
            .map_err(|_| Error::Refused("invalid CID CFF font"))?;
        if !font.is_cid() || font.num_subfonts() == 0 || font.num_subfonts() > 256 {
            return Err(Error::Refused("invalid CFF font dictionary count"));
        }
        let count = font.num_glyphs();
        if count
            != u32::from(
                face.maxp()
                    .map_err(|_| Error::Refused("font glyph count is missing"))?
                    .num_glyphs(),
            )
        {
            return Err(Error::Refused("CFF and OpenType glyph counts disagree"));
        }
        let charset = font
            .charset()
            .ok_or(Error::Refused("invalid CFF charset"))?;
        let glyph_cids = mapping(&charset, count)?;
        charge(&mut work, count as usize)?;
        if cid_count > 65536 || glyph_cids.iter().any(|cid| u32::from(*cid) >= cid_count) {
            return Err(Error::Refused("CFF CIDCount does not contain its charset"));
        }
        let fds = program
            .get(fd_offset.ok_or(Error::Refused("missing CFF FDSelect"))?..)
            .ok_or(Error::Refused("invalid CFF FDSelect offset"))?;
        fd_selection(fds, count, font.num_subfonts())?;
        let array = skrifa::raw::ps::cff::index::Index::new(
            program
                .get(fd_array.ok_or(Error::Refused("missing CFF FDArray"))?..)
                .ok_or(Error::Refused("invalid CFF FDArray offset"))?,
            false,
        )
        .map_err(|_| Error::Refused("invalid CFF FDArray index"))?;
        if array.count() != u32::from(font.num_subfonts()) {
            return Err(Error::Refused("invalid CFF FDArray count"));
        }
        valid_index(font.global_subrs(), &mut work)?;
        let mut seen_subrs = HashSet::new();
        let mut seen_private = HashSet::new();
        for index in 0..array.count() {
            let mut private = false;
            for entry in dict::entries(
                array
                    .get(index as usize)
                    .map_err(|_| Error::Refused("invalid CFF font dictionary index"))?,
                None,
            ) {
                charge(&mut work, 1)?;
                match entry.map_err(|_| Error::Refused("malformed CFF font dictionary"))? {
                    dict::Entry::PrivateDictRange(range) => {
                        if private
                            || range.start >= range.end
                            || program.get(range.clone()).is_none()
                        {
                            return Err(Error::Refused("invalid CFF private dictionary range"));
                        }
                        private = true;
                        if seen_private.insert((range.start, range.end)) {
                            let mut subrs = false;
                            for entry in dict::entries(&program[range.clone()], None) {
                                charge(&mut work, 1)?;
                                match entry.map_err(|_| {
                                    Error::Refused("malformed CFF private dictionary")
                                })? {
                                    dict::Entry::SubrsOffset(offset) => {
                                        if subrs {
                                            return Err(Error::Refused(
                                                "duplicate CFF local subroutine offset",
                                            ));
                                        }
                                        subrs = true;
                                        let offset = range.start.checked_add(offset).ok_or(
                                            Error::Refused("CFF local subroutine offset overflow"),
                                        )?;
                                        if seen_subrs.insert(offset) {
                                            let index = skrifa::raw::ps::cff::index::Index::new(
                                                program.get(offset..).ok_or(Error::Refused(
                                                    "CFF local subroutine offset is outside font",
                                                ))?,
                                                false,
                                            )
                                            .map_err(|_| {
                                                Error::Refused("invalid CFF local subroutine index")
                                            })?;
                                            valid_index(&index, &mut work)?;
                                        }
                                    }
                                    dict::Entry::VariationStoreIndex(_)
                                    | dict::Entry::VariationStoreOffset(_) => {
                                        return Err(Error::Refused(
                                            "variable CFF private dictionary is unsupported",
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    dict::Entry::FontMatrix(_) => {
                        // Fontations normalizes per-FD matrices independently;
                        // this initial horizontal tier accepts inherited matrices only.
                        return Err(Error::Refused(
                            "per-dictionary CFF font matrices are unsupported",
                        ));
                    }
                    dict::Entry::FontName(_) => {}
                    _ => return Err(Error::Refused("unsupported CFF font dictionary operator")),
                }
            }
            if !private {
                return Err(Error::Refused("CFF private dictionary is missing"));
            }
        }
        for index in 0..font.num_subfonts() {
            font.subfont(index, &[])
                .map_err(|_| Error::Refused("invalid CFF private font dictionary"))?;
        }
        for gid in 0..count {
            if font
                .subfont_index(GlyphId::new(gid))
                .is_none_or(|index| index >= font.num_subfonts())
            {
                return Err(Error::Refused(
                    "CFF glyph selects an invalid font dictionary",
                ));
            }
            font.charstrings()
                .get(gid as usize)
                .map_err(|_| Error::Refused("invalid CFF charstring index"))?;
        }
        Ok(Self {
            program,
            registry,
            ordering,
            supplement,
            glyph_cids,
        })
    }
    pub fn program_bytes(&self) -> &'a [u8] {
        self.program
    }
    pub fn cid(&self, gid: u16) -> Result<u16> {
        self.glyph_cids
            .get(usize::from(gid))
            .copied()
            .ok_or(Error::Refused("shaped CFF glyph is out of range"))
    }
    pub fn system_info(&self) -> Dictionary {
        dictionary! {"Registry"=>Object::string_literal(self.registry.clone()),"Ordering"=>Object::string_literal(self.ordering.clone()),"Supplement"=>self.supplement}
    }
    pub fn embed_descendant(
        &self,
        document: &mut Document,
        descriptor: ObjectId,
        font_name: &[u8],
        mappings: &[(u16, u16, f32)],
    ) -> Result<(ObjectId, ObjectId)> {
        if mappings.len() > 65534 {
            return Err(Error::Limit("CFF encoding exceeds 65534 codes"));
        }
        let mut widths = BTreeMap::new();
        let mut codes = HashSet::new();
        let registry = std::str::from_utf8(&self.registry)
            .map_err(|_| Error::Invalid("invalid CFF registry"))?;
        let ordering = std::str::from_utf8(&self.ordering)
            .map_err(|_| Error::Invalid("invalid CFF ordering"))?;
        let mut cmap = format!(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry ({registry}) /Ordering ({ordering}) /Supplement {} >> def\n/CMapName /NativeFormCFFEncoding def\n/CMapType 1 def\n/WMode 0 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
            self.supplement
        );
        for chunk in mappings.chunks(100) {
            writeln!(cmap, "{} begincidchar", chunk.len())
                .map_err(|_| Error::Invalid("CFF CMap formatting"))?;
            for &(gid, code, width) in chunk {
                let cid = self.cid(gid)?;
                if code == 0 || gid == 0 || !codes.insert(code) || !width.is_finite() || width < 0.0
                {
                    return Err(Error::Refused("invalid CFF encoding or glyph width"));
                }
                if let Some(previous) = widths.insert(cid, width)
                    && previous != width
                {
                    return Err(Error::Refused("inconsistent CFF CID width"));
                }
                writeln!(cmap, "<{code:04X}> {cid}")
                    .map_err(|_| Error::Invalid("CFF CMap formatting"))?;
            }
            cmap.push_str("endcidchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let mut width_array = Vec::with_capacity(widths.len() * 2);
        for (cid, width) in widths {
            width_array.push(Object::Integer(i64::from(cid)));
            width_array.push(Object::Array(vec![Object::Real(width)]));
        }
        let encoding=document.add_object(Stream::new(dictionary!{"Type"=>"CMap","CMapName"=>"NativeFormCFFEncoding","CIDSystemInfo"=>self.system_info(),"WMode"=>0},cmap.into_bytes()));
        let descendant=document.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType0","BaseFont"=>Object::Name(font_name.to_vec()),"CIDSystemInfo"=>self.system_info(),"FontDescriptor"=>descriptor,"DW"=>0,"W"=>Object::Array(width_array)});
        Ok((descendant, encoding))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use skrifa::raw::{FontData, ps::cff::charset::Charset};
    #[test]
    fn nonidentity_charset_controls_encoding_and_cid_widths() {
        let data = [0, 0, 0, 0, 0, 4, 0, 2];
        let charset = Charset::new(FontData::new(&data), 3, 3).unwrap();
        let cids = mapping(&charset, 3).unwrap();
        assert_eq!(cids, vec![0, 4, 2]);
        let plan = CffPlan {
            program: &[],
            registry: b"Adobe".to_vec(),
            ordering: b"Identity".to_vec(),
            supplement: 0,
            glyph_cids: cids,
        };
        let mut document = Document::new();
        let descriptor = document.add_object(dictionary! {});
        let (descendant, encoding) = plan
            .embed_descendant(
                &mut document,
                descriptor,
                b"Test",
                &[(1, 1, 400.0), (2, 2, 200.0), (1, 3, 400.0)],
            )
            .unwrap();
        let encoded = document.get_object(encoding).unwrap().as_stream().unwrap();
        let text = std::str::from_utf8(&encoded.content).unwrap();
        assert!(text.contains("<0001> 4\n<0002> 2\n<0003> 4"));
        let font = document.get_dictionary(descendant).unwrap();
        assert!(!font.has(b"CIDToGIDMap"));
        assert_eq!(
            font.get(b"W").unwrap(),
            &Object::Array(vec![
                2.into(),
                Object::Array(vec![Object::Real(200.0)]),
                4.into(),
                Object::Array(vec![Object::Real(400.0)])
            ])
        );
    }
    #[test]
    fn malformed_and_duplicate_charsets_are_rejected() {
        for data in [
            vec![0, 0, 0, 0, 0, 2, 0, 2],
            vec![0, 0, 0, 0, 0, 0, 0, 2],
            vec![0, 0, 0, 0, 0, 2],
            vec![0, 0, 0, 2, 0xff, 0xff, 0, 1],
        ] {
            let charset = Charset::new(FontData::new(&data), 3, 3).unwrap();
            assert!(mapping(&charset, 3).is_err());
        }
        let predefined = Charset::new(FontData::new(&[]), 0, 3).unwrap();
        assert!(mapping(&predefined, 3).is_err());
        assert!(mapping(&predefined, 65536).is_err());
    }
    #[test]
    fn fdselect_requires_complete_sorted_bounded_ranges() {
        assert!(fd_selection(&[3, 0, 2, 0, 0, 0, 0, 2, 1, 0, 3], 3, 2).is_ok());
        for bytes in [
            vec![3, 0, 0],
            vec![3, 0, 1, 0, 1, 0, 0, 3],
            vec![3, 0, 2, 0, 0, 0, 0, 0, 1, 0, 3],
            vec![3, 0, 1, 0, 0, 2, 0, 3],
            vec![3, 0, 1, 0, 0, 0, 0, 2],
            vec![0, 0, 2, 0],
            vec![4],
        ] {
            assert!(fd_selection(&bytes, 3, 2).is_err());
        }
    }
    #[test]
    fn actual_bundled_font_preserves_raw_cff_and_validates_all_cids() {
        let bytes = include_bytes!(
            "../../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf"
        );
        let face = FontRef::new(bytes).unwrap();
        let plan = CffPlan::new(&face).unwrap();
        assert_eq!(
            plan.program_bytes(),
            face.cff().unwrap().offset_data().as_bytes()
        );
        assert_eq!(plan.glyph_cids.len(), 65535);
        assert_eq!(plan.registry, b"Adobe");
        assert_eq!(plan.ordering, b"Identity");
        assert_eq!(plan.supplement, 0);
        for (gid, cid) in [(34, 34), (9544, 9544), (58199, 58199)] {
            assert_eq!(plan.cid(gid).unwrap(), cid);
        }
        assert!(plan.cid(65535).is_err());
    }
    #[test]
    fn invalid_encoding_does_not_mutate_document() {
        let plan = CffPlan {
            program: &[],
            registry: b"Adobe".to_vec(),
            ordering: b"Identity".to_vec(),
            supplement: 0,
            glyph_cids: vec![0, 4, 2],
        };
        for mappings in [
            vec![(1, 0, 200.0)],
            vec![(1, 1, f32::NAN)],
            vec![(3, 1, 200.0)],
            vec![(1, 1, 200.0), (2, 1, 300.0)],
            vec![(1, 1, 200.0), (1, 2, 300.0)],
        ] {
            let mut document = Document::new();
            let descriptor = document.add_object(dictionary! {});
            let before = document.objects.clone();
            assert!(
                plan.embed_descendant(&mut document, descriptor, b"Test", &mappings)
                    .is_err()
            );
            assert_eq!(before, document.objects);
        }
    }
    #[test]
    fn validation_work_is_bounded_before_index_iteration() {
        let mut work = 2_000_000;
        assert!(matches!(charge(&mut work, 1), Err(Error::Limit(_))));
        let mut work = usize::MAX;
        assert!(matches!(charge(&mut work, 1), Err(Error::Limit(_))));
    }
}

#[cfg(test)]
mod matrix_tests {
    use super::*;
    use skrifa::raw::{
        ps::transform::{FontMatrix, ScaledFontMatrix},
        types::Fixed,
    };
    #[test]
    fn layout_matrix_requires_equal_scales_exact_units_and_no_translation_or_skew() {
        let identity = FontMatrix::from_elements([
            Fixed::ONE,
            Fixed::ZERO,
            Fixed::ZERO,
            Fixed::ONE,
            Fixed::ZERO,
            Fixed::ZERO,
        ]);
        let matrix = ScaledFontMatrix {
            matrix: identity,
            scale: 1000,
        };
        assert!(canonical_matrix(&matrix, 1000));
        assert!(!canonical_matrix(&matrix, 2048));
        assert!(!canonical_matrix(&matrix, 0));
        for index in 0..6 {
            let mut elements = identity.elements();
            elements[index] = if index == 0 || index == 3 {
                Fixed::from_i32(2)
            } else {
                Fixed::ONE
            };
            assert!(!canonical_matrix(
                &ScaledFontMatrix {
                    matrix: FontMatrix::from_elements(elements),
                    scale: 1000
                },
                1000
            ));
        }
        assert!(!canonical_matrix(
            &ScaledFontMatrix {
                matrix: identity,
                scale: 0
            },
            1000
        ));
        assert!(!canonical_matrix(
            &ScaledFontMatrix {
                matrix: identity,
                scale: -1000
            },
            1000
        ));
    }
}
