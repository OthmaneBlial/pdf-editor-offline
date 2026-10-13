#![allow(clippy::unwrap_used)]
use serde_json::json;
use std::{
    path::Path,
    process::{Command, Output},
};
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
fn run(input: &Path, request: &Path, output: &Path, dry: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
    cmd.arg("add-vector-overlay")
        .arg(input)
        .arg("--overlay-file")
        .arg(request)
        .arg("-o")
        .arg(output)
        .arg("--json");
    if dry {
        cmd.arg("--dry-run");
    }
    cmd.output().unwrap()
}
#[test]
fn production_cli_vector_dry_run_verified_output_no_clobber_and_signed_refusal() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let request = temp.path().join("overlay.json");
    let output = temp.path().join("new.pdf");
    std::fs::write(&input, PDF).unwrap();
    let value = json!({"page":1,"shape":{"shape":"ellipse","rectangle":[350,400,550,500]},"stroke":{"color":[0.8,0.1,0.0],"width":4},"fill":[0.0,0.4,0.6],"opacity":0.5});
    std::fs::write(&request, serde_json::to_vec(&value).unwrap()).unwrap();
    let dry = run(&input, &request, &output, true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["data"]["operation"], "add-vector-overlay");
    assert_eq!(report["data"]["secure_erasure"], false);
    let actual = run(&input, &request, &output, false);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stdout)
    );
    let saved = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    let original = pdf_core::PdfDocument::from_bytes(PDF, None).unwrap();
    assert_eq!(
        pdf.list_form_fields().unwrap(),
        original.list_form_fields().unwrap()
    );
    assert_eq!(
        pdf.extract_text(&[1]).unwrap(),
        original.extract_text(&[1]).unwrap()
    );
    assert_eq!(run(&input, &request, &output, false).status.code(), Some(3));
    assert_eq!(std::fs::read(output).unwrap(), saved);
    assert_eq!(std::fs::read(input).unwrap(), PDF);
    let signed = temp.path().join("signed.pdf");
    std::fs::write(
        &signed,
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    let refused = temp.path().join("refused.pdf");
    assert_eq!(
        run(&signed, &request, &refused, false).status.code(),
        Some(4)
    );
    assert!(!refused.exists());
}
#[test]
fn cli_strict_requests_invalid_paint_and_large_input_do_not_publish() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    std::fs::write(&input, PDF).unwrap();
    let request = temp.path().join("bad.json");
    let output = temp.path().join("new.pdf");
    for bytes in [b"{}".to_vec(), b"[]".to_vec(), vec![b' '; 256 * 1024 + 1]] {
        std::fs::write(&request, bytes).unwrap();
        assert!(!run(&input, &request, &output, false).status.success());
        assert!(!output.exists());
    }
    assert_eq!(std::fs::read(input).unwrap(), PDF);
}
