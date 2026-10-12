//! One command per bounded child. Authorized filesystem access is unchanged.
use super::{
    Cli, Command, error_class, error_envelope, finish_output, output_json, output_stdout, run,
};
use clap::Parser;
use pdf_worker::{PayloadKind, Reply, Request};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

pub(super) const CHILD_FLAG: &str = "--pdf-cli-worker";
const MAX_JSON: usize = pdf_worker::MAX_JSON_BYTES;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    arguments: Vec<OsString>,
    password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Format {
    Json,
    HumanJson,
    Text,
    HumanError,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    code: u8,
    format: Format,
    error_kind: Option<String>,
}

fn document_command(command: &Command) -> bool {
    !matches!(command, Command::Capabilities | Command::Completions { .. })
}

pub(super) fn serve() -> ExitCode {
    match pdf_worker::serve_once(handle) {
        Ok(()) => ExitCode::SUCCESS,
        // The parent receives a transport failure; never print into the framed output.
        Err(_) => ExitCode::from(5),
    }
}

fn handle(request: Request) -> Result<Reply, String> {
    if !request.payload.is_empty() {
        return Err("Unexpected CLI request payload".into());
    }
    let invocation: Invocation =
        serde_json::from_value(request.control).map_err(|_| "Invalid CLI invocation")?;
    validate_invocation(&invocation)?;
    let cli =
        Cli::try_parse_from(&invocation.arguments).map_err(|_| "Invalid CLI child arguments")?;
    if !document_command(&cli.command) {
        return Err("Local commands cannot run in the document worker".into());
    }
    if cli.password_stdin != invocation.password.is_some() {
        return Err("Invalid password transport".into());
    }
    let result = run(&cli, invocation.password.as_deref());
    prepare_output(cli.json, result)
}

fn validate_invocation(invocation: &Invocation) -> Result<(), String> {
    if invocation.arguments.is_empty() {
        return Err("CLI invocation must include the executable argument".into());
    }
    for argument in &invocation.arguments {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            if argument.as_bytes().contains(&0) {
                return Err("Invalid native CLI argument".into());
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            if argument.encode_wide().any(|unit| unit == 0) {
                return Err("Invalid native CLI argument".into());
            }
        }
    }
    if invocation
        .password
        .as_ref()
        .is_some_and(|password| password.len() > 4096)
    {
        return Err("Password input exceeds 4096 bytes".into());
    }
    Ok(())
}

struct BoundedOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("CLI output exceeds its transport limit"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("CLI output allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn prepare_output(json_requested: bool, result: pdf_core::Result<Value>) -> Result<Reply, String> {
    let (output, payload) = match result {
        Ok(data) => {
            if !json_requested && let Some(text) = data.get("text").and_then(Value::as_str) {
                if text.len() > pdf_core::MAX_INPUT_BYTES {
                    return Err("CLI text exceeds its transport limit".into());
                }
                (
                    Output {
                        code: 0,
                        format: Format::Text,
                        error_kind: None,
                    },
                    text.as_bytes().to_vec(),
                )
            } else {
                let data = if json_requested {
                    json!({"ok":true,"data":data})
                } else {
                    data
                };
                let mut writer = BoundedOutput {
                    bytes: Vec::new(),
                    limit: MAX_JSON,
                };
                if json_requested {
                    serde_json::to_writer(&mut writer, &data)
                } else {
                    serde_json::to_writer_pretty(&mut writer, &data)
                }
                .map_err(|_| "CLI JSON exceeds its transport limit")?;
                writer
                    .write_all(b"\n")
                    .map_err(|_| "CLI JSON exceeds its transport limit")?;
                (
                    Output {
                        code: 0,
                        format: if json_requested {
                            Format::Json
                        } else {
                            Format::HumanJson
                        },
                        error_kind: None,
                    },
                    writer.bytes,
                )
            }
        }
        Err(error) => {
            let (code, kind) = error_class(&error);
            if json_requested {
                let mut writer = BoundedOutput {
                    bytes: Vec::new(),
                    limit: MAX_JSON,
                };
                serde_json::to_writer(&mut writer, &error_envelope(kind, &error.to_string()))
                    .map_err(|_| "CLI error output exceeds its transport limit")?;
                writer
                    .write_all(b"\n")
                    .map_err(|_| "CLI error output exceeds its transport limit")?;
                (
                    Output {
                        code,
                        format: Format::Json,
                        error_kind: Some(kind.into()),
                    },
                    writer.bytes,
                )
            } else {
                (
                    Output {
                        code,
                        format: Format::HumanError,
                        error_kind: Some(kind.into()),
                    },
                    format!("Error: {error}\n").into_bytes(),
                )
            }
        }
    };
    let kind = match output.format {
        Format::Json | Format::HumanJson => PayloadKind::Json,
        Format::Text | Format::HumanError => PayloadKind::Text,
    };
    Ok(Reply {
        control: serde_json::to_value(output).map_err(|_| "Invalid CLI output control")?,
        payload,
        kind,
    })
}

pub(super) fn execute(
    arguments: Vec<OsString>,
    password: Option<String>,
    json_requested: bool,
) -> ExitCode {
    let result = (|| {
        let executable = std::env::current_exe().map_err(pdf_worker::Error::Io)?;
        let invocation = Invocation {
            arguments,
            password,
        };
        validate_invocation(&invocation).map_err(pdf_worker::Error::Limits)?;
        let control = serde_json::to_value(invocation)
            .map_err(|_| pdf_worker::Error::Protocol("invalid CLI invocation"))?;
        let reply = pdf_worker::execute_cli(
            &executable,
            &[OsString::from(CHILD_FLAG)],
            Request {
                control,
                payload: Vec::new(),
            },
            pdf_worker::Limits::default(),
        )?;
        let output = validate_reply(&reply, json_requested).map_err(pdf_worker::Error::Protocol)?;
        Ok::<_, pdf_worker::Error>((output, reply.payload))
    })();
    match result {
        Ok((output, payload)) => {
            if output.format == Format::HumanError {
                let mut stderr = io::stderr().lock();
                finish_output(
                    stderr.write_all(&payload).and_then(|()| stderr.flush()),
                    output.code,
                )
            } else {
                output_stdout(&payload, output.code)
            }
        }
        Err(error) => {
            let (code, kind) = transport_failure_class(&error);
            let detail = match &error {
                pdf_worker::Error::Timeout => "CLI worker timed out".to_owned(),
                _ => error.to_string(),
            };
            let message = format!("CLI worker failed; output files may already exist: {detail}");
            if json_requested {
                output_json(&error_envelope(kind, &message), false, code)
            } else {
                finish_output(writeln!(io::stderr().lock(), "Error: {message}"), code)
            }
        }
    }
}

fn transport_failure_class(error: &pdf_worker::Error) -> (u8, &'static str) {
    match error {
        pdf_worker::Error::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::PermissionDenied
                    | io::ErrorKind::NotADirectory
                    | io::ErrorKind::IsADirectory
            ) =>
        {
            (3, "filesystem")
        }
        // A failed pipe is a failed worker reply, not an error opening the user's document.
        _ => (5, "resource_limit"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn reply(code: u8, format: Format, kind: Option<&str>, payload: &[u8]) -> Reply {
        let payload_kind = if matches!(format, Format::Json | Format::HumanJson) {
            PayloadKind::Json
        } else {
            PayloadKind::Text
        };
        Reply {
            control: serde_json::to_value(Output {
                code,
                format,
                error_kind: kind.map(str::to_owned),
            })
            .unwrap(),
            payload: payload.to_vec(),
            kind: payload_kind,
        }
    }

    #[test]
    fn output_roundtrips_preserve_json_pretty_human_and_unmodified_text() {
        let data = json!({"text":"Hello\n中文\t"});
        let json = prepare_output(true, Ok(data.clone())).unwrap();
        assert_eq!(
            json.payload,
            b"{\"data\":{\"text\":\"Hello\\n\xe4\xb8\xad\xe6\x96\x87\\t\"},\"ok\":true}\n"
        );
        assert_eq!(validate_reply(&json, true).unwrap().code, 0);
        let text = prepare_output(false, Ok(data)).unwrap();
        assert_eq!(text.payload, "Hello\n中文\t".as_bytes());
        assert_eq!(validate_reply(&text, false).unwrap().format, Format::Text);
        let human = prepare_output(false, Ok(json!({"pages":2}))).unwrap();
        assert_eq!(human.payload, b"{\n  \"pages\": 2\n}\n");
        assert_eq!(
            validate_reply(&human, false).unwrap().format,
            Format::HumanJson
        );
    }

    #[test]
    fn error_output_retains_original_codes_envelopes_and_stderr_message() {
        for (error, code, kind) in [
            (pdf_core::Error::Refused("guard"), 4, "refused"),
            (pdf_core::Error::Limit("bound"), 5, "resource_limit"),
            (
                pdf_core::Error::Io(io::Error::other("disk")),
                3,
                "filesystem",
            ),
            (pdf_core::Error::Invalid("PDF"), 1, "document"),
        ] {
            let expected = format!("Error: {error}\n");
            let human = prepare_output(false, Err(error)).unwrap();
            assert_eq!(human.payload, expected.as_bytes());
            let output = validate_reply(&human, false).unwrap();
            assert_eq!(output.code, code);
            assert_eq!(output.error_kind.as_deref(), Some(kind));
            let json = reply(
                code,
                Format::Json,
                Some(kind),
                serde_json::to_vec(&error_envelope(kind, "original failure"))
                    .unwrap()
                    .as_slice(),
            );
            assert_eq!(validate_reply(&json, true).unwrap().code, code);
        }
    }

    #[test]
    fn response_validation_rejects_mismatches_and_malformed_envelopes() {
        for invalid in [
            reply(6, Format::Text, None, b"text"),
            reply(
                4,
                Format::Json,
                Some("document"),
                br#"{"ok":false,"error":{"kind":"document","message":"error"}}"#,
            ),
            reply(
                4,
                Format::Json,
                Some("refused"),
                br#"{"ok":true,"data":{}}"#,
            ),
            reply(
                0,
                Format::Json,
                None,
                br#"{"ok":false,"error":{"kind":"document","message":"error"}}"#,
            ),
            reply(0, Format::Json, None, br#"{"ok":true}"#),
            reply(0, Format::Json, None, br#"{"ok":true,"ok":true,"data":{}}"#),
            reply(0, Format::Json, None, br#"{"ok":true,"data":{}} trailing"#),
            reply(
                0,
                Format::Json,
                None,
                br#"{"ok":true,"data":{},"unknown":0}"#,
            ),
            reply(0, Format::Text, None, b"\xff"),
            reply(0, Format::HumanError, None, b"Error: invalid success\n"),
        ] {
            let json_requested = invalid.kind == PayloadKind::Json;
            assert!(validate_reply(&invalid, json_requested).is_err());
        }
        let mut invalid = reply(0, Format::Text, None, b"hello");
        invalid.kind = PayloadKind::Pdf;
        assert!(validate_reply(&invalid, false).is_err());
        let valid = reply(0, Format::Json, None, br#"{"ok":true,"data":{}}"#);
        assert!(validate_reply(&valid, false).is_err());
    }

    #[test]
    fn native_argv_roundtrip_keeps_empty_large_values_and_nonunicode_units() {
        let mut arguments = vec![
            OsString::from("pdf-editor-offline"),
            OsString::from(""),
            OsString::from("x".repeat(64 * 1024)),
        ];
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            arguments.push(OsString::from_vec(vec![b'a', 0xff, b'b']));
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            arguments.push(OsString::from_wide(&[0x61, 0xd800, 0x62]));
        }
        let invocation = Invocation {
            arguments: arguments.clone(),
            password: Some("private\npassword".into()),
        };
        let decoded: Invocation =
            serde_json::from_slice(&serde_json::to_vec(&invocation).unwrap()).unwrap();
        assert_eq!(decoded.arguments, arguments);
        assert_eq!(decoded.password, invocation.password);
        assert!(validate_invocation(&decoded).is_ok());
    }

    #[test]
    fn request_rejects_invalid_password_payload_and_local_commands() {
        let invocation = Invocation {
            arguments: vec!["pdf-editor-offline".into(), "capabilities".into()],
            password: None,
        };
        let request = Request {
            control: serde_json::to_value(invocation).unwrap(),
            payload: Vec::new(),
        };
        assert!(handle(request).is_err());
        let invocation = Invocation {
            arguments: vec![
                "pdf-editor-offline".into(),
                "info".into(),
                "missing.pdf".into(),
            ],
            password: Some("x".repeat(4097)),
        };
        assert!(validate_invocation(&invocation).is_err());
        let invocation = Invocation {
            arguments: vec![
                "pdf-editor-offline".into(),
                "info".into(),
                "missing.pdf".into(),
            ],
            password: Some("not from stdin".into()),
        };
        assert!(
            handle(Request {
                control: serde_json::to_value(invocation).unwrap(),
                payload: Vec::new()
            })
            .is_err()
        );
    }

    #[test]
    fn wire_bounded_many_arguments_keep_the_existing_image_refusal_code() {
        let invocation = Invocation {
            arguments: vec![OsString::from("small"); 20_129],
            password: None,
        };
        assert!(validate_invocation(&invocation).is_ok());
        assert!(serde_json::to_vec(&invocation).unwrap().len() < pdf_worker::MAX_CONTROL_BYTES);
        let cli = Cli {
            json: true,
            password_stdin: false,
            command: Command::ImagesToPdf {
                inputs: vec![std::path::PathBuf::from("not-read.png"); 20_129],
                output: std::path::PathBuf::from("never-created.pdf"),
                dry_run: false,
            },
        };
        let response = prepare_output(true, run(&cli, None)).unwrap();
        let output = validate_reply(&response, true).unwrap();
        assert_eq!(output.code, 5);
        assert_eq!(output.error_kind.as_deref(), Some("resource_limit"));
        let envelope: Value = serde_json::from_slice(&response.payload).unwrap();
        assert!(
            envelope["error"]["message"]
                .as_str()
                .unwrap()
                .contains("more than 20000 image pages")
        );
    }

    #[test]
    fn bounded_json_writer_and_existing_broken_pipe_semantics_are_preserved() {
        let mut writer = BoundedOutput {
            bytes: Vec::new(),
            limit: 4,
        };
        writer.write_all(b"1234").unwrap();
        assert!(writer.write_all(b"5").is_err());
        assert_eq!(writer.bytes, b"1234");
        assert_eq!(
            finish_output(Err(io::Error::from(io::ErrorKind::BrokenPipe)), 0),
            ExitCode::from(0)
        );
        assert_eq!(
            finish_output(Err(io::Error::from(io::ErrorKind::BrokenPipe)), 4),
            ExitCode::from(4)
        );
        for kind in [
            io::ErrorKind::UnexpectedEof,
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::OutOfMemory,
        ] {
            assert_eq!(
                transport_failure_class(&pdf_worker::Error::Io(io::Error::from(kind))),
                (5, "resource_limit")
            );
        }
        assert_eq!(
            transport_failure_class(&pdf_worker::Error::Timeout),
            (5, "resource_limit")
        );
        assert_eq!(
            transport_failure_class(&pdf_worker::Error::Io(io::Error::from(
                io::ErrorKind::PermissionDenied
            ))),
            (3, "filesystem")
        );
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    ok: bool,
    data: Option<serde::de::IgnoredAny>,
    error: Option<EnvelopeError>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeError {
    kind: String,
    message: String,
}

fn validate_reply(reply: &Reply, json_requested: bool) -> Result<Output, &'static str> {
    let output: Output = serde_json::from_value(reply.control.clone())
        .map_err(|_| "invalid CLI response control")?;
    let expected_kind = match output.code {
        0 => None,
        1 => Some("document"),
        2 => Some("arguments"),
        3 => Some("filesystem"),
        4 => Some("refused"),
        5 => Some("resource_limit"),
        _ => return Err("invalid CLI response exit code"),
    };
    if output.error_kind.as_deref() != expected_kind {
        return Err("CLI response exit code disagrees with error kind");
    }
    let json_format = matches!(output.format, Format::Json | Format::HumanJson);
    if (reply.kind
        != if json_format {
            PayloadKind::Json
        } else {
            PayloadKind::Text
        })
        || reply.payload.len()
            > if json_format {
                MAX_JSON
            } else {
                pdf_core::MAX_INPUT_BYTES
            }
        || (json_requested != (output.format == Format::Json))
    {
        return Err("invalid CLI response format");
    }
    std::str::from_utf8(&reply.payload).map_err(|_| "CLI output is not UTF-8")?;
    match output.format {
        Format::Json => {
            let envelope: Envelope =
                serde_json::from_slice(&reply.payload).map_err(|_| "invalid CLI JSON envelope")?;
            if output.code == 0 {
                if !envelope.ok || envelope.data.is_none() || envelope.error.is_some() {
                    return Err("invalid CLI success envelope");
                }
            } else {
                let error = envelope.error.ok_or("missing CLI error envelope")?;
                if envelope.ok
                    || envelope.data.is_some()
                    || Some(error.kind.as_str()) != expected_kind
                    || error.message.is_empty()
                {
                    return Err("invalid CLI error envelope");
                }
            }
        }
        Format::HumanJson => {
            if output.code != 0 {
                return Err("invalid human JSON exit code");
            }
            serde_json::from_slice::<serde::de::IgnoredAny>(&reply.payload)
                .map_err(|_| "invalid human JSON output")?;
        }
        Format::Text => {
            if output.code != 0 {
                return Err("invalid text exit code");
            }
        }
        Format::HumanError => {
            if output.code == 0
                || !reply.payload.starts_with(b"Error: ")
                || !reply.payload.ends_with(b"\n")
            {
                return Err("invalid human error output");
            }
        }
    }
    Ok(output)
}
