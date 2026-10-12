use crate::{Error, Limits, MAX_CONTROL_BYTES, MAX_PDF_BYTES, MAX_RGBA_BYTES, Result};
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
    };
    header[10..14].copy_from_slice(&(control.len() as u32).to_be_bytes());
    header[14..22].copy_from_slice(&(payload.len() as u64).to_be_bytes());
    header[22..].copy_from_slice(&Sha256::digest(payload));
    writer.write_all(&header)?;
    writer.write_all(&control)?;
    writer.write_all(payload)?;
    Ok(())
}
fn read_frame(reader: &mut impl Read, frame: u8) -> Result<Reply> {
    let mut header = [0u8; HEADER];
    reader.read_exact(&mut header)?;
    if &header[..8] != MAGIC || header[8] != frame {
        return Err(Error::Protocol("magic/version/frame type mismatch"));
    }
    let kind = match header[9] {
        0 => PayloadKind::None,
        1 => PayloadKind::Pdf,
        2 => PayloadKind::Rgba,
        _ => return Err(Error::Protocol("unknown payload kind")),
    };
    if (frame == READY && kind != PayloadKind::None)
        || (frame == REQUEST && kind != PayloadKind::Pdf)
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
    let mut encoded = vec![0; control_len];
    reader.read_exact(&mut encoded)?;
    let control =
        serde_json::from_slice(&encoded).map_err(|_| Error::Protocol("invalid control JSON"))?;
    let mut payload = vec![0; payload_len as usize];
    reader.read_exact(&mut payload)?;
    if Sha256::digest(&payload).as_slice() != &header[22..] {
        return Err(Error::Protocol("binary payload checksum mismatch"));
    }
    Ok(Reply {
        control,
        payload,
        kind,
    })
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
    let ready = read_frame(reader, READY)?;
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
    let request = read_frame(reader, REQUEST)?;
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
pub(crate) fn read_reply(reader: &mut impl Read) -> Result<Reply> {
    read_frame(reader, REPLY)
}
