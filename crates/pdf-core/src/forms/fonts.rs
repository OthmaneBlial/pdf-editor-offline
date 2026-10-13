//! Explicit TrueType/CID CFF1 embedding and bounded OpenType shaping.
#[path = "cff.rs"]
mod cff;
use crate::{Error, Result};
use harfrust::{BufferClusterLevel, Direction, ShapeOptions, ShaperData, UnicodeBuffer};
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    raw::TableProvider,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use unicode_bidi::{Level, ParagraphBidiInfo};
use unicode_script::{Script, ScriptExtension, UnicodeScript};

/// Maximal compatible Script_Extension runs. Common/inherited characters
/// stay in the run; an incompatible inherited mark is refused, never detached.
fn script_runs(text: &str) -> Result<Vec<(std::ops::Range<usize>, Script)>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut scripts = ScriptExtension::from(Script::Common);
    for (index, character) in text.char_indices() {
        let extension = character.script_extension();
        let intersection = scripts.intersection(extension);
        if intersection.is_empty() {
            if character.script() == Script::Inherited {
                return Err(Error::Refused(
                    "combining mark is incompatible with its text script",
                ));
            }
            let script = scripts.iter().next().unwrap_or(Script::Common);
            runs.push((start..index, script));
            start = index;
            scripts = extension;
        } else {
            scripts = intersection;
        }
    }
    if start < text.len() {
        runs.push((
            start..text.len(),
            scripts.iter().next().unwrap_or(Script::Common),
        ));
    }
    Ok(runs)
}

pub(super) struct Glyph {
    pub cid: u16,
    pub x: f64,
    pub y: f64,
    pub invisible: bool,
}
// Same default-ignorable ranges used by harfrust's shaping implementation.
fn ignorable(c: char) -> bool {
    matches!(u32::from(c), 0x00ad | 0x034f | 0x061c | 0x17b4..=0x17b5 |
        0x180b..=0x180e | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f |
        0xfe00..=0xfe0f | 0xfeff | 0xfff0..=0xfff8 | 0x1d173..=0x1d17a | 0xe0000..=0xe0fff)
}
pub(super) struct Line {
    pub glyphs: Vec<Glyph>,
    pub bounds: [f64; 4],
}
pub(super) struct FontPlan<'a> {
    bytes: &'a [u8],
    face: FontRef<'a>,
    shaper: ShaperData,
    name: String,
    em: f64,
    ascent: f64,
    descent: f64,
    mappings: BTreeMap<(u16, String), u16>,
    mapping_bytes: usize,
    outlined: std::collections::BTreeSet<u16>,
    cff: Option<cff::CffPlan<'a>>,
}

pub(super) fn unicode_string(text: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

impl<'a> FontPlan<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::Limit("font exceeds 16 MiB"));
        }
        let face = FontRef::new(bytes).map_err(|_| {
            Error::Refused("a standalone complete TrueType or CID-keyed CFF font is required")
        })?;
        if [b"COLR", b"CPAL", b"CBDT", b"CBLC", b"sbix", b"SVG "]
            .iter()
            .any(|tag| face.table_data(skrifa::raw::types::Tag::new(tag)).is_some())
        {
            return Err(Error::Refused(
                "color, bitmap and SVG fonts are not supported",
            ));
        }
        if face.fvar().is_ok() || face.cff2().is_ok() || face.glyf().is_ok() == face.cff().is_ok() {
            return Err(Error::Refused(
                "only static TrueType or CID-keyed CFF1 outlines are supported",
            ));
        }
        let cff = if face.cff().is_ok() {
            Some(cff::CffPlan::new(&face)?)
        } else {
            None
        };
        let bytes = cff.as_ref().map_or(bytes, |plan| plan.program_bytes());
        let permissions = face
            .os2()
            .map_err(|_| Error::Refused("font embedding permissions are missing"))?
            .fs_type();
        if permissions & (0x0002 | 0x0004 | 0x0200) != 0 {
            return Err(Error::Refused(
                "font permissions do not allow editable outline embedding",
            ));
        }
        let metrics = face.metrics(Size::unscaled(), LocationRef::default());
        if metrics.units_per_em == 0 || metrics.glyph_count == 0 || metrics.bounds.is_none() {
            return Err(Error::Refused("font has no usable metrics or outlines"));
        }
        let name = face
            .localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
            .next()
            .map(|value| value.to_string())
            .ok_or(Error::Refused("font PostScript name is missing"))?;
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::Refused("font PostScript name is unsupported"));
        }
        let em = f64::from(metrics.units_per_em);
        let ascent = f64::from(metrics.ascent) / em;
        let descent = f64::from(metrics.descent) / em;
        if !ascent.is_finite() || !descent.is_finite() || ascent <= descent {
            return Err(Error::Refused("invalid font vertical metrics"));
        }
        Ok(Self {
            bytes,
            face: face.clone(),
            shaper: ShaperData::new(&face),
            name,
            em,
            ascent,
            descent,
            mappings: BTreeMap::new(),
            mapping_bytes: 0,
            outlined: std::collections::BTreeSet::new(),
            cff,
        })
    }

    fn register(&mut self, gid: u16, text: String) -> Result<u16> {
        if text.encode_utf16().count() > 256 {
            return Err(Error::Refused(
                "shaped glyph Unicode mapping exceeds 256 UTF-16 units",
            ));
        }
        let key = (gid, text);
        if let Some(cid) = self.mappings.get(&key) {
            return Ok(*cid);
        }
        if self.mappings.len() >= 65_534 {
            return Err(Error::Limit("text font exceeds 65534 glyph mappings"));
        }
        self.mapping_bytes = self
            .mapping_bytes
            .checked_add(key.1.len())
            .ok_or(Error::Limit("glyph mapping overflow"))?;
        if self.mapping_bytes > 4 * 1024 * 1024 {
            return Err(Error::Limit("glyph mappings exceed 4 MiB"));
        }
        let cid = u16::try_from(self.mappings.len() + 1)
            .map_err(|_| Error::Limit("glyph mapping overflow"))?;
        self.mappings.insert(key, cid);
        Ok(cid)
    }

    pub fn shape(&mut self, text: &str) -> Result<Line> {
        if text.is_empty() {
            return Ok(Line {
                glyphs: Vec::new(),
                bounds: [0.0, self.descent, 0.0, self.ascent],
            });
        }
        if text.len() > 64 * 1024 {
            return Err(Error::Limit("text field value exceeds 64 KiB"));
        }
        if text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            return Err(Error::Refused(
                "single-line text cannot contain line breaks or control characters",
            ));
        }
        if text.chars().any(|c| {
            !ignorable(c)
                && self
                    .face
                    .charmap()
                    .map(c)
                    .is_none_or(|gid| gid.to_u32() == 0)
        }) {
            return Err(Error::Refused(
                "the supplied font lacks a required Unicode character",
            ));
        }
        let base = if unicode_bidi::get_base_direction(text) == unicode_bidi::Direction::Rtl {
            Level::rtl()
        } else {
            Level::ltr()
        };
        let bidi = ParagraphBidiInfo::new(text, Some(base));
        let (levels, runs) = bidi.visual_runs(0..text.len());
        let mut glyphs = Vec::new();
        let mut pen = 0.0;
        let mut bounds = [0.0, self.descent, 0.0, self.ascent];
        let face = self.face.clone();
        let glyph_metrics = face.glyph_metrics(Size::unscaled(), LocationRef::default());
        for run in runs {
            let directional = text
                .get(run.clone())
                .ok_or(Error::Invalid("invalid bidirectional text boundary"))?;
            let rtl = levels.get(run.start).is_some_and(Level::is_rtl);
            let mut scripts = script_runs(directional)?;
            if rtl {
                scripts.reverse();
            }
            for (script_range, script) in scripts {
                let input = directional
                    .get(script_range)
                    .ok_or(Error::Invalid("invalid script boundary"))?;
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(input);
                buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
                buffer.set_direction(if rtl {
                    Direction::RightToLeft
                } else {
                    Direction::LeftToRight
                });
                buffer.set_script(
                    script
                        .short_name()
                        .parse()
                        .map_err(|_| Error::Refused("unsupported shaping script"))?,
                );
                buffer.guess_segment_properties();
                let shaped = self
                    .shaper
                    .shaper(&self.face)
                    .build()
                    .shape(buffer, ShapeOptions::new());
                let infos = shaped.glyph_infos();
                let positions = shaped.glyph_positions();
                if glyphs.len().saturating_add(infos.len()) > 65_534 {
                    return Err(Error::Limit("text field exceeds 65534 shaped glyphs"));
                }
                let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                for (index, glyph) in infos.iter().enumerate() {
                    clusters
                        .entry(glyph.cluster as usize)
                        .or_default()
                        .push(index);
                }
                let starts: Vec<_> = clusters.keys().copied().collect();
                let mut logical = BTreeMap::new();
                for (ordinal, start) in starts.iter().enumerate() {
                    let end = starts.get(ordinal + 1).copied().unwrap_or(input.len());
                    let cluster = input
                        .get(*start..end)
                        .ok_or(Error::Invalid("invalid shaped text cluster"))?;
                    let chars: Vec<_> = cluster
                        .char_indices()
                        .map(|(index, _)| index)
                        .chain(std::iter::once(cluster.len()))
                        .collect();
                    let indices = &clusters[start];
                    if indices.len() > chars.len().saturating_sub(1) {
                        return Err(Error::Refused(
                            "font shaping requires an unsupported extraction mapping",
                        ));
                    }
                    for (index, glyph_index) in indices.iter().enumerate() {
                        let end = if index + 1 == indices.len() {
                            cluster.len()
                        } else {
                            chars[index + 1]
                        };
                        logical.insert(*glyph_index, cluster[chars[index]..end].to_owned());
                    }
                }
                for (index, (glyph, position)) in infos.iter().zip(positions).enumerate() {
                    let unicode = logical
                        .remove(&index)
                        .ok_or(Error::Invalid("missing glyph text mapping"))?;
                    let invisible = unicode.chars().all(ignorable);
                    let gid = u16::try_from(glyph.glyph_id)
                        .map_err(|_| Error::Refused("font glyph index exceeds 16-bit limits"))?;
                    if gid == 0 {
                        return Err(Error::Refused("font shaping produced a missing glyph"));
                    }
                    if let Some(cff) = &self.cff {
                        cff.cid(gid)?;
                    }
                    let x = pen + f64::from(position.x_offset) / self.em;
                    let y = f64::from(position.y_offset) / self.em;
                    let ink = glyph_metrics.bounds(GlyphId::new(u32::from(gid)));
                    if !unicode.chars().all(|c| c.is_whitespace() || ignorable(c))
                        && ink.is_none_or(|ink| ink.x_min >= ink.x_max || ink.y_min >= ink.y_max)
                    {
                        return Err(Error::Refused(
                            "a required text glyph has no visible outline",
                        ));
                    }
                    if !unicode.chars().all(|c| c.is_whitespace() || ignorable(c))
                        && !self.outlined.contains(&gid)
                    {
                        if self.cff.is_none() {
                            let loca = face
                                .loca(None)
                                .map_err(|_| Error::Refused("invalid glyph locations"))?;
                            let offset = loca
                                .get_raw(usize::from(gid))
                                .ok_or(Error::Refused("invalid glyph location"))?
                                as usize;
                            let data = face
                                .table_data(skrifa::raw::types::Tag::new(b"glyf"))
                                .ok_or(Error::Refused("missing glyph outlines"))?;
                            let contours = data
                                .as_bytes()
                                .get(offset..offset + 2)
                                .ok_or(Error::Refused("invalid glyph outline"))?;
                            if contours == [0, 0] {
                                return Err(Error::Refused(
                                    "a required text glyph has no visible outline",
                                ));
                            }
                        }
                        let outlines = face.outline_glyphs();
                        let outline =
                            outlines
                                .get(GlyphId::new(u32::from(gid)))
                                .ok_or(Error::Refused(
                                    "a required text glyph has no visible outline",
                                ))?;
                        if outline.draw_memory_size(skrifa::outline::Hinting::None)
                            > 4 * 1024 * 1024
                        {
                            return Err(Error::Limit("glyph outline exceeds 4 MiB working memory"));
                        }
                        let mut pen = skrifa::outline::pen::ControlBoundsPen::new();
                        outline
                            .draw(
                                skrifa::outline::DrawSettings::unhinted(
                                    Size::unscaled(),
                                    LocationRef::default(),
                                ),
                                &mut pen,
                            )
                            .map_err(|_| {
                                Error::Refused("a required text glyph has an invalid outline")
                            })?;
                        if pen
                            .bounding_box()
                            .is_none_or(|ink| ink.x_min >= ink.x_max || ink.y_min >= ink.y_max)
                        {
                            return Err(Error::Refused(
                                "a required text glyph has no visible outline",
                            ));
                        }
                        self.outlined.insert(gid);
                    }
                    if let Some(ink) = ink.filter(|_| !invisible) {
                        bounds[0] = bounds[0].min(x + f64::from(ink.x_min) / self.em);
                        bounds[1] = bounds[1].min(y + f64::from(ink.y_min) / self.em);
                        bounds[2] = bounds[2].max(x + f64::from(ink.x_max) / self.em);
                        bounds[3] = bounds[3].max(y + f64::from(ink.y_max) / self.em);
                    }
                    let cid = self.register(gid, unicode)?;
                    glyphs.push(Glyph {
                        cid,
                        x,
                        y,
                        invisible,
                    });
                    pen += f64::from(position.x_advance) / self.em;
                    bounds[2] = bounds[2].max(pen);
                    if !pen.is_finite() || bounds.iter().any(|n| !n.is_finite()) {
                        return Err(Error::Limit("text layout is not finite"));
                    }
                }
            }
        }
        Ok(Line { glyphs, bounds })
    }

    pub fn existing_program(&self, document: &Document) -> Result<Option<ObjectId>> {
        let mut seen = std::collections::BTreeSet::new();
        let mut remaining = 32 * 1024 * 1024usize;
        for object in document.objects.values() {
            let Ok(descriptor) = object.as_dict() else {
                continue;
            };
            if descriptor.get(b"Type").and_then(Object::as_name).ok() != Some(b"FontDescriptor")
                || descriptor.get(b"FontName").and_then(Object::as_name).ok()
                    != Some(self.name.as_bytes())
            {
                continue;
            }
            let Ok(id) = descriptor
                .get(if self.cff.is_some() {
                    b"FontFile3"
                } else {
                    b"FontFile2"
                })
                .and_then(Object::as_reference)
            else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            let stream = document.get_object(id)?.as_stream()?;
            let bytes = stream.get_plain_content_with_limit(remaining.min(16 * 1024 * 1024))?;
            remaining = remaining
                .checked_sub(bytes.len())
                .ok_or(Error::Limit("existing font program budget"))?;
            if bytes == self.bytes {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    pub fn embed(&self, document: &mut Document, existing: Option<ObjectId>) -> Result<ObjectId> {
        let metrics = self.face.metrics(Size::unscaled(), LocationRef::default());
        let bounds = metrics
            .bounds
            .ok_or(Error::Refused("font has no outline bounds"))?;
        let scale = 1000.0 / self.em;
        let number = |value: f32| Object::Real((f64::from(value) * scale) as f32);
        let program = existing.unwrap_or_else(|| {
            let dict = if self.cff.is_some() {
                dictionary! {"Subtype"=>"CIDFontType0C"}
            } else {
                dictionary! {"Length1"=>self.bytes.len() as i64}
            };
            document.add_object(Stream::new(dict, self.bytes.to_vec()))
        });
        let mut descriptor = dictionary! {
            "Type"=>"FontDescriptor", "FontName"=>Object::Name(self.name.as_bytes().to_vec()), "Flags"=>4,
            "ItalicAngle"=>metrics.italic_angle, "Ascent"=>number(metrics.ascent), "Descent"=>number(metrics.descent),
            "CapHeight"=>number(metrics.cap_height.unwrap_or(metrics.ascent)), "StemV"=>80,
            "FontBBox"=>vec![number(bounds.x_min),number(bounds.y_min),number(bounds.x_max),number(bounds.y_max)],
        };
        descriptor.set(
            if self.cff.is_some() {
                "FontFile3"
            } else {
                "FontFile2"
            },
            program,
        );
        let descriptor = document.add_object(descriptor);
        let mut sorted: Vec<_> = self.mappings.iter().collect();
        sorted.sort_by_key(|(_, cid)| **cid);
        let mut gids = vec![0, 0];
        let mut widths = Vec::new();
        let mut cff_mappings = Vec::new();
        let advances = self
            .face
            .glyph_metrics(Size::unscaled(), LocationRef::default());
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /NativeFormUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        for chunk in sorted.chunks(100) {
            writeln!(cmap, "{} beginbfchar", chunk.len())
                .map_err(|_| Error::Invalid("font CMap formatting"))?;
            for ((gid, text), cid) in chunk {
                gids.extend_from_slice(&gid.to_be_bytes());
                let width = advances
                    .advance_width(GlyphId::new(u32::from(*gid)))
                    .ok_or(Error::Refused("font glyph has no advance"))?;
                widths.push(number(width));
                if self.cff.is_some() {
                    cff_mappings.push((*gid, **cid, (f64::from(width) * scale) as f32));
                }
                write!(cmap, "<{cid:04X}> <")
                    .map_err(|_| Error::Invalid("font CMap formatting"))?;
                for unit in text.encode_utf16() {
                    write!(cmap, "{unit:04X}")
                        .map_err(|_| Error::Invalid("font CMap formatting"))?;
                }
                cmap.push_str(">\n");
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        if cmap.len() > crate::MAX_STREAM_BYTES {
            return Err(Error::Limit("font Unicode map exceeds 32 MiB"));
        }
        let unicode = document.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        let (descendant, encoding) = if let Some(cff) = &self.cff {
            let (descendant, encoding) =
                cff.embed_descendant(document, descriptor, self.name.as_bytes(), &cff_mappings)?;
            (descendant, Object::Reference(encoding))
        } else {
            let map = document.add_object(Stream::new(dictionary! {}, gids));
            let descendant = document.add_object(dictionary! {
            "Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>Object::Name(self.name.as_bytes().to_vec()),
            "CIDSystemInfo"=>dictionary! {"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
            "FontDescriptor"=>descriptor,"CIDToGIDMap"=>map,"DW"=>0,"W"=>vec![1.into(),Object::Array(widths)],
        });
            (descendant, Object::Name(b"Identity-H".to_vec()))
        };
        Ok(document.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>Object::Name(self.name.as_bytes().to_vec()),"Encoding"=>encoding,"DescendantFonts"=>vec![descendant.into()],"ToUnicode"=>unicode}))
    }
}
