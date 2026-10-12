use crate::{
    Error, Limits, MAX_CONTROL_BYTES, MAX_JSON_BYTES, MAX_PDF_BYTES, MAX_RGBA_BYTES,
    MAX_TEXT_BYTES, Result,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const MAGIC: &[u8; 8] = b"PDFWRK01";
const READY: u8 = 1;
const REQUEST: u8 = 2;
const REPLY: u8 = 3;
const HEADER: usize = 54;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    None,
    Pdf,
    Rgba,
    Json,
    Text,
}

#[derive(Clone, Copy)]
pub(crate) enum ReplyPolicy {
    Document,
    Cli,
}

impl ReplyPolicy {
    fn accepts(self, kind: PayloadKind) -> bool {
        match self {
            Self::Document => matches!(
                kind,
                PayloadKind::None | PayloadKind::Pdf | PayloadKind::Rgba
            ),
            Self::Cli => matches!(
                kind,
                PayloadKind::None | PayloadKind::Json | PayloadKind::Text
            ),
        }
    }
}
#[derive(Debug)]
pub struct Request {
    pub control: Value,
    pub payload: Vec<u8>,
}
#[derive(Debug)]
pub struct Reply {
    pub control: Value,
    pub payload: Vec<u8>,
    pub kind: PayloadKind,
}

fn limit(kind: PayloadKind) -> usize {
    match kind {
        PayloadKind::None => 0,
        PayloadKind::Pdf => MAX_PDF_BYTES,
        PayloadKind::Rgba => MAX_RGBA_BYTES,
        PayloadKind::Json => MAX_JSON_BYTES,
        PayloadKind::Text => MAX_TEXT_BYTES,
    }
}
fn write_frame(
    writer: &mut impl Write,
    frame: u8,
    kind: PayloadKind,
    control: &Value,
    payload: &[u8],
) -> Result<()> {
    if payload.len() > limit(kind) {
        return Err(Error::Protocol("binary payload exceeds kind limit"));
    }
    let control =
        serde_json::to_vec(control).map_err(|_| Error::Protocol("control cannot be encoded"))?;
    if control.len() > MAX_CONTROL_BYTES {
        return Err(Error::Protocol("control exceeds 8 MiB"));
    }
    let mut header = [0u8; HEADER];
    header[..8].copy_from_slice(MAGIC);
    header[8] = frame;
    header[9] = match kind {
        PayloadKind::None => 0,
        PayloadKind::Pdf => 1,
        PayloadKind::Rgba => 2,
        PayloadKind::Json => 3,
        PayloadKind::Text => 4,
    };
    header[10..14].copy_from_slice(&(control.len() as u32).to_be_bytes());
    header[14..22].copy_from_slice(&(payload.len() as u64).to_be_bytes());
    header[22..].copy_from_slice(&Sha256::digest(payload));
    writer.write_all(&header)?;
    writer.write_all(&control)?;
    writer.write_all(payload)?;
    Ok(())
}
fn read_frame(reader: &mut impl Read, frame: u8, policy: ReplyPolicy) -> Result<Reply> {
    let mut header = [0u8; HEADER];
    reader.read_exact(&mut header)?;
    if &header[..8] != MAGIC || header[8] != frame {
        return Err(Error::Protocol("magic/version/frame type mismatch"));
    }
    let kind = match header[9] {
        0 => PayloadKind::None,
        1 => PayloadKind::Pdf,
        2 => PayloadKind::Rgba,
        3 => PayloadKind::Json,
        4 => PayloadKind::Text,
        _ => return Err(Error::Protocol("unknown payload kind")),
    };
    if (frame == READY && kind != PayloadKind::None)
        || (frame == REQUEST && kind != PayloadKind::Pdf)
        || (frame == REPLY && !policy.accepts(kind))
    {
        return Err(Error::Protocol("payload kind does not match frame role"));
    }
    let control_len = u32::from_be_bytes(
        header[10..14]
            .try_into()
            .map_err(|_| Error::Protocol("control length"))?,
    ) as usize;
    let payload_len = u64::from_be_bytes(
        header[14..22]
            .try_into()
            .map_err(|_| Error::Protocol("payload length"))?,
    );
    if control_len > MAX_CONTROL_BYTES || payload_len > limit(kind) as u64 {
        return Err(Error::Protocol("frame length exceeds limit"));
    }
    let encoded = read_bytes(reader, control_len)?;
    let control =
        serde_json::from_slice(&encoded).map_err(|_| Error::Protocol("invalid control JSON"))?;
    let payload = read_bytes(reader, payload_len as usize)?;
    if Sha256::digest(&payload).as_slice() != &header[22..] {
        return Err(Error::Protocol("binary payload checksum mismatch"));
    }
    Ok(Reply {
        control,
        payload,
        kind,
    })
}

fn read_bytes(reader: &mut impl Read, length: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    while bytes.len() < length {
        let count = (length - bytes.len()).min(chunk.len());
        reader.read_exact(&mut chunk[..count])?;
        let needed = bytes.len() + count;
        if bytes.capacity() < needed {
            let capacity = bytes
                .capacity()
                .max(chunk.len())
                .saturating_mul(2)
                .max(needed)
                .min(length);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(|_| Error::Protocol("frame allocation exceeds available memory"))?;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}
pub(crate) fn write_ready(writer: &mut impl Write, limits: Limits) -> Result<()> {
    write_frame(
        writer,
        READY,
        PayloadKind::None,
        &serde_json::json!({"memory":limits.memory,"cpu":limits.cpu}),
        &[],
    )
}
pub(crate) fn read_ready(reader: &mut impl Read, limits: Limits) -> Result<()> {
    let ready = read_frame(reader, READY, ReplyPolicy::Document)?;
    if ready.kind != PayloadKind::None
        || ready.control != serde_json::json!({"memory":limits.memory,"cpu":limits.cpu})
    {
        return Err(Error::Protocol("resource handshake mismatch"));
    }
    Ok(())
}
pub(crate) fn write_request(writer: &mut impl Write, request: &Request) -> Result<()> {
    write_frame(
        writer,
        REQUEST,
        PayloadKind::Pdf,
        &request.control,
        &request.payload,
    )
}
pub(crate) fn read_request(reader: &mut impl Read) -> Result<Request> {
    let request = read_frame(reader, REQUEST, ReplyPolicy::Document)?;
    if request.kind != PayloadKind::Pdf {
        return Err(Error::Protocol("request payload must use input budget"));
    }
    Ok(Request {
        control: request.control,
        payload: request.payload,
    })
}
pub(crate) fn write_reply(writer: &mut impl Write, reply: &Reply) -> Result<()> {
    write_frame(writer, REPLY, reply.kind, &reply.control, &reply.payload)
}
pub(crate) fn read_reply(reader: &mut impl Read, policy: ReplyPolicy) -> Result<Reply> {
    read_frame(reader, REPLY, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn cli_output_above_control_budget_roundtrips_without_widening_document_role() -> Result<()> {
        for kind in [PayloadKind::Text, PayloadKind::Json] {
            let mut payload = vec![b'a'; MAX_CONTROL_BYTES + 4096];
            if kind == PayloadKind::Json {
                payload.insert(0, b'"');
                payload.push(b'"');
            }
            let reply = Reply {
                control: serde_json::json!({"code":0}),
                payload,
                kind,
            };
            let mut encoded = Vec::new();
            write_reply(&mut encoded, &reply)?;
            let mut reader = Cursor::new(&encoded);
            let decoded = read_reply(&mut reader, ReplyPolicy::Cli)?;
            assert_eq!(decoded.kind, kind);
            assert_eq!(decoded.payload, reply.payload);
            assert_eq!(decoded.control, reply.control);
            let mut reader = Cursor::new(&encoded);
            assert!(matches!(
                read_reply(&mut reader, ReplyPolicy::Document),
                Err(Error::Protocol("payload kind does not match frame role"))
            ));
            assert_eq!(reader.position(), HEADER as u64);
        }
        Ok(())
    }

    #[test]
    fn reply_role_and_size_errors_precede_control_or_payload_allocation() {
        for (kind, length) in [(3, MAX_JSON_BYTES + 1), (4, MAX_TEXT_BYTES + 1), (255, 0)] {
            let mut header = [0u8; HEADER];
            header[..8].copy_from_slice(MAGIC);
            header[8] = REPLY;
            header[9] = kind;
            header[14..22].copy_from_slice(&(length as u64).to_be_bytes());
            let mut reader = Cursor::new(header);
            assert!(matches!(
                read_reply(&mut reader, ReplyPolicy::Cli),
                Err(Error::Protocol(_))
            ));
            assert_eq!(reader.position(), HEADER as u64);
        }
        let reply = Reply {
            control: serde_json::json!({}),
            payload: vec![1],
            kind: PayloadKind::Pdf,
        };
        let mut encoded = Vec::new();
        assert!(write_reply(&mut encoded, &reply).is_ok());
        let mut reader = Cursor::new(encoded);
        assert!(matches!(
            read_reply(&mut reader, ReplyPolicy::Cli),
            Err(Error::Protocol("payload kind does not match frame role"))
        ));
        assert_eq!(reader.position(), HEADER as u64);
    }

    #[test]
    fn oversized_declared_body_with_no_bytes_fails_on_first_small_read() {
        struct Empty;
        impl Read for Empty {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                assert!(bytes.len() <= 64 * 1024);
                Ok(0)
            }
        }
        assert!(matches!(
            read_bytes(&mut Empty, MAX_JSON_BYTES),
            Err(Error::Io(_))
        ));
    }
}
