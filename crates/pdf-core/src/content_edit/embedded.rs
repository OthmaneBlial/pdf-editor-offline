//! Complete TrueType first tier; positioned glyph runs are a separate extension.
use super::*;
use crate::forms::content_font::ContentFontPlan;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextEmbeddedChange {
    pub identity: ContentTextIdentity,
    pub expected: String,
    pub replacement: Option<String>,
    #[serde(default)]
    pub layout_policy: ContentTextLayoutPolicy,
}

impl PdfDocument {
    /// A real CID font/string change of one original final show. This first tier
    /// requires explicit width change, zero Tc/Tw and unpositioned logical glyphs.
    pub fn change_content_text_embedded_font(
        &mut self,
        page: u32,
        change: &ContentTextEmbeddedChange,
        font_bytes: &[u8],
    ) -> Result<()> {
        self.ensure_editable()?;
        if !matches!(
            change.layout_policy,
            ContentTextLayoutPolicy::AllowWidthChange
        ) {
            return Err(Error::Refused(
                "embedded font changes require explicit width-change policy in this tier",
            ));
        }
        let run = self
            .content_text_runs(page)?
            .into_iter()
            .find(|run| run.identity == change.identity)
            .ok_or(Error::Refused("embedded font inventory is stale"))?;
        if !run.properties_writable || run.text.as_deref() != Some(change.expected.as_str()) {
            return Err(Error::Refused(
                "embedded font target is unsupported or its expected text changed",
            ));
        }
        let value = change.replacement.as_deref().unwrap_or(&change.expected);
        if change.expected.len().saturating_add(value.len()) > MAX_TEXT {
            return Err(Error::Limit("embedded font text budget"));
        }
        let layout = run
            .layout
            .as_ref()
            .ok_or(Error::Refused("bounded original layout unavailable"))?;
        let mut plan = ContentFontPlan::new(font_bytes)?;
        let (show_operand, widths, advances) = plan.show(value)?;
        if matches!(show_operand, Object::String(..)) {
            layout.position_widths(&widths)?;
        } else {
            layout.position_advances(&advances)?;
        }
        let PageContent {
            page_id,
            stream_id,
            bytes,
            content,
            spans,
        } = single_content(self, page)?;
        self.content_property_context(page_id, &content)?;
        let (mut resources, mut fonts) = crate::forms::content_font_resources(self, page_id)?;
        // Seven embedding objects plus the cloned page stream; existing program
        // reuse may need fewer. Reserve the full bound before any candidate work.
        if self.document.objects.len().saturating_add(8) > MAX_OBJECTS
            || (self.document.max_id as usize).saturating_add(8) >= MAX_OBJECTS
        {
            return Err(Error::Limit("embedded content font object budget"));
        }
        let name = face::fresh_font_name(&fonts, &mut 0)?;
        let show = change.identity.operation as usize;
        let start = content.operations[..show]
            .iter()
            .rposition(|op| op.operator == "BT")
            .ok_or(Error::Invalid("missing original text object"))?;
        let setter = start
            + content.operations[start..show]
                .iter()
                .rposition(|op| op.operator == "Tf")
                .ok_or(Error::Invalid("missing original font setter"))?;
        let mut insert = format!("\n/{} ", String::from_utf8_lossy(&name)).into_bytes();
        insert.extend_from_slice(&bytes[spans[setter].operands[1].clone()]);
        insert.extend_from_slice(b" Tf\n");
        // Only the selected encoded string changes; every other source byte survives.
        let operator = if matches!(show_operand, Object::Array(_)) {
            "TJ"
        } else {
            "Tj"
        };
        let operand = lopdf::content::Content {
            operations: vec![lopdf::content::Operation::new(operator, vec![show_operand])],
        }
        .encode()?;
        let (parsed, token) = crate::content_spans::decode_spanned(&operand)?;
        if parsed.operations.len() != 1 {
            return Err(Error::Invalid("invalid generated direct show"));
        }
        let mut splices = properties::scope_text_object(&bytes, &content, &spans, show)?;
        splices.push((spans[show].full.start..spans[show].full.start, insert));
        splices.push((
            spans[show].operands[0].clone(),
            operand[token[0].operands[0].clone()].to_vec(),
        ));
        splices.push((spans[show].operator.clone(), operator.as_bytes().to_vec()));
        let encoded = splice_content(&bytes, splices)?;
        let mut candidate = self.clone();
        crate::require_pdf_1_4(&mut candidate.document);
        let font = plan.embed(&mut candidate.document)?;
        fonts.set(name, font);
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
