//! Bounded AcroForm discovery and transactional supported form filling. No scripts run.
//! Text/choice appearances and page overlays share font/layout helpers; XFA is unsupported.
//! Field/reference depth is limited to 128, traversal to two million lookups,
//! and the graph to 100,000 nodes / 10,000 terminal fields. Names, values and
//! intermediate inherited-name copies share a 4 MiB text budget. Unique button
//! appearance decoding is capped at 32 MiB per stream and 64 MiB in aggregate.
//! Appearance validation checks references, Form type, finite geometry and
//! bounded decoding; it is not a proof of every font or drawing operator.
use crate::{Error, PdfDocument, Result};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod appearance;
pub use appearance::TextChange;
mod image_instances;
pub use image_instances::{ImageInstance, ImageSelection};
mod image_overlay;
pub use image_overlay::ImageOverlay;
mod numbering;
mod overlay;
pub use numbering::{NumberPosition, PageNumbering};
pub use overlay::{TextAlignment, TextOverlay};
mod vector;
pub use vector::{MAX_VECTOR_POINTS, MAX_VECTOR_STROKES, VectorOverlay, VectorShape, VectorStroke};
mod choice;
pub use choice::{ChoiceChange, ChoiceField, ChoiceOption};

const MAX_NODES: usize = 100_000;
const MAX_FIELDS: usize = 10_000;
const MAX_WORK: usize = 2_000_000;
const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;
const MAX_VALUE_BYTES: usize = 64 * 1024;
const RADIO: u32 = 1 << 15;
const PUSH: u32 = 1 << 16;
const UNISON: u32 = 1 << 25;
const NO_TOGGLE: u32 = 1 << 14;
const UNSUPPORTED_FIELD_KIND: &str =
    "This field type needs a later appearance-generation or signing stage";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormFieldKind {
    Text,
    CheckBox,
    RadioButton,
    PushButton,
    Choice,
    Signature,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FormValue {
    Empty,
    Text(String),
    Name(Vec<u8>),
    TextArray(Vec<String>),
    Unsupported,
    PasswordOmitted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormWidget {
    pub object_id: ObjectId,
    pub page: u32,
    pub rectangle: [f32; 4],
    /// PDF appearance names are arbitrary bytes, not Unicode text strings.
    pub appearance_state: Option<Vec<u8>>,
    pub on_state: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormField {
    pub object_id: ObjectId,
    pub name: String,
    pub kind: FormFieldKind,
    pub flags: u32,
    pub value: FormValue,
    pub widgets: Vec<FormWidget>,
    pub writable_button: bool,
    /// Button-tier refusal; text editing uses `text_refusal_reason` independently.
    pub refusal_reason: Option<String>,
    /// Structural eligibility only; explicit font coverage and layout are checked on fill.
    pub writable_text: bool,
    pub text_refusal_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ButtonSelection {
    Off,
    /// Select this widget's existing on-state. Distinguishes radio widgets
    /// whose export names happen to be identical without RadiosInUnison.
    On {
        widget: ObjectId,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonChange {
    pub name: String,
    pub selection: ButtonSelection,
}

struct Budget {
    work: usize,
    report: usize,
    appearance_bytes: usize,
    streams: BTreeSet<ObjectId>,
}
impl Budget {
    fn new() -> Self {
        Self {
            work: 0,
            report: 0,
            appearance_bytes: 0,
            streams: BTreeSet::new(),
        }
    }
    fn work(&mut self, amount: usize) -> Result<()> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(Error::Limit("form work budget overflow"))?;
        if self.work > MAX_WORK {
            return Err(Error::Limit("form traversal exceeds two million values"));
        }
        Ok(())
    }
    fn report(&mut self, amount: usize) -> Result<()> {
        self.report = self
            .report
            .checked_add(amount)
            .ok_or(Error::Limit("form report budget overflow"))?;
        if self.report > MAX_REPORT_BYTES {
            return Err(Error::Limit("form names and values exceed 4 MiB"));
        }
        Ok(())
    }
    fn resolve<'a>(
        &mut self,
        document: &'a Document,
        mut object: &'a Object,
    ) -> Result<(Option<ObjectId>, &'a Object)> {
        let mut seen = BTreeSet::new();
        let mut id = None;
        loop {
            self.work(1)?;
            let Object::Reference(reference) = object else {
                return Ok((id, object));
            };
            if !seen.insert(*reference) {
                return Err(Error::Invalid("cyclic form reference"));
            }
            if seen.len() > 128 {
                return Err(Error::Limit("form reference exceeds 128 levels"));
            }
            id = Some(*reference);
            object = document
                .objects
                .get(reference)
                .ok_or(Error::Invalid("missing form reference"))?;
        }
    }
}

impl PdfDocument {
    /// Effective field names/types/flags/values and existing widget placements.
    /// This validates the field graph; it does not execute calculations or XFA.
    pub fn list_form_fields(&self) -> Result<Vec<FormField>> {
        discover(self)
    }

    /// Fill supported checkbox/radio fields as one transaction. Appearance
    /// streams and unrelated objects stay unchanged. This is ordinary editing,
    /// not removal of old values from previous copies or revisions.
    pub fn fill_buttons(&mut self, changes: &[ButtonChange]) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_FIELDS {
            return Err(Error::Refused("select between one and 10000 button fields"));
        }
        let fields = self.list_form_fields()?;
        let mut updates = Vec::new();
        let mut names = BTreeSet::new();
        for change in changes {
            if !names.insert(change.name.as_str()) {
                return Err(Error::Refused("duplicate field in button transaction"));
            }
            let mut matches = fields.iter().filter(|field| field.name == change.name);
            let field = matches
                .next()
                .ok_or(Error::Refused("button field name not found"))?;
            if matches.next().is_some() {
                return Err(Error::Refused(
                    "ambiguous form field name requires grouped-field support",
                ));
            }
            if !field.writable_button {
                return Err(Error::Refused(
                    "field cannot be filled safely in the current button tier",
                ));
            }
            let selected = match change.selection {
                ButtonSelection::Off => {
                    if field.flags & 2 != 0
                        || (field.kind == FormFieldKind::RadioButton
                            && field.flags & NO_TOGGLE != 0)
                    {
                        return Err(Error::Refused(
                            "required or NoToggleToOff button cannot be cleared",
                        ));
                    }
                    None
                }
                ButtonSelection::On { widget } => Some(
                    field
                        .widgets
                        .iter()
                        .find(|entry| entry.object_id == widget)
                        .ok_or(Error::Refused(
                            "selected widget does not belong to the button field",
                        ))?,
                ),
            };
            let value = selected
                .and_then(|widget| widget.on_state.as_deref())
                .unwrap_or(b"Off")
                .to_vec();
            updates.push((field.object_id, b"V".to_vec(), value.clone()));
            for widget in &field.widgets {
                let on = selected.is_some_and(|selected| {
                    if field.kind == FormFieldKind::RadioButton && field.flags & UNISON == 0 {
                        widget.object_id == selected.object_id
                    } else {
                        widget.on_state.as_deref() == Some(value.as_slice())
                    }
                });
                updates.push((
                    widget.object_id,
                    b"AS".to_vec(),
                    if on { value.clone() } else { b"Off".to_vec() },
                ));
            }
        }
        let mut candidate = self.clone();
        for (id, key, value) in updates {
            candidate
                .document
                .get_dictionary_mut(id)?
                .set(key, Object::Name(value));
        }
        let candidate = Self::from_document(candidate.document, candidate.encrypted_input)?;
        // Revalidate every affected value/appearance relationship before commit.
        let after = candidate.list_form_fields()?;
        for change in changes {
            if !after
                .iter()
                .any(|field| field.name == change.name && field.writable_button)
            {
                return Err(Error::Verification(
                    "filled button relationship did not validate",
                ));
            }
        }
        *self = candidate;
        Ok(())
    }
}

fn inherited<'a>(
    document: &'a Document,
    id: ObjectId,
    key: &[u8],
    budget: &mut Budget,
) -> Result<Option<&'a Object>> {
    let mut current = id;
    let mut seen = BTreeSet::new();
    loop {
        budget.work(1)?;
        if !seen.insert(current) {
            return Err(Error::Invalid("cyclic field parent"));
        }
        if seen.len() > 128 {
            return Err(Error::Limit("field inheritance exceeds 128 levels"));
        }
        let dictionary = document
            .objects
            .get(&current)
            .ok_or(Error::Invalid("missing field parent"))?
            .as_dict()?;
        if let Ok(value) = dictionary.get(key) {
            return Ok(Some(budget.resolve(document, value)?.1));
        }
        let Ok(parent) = dictionary.get(b"Parent") else {
            return Ok(None);
        };
        current = budget
            .resolve(document, parent)?
            .0
            .ok_or(Error::Invalid("direct form parent"))?;
    }
}

fn text(value: &Object, budget: &mut Budget) -> Result<String> {
    let bytes = value.as_str()?;
    if bytes.len() > MAX_VALUE_BYTES * 2 + 2 {
        return Err(Error::Limit("encoded form string exceeds 128 KiB"));
    }
    let string = crate::strings::decode(value)?;
    if string.len() > MAX_VALUE_BYTES {
        return Err(Error::Limit("form string exceeds 64 KiB"));
    }
    budget.report(string.len())?;
    Ok(string)
}

fn value(
    document: &Document,
    object: Option<&Object>,
    password: bool,
    budget: &mut Budget,
) -> Result<FormValue> {
    if password {
        return Ok(FormValue::PasswordOmitted);
    }
    match object {
        None | Some(Object::Null) => Ok(FormValue::Empty),
        Some(Object::String(..)) => Ok(FormValue::Text(text(
            object.ok_or(Error::Invalid("missing value"))?,
            budget,
        )?)),
        Some(Object::Name(name)) => {
            budget.report(name.len())?;
            Ok(FormValue::Name(name.clone()))
        }
        Some(Object::Array(values)) => {
            if values.len() > MAX_FIELDS {
                return Err(Error::Limit("form value array exceeds 10000 entries"));
            }
            budget.work(values.len())?;
            let mut strings = Vec::new();
            for value in values {
                let value = budget.resolve(document, value)?.1;
                if !matches!(value, Object::String(..)) {
                    return Ok(FormValue::Unsupported);
                }
                strings.push(text(value, budget)?);
            }
            Ok(FormValue::TextArray(strings))
        }
        _ => Ok(FormValue::Unsupported),
    }
}

fn numeric<const N: usize>(
    document: &Document,
    object: &Object,
    budget: &mut Budget,
) -> Result<[f32; N]> {
    let array = budget.resolve(document, object)?.1.as_array()?;
    if array.len() != N {
        return Err(Error::Invalid("form geometry has incorrect arity"));
    }
    let mut output = [0.0; N];
    for (slot, value) in output.iter_mut().zip(array) {
        *slot = budget.resolve(document, value)?.1.as_float()?;
        if !slot.is_finite() {
            return Err(Error::Invalid("nonfinite form geometry"));
        }
    }
    Ok(output)
}

fn appearance(document: &Document, object: &Object, budget: &mut Budget) -> Result<()> {
    let (id, object) = budget.resolve(document, object)?;
    let id = id.ok_or(Error::Refused("appearance streams must be indirect"))?;
    let stream = object.as_stream()?;
    if budget
        .resolve(document, stream.dict.get(b"Subtype")?)?
        .1
        .as_name()?
        != b"Form"
    {
        return Err(Error::Invalid("button appearance is not a Form XObject"));
    }
    let bbox = numeric::<4>(document, stream.dict.get(b"BBox")?, budget)?;
    if !(bbox[2] > bbox[0] && bbox[3] > bbox[1]) {
        return Err(Error::Invalid("empty button appearance bounds"));
    }
    if let Ok(matrix) = stream.dict.get(b"Matrix") {
        let m = numeric::<6>(document, matrix, budget)?;
        let determinant = f64::from(m[0]) * f64::from(m[3]) - f64::from(m[1]) * f64::from(m[2]);
        if determinant == 0.0 {
            return Err(Error::Invalid("singular button appearance matrix"));
        }
        for x in [bbox[0], bbox[2]] {
            for y in [bbox[1], bbox[3]] {
                let tx = (f64::from(m[0]) * f64::from(x)
                    + f64::from(m[2]) * f64::from(y)
                    + f64::from(m[4])) as f32;
                let ty = (f64::from(m[1]) * f64::from(x)
                    + f64::from(m[3]) * f64::from(y)
                    + f64::from(m[5])) as f32;
                if !tx.is_finite() || !ty.is_finite() {
                    return Err(Error::Invalid("button appearance matrix overflows"));
                }
            }
        }
    }
    if let Ok(resources) = stream.dict.get(b"Resources") {
        budget.resolve(document, resources)?.1.as_dict()?;
    }
    if !budget.streams.contains(&id) {
        let remaining = (64 * 1024 * 1024usize)
            .saturating_sub(budget.appearance_bytes)
            .min(crate::MAX_STREAM_BYTES);
        let decoded =
            stream
                .get_plain_content_with_limit(remaining)
                .map_err(|error| match error {
                    lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded {
                        ..
                    }) => Error::Limit("button appearances exceed decompression budget"),
                    _ => Error::Refused("button appearance content cannot be decoded safely"),
                })?;
        budget.appearance_bytes += decoded.len();
        budget.streams.insert(id);
    }
    Ok(())
}

fn form_readonly(
    pdf: &PdfDocument,
    document: &Document,
    form: &Dictionary,
    catalog: &Dictionary,
    budget: &mut Budget,
) -> Result<Option<&'static str>> {
    let document_scripts = catalog
        .get(b"Names")
        .ok()
        .map(|names| {
            budget
                .resolve(document, names)
                .and_then(|(_, names)| Ok(names.as_dict()?.has(b"JavaScript")))
        })
        .transpose()?
        .unwrap_or(false);
    let page_actions = pdf.pages.iter().any(|id| {
        document
            .objects
            .get(id)
            .and_then(|object| object.as_dict().ok())
            .is_some_and(|page| page.has(b"AA"))
    });
    let readonly = if pdf.encrypted_input {
        Some("Encrypted input is read-only")
    } else if pdf.info()?.signature_fields > 0 {
        Some("Signature-bearing input is read-only")
    } else if form.has(b"XFA") {
        Some("XFA requires a separate filling implementation")
    } else if form.has(b"CO")
        || catalog.has(b"AA")
        || catalog.has(b"OpenAction")
        || document_scripts
        || page_actions
    {
        Some("Form calculations or automatic document actions are unsupported")
    } else if form
        .get(b"NeedAppearances")
        .ok()
        .map(|value| {
            budget
                .resolve(document, value)
                .and_then(|(_, value)| Ok(value.as_bool()?))
        })
        .transpose()?
        .unwrap_or(false)
    {
        Some("NeedAppearances documents require complete appearance regeneration")
    } else {
        None
    };
    Ok(readonly)
}

fn discover(pdf: &PdfDocument) -> Result<Vec<FormField>> {
    let document = &pdf.document;
    let mut budget = Budget::new();
    let catalog = budget
        .resolve(document, document.trailer.get(b"Root")?)?
        .1
        .as_dict()?;
    let Ok(form) = catalog.get(b"AcroForm") else {
        return Ok(Vec::new());
    };
    let form = budget.resolve(document, form)?.1.as_dict()?;
    let readonly = form_readonly(pdf, document, form, catalog, &mut budget)?;
    let roots = budget
        .resolve(document, form.get(b"Fields")?)?
        .1
        .as_array()?;
    if roots.len() > MAX_NODES {
        return Err(Error::Limit("more than 100000 form roots"));
    }
    let mut pending = Vec::new();
    for root in roots.iter().rev() {
        let id = budget
            .resolve(document, root)?
            .0
            .ok_or(Error::Invalid("direct field dictionary"))?;
        pending.push((id, None, String::new(), 0usize));
    }
    let mut seen = BTreeSet::new();
    let mut fields = Vec::new();
    let placements = placements(pdf, &mut budget)?;
    while let Some((id, parent, prefix, depth)) = pending.pop() {
        if depth >= 128 {
            return Err(Error::Limit("field tree exceeds 128 levels"));
        }
        if !seen.insert(id) {
            return Err(Error::Invalid("cyclic or multiply referenced field tree"));
        }
        if seen.len() > MAX_NODES {
            return Err(Error::Limit("more than 100000 field tree nodes"));
        }
        let dictionary = document
            .objects
            .get(&id)
            .ok_or(Error::Invalid("missing field object"))?
            .as_dict()?;
        let actual_parent = dictionary
            .get(b"Parent")
            .ok()
            .map(|object| {
                budget
                    .resolve(document, object)
                    .and_then(|(id, _)| id.ok_or(Error::Invalid("direct form parent")))
            })
            .transpose()?;
        if actual_parent != parent {
            return Err(Error::Invalid("field parent and Kids edge disagree"));
        }
        let part = dictionary
            .get(b"T")
            .ok()
            .map(|value| {
                budget
                    .resolve(document, value)
                    .and_then(|(_, value)| text(value, &mut budget))
            })
            .transpose()?
            .unwrap_or_default();
        if part.contains('.') || part.contains('\0') {
            return Err(Error::Refused(
                "partial field names containing periods or NUL are unsupported",
            ));
        }
        let name = if prefix.is_empty() {
            part
        } else if part.is_empty() {
            prefix
        } else {
            format!("{prefix}.{part}")
        };
        if name.len() > 4096 {
            return Err(Error::Limit("full field name exceeds 4096 bytes"));
        }
        let kids = dictionary
            .get(b"Kids")
            .ok()
            .map(|object| {
                budget
                    .resolve(document, object)
                    .and_then(|(_, object)| Ok(object.as_array()?.as_slice()))
            })
            .transpose()?
            .unwrap_or(&[]);
        let merged = dictionary
            .get(b"Subtype")
            .ok()
            .map(|value| {
                budget
                    .resolve(document, value)
                    .and_then(|(_, value)| Ok(value.as_name()? == b"Widget"))
            })
            .transpose()?
            .unwrap_or(false);
        if merged && !kids.is_empty() {
            return Err(Error::Invalid("merged field widget cannot have Kids"));
        }
        if kids.len() > MAX_NODES.saturating_sub(seen.len() + pending.len()) {
            return Err(Error::Limit("form pending nodes exceed budget"));
        }
        let mut child_fields = Vec::new();
        let mut widgets = Vec::new();
        for child in kids {
            let (child_id, object) = budget.resolve(document, child)?;
            let child_id = child_id.ok_or(Error::Invalid("direct field child"))?;
            let dict = object.as_dict()?;
            let widget = dict
                .get(b"Subtype")
                .ok()
                .map(|value| {
                    budget
                        .resolve(document, value)
                        .and_then(|(_, value)| Ok(value.as_name()? == b"Widget"))
                })
                .transpose()?
                .unwrap_or(false);
            if widget && !dict.has(b"T") {
                widgets.push(child_id);
            } else {
                child_fields.push(child_id);
            }
        }
        if !child_fields.is_empty() {
            if !widgets.is_empty() {
                return Err(Error::Invalid("mixed widget and field children"));
            }
            budget.report(
                name.len()
                    .checked_mul(child_fields.len())
                    .ok_or(Error::Limit("form inherited names exceed budget"))?,
            )?;
            for child in child_fields.into_iter().rev() {
                pending.push((child, Some(id), name.clone(), depth + 1));
            }
            continue;
        }
        if fields.len() >= MAX_FIELDS {
            return Err(Error::Limit("more than 10000 terminal fields"));
        }
        if merged {
            if !widgets.is_empty() {
                return Err(Error::Invalid("merged widget has widget children"));
            }
            widgets.push(id);
        }
        let flags = inherited(document, id, b"Ff", &mut budget)?
            .map(Object::as_i64)
            .transpose()?
            .unwrap_or(0);
        let flags = u32::try_from(flags)
            .map_err(|_| Error::Invalid("field flags outside unsigned 32-bit range"))?;
        let ft = inherited(document, id, b"FT", &mut budget)?
            .ok_or(Error::Invalid("terminal field has no inherited type"))?
            .as_name()?;
        let kind = match ft {
            b"Tx" => FormFieldKind::Text,
            b"Btn" if flags & PUSH != 0 => FormFieldKind::PushButton,
            b"Btn" if flags & RADIO != 0 => FormFieldKind::RadioButton,
            b"Btn" => FormFieldKind::CheckBox,
            b"Ch" => FormFieldKind::Choice,
            b"Sig" => FormFieldKind::Signature,
            _ => FormFieldKind::Other,
        };
        let current = value(
            document,
            inherited(document, id, b"V", &mut budget)?,
            ft == b"Tx" && flags & (1 << 13) != 0,
            &mut budget,
        )?;
        let button = matches!(kind, FormFieldKind::CheckBox | FormFieldKind::RadioButton);
        let is_text = kind == FormFieldKind::Text;
        let mut reason = readonly.or(if flags & 1 != 0 {
            Some("ReadOnly field")
        } else if !button && kind != FormFieldKind::Text {
            Some("This field type needs a later appearance-generation or signing stage")
        } else if button && widgets.is_empty() {
            Some("Button has no visible widget")
        } else if button && dictionary.has(b"Opt") {
            Some("Button Opt export mapping is unsupported")
        } else {
            None
        });
        if name.is_empty() {
            reason = Some("Unnamed field cannot be selected safely");
        }
        let mut entries = Vec::new();
        for widget in widgets {
            budget.work(1)?;
            if widget != id && !seen.insert(widget) {
                return Err(Error::Invalid("shared or cyclic widget child"));
            }
            if seen.len() > MAX_NODES {
                return Err(Error::Limit(
                    "more than 100000 form nodes including widgets",
                ));
            }
            let dict = document
                .objects
                .get(&widget)
                .ok_or(Error::Invalid("missing widget"))?
                .as_dict()?;
            if widget != id {
                let parent = budget.resolve(document, dict.get(b"Parent")?)?.0;
                if parent != Some(id) || dict.has(b"Kids") {
                    return Err(Error::Invalid("widget parent or Kids edge is invalid"));
                }
                if [b"FT".as_slice(), b"Ff", b"V", b"DV"]
                    .iter()
                    .any(|key| dict.has(key))
                {
                    reason = Some("Widget field attributes require grouped-field support");
                }
            }
            let page = *placements
                .get(&widget)
                .ok_or(Error::Invalid("widget is absent from page annotations"))?;
            if let Ok(p) = dict.get(b"P")
                && budget.resolve(document, p)?.0 != Some(pdf.pages[page as usize - 1])
            {
                return Err(Error::Invalid(
                    "widget page reference disagrees with annotation placement",
                ));
            }
            let rectangle = numeric::<4>(document, dict.get(b"Rect")?, &mut budget)?;
            if !(rectangle[2] > rectangle[0] && rectangle[3] > rectangle[1]) {
                return Err(Error::Invalid("empty widget rectangle"));
            }
            let state = dict
                .get(b"AS")
                .ok()
                .map(|object| {
                    budget.resolve(document, object).and_then(|(_, object)| {
                        let state = object.as_name()?;
                        if state.len() > 1024 {
                            return Err(Error::Limit("widget appearance state exceeds 1024 bytes"));
                        }
                        Ok(state.to_vec())
                    })
                })
                .transpose()?;
            let mut on_state = None;
            if button {
                match button_appearance(document, dict, &mut budget) {
                    Ok(on) => on_state = Some(on),
                    Err(Error::Limit(message)) => return Err(Error::Limit(message)),
                    Err(_) => reason = Some("Button lacks valid Off/on appearance streams"),
                }
            }
            let annotation_flags = dict
                .get(b"F")
                .ok()
                .map(|object| {
                    budget
                        .resolve(document, object)
                        .and_then(|(_, object)| Ok(object.as_i64()?))
                })
                .transpose()?
                .unwrap_or(0);
            if annotation_flags & (128 | 512) != 0 {
                reason = Some("Locked widget");
            }
            if dict.has(b"A") || dict.has(b"AA") {
                reason = Some("Widget actions require a separate semantics implementation");
            }
            if dict.has(b"StructParent") {
                reason = Some(
                    "Tagged widget accessibility state requires a separate update implementation",
                );
            }
            budget.report(
                state.as_ref().map_or(0, Vec::len) + on_state.as_ref().map_or(0, Vec::len),
            )?;
            entries.push(FormWidget {
                object_id: widget,
                page,
                rectangle,
                appearance_state: state,
                on_state,
            });
        }
        if inherited(document, id, b"AA", &mut budget)?.is_some()
            || inherited(document, id, b"A", &mut budget)?.is_some()
        {
            reason = Some("Field actions require a separate semantics implementation");
        }
        if button
            && !relationship(
                &current,
                &entries,
                kind == FormFieldKind::RadioButton,
                flags & UNISON != 0,
            )
        {
            reason = Some("Button V and widget AS are inconsistent");
        }
        budget.report(name.len())?;
        let mut field = FormField {
            object_id: id,
            name,
            kind,
            flags,
            value: current,
            widgets: entries,
            writable_button: button && reason.is_none(),
            refusal_reason: reason
                .or(if !button {
                    Some(UNSUPPORTED_FIELD_KIND)
                } else {
                    None
                })
                .map(str::to_owned),
            writable_text: false,
            text_refusal_reason: if is_text {
                reason.map(str::to_owned)
            } else {
                Some("This field is not a text field".into())
            },
        };
        if is_text && field.text_refusal_reason.is_none() {
            match appearance::text_eligibility(pdf, &field, &mut budget) {
                Ok(()) => field.writable_text = true,
                Err(error @ Error::Limit(_)) => return Err(error),
                Err(error) => {
                    let message = error.to_string();
                    budget.report(message.len())?;
                    field.text_refusal_reason = Some(message);
                }
            }
        }
        fields.push(field);
    }
    if placements.keys().any(|id| !seen.contains(id)) {
        return Err(Error::Invalid(
            "page widget is not represented by the field tree",
        ));
    }
    let mut counts = BTreeMap::new();
    for field in &fields {
        *counts.entry(field.name.as_str()).or_insert(0usize) += 1;
    }
    let ambiguous: BTreeSet<String> = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, _)| name.to_owned())
        .collect();
    for field in &mut fields {
        if ambiguous.contains(&field.name) {
            field.writable_button = false;
            field.writable_text = false;
            field.text_refusal_reason =
                Some("Duplicate qualified names require grouped-field support".into());
            field.refusal_reason =
                Some("Duplicate qualified names require grouped-field support".into());
        }
    }
    Ok(fields)
}

fn button_appearance(
    document: &Document,
    widget: &Dictionary,
    budget: &mut Budget,
) -> Result<Vec<u8>> {
    let ap = budget.resolve(document, widget.get(b"AP")?)?.1.as_dict()?;
    let normal = budget.resolve(document, ap.get(b"N")?)?.1.as_dict()?;
    if normal.len() != 2 || !normal.has(b"Off") {
        return Err(Error::Refused(
            "button needs exactly Off and one on appearance",
        ));
    }
    let mut on = None;
    for (name, stream) in normal.iter() {
        if name.is_empty() || name.len() > 1024 {
            return Err(Error::Limit("appearance state name exceeds limit"));
        }
        appearance(document, stream, budget)?;
        if name != b"Off" {
            on = Some(name.clone());
        }
    }
    on.ok_or(Error::Invalid("missing button on state"))
}

fn relationship(value: &FormValue, widgets: &[FormWidget], radio: bool, unison: bool) -> bool {
    let name = match value {
        FormValue::Empty => b"Off".as_slice(),
        FormValue::Name(name) => name.as_slice(),
        _ => return false,
    };
    let mut on = 0;
    for widget in widgets {
        let Some(state) = widget.appearance_state.as_deref() else {
            return false;
        };
        if state != b"Off" {
            if Some(state) != widget.on_state.as_deref() || state != name {
                return false;
            }
            on += 1;
        }
        if (!radio || unison) && widget.on_state.as_deref() == Some(name) && state != name {
            return false;
        }
    }
    if name == b"Off" {
        on == 0
    } else {
        on > 0 && (!radio || unison || on == 1)
    }
}

fn placements(pdf: &PdfDocument, budget: &mut Budget) -> Result<BTreeMap<ObjectId, u32>> {
    let mut placements = BTreeMap::new();
    for (index, page) in pdf.pages.iter().enumerate() {
        let dictionary = pdf
            .document
            .objects
            .get(page)
            .ok_or(Error::Invalid("missing page"))?
            .as_dict()?;
        let Ok(annotations) = dictionary.get(b"Annots") else {
            continue;
        };
        let entries = budget.resolve(&pdf.document, annotations)?.1.as_array()?;
        if entries.len() > MAX_NODES {
            return Err(Error::Limit("too many page annotations for form discovery"));
        }
        for object in entries {
            let (id, object) = budget.resolve(&pdf.document, object)?;
            let dictionary = object.as_dict()?;
            let widget = dictionary
                .get(b"Subtype")
                .ok()
                .map(|value| {
                    budget
                        .resolve(&pdf.document, value)
                        .and_then(|(_, object)| Ok(object.as_name()? == b"Widget"))
                })
                .transpose()?
                .unwrap_or(false);
            if widget {
                let id = id.ok_or(Error::Refused(
                    "direct widgets require normalization before form filling",
                ))?;
                if placements.insert(id, index as u32 + 1).is_some() {
                    return Err(Error::Invalid(
                        "widget has multiple page annotation occurrences",
                    ));
                }
                if placements.len() > MAX_NODES {
                    return Err(Error::Limit("more than 100000 widget placements"));
                }
            }
        }
    }
    Ok(placements)
}
