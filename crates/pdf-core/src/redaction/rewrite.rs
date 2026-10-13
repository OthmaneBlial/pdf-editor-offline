use super::*;
use crate::ContentTextLayout;
use crate::content_edit::layout::GraphicsTextState;

#[derive(Clone, Default)]
struct State<'a> {
    graphics: GraphicsTextState,
    font: Option<&'a Font>,
    size: f64,
    leading: f64,
}
fn number(bytes: &[u8], range: &std::ops::Range<usize>, parsed: &Object) -> Result<f64> {
    if !parsed.as_float().is_ok_and(f32::is_finite) || range.len() > 128 {
        return Err(Error::Refused(
            "invalid or excessive redaction numeric parameter",
        ));
    }
    let value = std::str::from_utf8(&bytes[range.clone()])
        .ok()
        .and_then(|raw| raw.parse::<f64>().ok())
        .ok_or(Error::Refused("ambiguous redaction numeric token"))?;
    if !value.is_finite() || value.abs() > 1_000_000.0 {
        return Err(Error::Refused(
            "redaction text-state magnitude exceeds one million",
        ));
    }
    Ok(value)
}

struct Show<'a> {
    ordinal: usize,
    font: &'a Font,
    layout: ContentTextLayout,
    segments: Vec<(usize, &'a [u8])>,
}
fn pieces(value: &[u8], mask: &[bool], show: &Show<'_>) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut at = 0;
    while at < value.len() {
        let removed = mask[at];
        let start = at;
        while at < value.len() && mask[at] == removed {
            at += 1;
        }
        if removed {
            let width = value[start..at].iter().try_fold(0u64, |sum, code| {
                let width = pdf_render::standard_ascii_width(&show.font.0, *code, show.font.1)
                    .ok_or(Error::Refused("standard font advance unavailable"))?;
                sum.checked_add(u64::from(width))
                    .ok_or(Error::Limit("redaction width overflow"))
            })?;
            let removed = &value[start..at];
            let spaces = removed.iter().filter(|code| **code == b' ').count();
            // TJ does not add Tc/Tw. Replace every removed glyph's own advance,
            // including the final glyph; horizontal scaling applies to both and
            // cancels in these thousandths-of-em compensation units.
            let adjustment = -(width as f64
                + 1000.0
                    * (removed.len() as f64 * show.layout.character_spacing
                        + spaces as f64 * show.layout.word_spacing)
                    / show.layout.font_size);
            if !adjustment.is_finite() || adjustment.abs() > 1e15 {
                return Err(Error::Limit("redaction compensation magnitude"));
            }
            output.extend_from_slice(format!(" {adjustment} ").as_bytes());
        } else {
            output.push(b'<');
            for byte in &value[start..at] {
                output.extend_from_slice(format!("{byte:02X}").as_bytes());
            }
            output.extend_from_slice(b"> ");
        }
    }
    Ok(output)
}
pub(super) fn plan(
    bytes: &[u8],
    fonts: &Fonts,
    media_box: [f32; 4],
    targets: &[Needle<'_>],
    search: &mut Search,
) -> Result<(Vec<u8>, usize, usize)> {
    let (program, spans) = crate::content_spans::decode_spanned(bytes)?;
    let mut block = false;
    let mut state = State::default();
    let mut saved = Vec::new();
    let mut positioned = false;
    let mut text: Vec<u8> = Vec::new();
    let mut shows: Vec<Show<'_>> = Vec::new();
    let mut changes = Vec::new();
    let mut changed_shows = 0;
    let mut matches = 0;
    let mut graphics_depth = 0usize;
    let mut background_path = false;
    let mut background_painted = false;
    let mut text_started = false;
    for (ordinal, op) in program.operations.iter().enumerate() {
        // A single complete page background is the only admitted painted path.
        // Require its fill immediately: no clipping, transformed/path lettering,
        // accumulated shapes, or background painting over already shown text.
        if background_path && !matches!(op.operator.as_str(), "f" | "F" | "f*") {
            return Err(Error::Refused(
                "redaction background rectangle needs an immediate fill",
            ));
        }
        match op.operator.as_str() {
            "BT" => {
                if block || !op.operands.is_empty() {
                    return Err(Error::Refused("invalid redaction text object"));
                }
                block = true;
                text_started = true;
                positioned = false;
                text.clear();
                shows.clear();
            }
            "ET" => {
                if !block || !op.operands.is_empty() {
                    return Err(Error::Refused("invalid redaction ET"));
                }
                block = false;
                let mut mask = vec![false; text.len()];
                for range in search.find(&text, targets, false)? {
                    mask[range].fill(true);
                    matches += 1;
                }
                for show in &shows {
                    if !show
                        .segments
                        .iter()
                        .any(|(at, s)| mask[*at..*at + s.len()].iter().any(|b| *b))
                    {
                        continue;
                    }
                    changed_shows += 1;
                    let op = &program.operations[show.ordinal];
                    let span = &spans[show.ordinal];
                    if op.operator != "TJ" {
                        let (at, value) = show.segments[0];
                        let mut replacement = Vec::new();
                        if op.operator == "\"" {
                            replacement.extend_from_slice(&bytes[span.operands[0].clone()]);
                            replacement.extend_from_slice(b" Tw ");
                            replacement.extend_from_slice(&bytes[span.operands[1].clone()]);
                            replacement.extend_from_slice(b" Tc T* ");
                        } else if op.operator == "'" {
                            replacement.extend_from_slice(b"T* ");
                        }
                        replacement.push(b'[');
                        replacement.extend(pieces(value, &mask[at..at + value.len()], show)?);
                        replacement.extend_from_slice(b"] TJ");
                        changes.push((span.full.clone(), replacement));
                    } else {
                        let range = span.operands[0].clone();
                        let items = crate::content_spans::array_item_spans(bytes, range.clone())?;
                        let values = op.operands[0].as_array()?;
                        if items.len() != values.len() {
                            return Err(Error::Refused("ambiguous redaction TJ spans"));
                        }
                        let mut replacement = b"[".to_vec();
                        let mut segment = 0;
                        for (value, item) in values.iter().zip(items) {
                            if matches!(value, Object::String(_, _)) {
                                let (at, s) = show.segments[segment];
                                segment += 1;
                                if mask[at..at + s.len()].iter().any(|b| *b) {
                                    replacement.extend(pieces(s, &mask[at..at + s.len()], show)?);
                                } else {
                                    replacement.extend_from_slice(&bytes[item]);
                                    replacement.push(b' ');
                                }
                            } else {
                                replacement.extend_from_slice(&bytes[item]);
                                replacement.push(b' ');
                            }
                        }
                        replacement.push(b']');
                        changes.push((range, replacement));
                    }
                }
            }
            "Tf" => {
                if op.operands.len() != 2
                    || !op.operands[1].as_float()?.is_finite()
                    || op.operands[1].as_float()? <= 0.0
                {
                    return Err(Error::Refused("unsupported redaction font selection"));
                }
                state.size = number(bytes, &spans[ordinal].operands[1], &op.operands[1])?;
                if state.size < 0.000001 {
                    return Err(Error::Refused("redaction font size is below one millionth"));
                }
                state.font = Some(
                    fonts
                        .get(op.operands[0].as_name()?)
                        .ok_or(Error::Refused("missing redaction font"))?,
                );
            }
            "Tc" | "Tw" | "Tz" | "Ts" => {
                if op.operands.len() != 1 {
                    return Err(Error::Refused("invalid redaction text-state arity"));
                }
                let value = number(bytes, &spans[ordinal].operands[0], &op.operands[0])?;
                if op.operator == "Tz" && (value == 0.0 || op.operands[0].as_float()? == 0.0) {
                    return Err(Error::Refused(
                        "redaction horizontal scaling cannot be zero",
                    ));
                }
                state.graphics.update_number(&op.operator, value)?;
            }
            "TL" => {
                if op.operands.len() != 1 {
                    return Err(Error::Refused("invalid redaction leading"));
                }
                state.leading = number(bytes, &spans[ordinal].operands[0], &op.operands[0])?;
            }
            "Tr" => {
                if op.operands.len() != 1 || !matches!(op.operands[0], Object::Integer(0 | 3)) {
                    return Err(Error::Refused(
                        "redaction only admits filled or invisible text",
                    ));
                }
            }
            "Tm" | "Td" | "TD" if block => {
                let n = if op.operator == "Tm" { 6 } else { 2 };
                if op.operands.len() != n
                    || op
                        .operands
                        .iter()
                        .any(|v| v.as_float().is_err() || !v.as_float().is_ok_and(f32::is_finite))
                {
                    return Err(Error::Refused("invalid redaction text position"));
                }
                positioned = true;
                if op.operator == "TD" {
                    state.leading = -number(bytes, &spans[ordinal].operands[1], &op.operands[1])?;
                }
            }
            "T*" if block => {
                if !positioned || !op.operands.is_empty() {
                    return Err(Error::Refused("invalid redaction next-line position"));
                }
            }
            "Tj" | "TJ" | "'" | "\"" if block => {
                let font = state
                    .font
                    .ok_or(Error::Refused("redaction text objects need explicit font"))?;
                let expected = if op.operator == "\"" { 3 } else { 1 };
                if !positioned || op.operands.len() != expected {
                    return Err(Error::Refused(
                        "redaction text objects need explicit position",
                    ));
                }
                if op.operator == "\"" {
                    for (index, operator) in [(0, "Tw"), (1, "Tc")] {
                        let value =
                            number(bytes, &spans[ordinal].operands[index], &op.operands[index])?;
                        state.graphics.update_number(operator, value)?;
                    }
                }
                let values = if op.operator == "TJ" {
                    op.operands[0].as_array()?.as_slice()
                } else {
                    &op.operands[expected - 1..]
                };
                let mut show = Show {
                    ordinal,
                    font,
                    layout: state
                        .graphics
                        .layout(state.size, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
                    segments: Vec::new(),
                };
                for value in values {
                    match value {
                        Object::String(s, _) => {
                            if !s.iter().all(|b| (32..=126).contains(b))
                                || (!font.1 && s.iter().any(|b| matches!(b, 39 | 96)))
                            {
                                return Err(Error::Refused(
                                    "redaction requires unambiguous printable ASCII glyph strings",
                                ));
                            }
                            if text.len().saturating_add(s.len()) > crate::MAX_STREAM_BYTES {
                                return Err(Error::Limit("redaction text object bytes"));
                            }
                            show.segments.push((text.len(), s.as_slice()));
                            text.extend(s);
                        }
                        Object::Integer(_) | Object::Real(_)
                            if op.operator == "TJ" && value.as_float()?.is_finite() => {}
                        _ => return Err(Error::Refused("invalid redaction text show operands")),
                    }
                }
                shows.push(show);
            }
            _ if block => {
                return Err(Error::Refused(
                    "text state/marked content is outside the proven redaction profile",
                ));
            }
            "q" => {
                if !op.operands.is_empty() || graphics_depth >= 32 {
                    return Err(Error::Refused("redaction graphics stack"));
                }
                graphics_depth += 1;
                saved.push(state.clone());
            }
            "Q" => {
                if !op.operands.is_empty() || graphics_depth == 0 {
                    return Err(Error::Refused("redaction graphics stack"));
                }
                graphics_depth -= 1;
                state = saved
                    .pop()
                    .ok_or(Error::Refused("unbalanced redaction text state"))?;
            }
            "re" => {
                let expected = [
                    media_box[0],
                    media_box[1],
                    media_box[2] - media_box[0],
                    media_box[3] - media_box[1],
                ];
                if text_started
                    || background_painted
                    || op.operands.len() != 4
                    || op.operands.iter().zip(expected).any(|(value, expected)| {
                        !value
                            .as_float()
                            .is_ok_and(|actual| actual.is_finite() && actual == expected)
                    })
                {
                    return Err(Error::Refused(
                        "redaction only admits one uniform full-page background before text",
                    ));
                }
                background_path = true;
            }
            "f" | "F" | "f*" => {
                if !background_path || !op.operands.is_empty() {
                    return Err(Error::Refused("redaction excludes arbitrary painted paths"));
                }
                background_path = false;
                background_painted = true;
            }
            "g" | "rg" | "k" => {
                // Color setters and the uniform background are kept as exact raw
                // bytes. Numeric encodings are not a visual/steganographic proof.
                let arity = match op.operator.as_str() {
                    "k" => 4,
                    "rg" => 3,
                    _ => 1,
                };
                if op.operands.len() != arity {
                    return Err(Error::Refused("invalid redaction artwork operator arity"));
                }
                for value in &op.operands {
                    match value {
                        Object::Integer(_) | Object::Real(_)
                            if value.as_float()?.is_finite()
                                && (0.0..=1.0).contains(&value.as_float()?) => {}
                        _ => return Err(Error::Refused("unsupported redaction artwork operand")),
                    }
                }
            }
            _ => {
                return Err(Error::Refused(
                    "opaque or unsupported content prevents proven redaction",
                ));
            }
        }
    }
    if block || graphics_depth != 0 || background_path {
        return Err(Error::Refused("unbalanced redaction content"));
    }
    let mut output = Vec::new();
    output
        .try_reserve(bytes.len())
        .map_err(|_| Error::Limit("redaction output allocation"))?;
    let mut at = 0;
    for (range, replacement) in &changes {
        if output
            .len()
            .saturating_add(range.start - at)
            .saturating_add(replacement.len())
            > crate::MAX_STREAM_BYTES
        {
            return Err(Error::Limit("redaction expanded output exceeds 32 MiB"));
        }
        output.extend_from_slice(&bytes[at..range.start]);
        output.extend(replacement);
        at = range.end;
    }
    output.extend_from_slice(&bytes[at..]);
    if output.len() > crate::MAX_STREAM_BYTES {
        return Err(Error::Limit("redaction expanded output exceeds 32 MiB"));
    }
    Ok((output, changed_shows, matches))
}
