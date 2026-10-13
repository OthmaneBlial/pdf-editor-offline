use super::*;

pub(super) struct Profile<'a> {
    pub(super) pdf: &'a PdfDocument,
    pub(super) seen: BTreeSet<ObjectId>,
    pub(super) work: usize,
    pub(super) decoded: usize,
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
    pub(super) fn pages(&mut self) -> Result<Vec<(Vec<u8>, Fonts)>> {
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
