#![allow(clippy::unwrap_used)]

use pdf_core::PdfDocument;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}
fn data(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}
fn fill(input: &Path, output: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "fill-button",
        input.to_str().unwrap(),
        "--field",
        "approved",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    args.extend(extra);
    cli(&args)
}

#[test]
fn real_cli_discovers_fills_dry_runs_and_preserves_source_and_appearances() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("forms.pdf");
    let output = temp.path().join("off.pdf");
    std::fs::write(&input, SOURCE).unwrap();
    let listing = data(cli(&["list-forms", input.to_str().unwrap(), "--json"]));
    assert_eq!(listing["fields"].as_array().unwrap().len(), 5);
    let human = cli(&["list-forms", input.to_str().unwrap()]);
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("approved")
    );
    let dry = data(fill(&input, &output, &["--off", "--dry-run"]));
    assert_eq!(dry["dry_run"], true);
    assert!(!output.exists());
    data(fill(&input, &output, &["--off"]));
    let saved = PdfDocument::open(&output, None).unwrap();
    let fields = saved.list_form_fields().unwrap();
    let approved = fields
        .iter()
        .find(|field| field.name == "approved")
        .unwrap();
    assert_eq!(
        approved.widgets[0].appearance_state.as_deref(),
        Some(b"Off".as_slice())
    );
    assert!(!saved.extract_text(&[1]).unwrap().contains('✓'));
    let original = lopdf::Document::load_mem(SOURCE).unwrap();
    let changed = lopdf::Document::load(&output).unwrap();
    for (id, object) in &original.objects {
        if matches!(object, lopdf::Object::Stream(_)) {
            assert_eq!(
                object,
                changed.objects.get(id).unwrap(),
                "appearance/content stream {id:?}"
            );
        }
    }
    let existing = std::fs::read(&output).unwrap();
    assert_eq!(fill(&input, &output, &["--off"]).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), existing);
    let on = temp.path().join("on.pdf");
    data(fill(&output, &on, &["--widget", "10", "0"]));
    assert!(
        PdfDocument::open(&on, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains('✓')
    );
    assert_eq!(std::fs::read(&input).unwrap(), SOURCE);
}

#[test]
fn invalid_selections_and_argument_conflicts_never_publish() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("forms.pdf");
    let output = temp.path().join("bad.pdf");
    std::fs::write(&input, SOURCE).unwrap();
    assert_eq!(
        fill(&input, &output, &["--off", "--widget", "10", "0"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fill(&input, &output, &[]).status.code(), Some(2));
    assert_eq!(
        fill(&input, &output, &["--widget", "8", "0"]).status.code(),
        Some(4)
    );
    assert_eq!(
        fill(&input, &output, &["--widget", "10", "65536"])
            .status
            .code(),
        Some(1)
    );
    assert!(!output.exists());
    assert_eq!(std::fs::read(input).unwrap(), SOURCE);
}

#[test]
fn password_stdin_discovery_and_signed_or_encrypted_write_refusals() {
    use std::io::Write;
    use std::process::Stdio;
    let temp = tempfile::tempdir().unwrap();
    let encrypted = temp.path().join("encrypted.pdf");
    let output = temp.path().join("refused.pdf");
    let mut document = lopdf::Document::load_mem(SOURCE).unwrap();
    document.trailer.set(
        "ID",
        vec![
            lopdf::Object::string_literal("form-id"),
            lopdf::Object::string_literal("form-id"),
        ],
    );
    let encryption = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &document,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    document.encrypt(&encryption).unwrap();
    document.save(&encrypted).unwrap();
    let original = std::fs::read(&encrypted).unwrap();
    let password_cli = |args: &[&str]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"reader\n").unwrap();
        child.wait_with_output().unwrap()
    };
    let fields = data(password_cli(&[
        "list-forms",
        encrypted.to_str().unwrap(),
        "--password-stdin",
        "--json",
    ]));
    assert!(
        fields["fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|field| field["writable_button"] == false)
    );
    let rejected = password_cli(&[
        "fill-button",
        encrypted.to_str().unwrap(),
        "--field",
        "approved",
        "--off",
        "-o",
        output.to_str().unwrap(),
        "--password-stdin",
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(4));
    assert!(!String::from_utf8_lossy(&rejected.stdout).contains("reader"));
    assert!(!output.exists());
    assert_eq!(std::fs::read(&encrypted).unwrap(), original);
    let signed = temp.path().join("signed.pdf");
    let mut document = lopdf::Document::load_mem(SOURCE).unwrap();
    let signature = document.add_object(lopdf::dictionary! {"Type" => "Sig"});
    document.trailer.set("SignatureEvidence", signature);
    document.save(&signed).unwrap();
    assert_eq!(fill(&signed, &output, &["--off"]).status.code(), Some(4));
    assert!(!output.exists());
}

fn render(path: &Path, name: &str) -> Vec<u8> {
    let binary = std::env::var_os("PDF_REFERENCE_BIN_DIR")
        .map(|dir| std::path::PathBuf::from(dir).join(name))
        .unwrap();
    let output = Command::new(binary)
        .args(["-f", "1", "-singlefile", "-r", "72", "-png"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn independent_render_and_structure_match_actual_button_state() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let off = temp.path().join("off.pdf");
    let on = temp.path().join("on.pdf");
    std::fs::write(&input, SOURCE).unwrap();
    data(fill(&input, &off, &["--off"]));
    data(fill(&off, &on, &["--widget", "10", "0"]));
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let version = Command::new(&qpdf).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let poppler = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let version = Command::new(poppler.join("pdftoppm"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
    for path in [&off, &on] {
        assert!(
            Command::new(&qpdf)
                .arg("--check")
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert_ne!(render(&input, "pdftoppm"), render(&off, "pdftoppm"));
    assert_eq!(render(&input, "pdftoppm"), render(&on, "pdftoppm"));
    let text = Command::new(poppler.join("pdftotext"))
        .arg(&off)
        .arg("-")
        .output()
        .unwrap();
    assert!(text.status.success());
    assert!(!String::from_utf8(text.stdout).unwrap().contains('✓'));
    assert_eq!(std::fs::read(input).unwrap(), SOURCE);
}
