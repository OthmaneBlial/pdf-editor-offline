#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::process::Command;
#[path = "../../../tests/support/redaction_fixture.rs"]
mod fixture;
#[test]
fn isolated_cli_redacts_verifies_dry_runs_and_refuses_without_clobber_or_query_leaks() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let output = temp.path().join("redacted.pdf");
    let targets = temp.path().join("targets.json");
    let source = fixture::fixture();
    std::fs::write(&input, &source).unwrap();
    std::fs::write(
        &targets,
        serde_json::to_vec(&pdf_core::TextRedactionRequest {
            targets: vec![fixture::SECRET.into()],
        })
        .unwrap(),
    )
    .unwrap();
    let run = |operation: &str, path: &std::path::Path, dry: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
        command
            .args(["--json", operation])
            .arg(path)
            .arg("--targets-file")
            .arg(&targets);
        if operation == "redact-text" {
            command.arg("-o").arg(&output);
            if dry {
                command.arg("--dry-run");
            }
        }
        command.output().unwrap()
    };
    let dry = run("redact-text", &input, true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    assert!(
        !dry.stdout
            .windows(fixture::SECRET.len())
            .any(|v| v == fixture::SECRET.as_bytes())
    );
    let result = run("redact-text", &input, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let json: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(json["data"]["report"]["changed_text_shows"], 2);
    assert_eq!(json["data"]["general_privacy_certificate"], false);
    let saved = std::fs::read(&output).unwrap();
    pdf_core::verify_redaction_bytes(&saved, None, &[fixture::SECRET.into()]).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    let text = pdf.extract_text(&[1, 2]).unwrap();
    assert!(text.contains("prefix") && text.contains("suffix"));
    assert!(!text.contains(fixture::SECRET));
    assert_eq!(
        run("verify-redaction", &output, false).status.code(),
        Some(0)
    );
    let verify_source = run("verify-redaction", &input, false);
    assert!(!verify_source.status.success());
    let error: Value = serde_json::from_slice(&verify_source.stdout).unwrap();
    assert_eq!(error["ok"], false);
    assert!(
        !verify_source
            .stdout
            .windows(fixture::SECRET.len())
            .any(|v| v == fixture::SECRET.as_bytes())
    );
    assert_eq!(run("redact-text", &input, false).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), source);
    let mut appended = saved.clone();
    appended.extend_from_slice(b"\n% <50524956415445>\n");
    let unsafe_copy = temp.path().join("ignored-carrier.pdf");
    std::fs::write(&unsafe_copy, appended).unwrap();
    assert!(
        !run("verify-redaction", &unsafe_copy, false)
            .status
            .success()
    );
    std::fs::write(&targets, br#"{"targets":["PRIVATE"],"path":"not-allowed"}"#).unwrap();
    assert_eq!(run("redact-text", &input, true).status.code(), Some(4));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
}
