use super::*;

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

impl PdfDocument {
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
        for (index, (bytes, fonts)) in pages.into_iter().enumerate() {
            if crate::content_spans::clear_content_comments(&bytes)?.1 != 0 {
                return Err(Error::Refused(
                    "redaction output contains unsupported content-comment carriers",
                ));
            }
            let media_box =
                crate::structure::page_box(&self.document, self.pages[index], b"MediaBox")?;
            let (_, count, _) = plan(&bytes, &fonts, media_box, &needles, &mut search)?;
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
                    "Tj" | "'" => Some(op.operands.as_slice()),
                    "\"" => Some(&op.operands[2..]),
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
