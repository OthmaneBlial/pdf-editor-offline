//! Page text plus selected, viewable annotation appearances; never substitutes field values.

use crate::{Error, MAX_INPUT_BYTES, MAX_STREAM_BYTES, Result, structure};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId, Stream};
use std::collections::{BTreeMap, HashSet};

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
            Self::Encoded(encoding) => append(output, &encoding.bytes_to_string(bytes)?),
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

fn font<'a>(
    document: &'a Document,
    resources: Option<&'a Dictionary>,
    name: &[u8],
) -> Result<Font<'a>> {
    let resources = resources.ok_or(Error::Invalid("appearance text has no font resources"))?;
    let fonts = resources.get_deref(b"Font", document)?.as_dict()?;
    let font = fonts.get_deref(name, document)?.as_dict()?;
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
}

impl<'a> ApText<'a> {
    fn extract(
        &mut self,
        object: &'a Object,
        inherited: Option<&'a Dictionary>,
        depth: usize,
        output: &mut String,
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
        self.interpret(&bytes, resources, depth, output)?;
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
    ) -> Result<()> {
        let content = Content::decode(bytes)?;
        let mut fonts = BTreeMap::new();
        let mut current: Option<Vec<u8>> = None;
        let mut stack = Vec::new();
        for operation in content.operations {
            match operation.operator.as_str() {
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
                    if !fonts.contains_key(name) {
                        fonts.insert(name.to_vec(), font(self.document, resources, name)?);
                    }
                    current = Some(name.to_vec());
                }
                "Tj" | "TJ" | "'" | "\"" => {
                    let name = current
                        .as_ref()
                        .ok_or(Error::Invalid("appearance text has no selected font"))?;
                    let decoder = fonts
                        .get(name)
                        .ok_or(Error::Invalid("missing appearance font encoding"))?;
                    if operation.operator == "'" || operation.operator == "\"" {
                        append(output, "\n")?;
                    }
                    let operands = if operation.operator == "\"" {
                        operation
                            .operands
                            .get(2..)
                            .ok_or(Error::Invalid("invalid appearance text operator"))?
                    } else {
                        &operation.operands
                    };
                    show(decoder, operands, output)?;
                }
                "ET" | "T*" => append(output, "\n")?,
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
                        self.extract(nested, Some(resources), depth + 1, output)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
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
    appearances.interpret(&content, resources, 0, &mut output)?;
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
        appearances.extract(appearance, resources, 0, &mut output)?;
    }
    Ok(output)
}
