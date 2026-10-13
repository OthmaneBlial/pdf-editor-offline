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
fn real_font_face_cli_requires_width_policy_preserves_source_and_publishes_once() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("changed.pdf");
    let bytes = fixture::source(fixture::CONTENT);
    std::fs::write(&input, &bytes).unwrap();
    let report = run(&[
        "inspect-content-text",
        input.to_str().unwrap(),
        "--page",
        "1",
    ]);
    assert!(report.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let hash = report["data"]["source_sha256"].as_str().unwrap();
    let operation = report["data"]["inventory"]["runs"][0]["identity"]["operation"]
        .as_u64()
        .unwrap()
        .to_string();
    let args = vec![
        "set-content-text-font",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--operation",
        &operation,
        "--source-sha256",
        hash,
        "--expected",
        "Original",
        "--font",
        "Times-Italic",
        "-o",
        output.to_str().unwrap(),
    ];
    let refused = run(&args);
    assert_eq!(
        refused.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&refused.stdout)
    );
    assert!(!output.exists());
    let mut args = args;
    args.extend(["--allow-width-change", "--dry-run"]);
    let dry = run(&args);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let data: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(data["data"]["font"], "times_italic");
    assert_eq!(data["data"]["original_matrix_preserved"], true);
    args.pop();
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let saved = std::fs::read(&output).unwrap();
    let mut pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    let edited = pdf.content_text_runs(1).unwrap().remove(0);
    assert_eq!(
        edited.standard_font,
        Some(pdf_core::ContentTextFont::TimesItalic)
    );
    assert!(edited.properties_writable && edited.writable);
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Original\nA B\n");
    assert_eq!(pdf.extract_text(&[2]).unwrap(), "Original\nA B\n");
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.).unwrap(),
        pdf_render::render_page(&saved, 1, 1.).unwrap()
    );
    assert_ne!(
        pdf_render::render_page(&bytes, 0, 1.).unwrap().rgba,
        pdf_render::render_page(&saved, 0, 1.).unwrap().rgba
    );
    pdf.replace_content_text(
        1,
        &[pdf_core::ContentTextChange {
            identity: edited.identity,
            expected: "Original".into(),
            replacement: "Replacement".into(),
            layout_policy: pdf_core::ContentTextLayoutPolicy::AllowWidthChange,
        }],
    )
    .unwrap();
    assert_eq!(run(&args).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    let mut stale = args.clone();
    let hash_index = stale
        .iter()
        .position(|arg| *arg == "--source-sha256")
        .unwrap()
        + 1;
    stale[hash_index] = "0000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(run(&stale).status.code(), Some(4));
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}

#[test]
fn font_face_cli_invalid_font_and_signed_source_publish_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("signed.pdf");
    let output = temp.path().join("never.pdf");
    let mut doc = lopdf::Document::load_mem(&fixture::source(fixture::CONTENT)).unwrap();
    doc.add_object(lopdf::dictionary! {"Type"=>"Sig"});
    doc.save(&input).unwrap();
    let bytes = std::fs::read(&input).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&bytes, None).unwrap();
    let operation = pdf.content_text_runs(1).unwrap()[0]
        .identity
        .operation
        .to_string();
    let hash = pdf_worker::payload_fingerprint(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    for font in ["Helvetica", "Unsupported"] {
        let result = run(&[
            "set-content-text-font",
            input.to_str().unwrap(),
            "--page",
            "1",
            "--operation",
            &operation,
            "--source-sha256",
            &hash,
            "--expected",
            "Original",
            "--font",
            font,
            "--allow-width-change",
            "-o",
            output.to_str().unwrap(),
        ]);
        assert!(!result.status.success());
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(data["ok"], false);
        assert!(!output.exists());
    }
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}
#[test]
fn real_property_cli_dry_run_identity_no_clobber_and_same_text_font() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("changed.pdf");
    const PRECISE: &[u8] =
        b"% exact CLI artwork\n10.1234567890123456 170.9876543210987654 5 5 re f\n";
    let bytes = fixture::source(&[PRECISE, fixture::CONTENT].concat());
    std::fs::write(&input, &bytes).unwrap();
    let report = run(&[
        "inspect-content-text",
        input.to_str().unwrap(),
        "--page",
        "1",
    ]);
    assert!(report.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    let hash = report["data"]["source_sha256"].as_str().unwrap();
    let operation = report["data"]["inventory"]["runs"][0]["identity"]["operation"]
        .as_u64()
        .unwrap()
        .to_string();
    assert_eq!(
        report["data"]["inventory"]["runs"][0]["properties_writable"],
        true
    );
    let mut args = vec![
        "set-content-text-properties",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--operation",
        &operation,
        "--source-sha256",
        hash,
        "--expected",
        "Original",
        "--font-size",
        "24",
        "--matrix-position",
        "20",
        "150",
        "--rgb",
        "1",
        "0",
        "0",
        "-o",
        output.to_str().unwrap(),
        "--dry-run",
    ];
    let dry = run(&args);
    assert!(
        dry.status.success(),
        "{:?} {}",
        dry.status,
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let data: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(
        data["data"]["after"]["baseline_start"],
        serde_json::json!([20.0, 150.0])
    );
    args.pop();
    let result = run(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let saved = std::fs::read(&output).unwrap();
    let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
    let raw = lopdf::Document::load_mem(&saved).unwrap();
    let id = raw
        .get_dictionary((3, 0))
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    assert!(
        raw.get_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .decompressed_content_with_limit(pdf_core::MAX_STREAM_BYTES)
            .unwrap()
            .starts_with(PRECISE)
    );
    assert_eq!(pdf.extract_text(&[1]).unwrap(), "Original\nA B\n");
    let original = pdf_core::PdfDocument::from_bytes(&bytes, None).unwrap();
    let edited_run = pdf.content_text_runs(1).unwrap().remove(0);
    assert_eq!(edited_run.standard_font.unwrap().pdf_name(), "Courier");
    assert_eq!(edited_run.layout.unwrap().font_size, 24.0);
    assert_eq!(
        pdf_render::render_page(&bytes, 1, 1.0).unwrap(),
        pdf_render::render_page(&saved, 1, 1.0).unwrap()
    );
    assert_eq!(
        original.extract_text(&[1]).unwrap(),
        pdf.extract_text(&[1]).unwrap()
    );
    assert_eq!(run(&args).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    std::fs::remove_file(&output).unwrap();
    let hash_index = args.iter().position(|arg| *arg == hash).unwrap();
    args[hash_index] = "0000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(run(&args).status.code(), Some(4));
    assert!(!output.exists());
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
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
