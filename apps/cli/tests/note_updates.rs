#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}
fn data(result: Output) -> Value {
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"].clone()
}
fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let stream = doc.add_object(Stream::new(
        dictionary! {},
        b"q 1 0 0 rg 20 30 20 20 re f Q".to_vec(),
    ));
    let mut kids = vec![];
    for _ in 0..2 {
        kids.push(
            doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream})
                .into(),
        );
    }
    doc.objects.insert(pages,dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>kids,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary! {}}.into());
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    doc
}
fn seed(path: &Path) {
    let mut document = fixture();
    let mut bytes = vec![];
    document.save_to(&mut bytes).unwrap();
    let mut document = PdfDocument::from_bytes(&bytes, None).unwrap();
    document
        .add_note(1, [50., 60., 74., 84.], "OLD", "Original")
        .unwrap();
    document.save_new(path).unwrap();
}
fn operation(name: &str, input: &Path, output: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        name,
        input.to_str().unwrap(),
        "--page",
        "1",
        "--annotation",
        "1",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    args.extend(extra);
    cli(&args)
}

#[test]
fn real_edit_remove_and_dry_runs_keep_originals_and_reopen_actual_values() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let edited = dir.path().join("edited.pdf");
    let removed = dir.path().join("removed.pdf");
    seed(&input);
    let original = std::fs::read(&input).unwrap();
    let dry = data(operation(
        "edit-note",
        &input,
        &edited,
        &[
            "--contents",
            "café 中文 😀",
            "--author",
            "Élodie",
            "--dry-run",
        ],
    ));
    assert_eq!(dry["note"]["contents"], "café 中文 😀");
    assert!(!edited.exists());
    let result = data(operation(
        "edit-note",
        &input,
        &edited,
        &["--contents", "café 中文 😀", "--author", "Élodie"],
    ));
    assert_eq!(result["appearance_regenerated"], false);
    assert_eq!(result["secure_erasure"], false);
    let notes = data(cli(&["list-notes", edited.to_str().unwrap(), "--json"]));
    assert_eq!(notes["notes"][0]["annotation_index"], 1);
    assert_eq!(notes["notes"][0]["contents"], "café 中文 😀");
    assert_eq!(notes["notes"][0]["author"], "Élodie");
    data(operation("remove-note", &edited, &removed, &["--dry-run"]));
    assert!(!removed.exists());
    let removed_result = data(operation("remove-note", &edited, &removed, &[]));
    assert_eq!(removed_result["secure_erasure"], false);
    assert!(
        PdfDocument::open(&removed, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()
            .is_empty()
    );
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert_eq!(
        PdfDocument::open(&edited, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()[0]
            .contents,
        "café 中文 😀"
    );
}

#[test]
fn usage_bad_index_locked_links_and_no_clobber_fail_with_typed_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let output = dir.path().join("output.pdf");
    seed(&input);
    let original = std::fs::read(&input).unwrap();
    let result = operation("edit-note", &input, &output, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["kind"],
        "arguments"
    );
    assert!(!output.exists());
    let result = cli(&[
        "remove-note",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--annotation",
        "0",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
    let result = operation("edit-note", &input, &input, &["--contents", "NEW"]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&input).unwrap(), original);
    std::fs::write(&output, b"existing").unwrap();
    let result = operation("remove-note", &input, &output, &[]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing");
    for key in ["Popup", "NM", "StructParent"] {
        let mut doc = Document::load(&input).unwrap();
        let note = PdfDocument::open(&input, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()[0]
            .object_id
            .unwrap();
        doc.get_dictionary_mut(note).unwrap().set(key, Object::Null);
        let blocked = dir.path().join(format!("blocked-{key}.pdf"));
        doc.save(&blocked).unwrap();
        let fresh = dir.path().join(format!("blocked-output-{key}.pdf"));
        for (name, extra) in [
            ("edit-note", vec!["--contents", "NEW"]),
            ("remove-note", vec![]),
        ] {
            let result = operation(name, &blocked, &fresh, &extra);
            assert_eq!(result.status.code(), Some(4));
            assert_eq!(
                serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["kind"],
                "refused"
            );
            assert!(!fresh.exists());
        }
    }
}

#[test]
fn signed_note_edits_refuse_before_any_file_is_published() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    seed(&input);
    let mut doc = Document::load(&input).unwrap();
    doc.add_object(dictionary! {"FT"=>"Sig"});
    let signed = dir.path().join("signed.pdf");
    doc.save(&signed).unwrap();
    let output = dir.path().join("output.pdf");
    assert_eq!(
        data(cli(&["list-notes", signed.to_str().unwrap(), "--json"]))["notes"][0]["contents"],
        "OLD"
    );
    for (name, extra) in [
        ("edit-note", vec!["--author", "new"]),
        ("remove-note", vec![]),
    ] {
        let result = operation(name, &signed, &output, &extra);
        assert_eq!(result.status.code(), Some(4));
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["kind"],
            "refused"
        );
        assert!(!output.exists());
    }
}

#[test]
fn sequential_cli_edits_survive_old_indirect_array_history_after_saved_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let seed_path = dir.path().join("seed.pdf");
    seed(&seed_path);
    let mut document = PdfDocument::open(&seed_path, None).unwrap();
    document
        .add_note(1, [90., 60., 114., 84.], "SECOND", "Original")
        .unwrap();
    let mut doc = Document::load_mem(&document.to_bytes().unwrap()).unwrap();
    let page = doc.get_pages()[&1];
    let entries = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let array = doc.add_object(Object::Array(entries));
    doc.get_dictionary_mut(page).unwrap().set("Annots", array);
    let input = dir.path().join("two-notes.pdf");
    doc.save(&input).unwrap();
    let first = dir.path().join("first.pdf");
    let second = dir.path().join("second.pdf");
    let final_path = dir.path().join("final.pdf");
    data(operation("remove-note", &input, &first, &[]));
    data(operation(
        "edit-note",
        &first,
        &second,
        &["--contents", "Updated second"],
    ));
    assert_eq!(
        PdfDocument::open(&second, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()[0]
            .contents,
        "Updated second"
    );
    data(operation("remove-note", &second, &final_path, &[]));
    assert!(
        PdfDocument::open(&final_path, None)
            .unwrap()
            .list_notes(&[1])
            .unwrap()
            .is_empty()
    );
    let final_doc = Document::load(&final_path).unwrap();
    assert_eq!(final_doc.objects.get(&array), doc.objects.get(&array));
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
    let result = Command::new(path).args(args).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    if args == ["-v"] || args == ["--version"] {
        [result.stdout, result.stderr].concat()
    } else {
        result.stdout
    }
}
fn model(path: &Path) -> Value {
    serde_json::from_slice(&reference(
        "qpdf",
        &[
            "--json",
            "--json-stream-data=inline",
            "--decode-level=all",
            path.to_str().unwrap(),
        ],
    ))
    .unwrap()
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn independent_appearance_pixels_and_logical_objects_survive_edit_and_ordinary_removal() {
    assert!(
        String::from_utf8(reference("pdftoppm", &["-v"]))
            .unwrap()
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    assert!(
        String::from_utf8(reference("qpdf", &["--version"]))
            .unwrap()
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pdf");
    let edited = dir.path().join("edited.pdf");
    let removed = dir.path().join("removed.pdf");
    seed(&input);
    let source = PdfDocument::open(&input, None).unwrap();
    let note = source.list_notes(&[1]).unwrap()[0].clone();
    let before = model(&input);
    let first_page = before["pages"][0]["object"].as_str().unwrap();
    let page_key = format!("obj:{first_page}");
    let note_key = format!(
        "obj:{} {} R",
        note.object_id.unwrap().0,
        note.object_id.unwrap().1
    );
    let edit = data(operation(
        "edit-note",
        &input,
        &edited,
        &["--contents", "café 中文 😀", "--author", "Élodie"],
    ));
    let new_id = edit["note"]["object_id"].as_array().unwrap();
    let new_key = format!("obj:{} {} R", new_id[0], new_id[1]);
    let after = model(&edited);
    let mut expected_note = before["qpdf"][1][&note_key]["value"].clone();
    expected_note["/Contents"] = json!("u:café 中文 😀");
    expected_note["/T"] = json!("u:Élodie");
    assert_eq!(after["qpdf"][1][&new_key]["value"], expected_note);
    let mut expected_page = before["qpdf"][1][&page_key]["value"].clone();
    expected_page["/Annots"] = json!([format!("{} {} R", new_id[0], new_id[1])]);
    assert_eq!(after["qpdf"][1][&page_key]["value"], expected_page);
    for (id, object) in before["qpdf"][1]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(id, _)| {
            *id != "trailer" && **id != page_key && object_is_logical(&before["qpdf"][1][*id])
        })
    {
        assert_eq!(after["qpdf"][1].get(id), Some(object), "{id}");
    }
    let pixels = |path: &Path| {
        reference(
            "pdftoppm",
            &[
                "-f",
                "1",
                "-l",
                "1",
                "-singlefile",
                "-r",
                "72",
                path.to_str().unwrap(),
            ],
        )
    };
    assert_eq!(pixels(&input), pixels(&edited));
    data(operation("remove-note", &edited, &removed, &[]));
    let removed_model = model(&removed);
    assert_eq!(
        removed_model["qpdf"][1][&new_key],
        after["qpdf"][1][&new_key]
    );
    assert_eq!(
        removed_model["qpdf"][1][&note_key],
        before["qpdf"][1][&note_key]
    );
    assert_eq!(
        removed_model["qpdf"][1][&page_key]["value"]["/Annots"],
        json!([])
    );
    expected_page["/Annots"] = json!([]);
    assert_eq!(removed_model["qpdf"][1][&page_key]["value"], expected_page);
    for (id, object) in after["qpdf"][1]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(id, _)| {
            *id != "trailer" && **id != page_key && object_is_logical(&after["qpdf"][1][*id])
        })
    {
        assert_eq!(
            removed_model["qpdf"][1].get(id),
            Some(object),
            "removed logical object {id}"
        );
    }
    let mut blank = fixture();
    let blank_path = dir.path().join("blank.pdf");
    blank.save(&blank_path).unwrap();
    assert_eq!(pixels(&removed), pixels(&blank_path));
}
fn object_is_logical(object: &Value) -> bool {
    !matches!(
        object["stream"]["dict"]["/Type"].as_str(),
        Some("/XRef" | "/ObjStm")
    )
}
