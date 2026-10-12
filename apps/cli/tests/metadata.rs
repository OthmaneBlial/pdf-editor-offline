#![allow(clippy::unwrap_used)]

use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use pdf_core::{MetadataKey, PdfDocument};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}
fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 40 Td (Metadata fixture text) Tj ET".to_vec(),
    ));
    let page=doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Contents"=>content,"Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let xmp = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><title>Independent XML title</title></x:xmpmeta>"
            .to_vec(),
    ));
    let info=doc.add_object(dictionary! {"Title"=>Object::string_literal("Original title"),"Author"=>Object::string_literal("Original author"),"Custom"=>dictionary! {"Untouched"=>vec![7.into(),Object::Boolean(true)]},"CreationDate"=>Object::string_literal("D:20261012030000Z")});
    let root=doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"Metadata"=>xmp,"Info"=>dictionary! {"Producer"=>Object::string_literal("nonstandard producer")},"AliasToOriginalInfo"=>info});
    doc.trailer.set("Root", root);
    doc.trailer.set("Info", info);
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal("metadata-first!!"),
            Object::string_literal("metadata-next!!!"),
        ],
    );
    doc
}
fn save(mut doc: Document, path: &Path) {
    doc.save(path).unwrap();
}
fn edit(
    command: &str,
    input: &Path,
    output: &Path,
    key: &str,
    value: Option<&str>,
    extra: &[&str],
) -> Output {
    let mut args = vec![
        command,
        input.to_str().unwrap(),
        "--key",
        key,
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    if let Some(value) = value {
        args.extend(["--value", value]);
    }
    args.extend(extra);
    cli(&args)
}
fn data(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

#[test]
fn set_read_remove_unicode_verified_copy_and_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("edited.pdf");
    save(fixture(), &input);
    let original = std::fs::read(&input).unwrap();
    let text = "Révision – café 中文 😀";
    let dry = data(&edit(
        "set-metadata",
        &input,
        &output,
        "title",
        Some(text),
        &["--dry-run"],
    ));
    assert_eq!(dry["metadata"]["fields"]["title"], text);
    assert!(!output.exists());
    let saved = data(&edit(
        "set-metadata",
        &input,
        &output,
        "title",
        Some(text),
        &[],
    ));
    assert_eq!(saved["metadata"]["fields"]["title"], text);
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let report = data(&cli(&["metadata", output.to_str().unwrap(), "--json"]));
    assert_eq!(report["fields"]["title"], text);
    assert_eq!(report["fields"]["author"], "Original author");
    assert_eq!(report["xmp_synchronized"], false);
    assert_eq!(report["secure_erasure"], false);
    let removed = dir.path().join("removed.pdf");
    data(&edit(
        "remove-metadata",
        &output,
        &removed,
        "author",
        None,
        &[],
    ));
    assert!(
        !PdfDocument::open(&removed, None)
            .unwrap()
            .metadata()
            .unwrap()
            .fields
            .contains_key(&MetadataKey::Author)
    );
    assert!(
        PdfDocument::open(&output, None)
            .unwrap()
            .metadata()
            .unwrap()
            .fields
            .contains_key(&MetadataKey::Author)
    );
}

#[test]
fn invalid_keys_limits_and_clobber_have_typed_errors_and_preserve_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("output.pdf");
    save(fixture(), &input);
    let original = std::fs::read(&input).unwrap();
    for (key, value, code, kind) in [
        ("creation-date", "value", 2, "arguments"),
        // Exceed the UTF-8 byte budget while staying below Windows' UTF-16 argv budget.
        ("title", &"界".repeat(21846), 5, "resource_limit"),
    ] {
        let result = edit("set-metadata", &input, &output, key, Some(value), &[]);
        assert_eq!(result.status.code(), Some(code));
        let result: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(result["error"]["kind"], kind);
        assert!(!output.exists());
    }
    let result = edit("set-metadata", &input, &input, "title", Some("new"), &[]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&input).unwrap(), original);
    std::fs::write(&output, b"existing").unwrap();
    let result = edit("remove-metadata", &input, &output, "title", None, &[]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing");
}

#[test]
fn signed_metadata_is_readable_but_edits_are_refused() {
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdfs/signed.pdf");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.pdf");
    assert!(data(&cli(&["metadata", input.to_str().unwrap(), "--json"]))["fields"].is_object());
    for command in ["set-metadata", "remove-metadata"] {
        let result = edit(
            command,
            &input,
            &output,
            "title",
            if command == "set-metadata" {
                Some("new")
            } else {
                None
            },
            &[],
        );
        assert_eq!(result.status.code(), Some(4));
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["error"]["kind"], "refused");
        assert!(!output.exists());
    }
}

#[test]
fn decrypted_metadata_is_readable_via_stdin_password_but_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("encrypted.pdf");
    let output = dir.path().join("output.pdf");
    let mut document = fixture();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &document,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    document.encrypt(&state).unwrap();
    save(document, &input);
    let original = std::fs::read(&input).unwrap();
    for command in ["metadata", "set-metadata", "remove-metadata"] {
        let mut args = vec![
            command,
            input.to_str().unwrap(),
            "--json",
            "--password-stdin",
        ];
        if command != "metadata" {
            args.extend(["--key", "title", "-o", output.to_str().unwrap()]);
        }
        if command == "set-metadata" {
            args.extend(["--value", "new"]);
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"reader\n").unwrap();
        let result = child.wait_with_output().unwrap();
        if command == "metadata" {
            assert_eq!(data(&result)["fields"]["author"], "Original author");
        } else {
            assert_eq!(result.status.code(), Some(4));
            let json: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(json["error"]["kind"], "refused");
            assert!(!output.exists());
        }
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let path = if name == "qpdf" {
        std::env::var_os("PDF_REFERENCE_QPDF").map_or_else(|| PathBuf::from(name), PathBuf::from)
    } else {
        std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
            || PathBuf::from(name),
            |directory| PathBuf::from(directory).join(name),
        )
    };
    let output = Command::new(path).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if args == ["-v"] || args == ["--version"] {
        [output.stdout, output.stderr].concat()
    } else {
        output.stdout
    }
}
fn model(path: &Path) -> Value {
    let mut model: Value = serde_json::from_slice(&reference(
        "qpdf",
        &[
            "--json",
            "--json-stream-data=inline",
            "--decode-level=all",
            path.to_str().unwrap(),
        ],
    ))
    .unwrap();
    let objects = model["qpdf"][1].as_object_mut().unwrap();
    let trailer = objects.get_mut("trailer").unwrap()["value"]
        .as_object_mut()
        .unwrap();
    for key in ["/Size", "/Prev", "/XRefStm", "/ID"] {
        trailer.remove(key);
    }
    if trailer.get("/Type") == Some(&json!("/XRef")) {
        for key in [
            "/Type",
            "/W",
            "/Index",
            "/Length",
            "/Filter",
            "/DecodeParms",
        ] {
            trailer.remove(key);
        }
    }
    let carriers = objects
        .iter()
        .filter(|(_, object)| {
            matches!(
                object["stream"]["dict"]["/Type"].as_str(),
                Some("/XRef" | "/ObjStm")
            )
        })
        .map(|(key, _)| key.strip_prefix("obj:").unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    for (id, object) in objects.iter() {
        if carriers.contains(id.strip_prefix("obj:").unwrap_or(id)) {
            continue;
        }
        let mut pending = vec![object];
        while let Some(value) = pending.pop() {
            match value {
                Value::String(reference) => assert!(
                    !carriers.contains(reference),
                    "logical reference to excluded carrier"
                ),
                Value::Array(values) => pending.extend(values),
                Value::Object(values) => pending.extend(values.values()),
                _ => {}
            }
        }
    }
    objects.retain(|id, _| !carriers.contains(id.strip_prefix("obj:").unwrap_or(id)));
    model
}
fn preserved(before: &Value, after: &Value, key: &str, value: Option<&str>) {
    assert_eq!(before["version"], 2);
    assert_eq!(after["version"], 2);
    let before_objects = before["qpdf"][1].as_object().unwrap();
    let after_objects = after["qpdf"][1].as_object().unwrap();
    for (id, object) in before_objects.iter().filter(|(id, _)| *id != "trailer") {
        assert_eq!(after_objects.get(id), Some(object), "logical object {id}");
    }
    assert_eq!(after_objects.len(), before_objects.len() + 1);
    let mut old_trailer = before_objects["trailer"]["value"].clone();
    let mut new_trailer = after_objects["trailer"]["value"].clone();
    let previous_info = old_trailer["/Info"].as_str().unwrap();
    let new_info = new_trailer["/Info"].as_str().unwrap();
    let mut expected = before_objects[&format!("obj:{previous_info}")]["value"].clone();
    if let Some(value) = value {
        expected[key] = json!(format!("u:{value}"));
    } else {
        expected.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(after_objects[&format!("obj:{new_info}")]["value"], expected);
    for trailer in [&mut old_trailer, &mut new_trailer] {
        for key in ["/Size", "/Info"] {
            trailer.as_object_mut().unwrap().remove(key);
        }
    }
    assert_eq!(old_trailer, new_trailer);
    assert_eq!(before["pages"], after["pages"]);
    assert_eq!(before["outlines"], after["outlines"]);
    assert_eq!(before["attachments"], after["attachments"]);
    assert_eq!(before["acroform"], after["acroform"]);
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2 executables"]
fn independent_unicode_and_full_logical_graph_preservation() {
    assert!(
        String::from_utf8(reference("pdftotext", &["-v"]))
            .unwrap()
            .lines()
            .any(|line| line == "pdftotext version 26.10.0")
    );
    assert!(
        String::from_utf8(reference("pdfinfo", &["-v"]))
            .unwrap()
            .lines()
            .any(|line| line == "pdfinfo version 26.10.0")
    );
    assert!(
        String::from_utf8(reference("qpdf", &["--version"]))
            .unwrap()
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let dir = tempfile::tempdir().unwrap();
    let synthetic = dir.path().join("synthetic.pdf");
    save(fixture(), &synthetic);
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdfs");
    let mut sources = vec![synthetic];
    for filename in [
        "forms.pdf",
        "bookmarks.pdf",
        "attachment.pdf",
        "layers-transparency.pdf",
        "image-scan.pdf",
        "mixed-fonts.pdf",
        "rotated-page.pdf",
    ] {
        sources.push(corpus.join(filename));
    }
    let value = "Révision – café 中文 😀";
    for (index, input) in sources.iter().enumerate() {
        let source_bytes = std::fs::read(input).unwrap();
        let before = model(input);
        let output = dir.path().join(format!("set-{index}.pdf"));
        data(&edit(
            "set-metadata",
            input,
            &output,
            "title",
            Some(value),
            &[],
        ));
        let after = model(&output);
        preserved(&before, &after, "/Title", Some(value));
        assert_eq!(
            reference(
                "pdftotext",
                &["-enc", "UTF-8", input.to_str().unwrap(), "-"]
            ),
            reference(
                "pdftotext",
                &["-enc", "UTF-8", output.to_str().unwrap(), "-"]
            )
        );
        let info = String::from_utf8(reference("pdfinfo", &[output.to_str().unwrap()])).unwrap();
        assert!(info.lines().any(|line| {
            line.strip_prefix("Title:")
                .is_some_and(|text| text.trim() == value)
        }));
        let removed = dir.path().join(format!("removed-{index}.pdf"));
        data(&edit(
            "remove-metadata",
            &output,
            &removed,
            "author",
            None,
            &[],
        ));
        preserved(&after, &model(&removed), "/Author", None);
        assert_eq!(std::fs::read(input).unwrap(), source_bytes);
    }
}
