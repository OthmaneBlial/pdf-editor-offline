#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/content_fixture.rs"]
mod fixture;
use std::process::Command;
const FONT: &[u8] = include_bytes!("../../desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
#[test]
fn actual_cli_chosen_font_requires_ack_and_source_identity_dry_runs_and_publishes_once() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("new.pdf");
    let font = temp.path().join("chosen.ttf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
    std::fs::write(&font, FONT).unwrap();
    let run = pdf_core::PdfDocument::from_bytes(&bytes, None)
        .unwrap()
        .content_text_runs(1)
        .unwrap()
        .remove(0);
    let hash = pdf_worker::payload_fingerprint(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let invoke = |ack: bool, dry: bool, hash: &str| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
        cmd.arg("set-content-text-font")
            .arg(&input)
            .args(["--page", "1", "--operation"])
            .arg(run.identity.operation.to_string())
            .args([
                "--source-sha256",
                hash,
                "--expected",
                "Original",
                "--replacement",
                "AV Café ΩЖ",
                "--font-file",
            ])
            .arg(&font)
            .arg("--output")
            .arg(&output)
            .arg("--json");
        if ack {
            cmd.arg("--allow-width-change");
        }
        if dry {
            cmd.arg("--dry-run");
        }
        cmd.output().unwrap()
    };
    assert_eq!(invoke(false, false, &hash).status.code(), Some(4));
    assert!(!output.exists());
    assert_eq!(invoke(true, false, &"00".repeat(32)).status.code(), Some(4));
    assert!(!output.exists());
    let dry = invoke(true, true, &hash);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let actual = invoke(true, false, &hash);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stdout)
    );
    let reply: serde_json::Value = serde_json::from_slice(&actual.stdout).unwrap();
    assert_eq!(
        reply["data"]["embedded_font"]["postscript_name"],
        "IBMPlexSans"
    );
    assert_eq!(reply["data"]["reflow"], false);
    let saved = std::fs::read(&output).unwrap();
    assert_eq!(invoke(true, false, &hash).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "AV Café ΩЖ\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    assert_eq!(std::fs::read(&font).unwrap(), FONT);
}
