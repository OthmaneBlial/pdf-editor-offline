#![allow(clippy::unwrap_used, clippy::expect_used)]

#[cfg(unix)]
use lopdf::{Document, Object, Stream, dictionary};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_pdf-editor-offline")
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pdfs")
        .join(name)
}
fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("CLI process")
}
fn value(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("JSON output")
}
#[cfg(unix)]
fn text_document(text: &str) -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let font = document
        .add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = document.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 10 Tf 20 700 Td ({text}) Tj ET").into_bytes(),
    ));
    let page=document.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    document.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    document.trailer.set("Root", root);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("isolated-first!!"),
            Object::string_literal("isolated-second!"),
        ],
    );
    document
}

#[test]
fn json_human_output_errors_exit_codes_and_verified_no_overwrite_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let input = fixture("bookmarks.pdf");
    let output = directory.path().join("rotated.pdf");
    let original = std::fs::read(&input).unwrap();
    let human = run(&["extract-text".as_ref(), input.as_os_str()]);
    assert!(human.status.success());
    assert!(!human.stdout.is_empty());
    let json = run(&[
        "extract-text".as_ref(),
        input.as_os_str(),
        "--json".as_ref(),
    ]);
    assert!(json.status.success());
    assert_eq!(
        value(&json)["data"]["text"].as_str().unwrap().as_bytes(),
        human.stdout
    );
    let args = [
        "rotate".as_ref(),
        input.as_os_str(),
        "--pages".as_ref(),
        "1".as_ref(),
        "--degrees".as_ref(),
        "90".as_ref(),
        "-o".as_ref(),
        output.as_os_str(),
        "--json".as_ref(),
    ];
    let rotated = run(&args);
    assert!(rotated.status.success());
    assert_eq!(value(&rotated)["ok"], true);
    let reopened = pdf_core::PdfDocument::open(&output, None).unwrap();
    assert_eq!(reopened.info().unwrap().pages[0].rotation, 90);
    let published = std::fs::read(&output).unwrap();
    let refused = run(&args);
    assert_eq!(refused.status.code(), Some(3));
    assert_eq!(value(&refused)["error"]["kind"], "filesystem");
    assert_eq!(std::fs::read(&output).unwrap(), published);
    assert!(
        std::fs::read(&input).unwrap() == original,
        "source bytes changed"
    );
    let malformed = run(&[
        "info".as_ref(),
        fixture("malformed.pdf").as_os_str(),
        "--json".as_ref(),
    ]);
    assert_eq!(malformed.status.code(), Some(1));
    assert_eq!(value(&malformed)["ok"], false);
    let human_error = run(&["info".as_ref(), fixture("malformed.pdf").as_os_str()]);
    assert_eq!(human_error.status.code(), Some(1));
    assert!(human_error.stdout.is_empty());
    assert!(human_error.stderr.starts_with(b"Error: "));
    let signed_output = directory.path().join("signed-must-not-publish.pdf");
    let signed = run(&[
        "rotate".as_ref(),
        fixture("signed.pdf").as_os_str(),
        "--pages".as_ref(),
        "1".as_ref(),
        "--degrees".as_ref(),
        "90".as_ref(),
        "-o".as_ref(),
        signed_output.as_os_str(),
        "--json".as_ref(),
    ]);
    assert_eq!(signed.status.code(), Some(4));
    assert_eq!(value(&signed)["error"]["kind"], "refused");
    assert!(!signed_output.exists());
    let arguments = run(&["info".as_ref(), "--json".as_ref()]);
    assert_eq!(arguments.status.code(), Some(2));
    assert_eq!(value(&arguments)["error"]["kind"], "arguments");
}

// APFS rejects non-UTF-8 names at creation. Linux tests native byte paths;
// Windows tests unpaired UTF-16 units. Actual publication needs each platform's CI.
#[cfg(any(target_os = "linux", windows))]
#[test]
fn non_unicode_native_paths_survive_child_argument_roundtrip_and_publication() {
    let directory = tempfile::tempdir().unwrap();
    #[cfg(target_os = "linux")]
    let names = {
        use std::os::unix::ffi::OsStringExt;
        (
            std::ffi::OsString::from_vec(b"source-\xff.pdf".to_vec()),
            std::ffi::OsString::from_vec(b"output-\xfe.pdf".to_vec()),
        )
    };
    #[cfg(windows)]
    let names = {
        use std::os::windows::ffi::OsStringExt;
        let name = |prefix: &str, unit: u16| {
            std::ffi::OsString::from_wide(
                &prefix
                    .encode_utf16()
                    .chain([unit])
                    .chain(".pdf".encode_utf16())
                    .collect::<Vec<_>>(),
            )
        };
        (name("source-", 0xd800), name("output-", 0xdc00))
    };
    let input = directory.path().join(names.0);
    let output = directory.path().join(names.1);
    std::fs::copy(fixture("bookmarks.pdf"), &input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = run(&[
        "rotate".as_ref(),
        input.as_os_str(),
        "--pages".as_ref(),
        "1".as_ref(),
        "--degrees".as_ref(),
        "90".as_ref(),
        "-o".as_ref(),
        output.as_os_str(),
        "--json".as_ref(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(value(&result)["ok"], true);
    assert_eq!(
        pdf_core::PdfDocument::open(&output, None)
            .unwrap()
            .info()
            .unwrap()
            .pages[0]
            .rotation,
        90
    );
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let again = run(&["info".as_ref(), output.as_os_str(), "--json".as_ref()]);
    assert!(again.status.success());
    assert_eq!(value(&again)["data"]["page_count"], 3);
}

#[test]
fn password_stdin_is_bounded_and_never_repeated_in_errors() {
    use std::io::Write;
    let secret = "synthetic-isolation-password";
    let mut child = Command::new(binary())
        .args([
            "info",
            "missing-test-file.pdf",
            "--password-stdin",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(secret.repeat(200).as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(5));
    assert_eq!(value(&result)["error"]["kind"], "resource_limit");
    assert!(!String::from_utf8_lossy(&result.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&result.stderr).contains(secret));
}

// This observes the actual child of the normal parent CLI, not a replaceable helper.
// The environment is intentionally cleared before spawning so inspecting that own
// child's environment cannot expose unrelated user credentials. No production
// executable override exists; shared transport tests own injected-fault coverage.
#[cfg(unix)]
#[test]
fn mandatory_real_child_handles_large_encrypted_text_without_password_argv_or_environment() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("large-encrypted.pdf");
    let expected = format!(
        "ISOLATED START {} ISOLATED END",
        "bounded text ".repeat(730_000)
    );
    assert!(expected.len() > 8 * 1024 * 1024);
    let secret = "synthetic-worker-stdin-secret";
    let mut source = text_document(&expected);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &source,
        owner_password: "synthetic-owner",
        user_password: secret,
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    source.save(&input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let mut child = Command::new(binary())
        .arg("extract-text")
        .arg(&input)
        .args(["--password-stdin", "--json"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("PDF_EDITOR_OFFLINE_DISABLE_ISOLATION", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{secret}\n").as_bytes())
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let output_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let error_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let started = Instant::now();
    let mut observed = false;
    let mut checked_environment = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        let ids = Command::new("pgrep")
            .args(["-P", &child.id().to_string()])
            .output()
            .unwrap();
        for id in String::from_utf8_lossy(&ids.stdout).split_whitespace() {
            let args = Command::new("ps")
                .args(["-p", id, "-o", "args="])
                .output()
                .unwrap();
            let args = String::from_utf8_lossy(&args.stdout);
            if args.contains("--pdf-cli-worker") {
                observed = true;
                assert!(!args.contains(secret));
                let mut inspect = Command::new("ps");
                if cfg!(target_os = "macos") {
                    inspect.args(["-Eww", "-p", id, "-o", "command="]);
                } else {
                    inspect.args(["eww", "-p", id, "-o", "args="]);
                }
                let environment = inspect.output().unwrap();
                if environment.status.success() && !environment.stdout.is_empty() {
                    assert!(!String::from_utf8_lossy(&environment.stdout).contains(secret));
                    checked_environment |=
                        String::from_utf8_lossy(&environment.stdout).contains("PDF_WORKER_CPU=");
                }
            }
        }
        if started.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("normal CLI did not finish within smoke deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = output_reader.join().unwrap();
    let stderr = error_reader.join().unwrap();
    assert!(
        status.success(),
        "CLI status {status:?}; stdout {}; stderr {}",
        String::from_utf8_lossy(&stdout[..stdout.len().min(1024)]),
        String::from_utf8_lossy(&stderr[..stderr.len().min(1024)])
    );
    assert!(
        observed,
        "normal parent never exposed an actual self-hosted child; isolation must be mandatory"
    );
    assert!(
        checked_environment,
        "targeted own-child environment inspection unavailable"
    );
    let result: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(result["ok"], true);
    let text = result["data"]["text"].as_str().unwrap();
    assert!(text.len() > 8 * 1024 * 1024);
    assert!(
        text.trim_end() == expected,
        "large text content mismatch: actual {} bytes, expected {} bytes",
        text.len(),
        expected.len()
    );
    assert!(!String::from_utf8_lossy(&stderr).contains(secret));
    assert!(!String::from_utf8_lossy(&stdout).contains(secret));
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn sole_selfhost_flag_enters_framed_worker_before_clap_and_rejects_truncated_request() {
    let worker = Command::new(binary())
        .arg("--pdf-cli-worker")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(worker.status.code(), Some(5));
    assert!(
        worker.stdout.starts_with(b"PDFWRK01"),
        "self-host must announce the resource-limited framed worker, not ordinary CLI output"
    );
    let public = Command::new(binary())
        .args(["--pdf-cli-worker", "--json"])
        .output()
        .unwrap();
    assert_eq!(public.status.code(), Some(2));
    assert_eq!(value(&public)["error"]["kind"], "arguments");
}
