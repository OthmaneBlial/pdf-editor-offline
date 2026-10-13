//! Bounded lexical locations for validated content programs; original bytes are never re-encoded.
use crate::{Error, Result};
use lopdf::content::{Content, Operation};
use std::ops::Range;

#[derive(Debug, Clone)]
pub(crate) struct OperatorSpan {
    pub operands: Vec<Range<usize>>,
    pub operator: Range<usize>,
    pub full: Range<usize>,
}
struct Lexer<'a> {
    bytes: &'a [u8],
    at: usize,
    work: usize,
    masked: Vec<Range<usize>>,
}
fn space(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}
fn delimiter(b: u8) -> bool {
    space(b)
        || matches!(
            b,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}
fn numeric(bytes: &[u8]) -> bool {
    let bytes = if matches!(bytes.first(), Some(b'+' | b'-')) {
        &bytes[1..]
    } else {
        bytes
    };
    let mut digits = 0;
    let mut dots = 0;
    for b in bytes {
        if b.is_ascii_digit() {
            digits += 1;
        } else if *b == b'.' {
            dots += 1;
        } else {
            return false;
        }
    }
    digits > 0 && dots <= 1
}
impl Lexer<'_> {
    fn charge(&mut self) -> Result<()> {
        self.work += 1;
        if self.work > 2_000_000 {
            return Err(Error::Limit(
                "content span lexer exceeds two million tokens",
            ));
        }
        Ok(())
    }
    fn skip(&mut self) -> Result<()> {
        loop {
            let start = self.at;
            while self.at < self.bytes.len() && space(self.bytes[self.at]) {
                self.at += 1;
            }
            if self.at > start {
                self.charge()?;
                self.masked.push(start..self.at);
            }
            if self.bytes.get(self.at) == Some(&b'%') {
                let start = self.at;
                while self.at < self.bytes.len() && !matches!(self.bytes[self.at], b'\r' | b'\n') {
                    self.at += 1;
                }
                self.charge()?;
                self.masked.push(start..self.at);
            } else {
                break;
            }
        }
        Ok(())
    }
    /// true means an operand; false means a bare operator token.
    fn token(&mut self, depth: usize) -> Result<(Range<usize>, bool)> {
        if depth >= 100 {
            return Err(Error::Limit("content span nesting exceeds 100"));
        }
        self.skip()?;
        self.charge()?;
        let start = self.at;
        let b = *self
            .bytes
            .get(self.at)
            .ok_or(Error::Invalid("unfinished content operand"))?;
        self.at += 1;
        match b {
            b'/' => {
                while self.at < self.bytes.len() && !delimiter(self.bytes[self.at]) {
                    if self.bytes[self.at] == b'#' {
                        if self.at + 2 >= self.bytes.len()
                            || !self.bytes[self.at + 1..self.at + 3]
                                .iter()
                                .all(u8::is_ascii_hexdigit)
                        {
                            return Err(Error::Refused("ambiguous content name escape"));
                        }
                        self.at += 3;
                    } else {
                        self.at += 1;
                    }
                }
            }
            b'(' => {
                let mut nesting = 1usize;
                while nesting > 0 {
                    let b = *self
                        .bytes
                        .get(self.at)
                        .ok_or(Error::Invalid("unfinished content literal string"))?;
                    self.at += 1;
                    match b {
                        b'\\' => {
                            let next = *self
                                .bytes
                                .get(self.at)
                                .ok_or(Error::Invalid("unfinished content string escape"))?;
                            self.at += 1;
                            if next == b'\r' && self.bytes.get(self.at) == Some(&b'\n') {
                                self.at += 1;
                            }
                        }
                        b'(' => {
                            nesting += 1;
                            if nesting + depth >= 100 {
                                return Err(Error::Limit("content literal nesting exceeds 100"));
                            }
                        }
                        b')' => nesting -= 1,
                        _ => {}
                    }
                }
            }
            b'[' => loop {
                self.skip()?;
                if self.bytes.get(self.at) == Some(&b']') {
                    self.at += 1;
                    break;
                }
                let (_, operand) = self.token(depth + 1)?;
                if !operand {
                    return Err(Error::Refused("unsupported token inside content array"));
                }
            },
            b'<' if self.bytes.get(self.at) == Some(&b'<') => {
                self.at += 1;
                loop {
                    self.skip()?;
                    if self.bytes.get(self.at..self.at + 2) == Some(b">>") {
                        self.at += 2;
                        break;
                    }
                    if self.bytes.get(self.at) != Some(&b'/') {
                        return Err(Error::Refused("content dictionary keys must be names"));
                    }
                    self.token(depth + 1)?;
                    let (_, operand) = self.token(depth + 1)?;
                    if !operand {
                        return Err(Error::Refused("unsupported content dictionary value"));
                    }
                }
            }
            b'<' => loop {
                let b = *self
                    .bytes
                    .get(self.at)
                    .ok_or(Error::Invalid("unfinished content hex string"))?;
                self.at += 1;
                if b == b'>' {
                    break;
                }
                if !space(b) && !b.is_ascii_hexdigit() {
                    return Err(Error::Refused("invalid content hex string"));
                }
            },
            b')' | b']' | b'>' | b'{' | b'}' => {
                return Err(Error::Refused(
                    "unbalanced or unsupported content delimiter",
                ));
            }
            _ => {
                while self.at < self.bytes.len() && !delimiter(self.bytes[self.at]) {
                    self.at += 1;
                }
                let atom = &self.bytes[start..self.at];
                return Ok((
                    start..self.at,
                    numeric(atom) || matches!(atom, b"true" | b"false" | b"null"),
                ));
            }
        }
        Ok((start..self.at, true))
    }
}
/// Reconcile lexical operators and top-level operands with lopdf's validated program.
/// Operators/operand ranges exclude comments and whitespace; full includes whitespace
/// between the first operand and operator. Inline images and ambiguous syntax refuse.
fn scan(bytes: &[u8]) -> Result<(Vec<OperatorSpan>, Vec<Range<usize>>)> {
    if bytes.len() > crate::MAX_STREAM_BYTES {
        return Err(Error::Limit("content span input exceeds 32 MiB"));
    }
    let mut lexer = Lexer {
        bytes,
        at: 0,
        work: 0,
        masked: vec![],
    };
    let mut spans = Vec::new();
    let mut operands = Vec::new();
    loop {
        lexer.skip()?;
        if lexer.at == bytes.len() {
            break;
        }
        let (range, operand) = lexer.token(0)?;
        if operand {
            operands.push(range);
            continue;
        }
        if matches!(&bytes[range.clone()], b"BI" | b"ID" | b"EI") {
            return Err(Error::Refused(
                "inline images have no supported content spans",
            ));
        }
        if spans.len() >= 100_000 {
            return Err(Error::Limit("content span operator budget"));
        }
        let start = operands.first().map_or(range.start, |r| r.start);
        let end = range.end;
        spans.push(OperatorSpan {
            operands: std::mem::take(&mut operands),
            operator: range,
            full: start..end,
        });
    }
    if !operands.is_empty() {
        return Err(Error::Refused("content spans have trailing operands"));
    }
    Ok((spans, lexer.masked))
}
fn reconcile(
    bytes: &[u8],
    validated: &Content<Vec<Operation>>,
    spans: &[OperatorSpan],
) -> Result<()> {
    if spans.len() != validated.operations.len() {
        return Err(Error::Refused(
            "content lexical operator count is ambiguous",
        ));
    }
    for (span, op) in spans.iter().zip(&validated.operations) {
        if &bytes[span.operator.clone()] != op.operator.as_bytes()
            || span.operands.len() != op.operands.len()
        {
            return Err(Error::Refused(
                "content lexical operators or operands disagree with parsed program",
            ));
        }
    }
    Ok(())
}
/// Lex raw content and validate its program with lopdf. Only comments and PDF
/// whitespace OUTSIDE strings/hex/name tokens are masked in the temporary parser
/// copy, because lopdf rejects valid inter-operand comments. Original bytes/ranges
/// remain intact. Operators and operand counts must reconcile exactly.
pub(crate) fn decode_spanned(bytes: &[u8]) -> Result<(Content<Vec<Operation>>, Vec<OperatorSpan>)> {
    let (spans, masked) = scan(bytes)?;
    let mut parser = bytes.to_vec();
    for range in masked {
        parser[range].fill(b' ');
    }
    let validated = Content::decode_strict(&parser)?;
    reconcile(bytes, &validated, &spans)?;
    Ok((validated, spans))
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[test]
    fn comments_escapes_nested_objects_and_high_precision_tokens_keep_exact_ranges() {
        let bytes=b"% initial\nq 0.1234567890123456789 0 0 1 2 3 cm\n/Im#31 % between\nDo BT /F#31 12 Tf (q \\(fake Do\\) \\053) Tj [<444f> -0.1234567890123456789 (ET)] TJ ET /Span << /Nested [true null << /Key (cm) >>] /ActualText <FEFF0041> >> BDC EMC Q % tail";
        let (content, spans) = decode_spanned(bytes).unwrap();
        assert_eq!(spans.len(), content.operations.len());
        let cm = content
            .operations
            .iter()
            .position(|op| op.operator == "cm")
            .unwrap();
        assert_eq!(
            &bytes[spans[cm].operands[0].clone()],
            b"0.1234567890123456789"
        );
        let draw = content
            .operations
            .iter()
            .position(|op| op.operator == "Do")
            .unwrap();
        assert_eq!(&bytes[spans[draw].operands[0].clone()], b"/Im#31");
        assert_eq!(&bytes[spans[draw].operator.clone()], b"Do");
        assert_eq!(&bytes[spans[draw].full.clone()], b"/Im#31 % between\nDo");
    }
    #[test]
    fn malformed_ambiguous_or_inline_data_fail_closed() {
        for bytes in [
            b"(unfinished".as_slice(),
            b"[1 q] TJ",
            b"<< /Key q >> BDC",
            b"/Bad#GG Do",
            b"BI /W 1 ID data EI",
            b"1 2",
            b"<GG> Tj",
        ] {
            assert!(decode_spanned(bytes).is_err());
        }
        let parsed = Content::decode_strict(b"q Q").unwrap();
        let (spans, _) = scan(b"q q").unwrap();
        assert!(reconcile(b"q q", &parsed, &spans).is_err());
    }
}
