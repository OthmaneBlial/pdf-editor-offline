#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}

fn fixture(text: &str, font: &str, color: [u8; 3], rotation: i64) -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let font = document
        .add_object(dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => font});
    let image = document.add_object(Stream::new(dictionary! {"Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8}, color.to_vec()));
    let content = document.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 20 Tf 30 220 Td ({text}) Tj ET q 50 0 0 50 30 120 cm /Im1 Do Q")
            .into_bytes(),
    ));
    let page = document
        .add_object(dictionary! {"Type" => "Page", "Parent" => pages, "Contents" => content});
    document.objects.insert(pages, dictionary! {"Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)], "MediaBox" => vec![0.into(), 0.into(), 200.into(), 300.into()], "Rotate" => rotation, "Resources" => dictionary! {"Font" => dictionary! {"F1" => font}, "XObject" => dictionary! {"Im1" => image}}}.into());
    let catalog = document.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages, "Info" => dictionary! {"Producer" => Object::string_literal("synthetic-producer")}});
    document.trailer.set("Root", catalog);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("synthetic-first!"),
            Object::string_literal("synthetic-next!!"),
        ],
    );
    document
}

fn save(mut document: Document, path: &Path) {
    document.save(path).unwrap();
}

#[test]
fn merge_preserves_input_order_fonts_images_object_collisions_and_sources() {
    let directory = tempfile::tempdir().unwrap();
    let left = directory.path().join("alpha.pdf");
    let right = directory.path().join("beta.pdf");
    let output = directory.path().join("merged.pdf");
    save(fixture("ALPHA", "Helvetica", [255, 0, 0], 0), &left);
    let mut incoming = fixture("BETA", "Times-Roman", [0, 0, 255], 90);
    incoming.add_object(dictionary! {"Nonce" => Object::string_literal("incoming-object-nonce")});
    save(incoming, &right);
    let before = [
        std::fs::read(&left).unwrap(),
        std::fs::read(&right).unwrap(),
    ];
    let result = cli(&[
        "merge",
        right.to_str().unwrap(),
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["data"]["page_count"], 3);
    let merged = PdfDocument::open(&output, None).unwrap();
    for (page, expected) in [(1, "BETA"), (2, "ALPHA"), (3, "BETA")] {
        assert_eq!(merged.extract_text(&[page]).unwrap().trim(), expected);
    }
    assert_eq!(
        merged
            .info()
            .unwrap()
            .pages
            .into_iter()
            .map(|page| page.rotation)
            .collect::<Vec<_>>(),
        [90, 0, 90]
    );
    let bytes = std::fs::read(&output).unwrap();
    for (index, original) in [
        before[1].as_slice(),
        before[0].as_slice(),
        before[1].as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            pdf_render::render_page(&bytes, index, 0.5).unwrap(),
            pdf_render::render_page(original, 0, 0.5).unwrap()
        );
    }
    let document = Document::load(&output).unwrap();
    assert_eq!(
        document
            .objects
            .values()
            .filter(|object| object.as_dict().is_ok_and(|dictionary| dictionary
                .get(b"Nonce")
                .and_then(Object::as_str)
                .ok()
                == Some(b"incoming-object-nonce")))
            .count(),
        2
    );
    assert_eq!(std::fs::read(&left).unwrap(), before[0]);
    assert_eq!(std::fs::read(&right).unwrap(), before[1]);
}

#[test]
fn failed_late_merge_and_advanced_or_signed_inputs_never_publish_partial_output() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.pdf");
    let second = directory.path().join("second.pdf");
    let conflict = directory.path().join("conflict.pdf");
    save(fixture("FIRST", "Helvetica", [255, 0, 0], 0), &first);
    save(fixture("SECOND", "Courier", [0, 255, 0], 0), &second);
    let mut conflicting = fixture("CONFLICT", "Helvetica", [0, 0, 255], 0);
    conflicting.catalog_mut().unwrap().set(
        "Info",
        dictionary! {"Producer" => Object::string_literal("different-producer")},
    );
    save(conflicting, &conflict);
    let output = directory.path().join("output.pdf");
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1");
    for inputs in [
        vec![first.clone(), second.clone(), conflict.clone()],
        vec![first.clone(), corpus.join("bookmarks.pdf")],
        vec![corpus.join("forms.pdf"), first.clone()],
        vec![first.clone(), corpus.join("layers-transparency.pdf")],
        vec![corpus.join("signed.pdf"), first.clone()],
        vec![first.clone(), corpus.join("signed.pdf")],
    ] {
        let before = inputs
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect::<Vec<_>>();
        let mut args = vec!["merge"];
        args.extend(inputs.iter().map(|path| path.to_str().unwrap()));
        args.extend(["-o", output.to_str().unwrap(), "--json"]);
        let result = cli(&args);
        assert_eq!(result.status.code(), Some(4), "{result:?}");
        assert!(!output.exists());
        for (path, expected) in inputs.iter().zip(before) {
            assert_eq!(std::fs::read(path).unwrap(), expected);
        }
    }
}

#[test]
fn merge_dry_run_argument_count_and_no_clobber_follow_shared_conventions() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.pdf");
    let output = directory.path().join("output.pdf");
    save(fixture("COPY", "Helvetica", [255, 0, 0], 0), &input);
    let mut args = vec![
        "merge",
        input.to_str().unwrap(),
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    args.push("--dry-run");
    let result = cli(&args);
    assert!(result.status.success());
    assert!(!output.exists());
    args.pop();
    std::fs::write(&output, b"sentinel").unwrap();
    assert_eq!(cli(&args).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), b"sentinel");
    let before = std::fs::read(&input).unwrap();
    assert_eq!(
        cli(&[
            "merge",
            input.to_str().unwrap(),
            input.to_str().unwrap(),
            "-o",
            input.to_str().unwrap(),
            "--json"
        ])
        .status
        .code(),
        Some(3)
    );
    assert_eq!(std::fs::read(&input).unwrap(), before);
    let result = cli(&[
        "merge",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["kind"],
        "arguments"
    );
}

#[test]
fn first_input_read_only_policy_precedes_later_loading_and_late_io_failure_is_atomic() {
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.pdf");
    let missing = directory.path().join("missing.pdf");
    let output = directory.path().join("output.pdf");
    save(fixture("PLAIN", "Helvetica", [255, 0, 0], 0), &plain);
    let original = std::fs::read(&plain).unwrap();
    let signed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1/signed.pdf");
    let result = cli(&[
        "merge",
        signed.to_str().unwrap(),
        missing.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(4), "{result:?}");
    assert!(!output.exists());
    let result = cli(&[
        "merge",
        plain.to_str().unwrap(),
        plain.to_str().unwrap(),
        missing.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(3), "{result:?}");
    assert!(!output.exists());
    assert_eq!(std::fs::read(&plain).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn merge_output_symlink_is_never_followed_or_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.pdf");
    let target = directory.path().join("target.pdf");
    let output = directory.path().join("linked.pdf");
    save(fixture("COPY", "Helvetica", [255, 0, 0], 0), &input);
    std::fs::write(&target, b"target-sentinel").unwrap();
    std::os::unix::fs::symlink(&target, &output).unwrap();
    let result = cli(&[
        "merge",
        input.to_str().unwrap(),
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&target).unwrap(), b"target-sentinel");
    assert!(output.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn encrypted_inputs_are_refused_in_either_position_even_with_valid_stdin_password() {
    use lopdf::{EncryptionState, EncryptionVersion, Permissions};
    use std::io::Write;
    use std::process::Stdio;

    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.pdf");
    let encrypted = directory.path().join("encrypted.pdf");
    let output = directory.path().join("output.pdf");
    save(fixture("PLAIN", "Helvetica", [255, 0, 0], 0), &plain);
    let mut source = fixture("ENCRYPTED", "Helvetica", [255, 0, 0], 0);
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &source,
        owner_password: "synthetic-owner",
        user_password: "synthetic-user",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    save(source, &encrypted);
    assert!(
        PdfDocument::open(&encrypted, Some("synthetic-user"))
            .unwrap()
            .info()
            .unwrap()
            .encrypted
    );
    let original = std::fs::read(&encrypted).unwrap();
    for inputs in [[&plain, &encrypted], [&encrypted, &plain]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args([
                "merge",
                inputs[0].to_str().unwrap(),
                inputs[1].to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
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
            .write_all(b"synthetic-user\n")
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.code(), Some(4), "{result:?}");
        assert!(!output.exists());
        assert_eq!(std::fs::read(&encrypted).unwrap(), original);
        assert!(!String::from_utf8_lossy(&result.stdout).contains("synthetic-user"));
    }
}
