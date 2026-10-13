//! Index-based choices: paired labels and duplicate export values are never guessed.
use super::appearance::fonts::{FontPlan, Line, unicode_string};
use super::appearance::{self, FieldPatch};
use super::{Budget, FormField, FormFieldKind, FormValue, MAX_FIELDS, inherited, text, value};
use crate::{Error, PdfDocument, Result};
use lopdf::{Object, ObjectId, content::Operation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    pub index: u32,
    pub export: String,
    pub display: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceField {
    pub field: FormField,
    pub options: Vec<ChoiceOption>,
    /// None means the stored value/indices cannot identify a consistent selection.
    pub selected_indices: Option<Vec<u32>>,
    pub default_value: FormValue,
    pub top_index: u32,
    pub combo: bool,
    pub multi_select: bool,
    pub editable_combo: bool,
    pub writable: bool,
    pub refusal_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceChange {
    pub name: String,
    /// Strictly increasing zero-based Opt indices; duplicates have distinct indices.
    pub indices: Vec<u32>,
    /// List scroll position. A combo cannot specify a top index.
    pub top_index: Option<u32>,
}

fn selected(
    options: &[ChoiceOption],
    current: &FormValue,
    indices: Option<Vec<u32>>,
) -> Option<Vec<u32>> {
    let values: Vec<&str> = match current {
        FormValue::Empty => vec![],
        FormValue::Text(value)
            if value.is_empty()
                && !options
                    .iter()
                    .any(|option| option.export.is_empty() || option.display.is_empty()) =>
        {
            vec![]
        }
        FormValue::Text(value) => vec![value],
        FormValue::TextArray(values) => values.iter().map(String::as_str).collect(),
        _ => return None,
    };
    let mut exports: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    let mut displays: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for option in options {
        exports
            .entry(&option.export)
            .or_default()
            .push(option.index);
        displays
            .entry(&option.display)
            .or_default()
            .push(option.index);
    }
    if let Some(indices) = indices {
        if indices.len() != values.len()
            || indices.iter().zip(&values).any(|(index, value)| {
                let option = &options[*index as usize];
                if exports.contains_key(value) {
                    option.export != *value
                } else {
                    option.display != *value
                }
            })
        {
            return None;
        }
        return Some(indices);
    }
    let mut result = BTreeSet::new();
    for value in values {
        let matches = exports.get(value).or_else(|| displays.get(value))?;
        if matches.len() != 1 || !result.insert(matches[0]) {
            return None;
        }
    }
    Some(result.into_iter().collect())
}

struct ListCell {
    line: Line,
    value: String,
    selected: bool,
}
struct ListLayout {
    cells: Vec<ListCell>,
    size: f64,
    row_height: f64,
}

fn list_operations(
    plan: &appearance::WidgetPlan,
    font_name: &[u8],
    list: &ListLayout,
) -> Result<Vec<Operation>> {
    let mut operations = vec![Operation::new("q", vec![])];
    let width = plan.width - 2.0 * plan.padding;
    for (index, cell) in list.cells.iter().enumerate() {
        let y = plan.height - plan.padding - (index + 1) as f64 * list.row_height;
        if cell.selected {
            operations.extend([
                Operation::new("q", vec![]),
                Operation::new("rg", vec![0.153.into(), 0.388.into(), 0.725.into()]),
                Operation::new(
                    "re",
                    vec![
                        (plan.padding as f32).into(),
                        (y as f32).into(),
                        (width as f32).into(),
                        (list.row_height as f32).into(),
                    ],
                ),
                Operation::new("f", vec![]),
                Operation::new("Q", vec![]),
            ]);
        }
        let style = appearance::Style {
            size: list.size,
            color: if cell.selected {
                Operation::new("g", vec![1.into()])
            } else {
                plan.style.color.clone()
            },
        };
        operations.extend(appearance::generated_row(
            appearance::LineLayout {
                line: &cell.line,
                value: &cell.value,
                style: &style,
                q: plan.q,
                rectangle: [plan.padding, y + 2.0, width, list.row_height - 4.0],
            },
            font_name,
        )?);
    }
    operations.push(Operation::new("Q", vec![]));
    Ok(operations)
}

impl PdfDocument {
    /// Read bounded choice options without executing field actions or regenerating appearances.
    pub fn list_choice_fields(&self) -> Result<Vec<ChoiceField>> {
        let mut budget = Budget::new();
        let fields = super::discover(self)?;
        if !fields
            .iter()
            .any(|field| field.kind == FormFieldKind::Choice)
        {
            return Ok(vec![]);
        }
        let catalog = budget
            .resolve(&self.document, self.document.trailer.get(b"Root")?)?
            .1
            .as_dict()?;
        let form = budget
            .resolve(&self.document, catalog.get(b"AcroForm")?)?
            .1
            .as_dict()?;
        let readonly = super::form_readonly(self, &self.document, form, catalog, &mut budget)?;
        let mut result = Vec::new();
        for field in fields
            .into_iter()
            .filter(|field| field.kind == FormFieldKind::Choice)
        {
            budget.report(field.name.len())?;
            let mut options = Vec::new();
            if let Some(array) = inherited(&self.document, field.object_id, b"Opt", &mut budget)? {
                let array = array.as_array()?;
                if array.len() > MAX_FIELDS {
                    return Err(Error::Limit("choice options exceed 10000 entries"));
                }
                for (index, item) in array.iter().enumerate() {
                    let item = budget.resolve(&self.document, item)?.1;
                    let (export, display) = if let Object::Array(pair) = item {
                        if pair.len() != 2 {
                            return Err(Error::Invalid(
                                "choice option must contain exactly export and display strings",
                            ));
                        }
                        let export =
                            text(budget.resolve(&self.document, &pair[0])?.1, &mut budget)?;
                        let display =
                            text(budget.resolve(&self.document, &pair[1])?.1, &mut budget)?;
                        (export, display)
                    } else {
                        let display = text(item, &mut budget)?;
                        (display.clone(), display)
                    };
                    options.push(ChoiceOption {
                        index: index as u32,
                        export,
                        display,
                    });
                }
            }
            let indices = inherited(&self.document, field.object_id, b"I", &mut budget)?
                .map(|array| {
                    let array = array.as_array()?;
                    if array.len() > options.len() {
                        return Err(Error::Invalid("choice selection exceeds option count"));
                    }
                    let mut indices = Vec::new();
                    for entry in array {
                        let index = budget.resolve(&self.document, entry)?.1.as_i64()?;
                        if index < 0
                            || index as usize >= options.len()
                            || indices
                                .last()
                                .is_some_and(|previous| *previous >= index as u32)
                        {
                            return Err(Error::Invalid(
                                "choice indices must be sorted unique and in range",
                            ));
                        }
                        indices.push(index as u32);
                    }
                    Ok(indices)
                })
                .transpose()?;
            let top = inherited(&self.document, field.object_id, b"TI", &mut budget)?
                .map(Object::as_i64)
                .transpose()?
                .unwrap_or(0);
            if top < 0 || top as usize >= options.len().max(1) {
                return Err(Error::Invalid("choice top index is outside options"));
            }
            let default = value(
                &self.document,
                inherited(&self.document, field.object_id, b"DV", &mut budget)?,
                false,
                &mut budget,
            )?;
            let chosen = selected(&options, &field.value, indices);
            let combo = field.flags & (1 << 17) != 0;
            let multi_select = field.flags & (1 << 21) != 0;
            let editable_combo = field.flags & (1 << 18) != 0;
            let mut reason = readonly.map(str::to_owned);
            if let Some(guard) = field
                .refusal_reason
                .as_deref()
                .filter(|reason| *reason != super::UNSUPPORTED_FIELD_KIND)
            {
                reason = Some(guard.to_owned());
            }
            if field.flags & 1 != 0 {
                reason = Some("Choice field is read only".into());
            }
            if options.is_empty() {
                reason = Some("Choice field has no options".into());
            }
            if editable_combo {
                reason =
                    Some("Editable combos require a separate free-entry implementation".into());
            }
            if combo && multi_select {
                reason = Some("Combo cannot allow multiple selection".into());
            }
            if chosen
                .as_ref()
                .is_none_or(|indices| !multi_select && indices.len() > 1)
            {
                reason = Some("Choice V and I cannot identify a consistent selection".into());
            }
            for key in [b"A".as_slice(), b"AA"] {
                if inherited(&self.document, field.object_id, key, &mut budget)?.is_some() {
                    reason = Some("Choice actions are unsupported".into());
                }
            }
            if reason.is_none() {
                for widget in &field.widgets {
                    let dict = self.document.get_dictionary(widget.object_id)?;
                    let flags = dict
                        .get(b"F")
                        .ok()
                        .map(|flags| -> Result<i64> {
                            Ok(budget.resolve(&self.document, flags)?.1.as_i64()?)
                        })
                        .transpose()?
                        .unwrap_or(0);
                    if flags & (2 | 32) != 0 {
                        reason = Some(
                            "Hidden or NoView choice widgets are read only in this tier".into(),
                        );
                        break;
                    }
                    match appearance::widget_plan(
                        self,
                        widget.object_id,
                        field.object_id,
                        "",
                        Line {
                            glyphs: vec![],
                            bounds: [0., 0., 0., 1.],
                            advance: 0.,
                        },
                        &mut budget,
                    ) {
                        Ok(_) => (),
                        Err(error @ Error::Limit(_)) => return Err(error),
                        Err(error) => {
                            reason = Some(error.to_string());
                            break;
                        }
                    }
                }
                if field.widgets.is_empty() {
                    reason = Some("Choice field has no visible widgets".into());
                }
            }
            result.push(ChoiceField {
                field,
                options,
                selected_indices: chosen,
                default_value: default,
                top_index: top as u32,
                combo,
                multi_select,
                editable_combo,
                writable: reason.is_none(),
                refusal_reason: reason,
            });
        }
        Ok(result)
    }

    /// Select existing choice options by index with real embedded-font appearances.
    /// Without an explicit top index, preserve scrolling when a selected row is
    /// visible; otherwise scroll to the first selected row. All indices remain selected.
    pub fn fill_choice(&mut self, changes: &[ChoiceChange], font_bytes: &[u8]) -> Result<()> {
        self.ensure_editable()?;
        if changes.is_empty() || changes.len() > MAX_FIELDS {
            return Err(Error::Refused("select between one and 10000 choice fields"));
        }
        let fields = self.list_choice_fields()?;
        let mut font = FontPlan::new(font_bytes)?;
        let mut budget = Budget::new();
        let mut names = BTreeSet::new();
        let mut plans = Vec::new();
        let mut patches = Vec::new();
        let mut lists: BTreeMap<ObjectId, ListLayout> = BTreeMap::new();
        let mut glyphs = 0usize;
        for change in changes {
            budget.report(change.name.len())?;
            if change.name.is_empty() || change.name.len() > 4096 || !names.insert(&change.name) {
                return Err(Error::Refused("choice names must be unique and bounded"));
            }
            let mut matching = fields
                .iter()
                .filter(|field| field.field.name == change.name);
            let field = matching
                .next()
                .ok_or(Error::Refused("choice field name not found"))?;
            if matching.next().is_some() || !field.writable {
                return Err(Error::Refused(
                    "an unambiguous writable choice field is required",
                ));
            }
            if change.indices.len() > field.options.len()
                || (!field.multi_select && change.indices.len() > 1)
                || change
                    .indices
                    .iter()
                    .any(|index| *index as usize >= field.options.len())
                || change.indices.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(Error::Refused(
                    "choice indices must be sorted unique and in range",
                ));
            }
            if change.indices.is_empty() && field.field.flags & 2 != 0 {
                return Err(Error::Refused("required choice cannot be cleared"));
            }
            if field.combo && change.top_index.is_some() {
                return Err(Error::Refused("a combo cannot set list scroll position"));
            }
            let mut top = change.top_index.unwrap_or(field.top_index) as usize;
            if top >= field.options.len() {
                return Err(Error::Refused("choice top index is outside options"));
            }
            let display = change
                .indices
                .first()
                .map(|index| field.options[*index as usize].display.as_str())
                .unwrap_or("");
            let shaped = if field.combo {
                vec![]
            } else {
                let mut lines = Vec::with_capacity(field.options.len());
                for option in &field.options {
                    budget.report(option.display.len())?;
                    let line = font.shape(&option.display)?;
                    glyphs += line.glyphs.len();
                    if glyphs > 100_000 {
                        return Err(Error::Limit(
                            "choice transaction exceeds 100000 planned glyphs",
                        ));
                    }
                    lines.push(line);
                }
                lines
            };
            let mut field_plans = Vec::new();
            let mut sizes = Vec::new();
            for widget in &field.field.widgets {
                let line = if field.combo {
                    font.shape(display)?
                } else {
                    Line {
                        glyphs: vec![],
                        bounds: [0., 0., 0., 1.],
                        advance: 0.,
                    }
                };
                glyphs += line.glyphs.len();
                if glyphs > 100_000 {
                    return Err(Error::Limit(
                        "choice transaction exceeds 100000 planned glyphs",
                    ));
                }
                let plan = appearance::widget_plan(
                    self,
                    widget.object_id,
                    field.field.object_id,
                    display,
                    line,
                    &mut budget,
                )?;
                if !field.combo {
                    let inner_w = plan.width - 2.0 * plan.padding;
                    let inner_h = plan.height - 2.0 * plan.padding;
                    let widest = shaped
                        .iter()
                        .map(|line| line.bounds[2] - line.bounds[0])
                        .fold(0.0, f64::max);
                    let tallest = shaped
                        .iter()
                        .map(|line| line.bounds[3] - line.bounds[1])
                        .fold(0.0, f64::max);
                    let size = if plan.style.size == 0.0 {
                        12.0_f64
                            .min((inner_h - 4.0) / tallest)
                            .min(if widest > 0.0 { inner_w / widest } else { 12.0 })
                    } else {
                        plan.style.size
                    };
                    let row_height = tallest * size + 4.0;
                    if !size.is_finite()
                        || size < 1.0
                        || !row_height.is_finite()
                        || row_height > inner_h + 0.001
                        || widest * size > inner_w + 0.001
                    {
                        return Err(Error::Refused("choice labels do not fit without overflow"));
                    }
                    let capacity =
                        ((inner_h / row_height).floor() as usize).min(field.options.len());
                    if capacity == 0 {
                        return Err(Error::Refused("choice list has no visible row"));
                    }
                    sizes.push((size, row_height, capacity));
                }
                field_plans.push(plan);
            }
            if !field.combo {
                let capacity = sizes
                    .iter()
                    .map(|(_, _, capacity)| *capacity)
                    .min()
                    .ok_or(Error::Refused("choice list has no widget"))?;
                if change.top_index.is_none()
                    && !change.indices.is_empty()
                    && !change
                        .indices
                        .iter()
                        .any(|index| (*index as usize) >= top && (*index as usize) < top + capacity)
                {
                    top = change.indices[0] as usize;
                }
                for (plan, (size, row_height, capacity)) in field_plans.iter().zip(sizes) {
                    let visible = capacity.min(field.options.len() - top);
                    let mut cells = Vec::with_capacity(visible);
                    for (index, line) in shaped.iter().enumerate().skip(top).take(visible) {
                        budget.report(field.options[index].display.len())?;
                        glyphs += line.glyphs.len();
                        if glyphs > 100_000 {
                            return Err(Error::Limit(
                                "choice transaction exceeds 100000 planned glyphs",
                            ));
                        }
                        cells.push(ListCell {
                            line: line.clone(),
                            value: field.options[index].display.clone(),
                            selected: change.indices.binary_search(&(index as u32)).is_ok(),
                        });
                    }
                    lists.insert(
                        plan.widget,
                        ListLayout {
                            cells,
                            size,
                            row_height,
                        },
                    );
                }
            }
            plans.extend(field_plans);
            let exports: Vec<_> = change
                .indices
                .iter()
                .map(|index| unicode_string(&field.options[*index as usize].export))
                .collect();
            let value = match exports.len() {
                0 => Some(Object::Null),
                1 => exports.into_iter().next(),
                _ => Some(Object::Array(exports)),
            };
            patches.push(FieldPatch {
                id: field.field.object_id,
                values: vec![
                    ("V", value),
                    (
                        "I",
                        Some(Object::Array(
                            change
                                .indices
                                .iter()
                                .map(|index| Object::Integer(i64::from(*index)))
                                .collect(),
                        )),
                    ),
                    (
                        "TI",
                        if field.combo {
                            None
                        } else {
                            Some(Object::Integer(top as i64))
                        },
                    ),
                ],
            });
        }
        appearance::publish_plans(
            self,
            font,
            plans,
            patches,
            &mut budget,
            |plan, font_name| {
                let replacement = if let Some(list) = lists.get(&plan.widget) {
                    list_operations(plan, font_name, list)?
                } else {
                    appearance::generated(plan, font_name)?
                };
                if plan.content.operations.len() - (plan.text_end - plan.text_start + 1)
                    + replacement.len()
                    > 100_000
                {
                    return Err(Error::Limit(
                        "generated choice appearance exceeds 100000 operators",
                    ));
                }
                Ok(replacement)
            },
        )
    }
}
