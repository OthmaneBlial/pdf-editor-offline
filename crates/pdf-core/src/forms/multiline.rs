//! UAX #14 (Unicode 15.0) wrapping, filtered by extended grapheme boundaries.
//! Logical breaks and whitespace remain in V/ActualText; no hyphen is invented.
use super::{
    WidgetPlan,
    fonts::{FontPlan, Glyph, Line},
};
use crate::{Error, Result};
use std::collections::BTreeSet;
use unicode_segmentation::UnicodeSegmentation;

const MAX_LINES: usize = 1024;
pub(in crate::forms) struct Layout {
    pub glyphs: Vec<Glyph>,
    pub size: f64,
    pub runs: Vec<(std::ops::Range<usize>, std::ops::Range<usize>)>,
}
struct Paragraph<'a> {
    text: &'a str,
    logical: std::ops::Range<usize>,
    breaks: Vec<usize>,
    lines: Vec<std::ops::Range<usize>>,
}
fn paragraphs(value: &str) -> Result<Vec<Paragraph<'_>>> {
    if value.len() > 64 * 1024 {
        return Err(Error::Limit("text field value exceeds 64 KiB"));
    }
    let mut parts = Vec::new();
    let mut start = 0;
    let mut characters = value.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if matches!(character, '\r' | '\n' | '\u{85}' | '\u{2029}') {
            let begin = start;
            start = index + character.len_utf8();
            if character == '\r' && characters.peek().is_some_and(|(_, c)| *c == '\n') {
                characters.next();
                start += 1;
            }
            parts.push((&value[begin..index], begin..start));
        }
    }
    parts.push((&value[start..], start..value.len()));
    if parts.len() > MAX_LINES {
        return Err(Error::Limit("multiline field exceeds 1024 explicit lines"));
    }
    let paragraphs: Vec<_> = parts
        .into_iter()
        .map(|(text, logical)| {
            let graphemes: BTreeSet<_> = text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
                .collect();
            let mut lines = Vec::new();
            let mut start = 0;
            for (index, character) in text.char_indices() {
                if character == '\u{2028}' {
                    lines.push(start..index);
                    start = index + character.len_utf8();
                }
            }
            lines.push(start..text.len());
            let mut breaks: Vec<_> = unicode_linebreak::linebreaks(text)
                .map(|(i, _)| i)
                .filter(|i| *i > 0 && graphemes.contains(i))
                // A discretionary hyphen needs a separately implemented visible
                // hyphen policy. Keep it invisible unless its break is unnecessary.
                .filter(|i| *i == text.len() || !text[..*i].ends_with('\u{ad}'))
                .collect();
            breaks.extend(lines.iter().map(|range| range.end));
            breaks.sort_unstable();
            breaks.dedup();
            Paragraph {
                text,
                logical,
                breaks,
                lines,
            }
        })
        .collect();
    if paragraphs
        .iter()
        .map(|paragraph| paragraph.lines.len())
        .sum::<usize>()
        > MAX_LINES
    {
        return Err(Error::Limit("multiline field exceeds 1024 explicit lines"));
    }
    Ok(paragraphs)
}
fn shape(
    font: &mut FontPlan<'_>,
    paragraph: &str,
    range: std::ops::Range<usize>,
    work: &mut usize,
) -> Result<Line> {
    *work = work
        .checked_add(paragraph.len().saturating_add(range.len()))
        .ok_or(Error::Limit("multiline planning overflow"))?;
    if *work > 2_000_000 {
        return Err(Error::Limit(
            "multiline planning exceeds two million text bytes",
        ));
    }
    font.shape_in_paragraph(paragraph, range)
}
fn attempt(
    font: &mut FontPlan<'_>,
    paragraphs: &[Paragraph<'_>],
    plan: &BoxLayout,
    size: f64,
    minimum_height: f64,
    work: &mut usize,
) -> Result<Option<Layout>> {
    let width = plan.width - 2.0 * plan.padding;
    let height = plan.height - 2.0 * plan.padding;
    let capacity = ((height / (minimum_height * size)).floor() as usize).min(MAX_LINES);
    if capacity == 0 {
        return Ok(None);
    }
    let mut lines = Vec::new();
    let mut logical_runs = Vec::new();
    for paragraph in paragraphs {
        for (segment_index, segment) in paragraph.lines.iter().enumerate() {
            if segment.is_empty() {
                lines.push(shape(font, paragraph.text, segment.clone(), work)?);
                logical_runs.push(
                    paragraph.logical.start + segment.start..paragraph.logical.start + segment.end,
                );
            } else {
                let mut start = segment.start;
                while start < segment.end {
                    let first = paragraph.breaks.partition_point(|end| *end <= start);
                    let mut low = first;
                    let mut high = paragraph.breaks.partition_point(|end| *end <= segment.end);
                    let mut best = None;
                    while low < high {
                        let mid = low + (high - low) / 2;
                        let end = paragraph.breaks[mid];
                        let line = shape(font, paragraph.text, start..end, work)?;
                        if (line.bounds[2] - line.bounds[0]) * size <= width + 0.001 {
                            best = Some((end, line));
                            low = mid + 1;
                        } else {
                            high = mid;
                        }
                    }
                    let Some((end, line)) = best else {
                        return Ok(None);
                    };
                    lines.push(line);
                    logical_runs
                        .push(paragraph.logical.start + start..paragraph.logical.start + end);
                    start = end;
                    if lines.len() > capacity {
                        return Ok(None);
                    }
                }
            }
            if lines.len() > capacity {
                return Ok(None);
            }
            let last = logical_runs
                .last_mut()
                .ok_or(Error::Invalid("multiline has no semantic line"))?;
            last.end = paragraph
                .lines
                .get(segment_index + 1)
                .map_or(paragraph.logical.end, |next| {
                    paragraph.logical.start + next.start
                });
        }
    }
    let ascent = lines
        .iter()
        .map(|line| line.bounds[3])
        .fold(f64::NEG_INFINITY, f64::max);
    let descent = lines
        .iter()
        .map(|line| line.bounds[1])
        .fold(f64::INFINITY, f64::min);
    let extent = ascent - descent;
    let leading = extent * 1.2;
    let total = (extent + (lines.len() - 1) as f64 * leading) * size;
    if !total.is_finite() || total > height + 0.001 {
        return Ok(None);
    }
    let mut glyphs = Vec::new();
    let mut runs = Vec::new();
    for (index, (line, logical)) in lines.into_iter().zip(logical_runs).enumerate() {
        let spare = width / size - (line.bounds[2] - line.bounds[0]);
        let x = -line.bounds[0]
            + match plan.alignment {
                1 => spare / 2.0,
                2 => spare,
                _ => 0.0,
            };
        let y = -ascent - index as f64 * leading;
        let first = glyphs.len();
        for mut glyph in line.glyphs {
            glyph.x += x;
            glyph.y += y;
            glyphs.push(glyph);
        }
        runs.push((first..glyphs.len(), logical));
    }
    Ok(Some(Layout { glyphs, size, runs }))
}
pub(in crate::forms) struct BoxLayout {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub size: f64,
    pub alignment: i64,
}
pub(super) fn layout(
    font: &mut FontPlan<'_>,
    value: &str,
    plan: &WidgetPlan,
    work: &mut usize,
) -> Result<Layout> {
    layout_box(
        font,
        value,
        &BoxLayout {
            width: plan.width,
            height: plan.height,
            padding: plan.padding,
            size: plan.style.size,
            alignment: plan.q,
        },
        work,
    )
}
pub(in crate::forms) fn layout_box(
    font: &mut FontPlan<'_>,
    value: &str,
    plan: &BoxLayout,
    work: &mut usize,
) -> Result<Layout> {
    let paragraphs = paragraphs(value)?;
    let mut minimum_height: f64 = 0.0;
    // Validate every character and outline even if a later geometric refusal
    // could otherwise avoid visiting an offscreen paragraph.
    for paragraph in &paragraphs {
        for segment in &paragraph.lines {
            let line = shape(font, paragraph.text, segment.clone(), work)?;
            minimum_height = minimum_height.max(line.bounds[3] - line.bounds[1]);
        }
    }
    if plan.size != 0.0 {
        return attempt(font, &paragraphs, plan, plan.size, minimum_height, work)?.ok_or(
            Error::Refused("multiline text does not fit without overflow"),
        );
    }
    let mut best = attempt(font, &paragraphs, plan, 1.0, minimum_height, work)?.ok_or(
        Error::Refused("multiline text cannot fit at the minimum one-point size"),
    )?;
    let mut low = 1.0;
    let mut high = ((plan.height - 2.0 * plan.padding) / minimum_height).min(1024.0);
    for _ in 0..14 {
        let size = (low + high) / 2.0;
        if let Some(candidate) = attempt(font, &paragraphs, plan, size, minimum_height, work)? {
            best = candidate;
            low = size;
        } else {
            high = size;
        }
    }
    Ok(best)
}
