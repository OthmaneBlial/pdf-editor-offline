//! Scoped edits of the original final-show text object, never an overlay.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextPropertyChange {
    pub identity: ContentTextIdentity,
    pub expected: String,
    #[serde(serialize_with = "serialize_font_size")]
    pub font_size: Option<f32>,
    /// Replaces only e/f of the original text matrix, in PDF text space.
    pub matrix_translation: Option<[f32; 2]>,
    /// Explicit DeviceRGB fill, scoped to the selected original text object.
    pub fill_rgb: Option<[f32; 3]>,
}

// serde_json otherwise converts Some(NaN/Infinity) to null, laundering a rejected
// optional size into an absent property when other changes are present.
fn serialize_font_size<S: serde::Serializer>(
    value: &Option<f32>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(serde::ser::Error::custom("non-finite font size"));
    }
    value.serialize(serializer)
}

impl ContentTextRun {
    pub fn property_position(
        &self,
        change: &ContentTextPropertyChange,
    ) -> Result<ContentTextPosition> {
        if !self.properties_writable
            || self.identity != change.identity
            || self.text.as_deref() != Some(change.expected.as_str())
        {
            return Err(Error::Refused("property inventory is stale or unsupported"));
        }
        if change.font_size.is_none()
            && change.matrix_translation.is_none()
            && change.fill_rgb.is_none()
        {
            return Err(Error::Invalid("provide at least one text property"));
        }
        let mut layout = self
            .layout
            .clone()
            .ok_or(Error::Refused("bounded text layout is unavailable"))?;
        if let Some(size) = change.font_size {
            if !size.is_finite() || size <= 0.0 {
                return Err(Error::Invalid("font size must be finite and positive"));
            }
            layout.font_size = f64::from(size);
        }
        if let Some([x, y]) = change.matrix_translation {
            layout.text_matrix[4] = f64::from(x);
            layout.text_matrix[5] = f64::from(y);
        }
        if let Some(rgb) = change.fill_rgb
            && rgb
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(Error::Invalid(
                "RGB components must be finite and between zero and one",
            ));
        }
        layout.position(
            self.standard_font
                .ok_or(Error::Refused("standard font metrics unavailable"))?,
            self.encoding
                .ok_or(Error::Refused("text encoding unavailable"))?,
            &change.expected,
        )
    }
}

impl PdfDocument {
    pub(super) fn content_property_context(
        &self,
        page_id: ObjectId,
        content: &Content,
    ) -> Result<()> {
        let resources = structure::inherited(&self.document, page_id, b"Resources")?
            .map(Object::as_dict)
            .transpose()?;
        if self.document.catalog()?.has(b"OutputIntents")
            || resources.is_some_and(|r| r.has(b"ColorSpace") || r.has(b"ExtGState"))
            || content.operations.iter().any(|op| {
                matches!(
                    op.operator.as_str(),
                    "gs" | "cs" | "CS" | "sc" | "SC" | "scn" | "SCN" | "k" | "K"
                )
            })
        {
            return Err(Error::Refused(
                "text properties require ordinary DeviceGray/DeviceRGB without profiles or extended graphics state",
            ));
        }
        for op in &content.operations {
            if matches!(op.operator.as_str(), "g" | "G" | "rg" | "RG") {
                let count = if matches!(op.operator.as_str(), "g" | "G") {
                    1
                } else {
                    3
                };
                if op.operands.len() != count
                    || op
                        .operands
                        .iter()
                        .any(|v| layout::numeric(v).map_or(true, |n| !(0.0..=1.0).contains(&n)))
                {
                    return Err(Error::Invalid("malformed ordinary color state"));
                }
            }
        }
        Ok(())
    }
    /// Changes original Tf/Tm/fill state. Old stream objects remain: this is not secure deletion.
    pub fn update_content_text_properties(
        &mut self,
        page: u32,
        changes: &[ContentTextPropertyChange],
    ) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_RUNS {
            return Err(Error::Invalid("provide 1 through 10000 property changes"));
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
        let mut selected = BTreeSet::new();
        let mut plans = Vec::new();
        let mut blocks = BTreeSet::new();
        let mut total = 0usize;
        for change in changes {
            total = total.saturating_add(change.expected.len());
            if total > MAX_TEXT {
                return Err(Error::Limit("property requests exceed 4 MiB"));
            }
            if !selected.insert(change.identity.operation) {
                return Err(Error::Invalid("duplicate property target"));
            }
            let run = runs
                .get(&change.identity.operation)
                .ok_or(Error::Refused("property inventory is stale"))?;
            run.property_position(change)?;
            let show = change.identity.operation as usize;
            let start = content.operations[..show]
                .iter()
                .rposition(|op| op.operator == "BT")
                .ok_or(Error::Invalid("missing text object"))?;
            let end = show + 1;
            if !blocks.insert(start) {
                return Err(Error::Invalid("overlapping text objects"));
            }
            let mut insert = Vec::from(b"\n".as_slice());
            if let Some(size) = change.font_size {
                let font = content.operations[start..show]
                    .iter()
                    .rposition(|op| op.operator == "Tf")
                    .ok_or(Error::Invalid("missing font setter"))?;
                insert.extend_from_slice(&bytes[spans[start + font].operands[0].clone()]);
                insert.extend_from_slice(format!(" {size} Tf\n").as_bytes());
            }
            if let Some([x, y]) = change.matrix_translation {
                let matrix = content.operations[start..show]
                    .iter()
                    .rposition(|op| op.operator == "Tm")
                    .ok_or(Error::Invalid("missing text matrix"))?;
                for operand in &spans[start + matrix].operands[..4] {
                    insert.extend_from_slice(&bytes[operand.clone()]);
                    insert.push(b' ');
                }
                insert.extend_from_slice(format!("{x} {y} Tm\n").as_bytes());
            }
            if let Some([r, g, b]) = change.fill_rgb {
                insert.extend_from_slice(format!("{r} {g} {b} rg\n").as_bytes());
            }
            // ET discards matrices, but font and text-state setters persist. Replay exactly
            // the original raw setters after restoring scoped color/graphics state.
            let mut restore = Vec::from(b"\nQ\n".as_slice());
            for index in start..=end {
                if matches!(
                    content.operations[index].operator.as_str(),
                    "Tf" | "Tr" | "Tc" | "Tw" | "Tz" | "Ts" | "g" | "rg"
                ) {
                    restore.extend_from_slice(&bytes[spans[index].full.clone()]);
                    restore.push(b'\n');
                }
            }
            let begin = spans[start].full.start;
            let show = spans[show].full.start;
            let finish = spans[end].full.end;
            plans.push((begin..begin, b"\nq\n".to_vec()));
            plans.push((show..show, insert));
            plans.push((finish..finish, restore));
        }
        self.publish_content_bytes(page_id, stream_id, splice_content(&bytes, plans)?)
    }
}
