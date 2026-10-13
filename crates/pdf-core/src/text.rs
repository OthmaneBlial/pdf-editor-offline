//! Page text plus selected, viewable annotation appearances; never substitutes field values.

use crate::{Error, MAX_INPUT_BYTES, MAX_STREAM_BYTES, Result, structure};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId, Stream};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Default)]
struct MarkedContext {
    suppressed: bool,
    depth: usize,
}

fn finite_number(object: &Object) -> bool {
    match object {
        Object::Integer(_) => true,
        Object::Real(value) => value.is_finite(),
        _ => false,
    }
}

fn validate_show(operator: &str, operands: &[Object], work: &mut usize) -> Result<()> {
    let items = match (operator, operands) {
        ("Tj" | "'", [Object::String(..)]) => None,
        ("TJ", [Object::Array(items)]) => Some(items),
        ("\"", [word_spacing, character_spacing, Object::String(..)])
            if finite_number(word_spacing) && finite_number(character_spacing) =>
        {
            None
        }
        _ => return Err(Error::Invalid("invalid text operands inside ActualText")),
    };
    *work = work
        .checked_add(operands.len())
        .and_then(|count| count.checked_add(items.map_or(0, Vec::len)))
        .ok_or(Error::Limit("text interpretation work overflow"))?;
    if *work > 2_000_000 {
        return Err(Error::Limit(
            "text interpretation exceeds two million operations/references",
        ));
    }
    if let Some(items) = items {
        for item in items {
            if !matches!(item, Object::String(..)) && !finite_number(item) {
                return Err(Error::Invalid("invalid text array item inside ActualText"));
            }
        }
    }
    Ok(())
}

enum Font<'a> {
    Encoded(Encoding<'a>),
    Dingbats,
}

fn append(output: &mut String, value: &str) -> Result<()> {
    if output.len().saturating_add(value.len()) > MAX_INPUT_BYTES {
        return Err(Error::Limit("extracted text exceeds 128 MiB"));
    }
    output.push_str(value);
    Ok(())
}

impl Font<'_> {
    fn show(&self, bytes: &[u8], output: &mut String) -> Result<()> {
        match self {
            Self::Encoded(encoding) => append_encoded(encoding, bytes, output, MAX_INPUT_BYTES),
            Self::Dingbats => {
                // shortcut: standard checkbox symbols only; add the full encoding with licensed attribution when other symbols are required.
                for byte in bytes {
                    let symbol = match byte {
                        b' ' => " ",
                        b'3' => "✓",
                        b'4' => "✔",
                        b'5' => "✕",
                        b'6' => "✖",
                        b'7' => "✗",
                        b'8' => "✘",
                        _ => {
                            return Err(Error::Refused(
                                "unsupported standard Dingbats appearance symbol",
                            ));
                        }
                    };
                    append(output, symbol)?;
                }
                Ok(())
            }
        }
    }
}

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

fn append_encoded(
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
                    emit(map.get_or_replacement_char(code, length))?;
                    code = 0;
                    length = 0;
                }
                code = code * 256 + u32::from(*byte);
                length += 1;
                if let Some(units) = map.get(code, length) {
                    emit(units)?;
                    code = 0;
                    length = 0;
                }
            }
            if length > 0 {
                emit(map.get_or_replacement_char(code, length))?;
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

fn font<'a>(
    document: &'a Document,
    resources: Option<&'a Dictionary>,
    name: &[u8],
) -> Result<Font<'a>> {
    let resources = resources.ok_or(Error::Invalid("appearance text has no font resources"))?;
    let fonts = resources.get_deref(b"Font", document)?.as_dict()?;
    let font = fonts.get_deref(name, document)?.as_dict()?;
    if font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0")
        && let Ok(unicode) = font.get(b"ToUnicode")
    {
        // ToUnicode maps character codes, independently of the code-to-CID
        // Encoding CMap. lopdf otherwise tries that Encoding stream first.
        // Copy only the indirect stream edge, not the whole font dictionary.
        let mut unicode_font = Dictionary::new();
        unicode_font.set("Type", "Font");
        unicode_font.set("ToUnicode", unicode.as_reference()?);
        return match unicode_font.get_font_encoding_with_limit(document, MAX_STREAM_BYTES)? {
            Encoding::UnicodeMapEncoding(map) => {
                Ok(Font::Encoded(Encoding::UnicodeMapEncoding(map)))
            }
            _ => Err(Error::Refused(
                "invalid explicit composite-font ToUnicode map",
            )),
        };
    }
    // Standard symbolic fonts use their built-in glyphs even when a producer names WinAnsi.
    if font.get(b"BaseFont").and_then(Object::as_name).ok() == Some(b"ZapfDingbats")
        && font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type1")
        && !font.has(b"FontDescriptor")
        && (!font.has(b"Encoding")
            || font.get(b"Encoding").and_then(Object::as_name).ok() == Some(b"WinAnsiEncoding"))
        && !font.has(b"ToUnicode")
    {
        return Ok(Font::Dingbats);
    }
    Ok(Font::Encoded(font.get_font_encoding_with_limit(
        document,
        MAX_STREAM_BYTES,
    )?))
}

fn show(font: &Font<'_>, operands: &[Object], output: &mut String) -> Result<()> {
    for operand in operands {
        match operand {
            Object::String(bytes, _) => font.show(bytes, output)?,
            Object::Array(items) => {
                for item in items {
                    match item {
                        Object::String(bytes, _) => font.show(bytes, output)?,
                        Object::Integer(value) if *value < -100 => append(output, " ")?,
                        Object::Real(value) if *value < -100.0 => append(output, " ")?,
                        Object::Integer(_) | Object::Real(_) => {}
                        _ => return Err(Error::Invalid("invalid appearance text array")),
                    }
                }
            }
            _ => return Err(Error::Invalid("invalid appearance text operand")),
        }
    }
    Ok(())
}

struct ApText<'a> {
    document: &'a Document,
    remaining_bytes: usize,
    active: HashSet<ObjectId>,
    calls: usize,
    actual_remaining: usize,
    semantic_work: usize,
}

impl<'a> ApText<'a> {
    fn extract(
        &mut self,
        object: &'a Object,
        inherited: Option<&'a Dictionary>,
        depth: usize,
        output: &mut String,
        marked: MarkedContext,
    ) -> Result<()> {
        self.calls += 1;
        if depth >= 32 || self.calls > 10_000 {
            return Err(Error::Limit("annotation appearance nesting/work budget"));
        }
        let (id, resolved) = self.document.dereference(object)?;
        if let Some(id) = id
            && !self.active.insert(id)
        {
            return Err(Error::Invalid("cyclic annotation appearance"));
        }
        let stream: &Stream = resolved.as_stream()?;
        let bytes = stream.decompressed_content_with_limit(self.remaining_bytes)?;
        self.remaining_bytes = self.remaining_bytes.saturating_sub(bytes.len());
        let resources = if stream.dict.has(b"Resources") {
            Some(
                stream
                    .dict
                    .get_deref(b"Resources", self.document)?
                    .as_dict()?,
            )
        } else {
            inherited
        };
        self.interpret(&bytes, resources, depth, output, marked)?;
        if let Some(id) = id {
            self.active.remove(&id);
        }
        Ok(())
    }

    fn interpret(
        &mut self,
        bytes: &[u8],
        resources: Option<&'a Dictionary>,
        depth: usize,
        output: &mut String,
        inherited_marked: MarkedContext,
    ) -> Result<()> {
        let content = Content::decode(bytes)?;
        let mut fonts = BTreeMap::new();
        let mut current: Option<Vec<u8>> = None;
        let mut stack = Vec::new();
        let mut marked: Vec<Option<String>> = Vec::new();
        let mut replacements = 0usize;
        for operation in content.operations {
            self.semantic_work = self
                .semantic_work
                .checked_add(1)
                .ok_or(Error::Limit("text interpretation work overflow"))?;
            if self.semantic_work > 2_000_000 {
                return Err(Error::Limit(
                    "text interpretation exceeds two million operations/references",
                ));
            }
            let suppressed = inherited_marked.suppressed || replacements > 0;
            match operation.operator.as_str() {
                "BMC" | "BDC" => {
                    if inherited_marked.depth + marked.len() >= 128 {
                        return Err(Error::Limit("marked content exceeds 128 levels"));
                    }
                    if operation.operands.len() != if operation.operator == "BMC" { 1 } else { 2 } {
                        return Err(Error::Invalid("invalid marked-content operands"));
                    }
                    operation.operands[0].as_name()?;
                    let replacement = if operation.operator == "BDC" {
                        self.actual_text(&operation.operands[1], resources)?
                    } else {
                        None
                    };
                    if replacement.is_some() {
                        replacements += 1;
                    }
                    marked.push(replacement);
                }
                "EMC" => {
                    if !operation.operands.is_empty() {
                        return Err(Error::Invalid("invalid marked-content end operands"));
                    }
                    let replacement = marked
                        .pop()
                        .ok_or(Error::Invalid("unbalanced marked-content end"))?;
                    if let Some(replacement) = replacement {
                        replacements -= 1;
                        if !inherited_marked.suppressed && replacements == 0 {
                            append(output, &replacement)?;
                        }
                    }
                }
                "q" => {
                    if stack.len() >= 32 {
                        return Err(Error::Limit("appearance graphics state nesting"));
                    }
                    stack.push(current.clone());
                }
                "Q" => {
                    current = stack
                        .pop()
                        .ok_or(Error::Invalid("unbalanced appearance graphics state"))?
                }
                "Tf" => {
                    let name = operation
                        .operands
                        .first()
                        .ok_or(Error::Invalid("missing appearance font"))?
                        .as_name()?;
                    if !suppressed && !fonts.contains_key(name) {
                        fonts.insert(name.to_vec(), font(self.document, resources, name)?);
                    }
                    current = Some(name.to_vec());
                }
                "Tj" | "TJ" | "'" | "\"" => {
                    validate_show(
                        &operation.operator,
                        &operation.operands,
                        &mut self.semantic_work,
                    )?;
                    if suppressed {
                        continue;
                    }
                    let operands = if operation.operator == "\"" {
                        operation
                            .operands
                            .get(2..)
                            .ok_or(Error::Invalid("invalid appearance text operator"))?
                    } else {
                        &operation.operands
                    };
                    let name = current
                        .as_ref()
                        .ok_or(Error::Invalid("appearance text has no selected font"))?;
                    if !fonts.contains_key(name) {
                        fonts.insert(name.clone(), font(self.document, resources, name)?);
                    }
                    let decoder = fonts
                        .get(name)
                        .ok_or(Error::Invalid("missing appearance font encoding"))?;
                    if operation.operator == "'" || operation.operator == "\"" {
                        append(output, "\n")?;
                    }
                    show(decoder, operands, output)?;
                }
                "ET" | "T*" if !suppressed => append(output, "\n")?,
                "Do" => {
                    let name = operation
                        .operands
                        .first()
                        .ok_or(Error::Invalid("missing appearance XObject name"))?
                        .as_name()?;
                    let resources =
                        resources.ok_or(Error::Invalid("appearance XObject has no resources"))?;
                    let xobjects = resources.get_deref(b"XObject", self.document)?.as_dict()?;
                    let nested = xobjects.get(name)?;
                    let stream = self.document.dereference(nested)?.1.as_stream()?;
                    if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") {
                        self.extract(
                            nested,
                            Some(resources),
                            depth + 1,
                            output,
                            MarkedContext {
                                suppressed,
                                depth: inherited_marked.depth + marked.len(),
                            },
                        )?;
                    }
                }
                _ => {}
            }
        }
        if !marked.is_empty() {
            return Err(Error::Invalid("unterminated marked-content sequence"));
        }
        Ok(())
    }

    fn semantic_resolve<'b>(&mut self, mut object: &'b Object) -> Result<&'b Object>
    where
        'a: 'b,
    {
        let mut seen = HashSet::new();
        while let Object::Reference(id) = object {
            self.semantic_work += 1;
            if self.semantic_work > 2_000_000 {
                return Err(Error::Limit("ActualText references exceed work budget"));
            }
            if !seen.insert(*id) {
                return Err(Error::Invalid("cyclic ActualText property reference"));
            }
            if seen.len() > 128 {
                return Err(Error::Limit(
                    "ActualText property reference exceeds 128 levels",
                ));
            }
            object = self
                .document
                .objects
                .get(id)
                .ok_or(Error::Invalid("missing ActualText property reference"))?;
        }
        Ok(object)
    }

    fn actual_text(
        &mut self,
        property: &Object,
        resources: Option<&'a Dictionary>,
    ) -> Result<Option<String>> {
        let dictionary = if let Object::Name(name) = property {
            let resources = resources.ok_or(Error::Invalid(
                "named marked-content property has no resources",
            ))?;
            let properties = self
                .semantic_resolve(resources.get(b"Properties")?)?
                .as_dict()?;
            self.semantic_resolve(properties.get(name)?)?.as_dict()?
        } else {
            self.semantic_resolve(property)?.as_dict()?
        };
        let Ok(actual) = dictionary.get(b"ActualText") else {
            return Ok(None);
        };
        let object = self.semantic_resolve(actual)?;
        let bytes = object.as_str()?;
        if bytes.len() > MAX_STREAM_BYTES
            || bytes.len() > self.actual_remaining.saturating_mul(2).saturating_add(2)
        {
            return Err(Error::Limit(
                "ActualText string exceeds remaining byte budget",
            ));
        }
        let value = crate::strings::decode(object)?;
        if value.len() > self.actual_remaining {
            return Err(Error::Limit(
                "ActualText decoded strings exceed 32 MiB per page",
            ));
        }
        self.actual_remaining -= value.len();
        Ok(Some(value))
    }
}

pub(crate) fn extract_page(document: &Document, page_id: ObjectId) -> Result<String> {
    let page = document.get_dictionary(page_id)?;
    let resources = structure::inherited(document, page_id, b"Resources")?
        .map(Object::as_dict)
        .transpose()?;
    let mut output = String::new();
    let mut appearances = ApText {
        document,
        remaining_bytes: MAX_STREAM_BYTES,
        active: HashSet::new(),
        calls: 0,
        actual_remaining: MAX_STREAM_BYTES,
        semantic_work: 0,
    };
    let mut content = Vec::new();
    let streams = document.get_page_contents(page_id);
    if streams.len() > 10_000 {
        return Err(Error::Limit("page content stream work budget"));
    }
    for id in streams {
        let stream = document.get_object(id)?.as_stream()?;
        let remaining = MAX_STREAM_BYTES.saturating_sub(content.len());
        let bytes = stream.decompressed_content_with_limit(remaining)?;
        if bytes.len().saturating_add(1) > remaining {
            return Err(Error::Limit("page content exceeds 32 MiB"));
        }
        content.extend_from_slice(&bytes);
        content.push(b'\n');
    }
    appearances.remaining_bytes -= content.len();
    appearances.interpret(
        &content,
        resources,
        0,
        &mut output,
        MarkedContext::default(),
    )?;
    let Ok(annotations) = page.get(b"Annots") else {
        return Ok(output);
    };
    let annotations = document.dereference(annotations)?.1.as_array()?;
    for annotation in annotations {
        let annotation = document.dereference(annotation)?.1.as_dict()?;
        let flags = if annotation.has(b"F") {
            annotation.get(b"F")?.as_i64()?
        } else {
            0
        };
        if flags & (1 | 2 | 32) != 0 {
            continue;
        }
        let Ok(ap) = annotation.get(b"AP") else {
            continue;
        };
        let ap = document.dereference(ap)?.1.as_dict()?;
        let Ok(normal) = ap.get(b"N") else { continue };
        let resolved = document.dereference(normal)?.1;
        let appearance = if let Object::Dictionary(states) = resolved {
            let Ok(state) = annotation.get(b"AS") else {
                continue;
            };
            let Ok(selected) = states.get(state.as_name()?) else {
                continue;
            };
            selected
        } else {
            normal
        };
        append(&mut output, "\n")?;
        appearances.extract(
            appearance,
            resources,
            0,
            &mut output,
            MarkedContext::default(),
        )?;
    }
    Ok(output)
}

#[cfg(test)]
mod bounded_encoding_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
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
}

#[cfg(test)]
mod actual_text_operand_tests {
    use super::*;
    #[test]
    fn nonfinite_suppressed_numbers_are_invalid() {
        for value in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
            let text = Object::string_literal("text");
            assert!(
                validate_show("TJ", &[Object::Array(vec![Object::Real(value)])], &mut 0).is_err()
            );
            assert!(
                validate_show(
                    "\"",
                    &[Object::Real(value), Object::Integer(0), text.clone()],
                    &mut 0
                )
                .is_err()
            );
            assert!(
                validate_show(
                    "\"",
                    &[Object::Integer(0), Object::Real(value), text],
                    &mut 0
                )
                .is_err()
            );
        }
    }
}
