//! Permanent removal for an intentionally small, fully inspected plaintext profile.
use crate::{Error, PdfDocument, Result};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRedactionRequest {
    pub targets: Vec<String>,
}
impl std::fmt::Debug for TextRedactionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRedactionRequest")
            .field("targets", &"[REDACTED]")
            .field("count", &self.targets.len())
            .finish()
    }
}
impl TextRedactionRequest {
    pub fn validate(&self) -> Result<()> {
        targets(&self.targets).map(|_| ())
    }
}
/// Verify the actual supplied file bytes, including ignored/trailing carrier
/// bytes; parsing a logical document alone is insufficient for this guarantee.
pub fn verify_redaction_bytes(
    bytes: &[u8],
    password: Option<&str>,
    values: &[String],
) -> Result<()> {
    let _ = targets(values)?;
    let pdf = PdfDocument::from_bytes(bytes, password)?;
    pdf.ensure_editable()?;
    // The outer PDF can carry escaped/encoded secrets which a logical parser
    // ignores. Certify only our exact full rewrite, not arbitrary ignored bytes.
    if pdf.verified_bytes()? != bytes {
        return Err(Error::Verification(
            "redaction verification requires exact native full-rewrite bytes",
        ));
    }
    pdf.verify_redaction_logical(values, bytes)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRedactionReport {
    pub changed_text_shows: usize,
    pub matched_targets: usize,
    pub affected_pages: Vec<u32>,
    pub full_rewrite: bool,
    pub profile: String,
    pub limitations: Vec<String>,
    pub cleanup: crate::SanitizationReport,
    pub rewritten_stream_orphans_removed: usize,
    pub content_comments_removed: usize,
}
struct Profile<'a> {
    pdf: &'a PdfDocument,
    seen: BTreeSet<ObjectId>,
    work: usize,
    decoded: usize,
}
struct Needle<'a> {
    bytes: &'a [u8],
    failure: Vec<usize>,
}
#[derive(Default)]
struct Search {
    work: usize,
    matches: usize,
    covered: usize,
}
impl Search {
    fn find(
        &mut self,
        bytes: &[u8],
        needles: &[Needle<'_>],
        first_only: bool,
    ) -> Result<Vec<std::ops::Range<usize>>> {
        let mut result = Vec::new();
        for needle in needles {
            // KMP is linear even for long repetitive prefixes. Refuse aggregate
            // search work before traversal, not after a pathological comparison.
            self.work = self.work.saturating_add(bytes.len().saturating_mul(2));
            if self.work > 256 * 1024 * 1024 {
                return Err(Error::Limit(
                    "redaction aggregate search exceeds 256 MiB comparison budget",
                ));
            }
            let mut matched = 0;
            for (index, byte) in bytes.iter().enumerate() {
                while matched > 0 && needle.bytes[matched] != *byte {
                    matched = needle.failure[matched - 1];
                }
                if needle.bytes[matched] == *byte {
                    matched += 1;
                }
                if matched == needle.bytes.len() {
                    self.matches += 1;
                    self.covered = self.covered.saturating_add(matched);
                    if self.covered > crate::MAX_INPUT_BYTES {
                        return Err(Error::Limit(
                            "redaction overlapping matches exceed 128 MiB work",
                        ));
                    }
                    if self.matches > 100_000 {
                        return Err(Error::Limit("redaction exceeds 100000 target occurrences"));
                    }
                    result.push(index + 1 - matched..index + 1);
                    if first_only {
                        return Ok(result);
                    }
                    matched = needle.failure[matched - 1];
                }
            }
        }
        Ok(result)
    }
    fn contains(&mut self, bytes: &[u8], needles: &[Needle<'_>]) -> Result<bool> {
        Ok(!self.find(bytes, needles, true)?.is_empty())
    }
}
fn targets(values: &[String]) -> Result<Vec<Needle<'_>>> {
    if values.is_empty()
        || values.len() > 100
        || values.iter().map(String::len).sum::<usize>() > 65536
    {
        return Err(Error::Limit(
            "redaction requires 1..100 targets, at most 64 KiB total",
        ));
    }
    if values
        .iter()
        .any(|s| s.is_empty() || s.len() > 4096 || !s.bytes().all(|b| (32..=126).contains(&b)))
    {
        return Err(Error::Refused(
            "redaction targets must be nonempty printable ASCII, at most 4096 bytes",
        ));
    }
    if values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(Error::Refused("redaction targets must be unique"));
    }
    Ok(values
        .iter()
        .map(|s| {
            let bytes = s.as_bytes();
            let mut failure = vec![0; bytes.len()];
            let mut matched = 0;
            for index in 1..bytes.len() {
                while matched > 0 && bytes[index] != bytes[matched] {
                    matched = failure[matched - 1];
                }
                if bytes[index] == bytes[matched] {
                    matched += 1;
                }
                failure[index] = matched;
            }
            Needle { bytes, failure }
        })
        .collect())
}
fn keys(dict: &Dictionary, allowed: &[&[u8]]) -> Result<()> {
    if dict
        .iter()
        .any(|(key, _)| !allowed.contains(&key.as_slice()))
    {
        return Err(Error::Refused(
            "redaction profile excludes custom or unsupported dictionary carriers",
        ));
    }
    Ok(())
}
impl<'a> Profile<'a> {
    fn step(&mut self, n: usize) -> Result<()> {
        self.work = self.work.saturating_add(n);
        if self.work > 2_000_000 {
            return Err(Error::Limit(
                "redaction graph work exceeds two million values",
            ));
        }
        Ok(())
    }
    fn resolve(&mut self, mut value: &'a Object) -> Result<&'a Object> {
        let mut active = BTreeSet::new();
        while let Object::Reference(id) = value {
            self.step(1)?;
            if active.len() >= 128 || !active.insert(*id) {
                return Err(Error::Refused("redaction reference cycle/depth"));
            }
            self.seen.insert(*id);
            value = self
                .pdf
                .document
                .objects
                .get(id)
                .ok_or(Error::Invalid("missing redaction reference"))?;
        }
        Ok(value)
    }
    fn numbers(&mut self, value: &'a Object, n: usize) -> Result<()> {
        let array = self.resolve(value)?.as_array()?;
        if array.len() != n {
            return Err(Error::Refused("unsupported redaction geometry"));
        }
        self.step(n)?;
        for value in array {
            let value = self.resolve(value)?;
            if !matches!(value, Object::Integer(_) | Object::Real(_))
                || !value.as_float()?.is_finite()
            {
                return Err(Error::Refused("invalid redaction geometry"));
            }
        }
        Ok(())
    }
    fn resources(&mut self, value: &'a Object) -> Result<BTreeMap<Vec<u8>, (String, bool)>> {
        let dict = self.resolve(value)?.as_dict()?;
        keys(dict, &[b"Font", b"ProcSet"])?;
        if let Ok(value) = dict.get(b"ProcSet") {
            let array = self.resolve(value)?.as_array()?;
            self.step(array.len())?;
            for value in array {
                if !matches!(self.resolve(value)?.as_name()?, b"PDF" | b"Text") {
                    return Err(Error::Refused(
                        "image/opaque resources prevent proven text redaction",
                    ));
                }
            }
        }
        let fonts = self.resolve(dict.get(b"Font")?)?.as_dict()?;
        self.step(fonts.len())?;
        let mut names = BTreeMap::new();
        for (name, value) in fonts {
            if name.len() > 128 || name.is_empty() || !name.iter().all(|b| (33..=126).contains(b)) {
                return Err(Error::Limit("redaction font resource name"));
            }
            let font = self.resolve(value)?.as_dict()?;
            keys(font, &[b"Type", b"Subtype", b"BaseFont", b"Encoding"])?;
            if font.get(b"Type")?.as_name()? != b"Font"
                || font.get(b"Subtype")?.as_name()? != b"Type1"
            {
                return Err(Error::Refused(
                    "redaction requires unembedded standard Type1 fonts",
                ));
            }
            if !matches!(
                font.get(b"BaseFont")?.as_name()?,
                b"Helvetica"
                    | b"Helvetica-Bold"
                    | b"Helvetica-Oblique"
                    | b"Helvetica-BoldOblique"
                    | b"Courier"
                    | b"Courier-Bold"
                    | b"Courier-Oblique"
                    | b"Courier-BoldOblique"
                    | b"Times-Roman"
                    | b"Times-Bold"
                    | b"Times-Italic"
                    | b"Times-BoldItalic"
            ) {
                return Err(Error::Refused("unsupported redaction font"));
            }
            let win_ansi = if let Ok(encoding) = font.get(b"Encoding") {
                if self.resolve(encoding)?.as_name()? != b"WinAnsiEncoding" {
                    return Err(Error::Refused("unsupported redaction font encoding"));
                }
                true
            } else {
                false
            };
            let base = std::str::from_utf8(font.get(b"BaseFont")?.as_name()?)
                .map_err(|_| Error::Refused("invalid standard font name"))?;
            names.insert(name.clone(), (base.to_owned(), win_ansi));
        }
        Ok(names)
    }
    fn content(&mut self, value: &'a Object, output: &mut Vec<u8>, depth: usize) -> Result<()> {
        if depth >= 128 {
            return Err(Error::Limit("redaction Contents nesting"));
        }
        let value = self.resolve(value)?;
        match value {
            Object::Array(values) => {
                self.step(values.len())?;
                for value in values {
                    self.content(value, output, depth + 1)?;
                }
            }
            Object::Stream(stream) => {
                keys(&stream.dict, &[b"Length", b"Filter", b"DecodeParms"])?;
                if let Ok(filter) = stream.dict.get(b"Filter")
                    && self.resolve(filter)?.as_name()? != b"FlateDecode"
                {
                    return Err(Error::Refused(
                        "redaction only supports plain or Flate content streams",
                    ));
                }
                if let Ok(params) = stream.dict.get(b"DecodeParms")
                    && !matches!(self.resolve(params)?, Object::Null)
                {
                    return Err(Error::Refused("redaction decoding parameters unsupported"));
                }
                let _ = self.resolve(stream.dict.get(b"Length")?)?.as_i64()?;
                let bytes = stream.get_plain_content_with_limit(
                    crate::MAX_STREAM_BYTES.saturating_sub(output.len()),
                )?;
                self.decoded = self.decoded.saturating_add(bytes.len());
                if self.decoded > crate::MAX_INPUT_BYTES {
                    return Err(Error::Limit(
                        "redaction aggregate decoded content exceeds 128 MiB",
                    ));
                }
                output
                    .try_reserve(bytes.len())
                    .map_err(|_| Error::Limit("redaction content allocation"))?;
                output.extend(bytes);
            }
            _ => return Err(Error::Refused("unsupported redaction Contents")),
        }
        Ok(())
    }
    fn pages(&mut self) -> Result<Vec<(Vec<u8>, Fonts)>> {
        keys(&self.pdf.document.trailer, &[b"Root", b"Size", b"ID"])?;
        if let Ok(size) = self.pdf.document.trailer.get(b"Size") {
            let _ = self.resolve(size)?.as_i64()?;
        }
        let root = self
            .resolve(self.pdf.document.trailer.get(b"Root")?)?
            .as_dict()?;
        keys(root, &[b"Type", b"Pages", b"Version", b"Names"])?;
        if let Ok(value) = root.get(b"Names")
            && !self.resolve(value)?.as_dict()?.is_empty()
        {
            return Err(Error::Refused("redaction excludes surviving name trees"));
        }
        if let Ok(value) = root.get(b"Version")
            && !matches!(
                self.resolve(value)?.as_name()?,
                b"1.0" | b"1.1" | b"1.2" | b"1.3" | b"1.4" | b"1.5" | b"1.6" | b"1.7" | b"2.0"
            )
        {
            return Err(Error::Refused("redaction catalog version unsupported"));
        }
        if root.get(b"Type")?.as_name()? != b"Catalog" {
            return Err(Error::Invalid("missing redaction catalog"));
        }
        let _ = self.resolve(root.get(b"Pages")?)?.as_dict()?;
        let mut checked = BTreeSet::new();
        let mut result = Vec::new();
        for id in &self.pdf.pages {
            let mut current = *id;
            let mut resources = None;
            for depth in 0..128 {
                self.seen.insert(current);
                let dict = self
                    .pdf
                    .document
                    .objects
                    .get(&current)
                    .ok_or(Error::Invalid("missing redaction page"))?
                    .as_dict()?;
                let ty = dict.get(b"Type")?.as_name()?;
                if !matches!(ty, b"Page" | b"Pages") {
                    return Err(Error::Invalid("invalid redaction page tree"));
                }
                if resources.is_none() {
                    resources = dict.get(b"Resources").ok();
                }
                if checked.insert(current) {
                    if (ty == b"Pages" && dict.has(b"Contents"))
                        || (ty == b"Page" && (dict.has(b"Count") || dict.has(b"Kids")))
                    {
                        return Err(Error::Refused(
                            "redaction page node has unsupported carriers",
                        ));
                    }
                    keys(
                        dict,
                        &[
                            b"Type",
                            b"Parent",
                            b"Kids",
                            b"Count",
                            b"Contents",
                            b"Resources",
                            b"MediaBox",
                            b"CropBox",
                            b"Rotate",
                        ],
                    )?;
                    if let Ok(value) = dict.get(b"Resources") {
                        self.resources(value)?;
                    }
                    for key in [b"MediaBox".as_slice(), b"CropBox"] {
                        if let Ok(value) = dict.get(key) {
                            self.numbers(value, 4)?;
                        }
                    }
                    if let Ok(value) = dict.get(b"Rotate") {
                        let _ = self.resolve(value)?.as_i64()?;
                    }
                    if ty == b"Pages" {
                        let _ = self.resolve(dict.get(b"Count")?)?.as_i64()?;
                        let kids = self.resolve(dict.get(b"Kids")?)?.as_array()?;
                        self.step(kids.len())?;
                        for kid in kids {
                            let _ = self.resolve(kid)?.as_dict()?;
                        }
                    }
                }
                if let Ok(parent) = dict.get(b"Parent") {
                    current = parent.as_reference()?;
                } else {
                    break;
                }
                if depth == 127 {
                    return Err(Error::Limit("redaction page inheritance"));
                }
            }
            let font_names = self.resources(resources.ok_or(Error::Refused(
                "redaction page needs explicit font resources",
            ))?)?;
            let page = self.pdf.document.get_object(*id)?.as_dict()?;
            let mut bytes = Vec::new();
            if let Ok(contents) = page.get(b"Contents") {
                self.content(contents, &mut bytes, 0)?;
            }
            result.push((bytes, font_names));
        }
        if self
            .pdf
            .document
            .objects
            .keys()
            .any(|id| !self.seen.contains(id))
        {
            return Err(Error::Refused(
                "redaction graph contains an unclassified reachable carrier",
            ));
        }
        Ok(result)
    }
}
type Font = (String, bool);
type Fonts = BTreeMap<Vec<u8>, Font>;
struct Show<'a> {
    ordinal: usize,
    font: &'a Font,
    segments: Vec<(usize, &'a [u8])>,
}
fn pieces(value: &[u8], mask: &[bool], font: &Font) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut at = 0;
    while at < value.len() {
        let removed = mask[at];
        let start = at;
        while at < value.len() && mask[at] == removed {
            at += 1;
        }
        if removed {
            let width = value[start..at].iter().try_fold(0u64, |sum, code| {
                let width = pdf_render::standard_ascii_width(&font.0, *code, font.1)
                    .ok_or(Error::Refused("standard font advance unavailable"))?;
                sum.checked_add(u64::from(width))
                    .ok_or(Error::Limit("redaction width overflow"))
            })?;
            output.extend_from_slice(format!(" -{width} ").as_bytes());
        } else {
            output.push(b'<');
            for byte in &value[start..at] {
                output.extend_from_slice(format!("{byte:02X}").as_bytes());
            }
            output.extend_from_slice(b"> ");
        }
    }
    Ok(output)
}
fn plan(
    bytes: &[u8],
    fonts: &Fonts,
    targets: &[Needle<'_>],
    search: &mut Search,
) -> Result<(Vec<u8>, usize, usize)> {
    let (program, spans) = crate::content_spans::decode_spanned(bytes)?;
    let mut block = false;
    let mut font = None;
    let mut positioned = false;
    let mut text: Vec<u8> = Vec::new();
    let mut shows: Vec<Show<'_>> = Vec::new();
    let mut changes = Vec::new();
    let mut changed_shows = 0;
    let mut matches = 0;
    let mut graphics_depth = 0usize;
    for (ordinal, op) in program.operations.iter().enumerate() {
        match op.operator.as_str() {
            "BT" => {
                if block || !op.operands.is_empty() {
                    return Err(Error::Refused("invalid redaction text object"));
                }
                block = true;
                font = None;
                positioned = false;
                text.clear();
                shows.clear();
            }
            "ET" => {
                if !block || !op.operands.is_empty() {
                    return Err(Error::Refused("invalid redaction ET"));
                }
                block = false;
                let mut mask = vec![false; text.len()];
                for range in search.find(&text, targets, false)? {
                    mask[range].fill(true);
                    matches += 1;
                }
                for show in &shows {
                    if !show
                        .segments
                        .iter()
                        .any(|(at, s)| mask[*at..*at + s.len()].iter().any(|b| *b))
                    {
                        continue;
                    }
                    changed_shows += 1;
                    let op = &program.operations[show.ordinal];
                    let span = &spans[show.ordinal];
                    if op.operator == "Tj" {
                        let (at, value) = show.segments[0];
                        let mut replacement = b"[".to_vec();
                        replacement.extend(pieces(value, &mask[at..at + value.len()], show.font)?);
                        replacement.extend_from_slice(b"] TJ");
                        changes.push((span.full.clone(), replacement));
                    } else {
                        let range = span.operands[0].clone();
                        let items = crate::content_spans::array_item_spans(bytes, range.clone())?;
                        let values = op.operands[0].as_array()?;
                        if items.len() != values.len() {
                            return Err(Error::Refused("ambiguous redaction TJ spans"));
                        }
                        let mut replacement = b"[".to_vec();
                        let mut segment = 0;
                        for (value, item) in values.iter().zip(items) {
                            if matches!(value, Object::String(_, _)) {
                                let (at, s) = show.segments[segment];
                                segment += 1;
                                if mask[at..at + s.len()].iter().any(|b| *b) {
                                    replacement.extend(pieces(
                                        s,
                                        &mask[at..at + s.len()],
                                        show.font,
                                    )?);
                                } else {
                                    replacement.extend_from_slice(&bytes[item]);
                                    replacement.push(b' ');
                                }
                            } else {
                                replacement.extend_from_slice(&bytes[item]);
                                replacement.push(b' ');
                            }
                        }
                        replacement.push(b']');
                        changes.push((range, replacement));
                    }
                }
            }
            "Tf" if block => {
                if op.operands.len() != 2
                    || !op.operands[1].as_float()?.is_finite()
                    || op.operands[1].as_float()? <= 0.0
                {
                    return Err(Error::Refused("unsupported redaction font selection"));
                }
                font = Some(
                    fonts
                        .get(op.operands[0].as_name()?)
                        .ok_or(Error::Refused("missing redaction font"))?,
                );
            }
            "Tm" | "Td" if block => {
                let n = if op.operator == "Tm" { 6 } else { 2 };
                if op.operands.len() != n
                    || op
                        .operands
                        .iter()
                        .any(|v| v.as_float().is_err() || !v.as_float().is_ok_and(f32::is_finite))
                {
                    return Err(Error::Refused("invalid redaction text position"));
                }
                positioned = true;
            }
            "Tj" | "TJ" if block => {
                let font =
                    font.ok_or(Error::Refused("redaction text objects need explicit font"))?;
                if !positioned || op.operands.len() != 1 {
                    return Err(Error::Refused(
                        "redaction text objects need explicit position",
                    ));
                }
                let values = if op.operator == "TJ" {
                    op.operands[0].as_array()?.as_slice()
                } else {
                    op.operands.as_slice()
                };
                let mut show = Show {
                    ordinal,
                    font,
                    segments: Vec::new(),
                };
                for value in values {
                    match value {
                        Object::String(s, _) => {
                            if !s.iter().all(|b| (32..=126).contains(b))
                                || (!font.1 && s.iter().any(|b| matches!(b, 39 | 96)))
                            {
                                return Err(Error::Refused(
                                    "redaction requires unambiguous printable ASCII glyph strings",
                                ));
                            }
                            if text.len().saturating_add(s.len()) > crate::MAX_STREAM_BYTES {
                                return Err(Error::Limit("redaction text object bytes"));
                            }
                            show.segments.push((text.len(), s.as_slice()));
                            text.extend(s);
                        }
                        Object::Integer(_) | Object::Real(_)
                            if op.operator == "TJ" && value.as_float()?.is_finite() => {}
                        _ => return Err(Error::Refused("invalid redaction text show operands")),
                    }
                }
                shows.push(show);
            }
            _ if block => {
                return Err(Error::Refused(
                    "text state/marked content is outside the proven redaction profile",
                ));
            }
            "q" => {
                if !op.operands.is_empty() || graphics_depth >= 32 {
                    return Err(Error::Refused("redaction graphics stack"));
                }
                graphics_depth += 1;
            }
            "Q" => {
                if !op.operands.is_empty() || graphics_depth == 0 {
                    return Err(Error::Refused("redaction graphics stack"));
                }
                graphics_depth -= 1;
            }
            "cm" | "m" | "l" | "c" | "v" | "y" | "h" | "re" | "S" | "s" | "f" | "F" | "f*"
            | "B" | "B*" | "b" | "b*" | "n" | "W" | "W*" | "w" | "J" | "j" | "M" | "d" | "G"
            | "g" | "RG" | "rg" | "K" | "k" => {
                // Artwork is retained byte-for-byte. Only finite numeric operands and
                // dash arrays are admitted: no string/property/resource carriers.
                let arity = match op.operator.as_str() {
                    "cm" | "c" => 6,
                    "m" | "l" | "d" => 2,
                    "v" | "y" | "re" | "K" | "k" => 4,
                    "RG" | "rg" => 3,
                    "w" | "J" | "j" | "M" | "G" | "g" => 1,
                    _ => 0,
                };
                if op.operands.len() != arity {
                    return Err(Error::Refused("invalid redaction artwork operator arity"));
                }
                if op.operator == "d"
                    && (op.operands[0].as_array().is_err() || op.operands[1].as_float().is_err())
                {
                    return Err(Error::Refused("invalid redaction dash operands"));
                }
                for value in &op.operands {
                    match value {
                        Object::Integer(_) | Object::Real(_) if value.as_float()?.is_finite() => {}
                        Object::Array(a)
                            if op.operator == "d"
                                && a.iter().all(|v| v.as_float().is_ok_and(f32::is_finite)) => {}
                        _ => return Err(Error::Refused("unsupported redaction artwork operand")),
                    }
                }
            }
            _ => {
                return Err(Error::Refused(
                    "opaque or unsupported content prevents proven redaction",
                ));
            }
        }
    }
    if block || graphics_depth != 0 {
        return Err(Error::Refused("unbalanced redaction content"));
    }
    let mut output = Vec::new();
    output
        .try_reserve(bytes.len())
        .map_err(|_| Error::Limit("redaction output allocation"))?;
    let mut at = 0;
    for (range, replacement) in &changes {
        if output
            .len()
            .saturating_add(range.start - at)
            .saturating_add(replacement.len())
            > crate::MAX_STREAM_BYTES
        {
            return Err(Error::Limit("redaction expanded output exceeds 32 MiB"));
        }
        output.extend_from_slice(&bytes[at..range.start]);
        output.extend(replacement);
        at = range.end;
    }
    output.extend_from_slice(&bytes[at..]);
    if output.len() > crate::MAX_STREAM_BYTES {
        return Err(Error::Limit("redaction expanded output exceeds 32 MiB"));
    }
    Ok((output, changed_shows, matches))
}
impl PdfDocument {
    /// Remove exact target glyphs into a verified separate document, compensating
    /// their standard-font advances so retained glyph positions remain unchanged.
    /// Supports only the reported strict plaintext profile; never overwrites source.
    pub fn redact_text_copy(
        &self,
        values: &[String],
    ) -> Result<(PdfDocument, TextRedactionReport)> {
        let needles = targets(values)?;
        self.ensure_editable()?;
        if self.document.objects.len().saturating_add(self.pages.len()) > crate::MAX_OBJECTS {
            return Err(Error::Limit("redaction page rewrite object budget"));
        }
        let (mut candidate, cleanup) = self.sanitized_copy()?;
        candidate.document.reference_table.cross_reference_type =
            lopdf::xref::XrefType::CrossReferenceTable;
        candidate.document.trailer.remove(b"ID");
        let pages = Profile {
            pdf: &candidate,
            seen: BTreeSet::new(),
            work: 0,
            decoded: 0,
        }
        .pages()?;
        let mut report=TextRedactionReport {
            changed_text_shows:0, matched_targets:0, affected_pages:Vec::new(),
            full_rewrite:true, profile:"ascii-standard-type1-explicit-text-state-v1".into(),
            cleanup, rewritten_stream_orphans_removed:0,content_comments_removed:0,
            limitations:vec![
                "Exact ASCII target glyphs are removed; their advance is retained as a TJ adjustment, preserving unmatched glyph positions.".into(),
                "Only printable ASCII, unembedded standard Type1 fonts and the explicitly inspected page/vector profile are supported; opaque, image, form, annotation and custom carriers refuse.".into(),
                "This proves absence of the supplied exact targets within this supported output profile, not absence of other private information or erasure of source, backups, recovery journals or filesystem copies.".into(),
            ],
        };
        let mut search = Search::default();
        let mut output_bytes = 0usize;
        for (index, (bytes, fonts)) in pages.iter().enumerate() {
            let (bytes, comments) = crate::content_spans::clear_content_comments(bytes)?;
            let (output, count, matches) = plan(&bytes, fonts, &needles, &mut search)?;
            output_bytes = output_bytes.saturating_add(output.len());
            if output_bytes > crate::MAX_INPUT_BYTES {
                return Err(Error::Limit(
                    "redaction aggregate expanded content exceeds 128 MiB",
                ));
            }
            report.content_comments_removed += comments;
            {
                if candidate.document.objects.len() >= crate::MAX_OBJECTS
                    || candidate.document.max_id as usize >= crate::MAX_OBJECTS
                {
                    return Err(Error::Limit("redaction object allocation"));
                }
                let id = candidate
                    .document
                    .add_object(Stream::new(Dictionary::new(), output));
                candidate
                    .document
                    .get_object_mut(candidate.pages[index])?
                    .as_dict_mut()?
                    .set("Contents", id);
                report.changed_text_shows += count;
                report.matched_targets += matches;
                if count != 0 || comments != 0 {
                    report.affected_pages.push(index as u32 + 1);
                }
            }
        }
        if report.changed_text_shows == 0 {
            return Err(Error::Refused(
                "no target occurs in supported page text objects",
            ));
        }
        // Reuse the existing sanitizer's bounded reachable-graph/full-rewrite proof
        // after page rewrites: the original text streams must not survive as orphans.
        let (candidate, second_cleanup) = candidate.sanitized_copy()?;
        report.rewritten_stream_orphans_removed = second_cleanup.unreachable_objects_removed;
        candidate.verify_redaction(values)?;
        Ok((candidate, report))
    }
    /// Independently validate the supported profile and exact target absence.
    /// A refusal is not a privacy success; this does not inspect other files/copies.
    pub fn verify_redaction(&self, values: &[String]) -> Result<()> {
        let _ = targets(values)?;
        self.ensure_editable()?;
        verify_redaction_bytes(&self.verified_bytes()?, None, values)
    }
    fn verify_redaction_logical(&self, values: &[String], bytes: &[u8]) -> Result<()> {
        let needles = targets(values)?;
        self.ensure_editable()?;
        if self.document.objects.values().any(|value|matches!(value,Object::Stream(stream) if stream.dict.has(b"Filter") || stream.dict.has(b"DecodeParms"))){return Err(Error::Refused("redaction verification requires plain content without opaque filter transport"));}
        if self.document.trailer.has(b"Prev")
            || self.document.trailer.has(b"XRefStm")
            || self.document.trailer.has(b"ID")
        {
            return Err(Error::Verification(
                "redacted output has old revision/identifier carriers",
            ));
        }
        let pages = Profile {
            pdf: self,
            seen: BTreeSet::new(),
            work: 0,
            decoded: 0,
        }
        .pages()?;
        let mut search = Search::default();
        let mut all_glyphs = Vec::new();
        for (bytes, fonts) in pages {
            if crate::content_spans::clear_content_comments(&bytes)?.1 != 0 {
                return Err(Error::Refused(
                    "redaction output contains unsupported content-comment carriers",
                ));
            }
            let (_, count, _) = plan(&bytes, &fonts, &needles, &mut search)?;
            if count != 0 || search.contains(&bytes, &needles)? {
                return Err(Error::Verification(
                    "redaction target remains in page text or decoded content",
                ));
            }
            // Also refuse targets reconstructed across independently positioned
            // shows/text objects/pages. We do not advertise those complex layouts
            // as supported redaction, and must not certify residual fragments.
            let (program, _) = crate::content_spans::decode_spanned(&bytes)?;
            for op in program.operations {
                let values = match op.operator.as_str() {
                    "Tj" => Some(op.operands.as_slice()),
                    "TJ" => Some(op.operands[0].as_array()?.as_slice()),
                    _ => None,
                };
                if let Some(values) = values {
                    for value in values {
                        if let Object::String(value, _) = value {
                            if all_glyphs.len().saturating_add(value.len()) > crate::MAX_INPUT_BYTES
                            {
                                return Err(Error::Limit("redaction aggregate glyph verification"));
                            }
                            all_glyphs.try_reserve(value.len()).map_err(|_| {
                                Error::Limit("redaction glyph verification allocation")
                            })?;
                            all_glyphs.extend(value);
                        }
                    }
                }
            }
        }
        if search.contains(&all_glyphs, &needles)? {
            return Err(Error::Verification(
                "redaction target reconstructs from residual glyph fragments",
            ));
        }
        let selected = (1..=self.pages.len() as u32).collect::<Vec<_>>();
        if search.contains(self.extract_text(&selected)?.as_bytes(), &needles)? {
            return Err(Error::Verification(
                "redaction target remains in extracted document text",
            ));
        }
        // All surviving values are classified by Profile. Scan names/strings as
        // well, so a target hidden in a resource name cannot receive a success.
        let mut pending = self.document.objects.values().collect::<Vec<_>>();
        let mut work = 0usize;
        while let Some(value) = pending.pop() {
            work += 1;
            if work.saturating_add(pending.len()) > 2_000_000 {
                return Err(Error::Limit("redaction verification value budget"));
            }
            match value {
                Object::String(s, _) | Object::Name(s) if search.contains(s, &needles)? => {
                    return Err(Error::Verification(
                        "redaction target remains in logical values",
                    ));
                }
                Object::Array(a) => {
                    if work.saturating_add(pending.len()).saturating_add(a.len()) > 2_000_000 {
                        return Err(Error::Limit("redaction verification queue"));
                    }
                    pending.extend(a);
                }
                Object::Dictionary(d) => {
                    if work.saturating_add(pending.len()).saturating_add(d.len()) > 2_000_000 {
                        return Err(Error::Limit("redaction verification dictionary queue"));
                    }
                    for (k, v) in d {
                        if search.contains(k, &needles)? {
                            return Err(Error::Verification(
                                "redaction target remains in dictionary keys",
                            ));
                        }
                        pending.push(v);
                    }
                }
                Object::Stream(s) => {
                    if work
                        .saturating_add(pending.len())
                        .saturating_add(s.dict.len())
                        > 2_000_000
                    {
                        return Err(Error::Limit("redaction verification stream queue"));
                    }
                    for (k, v) in &s.dict {
                        if search.contains(k, &needles)? {
                            return Err(Error::Verification(
                                "redaction target remains in stream keys",
                            ));
                        }
                        pending.push(v);
                    }
                }
                _ => {}
            }
        }
        if search.contains(bytes, &needles)? {
            return Err(Error::Verification(
                "redaction target remains in full serialized file",
            ));
        }
        Ok(())
    }
}
