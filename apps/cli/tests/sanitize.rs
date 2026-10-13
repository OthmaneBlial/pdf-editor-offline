#![allow(clippy::unwrap_used)]
use pdf_core::PdfDocument;
use serde_json::Value;
use std::process::Command;
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/attachment.pdf");
#[test]
fn actual_isolated_cli_dry_run_new_copy_and_no_clobber() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("copy.pdf");
    std::fs::write(&input, SOURCE).unwrap();
    let run = |dry: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
        command
            .args(["--json", "sanitize"])
            .arg(&input)
            .arg("-o")
            .arg(&output);
        if dry {
            command.arg("--dry-run");
        }
        command.output().unwrap()
    };
    let dry = run(true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let value: Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(value["data"]["privacy_proof"], false);
    assert_eq!(value["data"]["report"]["full_rewrite"], true);
    assert!(
        value["data"]["report"]["embedded_file_name_trees_removed"]
            .as_u64()
            .unwrap()
            > 0
    );
    let result = run(false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let before = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let after = PdfDocument::open(&output, None).unwrap();
    assert_eq!(before.info().unwrap().pages, after.info().unwrap().pages);
    assert_eq!(
        before.extract_text(&[1]).unwrap(),
        after.extract_text(&[1]).unwrap()
    );
    assert!(after.inspect_security().unwrap().embedded_files.is_empty());
    let saved = std::fs::read(&output).unwrap();
    let refused = run(false);
    assert_eq!(refused.status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), SOURCE);
}
#[test]
fn signed_and_malformed_sources_refuse_without_output() {
    for (source, code, kind) in [
        (
            include_bytes!("../../../tests/fixtures/pdfs/signed.pdf").as_slice(),
            4,
            "refused",
        ),
        (b"invalid".as_slice(), 1, "document"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source.pdf");
        let output = temp.path().join("copy.pdf");
        std::fs::write(&input, source).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args(["--json", "sanitize"])
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(code));
        let json: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json["error"]["kind"], kind);
        assert!(!output.exists());
        assert_eq!(std::fs::read(input).unwrap(), source);
    }
}

#[test]
fn decrypted_input_remains_readonly_with_password_only_on_stdin() {
    use lopdf::{EncryptionState, EncryptionVersion, Object, Permissions};
    use std::{io::Write, process::Stdio};
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("encrypted.pdf");
    let output = temp.path().join("copy.pdf");
    let mut pdf = lopdf::Document::load_mem(SOURCE).unwrap();
    pdf.trailer.set(
        "ID",
        vec![
            Object::string_literal("sanitize-first-id"),
            Object::string_literal("sanitize-next-id!"),
        ],
    );
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &pdf,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    pdf.encrypt(&state).unwrap();
    pdf.save(&input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(["--json", "--password-stdin", "sanitize"])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"reader\n").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(4));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["error"]["kind"], "refused");
    assert!(!String::from_utf8_lossy(&result.stdout).contains("reader"));
    assert!(!output.exists());
    assert_eq!(std::fs::read(input).unwrap(), original);
}
