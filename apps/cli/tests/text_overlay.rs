#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] = include_bytes!("../../desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn run(input: &Path, font: &Path, output: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("add-text-overlay")
        .arg(input)
        .args([
            "--page",
            "1",
            "--rect",
            "350",
            "400",
            "550",
            "500",
            "--text",
            "Overlay é\nSecond line ΩЖ",
            "--font-file",
        ])
        .arg(font)
        .args([
            "--font-size",
            "12",
            "--color",
            "0.1",
            "0.3",
            "0.2",
            "--align",
            "center",
            "-o",
        ])
        .arg(output)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn production_cli_overlay_dry_run_new_copy_errors_and_no_clobber() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("overlay.pdf");
    std::fs::write(&input, PDF).unwrap();
    std::fs::write(&font, FONT).unwrap();
    let dry = run(&input, &font, &output, &["--dry-run"]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["data"]["operation"], "add-text-overlay");
    assert_eq!(report["data"]["existing_text_edited"], false);
    assert_eq!(report["data"]["secure_erasure"], false);
    let saved = run(&input, &font, &output, &[]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stdout)
    );
    let bytes = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(
        pdf.extract_text(&[1])
            .unwrap()
            .contains("Overlay é\nSecond line ΩЖ")
    );
    assert_eq!(run(&input, &font, &output, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    std::fs::write(&font, b"bad font").unwrap();
    let failed = temp.path().join("failed.pdf");
    assert!(!run(&input, &font, &failed, &[]).status.success());
    assert!(!failed.exists());
    assert_eq!(std::fs::read(&input).unwrap(), PDF);
    let signed = temp.path().join("signed.pdf");
    std::fs::write(
        &signed,
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    std::fs::write(&font, FONT).unwrap();
    assert_eq!(run(&signed, &font, &failed, &[]).status.code(), Some(4));
    assert!(!failed.exists());
}
