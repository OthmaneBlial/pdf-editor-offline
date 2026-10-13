//! Fault injection fixture; excluded from production builds and document handler.
#![allow(clippy::unwrap_used)]
use pdf_worker::{PayloadKind, Reply, Request};
use std::io::Write;

#[inline(never)]
#[allow(clippy::large_stack_arrays)]
fn processing_stack_probe() -> u8 {
    let mut bytes = [0_u8; 2 * 1024 * 1024];
    let last = bytes.len() - 1;
    bytes[0] = 17;
    bytes[last] = 29;
    std::hint::black_box(&bytes);
    bytes[0] + bytes[last]
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "supervisor" {
        let marker = std::env::args().nth(2).unwrap();
        let result = pdf_worker::execute(
            &std::env::current_exe().unwrap(),
            &["watch-parent".into()],
            Request {
                control: serde_json::json!({"marker":marker}),
                payload: Vec::new(),
            },
            pdf_worker::Limits::default(),
        );
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    // Reserve without touching before Darwin's VM baseline is captured. This
    // fixture proves the parent watchdog catches commits within reservations.
    let mut resident = if mode == "resident" {
        Vec::<u8>::with_capacity(96 * 1024 * 1024)
    } else {
        Vec::new()
    };
    if mode == "malformed" {
        std::io::stdout().write_all(&[0; 54]).unwrap();
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        return;
    }
    if mode == "truncated" {
        std::io::stdout().write_all(b"PDFW").unwrap();
        return;
    }
    if mode == "oversize" {
        let mut frame = [0u8; 54];
        frame[..8].copy_from_slice(b"PDFWRK01");
        frame[8] = 1;
        frame[10..14].copy_from_slice(&(pdf_worker::MAX_CONTROL_BYTES as u32 + 1).to_be_bytes());
        std::io::stdout().write_all(&frame).unwrap();
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        return;
    }
    let result = pdf_worker::serve_once(|request: Request| match mode.as_str() {
        "processing-stack" => Ok(Reply {
            control: serde_json::json!({
                "probe": processing_stack_probe(),
                "thread": std::thread::current().name(),
            }),
            payload: Vec::new(),
            kind: PayloadKind::None,
        }),
        "json" | "text" => {
            let mut payload = vec![b'a'; pdf_worker::MAX_CONTROL_BYTES + 4096];
            let kind = if mode == "json" {
                payload.insert(0, b'"');
                payload.push(b'"');
                PayloadKind::Json
            } else {
                PayloadKind::Text
            };
            Ok(Reply {
                control: request.control,
                payload,
                kind,
            })
        }
        "cli-oversize" | "cli-text-oversize" => {
            let mut frame = [0u8; 54];
            frame[..8].copy_from_slice(b"PDFWRK01");
            frame[8] = 3;
            let (kind, limit) = if mode == "cli-oversize" {
                (3, pdf_worker::MAX_JSON_BYTES)
            } else {
                (4, pdf_worker::MAX_TEXT_BYTES)
            };
            frame[9] = kind;
            frame[14..22].copy_from_slice(&(limit as u64 + 1).to_be_bytes());
            std::io::stdout().write_all(&frame).unwrap();
            std::io::stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
            std::process::exit(0);
        }
        "watch-parent" => {
            let marker = request.control["marker"].as_str().unwrap();
            std::fs::write(marker, std::process::id().to_string()).unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        "resident" => {
            resident.resize(96 * 1024 * 1024, 42);
            std::hint::black_box(&resident);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        "abort" => std::process::abort(),
        "timeout" => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
        "cpu" => loop {
            std::hint::black_box(123_u64.wrapping_mul(789));
        },
        "stderr" => loop {
            if std::io::stderr().write_all(&[b'x'; 4096]).is_err() {
                return Err("stderr disconnected".into());
            }
        },
        "memory" => {
            let small = pdf_worker::allocation_probe(8 * 1024 * 1024).map_err(|e| e.to_string())?;
            let large =
                pdf_worker::allocation_probe(96 * 1024 * 1024).map_err(|e| e.to_string())?;
            Ok(Reply {
                control: serde_json::json!({"small_mapping_succeeded":small.is_none(),"allocation_refused":large.is_some(),"kernel_error":large}),
                payload: Vec::new(),
                kind: PayloadKind::None,
            })
        }
        "checksum" | "cli-checksum" => {
            let json = b"{}";
            let mut frame = [0u8; 54];
            frame[..8].copy_from_slice(b"PDFWRK01");
            frame[8] = 3;
            frame[9] = if mode == "cli-checksum" { 3 } else { 1 };
            frame[10..14].copy_from_slice(&2u32.to_be_bytes());
            frame[14..22].copy_from_slice(&1u64.to_be_bytes());
            let mut out = std::io::stdout().lock();
            out.write_all(&frame).unwrap();
            out.write_all(json).unwrap();
            out.write_all(&[1]).unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        "trailing" => {
            use sha2::Digest;
            let mut frame = [0; 54];
            frame[..8].copy_from_slice(b"PDFWRK01");
            frame[8] = 3;
            frame[10..14].copy_from_slice(&2u32.to_be_bytes());
            frame[22..].copy_from_slice(&sha2::Sha256::digest([]));
            let mut out = std::io::stdout().lock();
            out.write_all(&frame).unwrap();
            out.write_all(b"{}").unwrap();
            out.write_all(b"unframed").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
        _ => Ok(Reply {
            control: request.control,
            payload: request.payload,
            kind: PayloadKind::Pdf,
        }),
    });
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
