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
