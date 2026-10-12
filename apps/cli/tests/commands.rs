#![allow(clippy::unwrap_used)]

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../trust_lab/corpus/v1")
        .join(name)
}
fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_rotates_a_real_fixture_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("copy.pdf");
    let input = fixture("bookmarks.pdf");
    let input = input.to_str().unwrap();
    let output = output.to_str().unwrap();
    let args = [
        "rotate",
        input,
        "--pages",
        "1",
        "--degrees",
        "90",
        "-o",
        output,
        "--json",
    ];
    let result = cli(&args);
    assert!(result.status.success(), "{:?}", result);
    assert_eq!(json(&result)["data"]["dry_run"], false);
    let info = cli(&["info", output, "--json"]);
    assert_eq!(json(&info)["data"]["pages"][0]["rotation"], 90);
    let original = cli(&["info", input, "--json"]);
    assert_eq!(json(&original)["data"]["pages"][0]["rotation"], 0);
    assert_eq!(cli(&args).status.code(), Some(3));
}

#[test]
fn dry_run_and_invalid_page_selection_never_create_output() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("copy.pdf");
    let input = fixture("bookmarks.pdf");
    let result = cli(&[
        "rotate",
        input.to_str().unwrap(),
        "--pages",
        "1",
        "--degrees",
        "90",
        "-o",
        output.to_str().unwrap(),
        "--dry-run",
        "--json",
    ]);
    assert!(result.status.success());
    assert_eq!(json(&result)["data"]["dry_run"], true);
    assert!(!output.exists());
    let invalid = cli(&[
        "rotate",
        input.to_str().unwrap(),
        "--pages",
        "0",
        "--degrees",
        "90",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(invalid.status.code(), Some(1));
    assert_eq!(json(&invalid)["ok"], false);
    assert!(!output.exists());
}

#[test]
fn text_search_compare_and_capabilities_are_real() {
    let input = fixture("bookmarks.pdf");
    let input = input.to_str().unwrap();
    let extracted = cli(&["extract-text", input, "--json"]);
    let data = json(&extracted);
    let text = data["data"]["text"].as_str().unwrap();
    assert!(!text.trim().is_empty());
    let term = text.split_whitespace().next().unwrap();
    let search = cli(&["search", input, term, "--json"]);
    assert!(
        !json(&search)["data"]["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let compared = cli(&["compare", input, input, "--json"]);
    assert_eq!(json(&compared)["data"]["text_equal"], true);
    assert_eq!(
        json(&cli(&["capabilities", "--json"]))["data"]["runtime"],
        "rust-native"
    );
    assert!(cli(&["completions", "zsh"]).status.success());
}

#[test]
fn malformed_input_has_structured_error() {
    let result = cli(&["info", fixture("malformed.pdf").to_str().unwrap(), "--json"]);
    assert!(!result.status.success());
    assert_eq!(json(&result)["ok"], false);
}

#[test]
fn security_inventory_reports_real_attachments_without_claiming_privacy() {
    let output = cli(&[
        "inspect-security",
        fixture("attachment.pdf").to_str().unwrap(),
        "--json",
    ]);
    assert!(output.status.success());
    let report = json(&output);
    assert_eq!(report["data"]["schema_version"], 1);
    assert_eq!(report["data"]["complete_privacy_proof"], false);
    assert!(report["data"]["embedded_file_streams"].as_u64().unwrap() > 0);
    assert!(
        report["data"]["revision_analysis"]
            .as_str()
            .unwrap()
            .starts_with("unavailable")
    );
}

fn plain_three_pages(path: &Path) {
    let demo =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_pdfs/demo-basic.pdf");
    let mut document = pdf_core::PdfDocument::open(demo, None).unwrap();
    document.insert_blank(2, 300.0, 400.0).unwrap();
    document.insert_blank(3, 500.0, 600.0).unwrap();
    document.rotate(&[2], 90).unwrap();
    document.rotate(&[3], 180).unwrap();
    document.save_new(path).unwrap();
}

fn rotations(path: &Path) -> Vec<i64> {
    pdf_core::PdfDocument::open(path, None)
        .unwrap()
        .info()
        .unwrap()
        .pages
        .into_iter()
        .map(|page| page.rotation)
        .collect()
}

#[test]
fn reverse_and_complete_reorder_preserve_caller_order_and_source() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    plain_three_pages(&input);
    let original = std::fs::read(&input).unwrap();
    for (operation, options, expected) in [
        ("reverse", vec![], vec![180, 90, 0]),
        ("reorder", vec!["--order", "3,1-2"], vec![180, 0, 90]),
    ] {
        let output = directory.path().join(format!("{operation}.pdf"));
        let mut args = vec![operation, input.to_str().unwrap()];
        args.extend(options);
        args.extend(["-o", output.to_str().unwrap(), "--json"]);
        let result = cli(&args);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(json(&result)["data"]["operation"], operation);
        assert_eq!(rotations(&output), expected);
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
}

#[test]
fn insert_extract_and_delete_use_one_based_positions_and_order() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    plain_three_pages(&input);
    let original = std::fs::read(&input).unwrap();
    let inserted = directory.path().join("inserted.pdf");
    let result = cli(&[
        "insert-blank",
        input.to_str().unwrap(),
        "--position",
        "2",
        "--width",
        "200",
        "--height",
        "250",
        "-o",
        inserted.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    let info = pdf_core::PdfDocument::open(&inserted, None)
        .unwrap()
        .info()
        .unwrap();
    assert_eq!(info.page_count, 4);
    assert_eq!(info.pages[1].media_box, [0.0, 0.0, 200.0, 250.0]);
    assert_eq!(rotations(&inserted), [0, 0, 90, 180]);
    let appended = directory.path().join("appended.pdf");
    assert!(
        cli(&[
            "insert-blank",
            input.to_str().unwrap(),
            "--position",
            "4",
            "-o",
            appended.to_str().unwrap()
        ])
        .status
        .success()
    );
    assert_eq!(rotations(&appended), [0, 90, 180, 0]);
    for (operation, selection, expected) in [
        ("extract-pages", "3,1", vec![180, 0]),
        ("delete-pages", "2", vec![0, 180]),
    ] {
        let output = directory.path().join(format!("{operation}.pdf"));
        let result = cli(&[
            operation,
            input.to_str().unwrap(),
            "--pages",
            selection,
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(rotations(&output), expected);
        let extracted = pdf_core::PdfDocument::open(&output, None).unwrap();
        let text_page = if operation == "extract-pages" { 2 } else { 1 };
        assert_eq!(
            extracted.extract_text(&[text_page]).unwrap(),
            pdf_core::PdfDocument::open(&input, None)
                .unwrap()
                .extract_text(&[1])
                .unwrap()
        );
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[test]
fn organization_argument_errors_and_failed_edits_produce_no_output() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    plain_three_pages(&input);
    let original = std::fs::read(&input).unwrap();
    let output = directory.path().join("output.pdf");
    for (operation, options, exit) in [
        ("reorder", vec!["--order", "1,1,2,3"], 1),
        ("reorder", vec!["--order", "1,2"], 1),
        ("extract-pages", vec!["--pages", "2,2"], 1),
        ("delete-pages", vec!["--pages", "1-3"], 1),
        ("insert-blank", vec!["--position", "0"], 1),
        ("insert-blank", vec!["--position", "5"], 1),
        ("insert-blank", vec!["--position", "2", "--width", "-1"], 1),
        ("crop", vec!["--rectangle", "0", "0", "400", "500"], 1),
        ("crop", vec!["--rectangle", "0", "0", "nan", "100"], 1),
        ("crop", vec!["--rectangle", "0", "0", "100"], 2),
        ("crop", vec![], 2),
    ] {
        let mut args = vec![operation, input.to_str().unwrap()];
        args.extend(options);
        args.extend(["-o", output.to_str().unwrap(), "--json"]);
        let result = cli(&args);
        assert_eq!(result.status.code(), Some(exit), "{operation}: {result:?}");
        if exit != 2 {
            assert_eq!(json(&result)["ok"], false);
        }
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
}

#[test]
fn crop_and_organization_dry_runs_and_no_clobber_are_consistent() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    plain_three_pages(&input);
    let output = directory.path().join("output.pdf");
    let args = [
        "crop",
        input.to_str().unwrap(),
        "--pages",
        "1",
        "--rectangle",
        "10",
        "20",
        "200",
        "300",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    let result = cli(&args);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        json(&result)["data"]["rectangle"],
        serde_json::json!([10.0, 20.0, 200.0, 300.0])
    );
    assert_eq!(rotations(&output), [0, 90, 180]);
    let saved = std::fs::read(&output).unwrap();
    assert_eq!(cli(&args).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    for (operation, options) in [
        ("reverse", vec![]),
        ("reorder", vec!["--order", "3,1,2"]),
        ("insert-blank", vec!["--position", "2"]),
        ("extract-pages", vec!["--pages", "3,1"]),
        ("delete-pages", vec!["--pages", "2"]),
    ] {
        let target = directory.path().join(format!("{operation}.pdf"));
        let mut args = vec![operation, input.to_str().unwrap()];
        args.extend(options);
        args.extend(["-o", target.to_str().unwrap(), "--dry-run", "--json"]);
        let result = cli(&args);
        assert!(result.status.success(), "{operation}: {result:?}");
        assert_eq!(json(&result)["data"]["dry_run"], true);
        assert!(!target.exists());
    }
}

#[test]
fn unsafe_subset_and_signed_organization_are_refused_without_output() {
    let directory = tempfile::tempdir().unwrap();
    for (operation, input, options) in [
        ("extract-pages", "bookmarks.pdf", vec!["--pages", "1"]),
        ("delete-pages", "bookmarks.pdf", vec!["--pages", "1"]),
        ("reverse", "signed.pdf", vec![]),
        ("reorder", "signed.pdf", vec!["--order", "1"]),
        (
            "crop",
            "signed.pdf",
            vec!["--rectangle", "0", "0", "100", "100"],
        ),
        ("insert-blank", "signed.pdf", vec!["--position", "2"]),
        ("extract-pages", "signed.pdf", vec!["--pages", "1"]),
        ("delete-pages", "signed.pdf", vec!["--pages", "1"]),
    ] {
        let input = fixture(input);
        let before = std::fs::read(&input).unwrap();
        let output = directory.path().join(format!("{operation}.pdf"));
        let mut args = vec![operation, input.to_str().unwrap()];
        args.extend(options);
        args.extend(["-o", output.to_str().unwrap(), "--json"]);
        let result = cli(&args);
        assert_eq!(result.status.code(), Some(4), "{operation}: {result:?}");
        assert_eq!(json(&result)["error"]["kind"], "refused");
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), before);
    }
}
