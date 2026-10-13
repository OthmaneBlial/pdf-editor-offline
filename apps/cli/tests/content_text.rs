#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/content_fixture.rs"]
mod fixture;
use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}
#[test]
fn proportional_variable_width_requires_explicit_opt_in_and_reports_actual_layout() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("new.pdf");
    let bytes = fixture::source_with_font(fixture::CONTENT, "Times-Roman");
    std::fs::write(&input, &bytes).unwrap();
    let inspected = run(&[
        "inspect-content-text",
        input.to_str().unwrap(),
        "--page",
        "1",
    ]);
    assert!(inspected.status.success());
    let data: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    let hash = data["data"]["source_sha256"].as_str().unwrap();
    let operation = data["data"]["inventory"]["runs"][0]["identity"]["operation"]
        .as_u64()
        .unwrap()
        .to_string();
    let mut args = vec![
        "replace-content-text",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--operation",
        &operation,
        "--source-sha256",
        hash,
        "--expected",
        "Original",
        "--replacement",
        "Longer text",
        "-o",
        output.to_str().unwrap(),
    ];
    let refused = run(&args);
    assert_eq!(refused.status.code(), Some(4));
    assert!(!output.exists());
    args.push("--allow-width-change");
    args.push("--dry-run");
    let preview = run(&args);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stdout)
    );
    assert!(!output.exists());
    let layout: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(layout["data"]["layout_policy"], "allow_width_change");
    assert_eq!(
        layout["data"]["before"]["baseline_start"],
        layout["data"]["after"]["baseline_start"]
    );
    assert_ne!(
        layout["data"]["before"]["nominal_advance"],
        layout["data"]["after"]["nominal_advance"]
    );
    args.pop();
    let saved = run(&args);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stdout)
    );
    let pdf = pdf_core::PdfDocument::open(&output, None).unwrap();
    assert_eq!(
        pdf.extract_text(&[1])
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["Longer", "text", "A", "B"]
    );
    assert_eq!(
        pdf.extract_text(&[2])
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["Original", "A", "B"]
    );
    let existing = std::fs::read(&output).unwrap();
    assert_eq!(run(&args).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), existing);
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
}
#[test]
fn real_inspection_replace_dry_run_save_and_no_clobber() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let output = temp.path().join("copy.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
    let inspected = run(&[
        "inspect-content-text",
        input.to_str().unwrap(),
        "--page",
        "1",
    ]);
    assert!(
        inspected.status.success(),
        "{:?} {}",
        inspected.status,
        String::from_utf8_lossy(&inspected.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    let data = &value["data"];
    let hash = data["source_sha256"].as_str().unwrap();
    let operation = data["inventory"]["runs"][0]["identity"]["operation"]
        .as_u64()
        .unwrap()
        .to_string();
    let replace = |extra: &[&str]| {
        let mut args = vec![
            "replace-content-text",
            input.to_str().unwrap(),
            "--page",
            "1",
            "--operation",
            &operation,
            "--source-sha256",
            hash,
            "--expected",
            "Original",
            "--replacement",
            "Replaced",
            "-o",
            output.to_str().unwrap(),
        ];
        args.extend(extra);
        run(&args)
    };
    assert!(replace(&["--dry-run"]).status.success());
    assert!(!output.exists());
    let saved = replace(&[]);
    assert!(
        saved.status.success(),
        "{:?} {} {}",
        saved.status,
        String::from_utf8_lossy(&saved.stdout),
        String::from_utf8_lossy(&saved.stderr)
    );
    let pdf = pdf_core::PdfDocument::open(&output, None).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Replaced\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    let before = std::fs::read(&output).unwrap();
    assert_eq!(replace(&[]).status.code(), Some(3));
    assert_eq!(std::fs::read(output).unwrap(), before);
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}
#[test]
fn stale_source_invalid_layout_and_arguments_never_publish() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    let output = temp.path().join("copy.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
    let doc = pdf_core::PdfDocument::from_bytes(&bytes, None).unwrap();
    let operation = doc.content_text_runs(1).unwrap()[0]
        .identity
        .operation
        .to_string();
    let hash = pdf_worker::payload_fingerprint(&bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    for (source, value, expected_code) in [
        ("0".repeat(64), "Replaced", 4),
        (hash.clone(), "Longer value", 4),
        (hash.clone(), "Réplaced", 4),
        ("bad".into(), "Replaced", 1),
    ] {
        let result = run(&[
            "replace-content-text",
            input.to_str().unwrap(),
            "--page",
            "1",
            "--operation",
            &operation,
            "--source-sha256",
            &source,
            "--expected",
            "Original",
            "--replacement",
            value,
            "-o",
            output.to_str().unwrap(),
        ]);
        assert_eq!(
            result.status.code(),
            Some(expected_code),
            "{:?} {} {}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!output.exists());
    }
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}
