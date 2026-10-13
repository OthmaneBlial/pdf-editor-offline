#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/split_fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../tests/support/image_removal_fixture.rs"]
mod fixture;
use std::process::Command;
#[test]
fn isolated_cli_removes_assets_dry_runs_refuses_stale_and_preserves_files() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("copy.pdf");
    let selections = temp.path().join("private-selection.json");
    let source = base::fixture(2);
    std::fs::write(&input, &source).unwrap();
    let selected = fixture::selections(&source);
    std::fs::write(&selections, serde_json::to_vec(&selected).unwrap()).unwrap();
    let run = |dry: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
        cmd.args(["--json", "redact-images"])
            .arg(&input)
            .arg("--selections-file")
            .arg(&selections)
            .arg("-o")
            .arg(&output);
        if dry {
            cmd.arg("--dry-run");
        }
        cmd.output().unwrap()
    };
    let dry = run(true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let result = run(false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["data"]["report"]["removed_draws"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(report["data"]["general_privacy_certificate"], false);
    let saved = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
    pdf.verify_image_redaction_bytes(&selected, &saved).unwrap();
    assert_eq!(run(false).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    std::fs::write(&selections, serde_json::to_vec(&selected[..1]).unwrap()).unwrap();
    assert_eq!(run(true).status.code(), Some(4));
    let mut stale = selected.clone();
    stale[0].source_hash = "0".repeat(64);
    std::fs::write(&selections, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(run(true).status.code(), Some(4));
    let mut unknown = serde_json::to_value(&selected).unwrap();
    unknown[0]["path"] = serde_json::json!("not-allowed");
    std::fs::write(&selections, serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert_eq!(run(true).status.code(), Some(4));
    assert_eq!(std::fs::read(input).unwrap(), source);
    assert_eq!(std::fs::read(output).unwrap(), saved);
}
