#![allow(clippy::unwrap_used)]
use std::{path::Path, process::Command};
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn run(input: &Path, font: &Path, output: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("number-pages")
        .arg(input)
        .args([
            "--pages",
            "4,1-3",
            "--start",
            "98",
            "--position",
            "header",
            "--margin",
            "10",
            "--height",
            "24",
            "--font-file",
        ])
        .arg(font)
        .arg("-o")
        .arg(output)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn real_cli_numbering_dry_run_new_copy_source_unchanged_and_no_clobber() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("numbered.pdf");
    let source = support::fixture(4);
    std::fs::write(&input, &source).unwrap();
    std::fs::write(
        &font,
        include_bytes!("../../desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
    )
    .unwrap();
    let dry = run(&input, &font, &output, &["--dry-run"]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let result = run(&input, &font, &output, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        json["data"]["numbering"]["pages"],
        serde_json::json!([1, 2, 3, 4])
    );
    let saved = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    for page in 1..=4 {
        assert!(
            pdf.extract_text(&[page])
                .unwrap()
                .contains(&(97 + page).to_string())
        );
    }
    assert_eq!(run(&input, &font, &output, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), source);
    let bad = temp.path().join("bad.pdf");
    assert!(
        !run(&input, &font, &bad, &["--font-size", "1024"])
            .status
            .success()
    );
    assert!(!bad.exists());
}
