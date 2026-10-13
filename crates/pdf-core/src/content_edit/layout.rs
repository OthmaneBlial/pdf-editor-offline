use super::{ContentTextEncoding, ContentTextFont};
use crate::{Error, Result};
use lopdf::Object;
use serde::{Deserialize, Serialize};

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/// A finite nominal text layout. It does not bound glyph ink or fit text to a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextLayout {
    pub font_size: f64,
    pub character_spacing: f64,
    pub word_spacing: f64,
    pub horizontal_scale: f64,
    pub rise: f64,
    pub text_matrix: [f64; 6],
    pub current_transform: [f64; 6],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTextPosition {
    pub nominal_advance: f64,
    pub baseline_start: [f64; 2],
    pub baseline_end: [f64; 2],
}
impl ContentTextLayout {
    pub(super) fn position(
        &self,
        font: ContentTextFont,
        encoding: ContentTextEncoding,
        value: &str,
    ) -> Result<ContentTextPosition> {
        let params = [
            self.font_size,
            self.character_spacing,
            self.word_spacing,
            self.horizontal_scale,
            self.rise,
        ];
        if params
            .into_iter()
            .chain(self.text_matrix)
            .chain(self.current_transform)
            .any(|value| !bounded(value))
            || self.font_size <= 0.0
            || self.horizontal_scale == 0.0
        {
            return Err(Error::Refused(
                "text layout has unsupported nonfinite, zero or excessive parameters",
            ));
        }
        let matrix = compose(self.current_transform, self.text_matrix);
        let baseline_start = [
            matrix[4] + matrix[2] * self.rise,
            matrix[5] + matrix[3] * self.rise,
        ];
        if baseline_start.into_iter().any(|value| !bounded(value))
            || [
                matrix[0] * self.font_size * self.horizontal_scale / 100.0,
                matrix[1] * self.font_size * self.horizontal_scale / 100.0,
                matrix[2] * self.font_size,
                matrix[3] * self.font_size,
            ]
            .into_iter()
            .any(|value| !bounded(value))
        {
            return Err(Error::Limit(
                "text baseline or nominal glyph basis exceeds one million content-space units",
            ));
        }
        let mut width = 0u64;
        let mut spaces = 0usize;
        let mut nominal_advance = 0.0;
        let mut baseline_end = baseline_start;
        for (index, code) in value.bytes().enumerate() {
            width = width
                .checked_add(u64::from(font.code_width(code, encoding)?))
                .ok_or(Error::Limit("text metric width overflow"))?;
            spaces += usize::from(code == b' ');
            nominal_advance = (width as f64 * self.font_size / 1000.0
                + (index + 1) as f64 * self.character_spacing
                + spaces as f64 * self.word_spacing)
                * self.horizontal_scale
                / 100.0;
            baseline_end = [
                baseline_start[0] + matrix[0] * nominal_advance,
                baseline_start[1] + matrix[1] * nominal_advance,
            ];
            if !bounded(nominal_advance) || baseline_end.into_iter().any(|value| !bounded(value)) {
                return Err(Error::Limit(
                    "nominal text layout exceeds one million content-space units",
                ));
            }
        }
        Ok(ContentTextPosition {
            nominal_advance,
            baseline_start,
            baseline_end,
        })
    }
}
fn bounded(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.0
}
pub(super) fn numeric(value: &Object) -> Result<f64> {
    match value {
        Object::Integer(value) => Ok(*value as f64),
        Object::Real(value) if value.is_finite() => Ok(f64::from(*value)),
        _ => Err(Error::Invalid("invalid numeric text-layout operand")),
    }
}
pub(super) fn matrix(values: &[Object]) -> Result<[f64; 6]> {
    if values.len() != 6 {
        return Err(Error::Invalid("text-layout matrix requires six numbers"));
    }
    let mut result = [0.0; 6];
    for (target, value) in result.iter_mut().zip(values) {
        *target = numeric(value)?;
    }
    Ok(result)
}
fn compose(left: [f64; 6], right: [f64; 6]) -> [f64; 6] {
    [
        left[0] * right[0] + left[2] * right[1],
        left[1] * right[0] + left[3] * right[1],
        left[0] * right[2] + left[2] * right[3],
        left[1] * right[2] + left[3] * right[3],
        left[0] * right[4] + left[2] * right[5] + left[4],
        left[1] * right[4] + left[3] * right[5] + left[5],
    ]
}
#[derive(Clone)]
pub(crate) struct GraphicsTextState {
    spacing: f64,
    word: f64,
    scale: f64,
    rise: f64,
    transform: [f64; 6],
}
impl Default for GraphicsTextState {
    fn default() -> Self {
        Self {
            spacing: 0.0,
            word: 0.0,
            scale: 100.0,
            rise: 0.0,
            transform: IDENTITY,
        }
    }
}
impl GraphicsTextState {
    pub(super) fn update(&mut self, operator: &str, values: &[Object]) -> Result<()> {
        if operator == "cm" {
            self.transform = compose(self.transform, matrix(values)?);
            return Ok(());
        }
        let [value] = values else {
            return Err(Error::Invalid("text-state operator requires one number"));
        };
        self.update_number(operator, numeric(value)?)
    }
    /// Reuse the state model with a validated raw-token number before f32 parsing
    /// loses precision; existing operator editing retains its original parser.
    pub(crate) fn update_number(&mut self, operator: &str, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(Error::Invalid("nonfinite text-state parameter"));
        }
        match operator {
            "Tc" => self.spacing = value,
            "Tw" => self.word = value,
            "Tz" => self.scale = value,
            "Ts" => self.rise = value,
            _ => return Err(Error::Invalid("unsupported layout parameter")),
        }
        Ok(())
    }
    pub(crate) fn layout(&self, size: f64, matrix: [f64; 6]) -> ContentTextLayout {
        ContentTextLayout {
            font_size: size,
            character_spacing: self.spacing,
            word_spacing: self.word,
            horizontal_scale: self.scale,
            rise: self.rise,
            text_matrix: matrix,
            current_transform: self.transform,
        }
    }
}
