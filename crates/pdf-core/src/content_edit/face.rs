//! Original final-show font changes; no overlay, clipping, fitting or fallback.
use super::*;
use lopdf::dictionary;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextFaceChange {
    pub identity: ContentTextIdentity,
    pub expected: String,
    pub font: ContentTextFont,
    /// None preserves the exact original string token, including its escapes.
    pub replacement: Option<String>,
    #[serde(default)]
    pub layout_policy: ContentTextLayoutPolicy,
}

impl ContentTextRun {
    pub fn face_position(&self, change: &ContentTextFaceChange) -> Result<ContentTextPosition> {
        if !self.properties_writable
            || self.identity != change.identity
            || self.text.as_deref() != Some(change.expected.as_str())
        {
            return Err(Error::Refused("font inventory is stale or unsupported"));
        }
        let value = change.replacement.as_deref().unwrap_or(&change.expected);
        if !ContentTextEncoding::WinAnsi.accepts_ascii(value) {
            return Err(Error::Refused(
                "standard face changes require printable ASCII",
            ));
        }
        if matches!(
            change.layout_policy,
            ContentTextLayoutPolicy::PreserveWordAdvances
        ) {
            let font = self
                .standard_font
                .ok_or(Error::Refused("source font unavailable"))?;
            let encoding = self
                .encoding
                .ok_or(Error::Refused("source encoding unavailable"))?;
            let old = change.expected.split(' ').collect::<Vec<_>>();
            let new = value.split(' ').collect::<Vec<_>>();
            if old.len() != new.len() {
                return Err(Error::Refused("font change alters spaces or word advances"));
            }
            if old.len() > 1
                && font.code_width(32, encoding)?
                    != change.font.code_width(32, ContentTextEncoding::WinAnsi)?
            {
                return Err(Error::Refused("font change alters nominal space advances"));
            }
            for (old, new) in old.into_iter().zip(new) {
                if (old.len() != new.len() && !self.allows_variable_length)
                    || font.width(old, encoding)?
                        != change.font.width(new, ContentTextEncoding::WinAnsi)?
                {
                    return Err(Error::Refused(
                        "font change requires explicit width-change policy",
                    ));
                }
            }
        }
        self.layout
            .as_ref()
            .ok_or(Error::Refused("bounded font layout unavailable"))?
            .position(change.font, ContentTextEncoding::WinAnsi, value)
    }
}

impl PdfDocument {
    /// Isolates a standard Latin face in the selected page's resources and text
    /// object. The original size, matrix and persistent later state are retained.
    /// Old source streams remain recoverable; this is not secure deletion.
    pub fn change_content_text_face(
        &mut self,
        page: u32,
        changes: &[ContentTextFaceChange],
    ) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_RUNS {
            return Err(Error::Invalid("provide 1 through 10000 font changes"));
        }
        let runs = self
            .content_text_runs(page)?
            .into_iter()
            .map(|run| (run.identity.operation, run))
            .collect::<BTreeMap<_, _>>();
        let PageContent {
            page_id,
            stream_id,
            bytes,
            content,
            spans,
        } = single_content(self, page)?;
        self.content_property_context(page_id, &content)?;
        let (mut resources, mut fonts) = crate::forms::content_font_resources(self, page_id)?;
        let extra = changes
            .len()
            .checked_add(1)
            .ok_or(Error::Limit("font allocation overflow"))?;
        if self.document.objects.len().saturating_add(extra) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(extra) >= MAX_OBJECTS
        {
            return Err(Error::Limit("font change object allocation budget"));
        }
        let mut selected = BTreeSet::new();
        let mut total = 0usize;
        let mut splices = Vec::new();
        let mut font_plans = Vec::new();
        let mut suffix = 0usize;
        for change in changes {
            total = total
                .saturating_add(change.expected.len())
                .saturating_add(change.replacement.as_ref().map_or(0, String::len));
            if total > MAX_TEXT {
                return Err(Error::Limit("font requests exceed 4 MiB"));
            }
            if !selected.insert(change.identity.operation) {
                return Err(Error::Invalid("duplicate font target"));
            }
            let run = runs
                .get(&change.identity.operation)
                .ok_or(Error::Refused("font inventory is stale"))?;
            run.face_position(change)?;
            let name = loop {
                suffix += 1;
                if suffix > 100_000 {
                    return Err(Error::Limit("font resource name budget"));
                }
                let name = format!("NativeContentFont{suffix}").into_bytes();
                if !fonts.has(&name) {
                    break name;
                }
            };
            // Reserve the name before planning the next target; no source resources change.
            fonts.set(name.clone(), Object::Null);
            let show = change.identity.operation as usize;
            let start = content.operations[..show]
                .iter()
                .rposition(|op| op.operator == "BT")
                .ok_or(Error::Invalid("missing font text object"))?;
            let setter = content.operations[start..show]
                .iter()
                .rposition(|op| op.operator == "Tf")
                .ok_or(Error::Invalid("missing original font setter"))?
                + start;
            let mut insert = format!("\n/{} ", String::from_utf8_lossy(&name)).into_bytes();
            insert.extend_from_slice(&bytes[spans[setter].operands[1].clone()]);
            insert.extend_from_slice(b" Tf\n");
            splices.push((spans[show].full.start..spans[show].full.start, insert));
            if let Some(value) = &change.replacement {
                splices.push((spans[show].operands[0].clone(), ascii_operand(value)));
            }
            splices.extend(properties::scope_text_object(
                &bytes, &content, &spans, show,
            )?);
            font_plans.push((name, change.font));
        }
        let encoded = splice_content(&bytes, splices)?;
        let mut candidate = self.clone();
        for (name, font) in font_plans {
            let id = candidate.document.add_object(dictionary! {
                "Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>Object::Name(font.pdf_name().as_bytes().to_vec()),
                "Encoding"=>"WinAnsiEncoding"
            });
            fonts.set(name, id);
        }
        resources.set("Font", fonts);
        candidate
            .document
            .get_dictionary_mut(page_id)?
            .set("Resources", resources);
        candidate.publish_content_bytes(page_id, stream_id, encoded)?;
        *self = candidate;
        Ok(())
    }
}
