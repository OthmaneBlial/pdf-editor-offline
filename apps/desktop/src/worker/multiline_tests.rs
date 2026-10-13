#![allow(clippy::unwrap_used)]
use super::*;
use pdf_core::{FormValue, PdfDocument};

fn fixture() -> Vec<u8> {
    let mut doc =
        lopdf::Document::load_mem(include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"))
            .unwrap();
    let field = doc.get_dictionary_mut((8, 0)).unwrap();
    let top = field.get(b"Rect").unwrap().as_array().unwrap()[3]
        .as_float()
        .unwrap();
    field.set("Ff", 1 << 12);
    field.set(
        "Rect",
        vec![72.into(), (top - 110.0).into(), 300.into(), top.into()],
    );
    field.set("DA", lopdf::Object::string_literal("/Helv 12 Tf 0 g"));
    let old = doc.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    dict.set("BBox", vec![0.into(), 0.into(), 228.into(), 110.into()]);
    let content = String::from_utf8(old.get_plain_content().unwrap())
        .unwrap()
        .replace("0 0 228 30 re", "0 0 228 110 re");
    doc.objects.insert(
        (9, 0),
        lopdf::Stream::new(dict, content.into_bytes()).into(),
    );
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn exercise(reference: bool) {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("multiline.pdf");
    let font = directory.path().join("font.ttf");
    let original = fixture();
    std::fs::write(&input, &original).unwrap();
    std::fs::write(
        &font,
        include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
    )
    .unwrap();
    let mut journal = Journal::open(directory.path().join("journal")).unwrap();
    let mut documents = HashMap::new();
    process_with_journal(
        &mut documents,
        Command::Load {
            id: 1,
            path: input.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    let key = FormsReadKey {
        tab: 1,
        revision: 0,
        generation: 1,
    };
    let Event::Forms { fields, .. } =
        process_with_journal(&mut documents, Command::Forms(key), Some(&mut journal)).unwrap()
    else {
        panic!("forms response");
    };
    let field = fields
        .iter()
        .find(|field| field.name == "full_name")
        .unwrap();
    assert!(
        field.writable_text && field.flags & (1 << 12) != 0,
        "parent validates real multiline capability"
    );
    let before = documents[&1].current.bytes.clone();
    let initial_tab = crate::state::Tab::new(1, input.clone(), documents[&1].status().unwrap());
    let original_preview = render(&documents[&1], initial_tab.key().unwrap(), 1.0).unwrap();
    let command = |key, value: &str, font_file: PathBuf| Command::Edit {
        id: 1,
        edit: Edit::FillText {
            key,
            changes: vec![pdf_core::TextChange {
                name: "full_name".into(),
                value: value.into(),
            }],
            buttons: vec![],
            choices: vec![],
            page: 1,
            font_file,
        },
    };
    let bad = directory.path().join("bad.ttf");
    std::fs::write(&bad, b"not a font").unwrap();
    let mut restricted =
        include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf").to_vec();
    let count = u16::from_be_bytes(restricted[4..6].try_into().unwrap()) as usize;
    let os2 = (0..count)
        .find_map(|index| {
            let at = 12 + index * 16;
            (&restricted[at..at + 4] == b"OS/2").then(|| {
                u32::from_be_bytes(restricted[at + 8..at + 12].try_into().unwrap()) as usize
            })
        })
        .unwrap();
    restricted[os2 + 8..os2 + 10].copy_from_slice(&2u16.to_be_bytes());
    let restricted_path = directory.path().join("restricted.ttf");
    std::fs::write(&restricted_path, restricted).unwrap();
    for invalid in [
        command(key, "First\nSecond", bad),
        command(key, "First\nSecond", restricted_path),
        command(key, "中文", font.clone()),
        command(key, &"A\n".repeat(50), font.clone()),
        command(
            FormsReadKey {
                revision: 99,
                ..key
            },
            "New\nValue",
            font.clone(),
        ),
        command(FormsReadKey { tab: 2, ..key }, "New\nValue", font.clone()),
    ] {
        assert!(process_with_journal(&mut documents, invalid, Some(&mut journal)).is_err());
        assert_eq!(documents[&1].current.bytes, before);
        assert!(documents[&1].undo.is_empty());
        assert_eq!(journal.recover().unwrap()[0].current.bytes, before);
    }
    let value = "Résumé e\u{301}\nSecond line ΩЖ";
    let Event::Updated {
        status,
        selected_page,
        ..
    } = process_with_journal(
        &mut documents,
        command(key, value, font.clone()),
        Some(&mut journal),
    )
    .unwrap()
    else {
        panic!("update");
    };
    assert!(status.dirty && status.can_undo);
    assert_eq!(selected_page, Some(0));
    let changed = documents[&1].current.bytes.clone();
    let tab = crate::state::Tab::new(1, input.clone(), status);
    let preview = render(&documents[&1], tab.key().unwrap(), 1.0).unwrap();
    assert_eq!([preview.width, preview.height], [595, 842]);
    assert_ne!(
        preview.rgba, original_preview.rgba,
        "real child renders the changed appearance"
    );
    assert_eq!(documents[&1].undo.len(), 1);
    let parsed = PdfDocument::from_bytes(&changed, None).unwrap();
    assert!(parsed.extract_text(&[1]).unwrap().contains(value));
    assert_eq!(
        parsed
            .list_form_fields()
            .unwrap()
            .into_iter()
            .find(|f| f.name == "full_name")
            .unwrap()
            .value,
        FormValue::Text(value.into())
    );
    assert!(
        process_with_journal(
            &mut documents,
            command(key, "Stale\nValue", font.clone()),
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(documents[&1].current.bytes, changed);
    assert_eq!(documents[&1].undo.len(), 1);
    for (edit, expected) in [(Edit::Undo, before.clone()), (Edit::Redo, changed.clone())] {
        process_with_journal(
            &mut documents,
            Command::Edit { id: 1, edit },
            Some(&mut journal),
        )
        .unwrap();
        assert_eq!(documents[&1].current.bytes, expected);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, expected);
    }
    let output = directory.path().join("saved.pdf");
    process_with_journal(
        &mut documents,
        Command::Save {
            id: 1,
            path: output.clone(),
        },
        Some(&mut journal),
    )
    .unwrap();
    assert!(!documents[&1].status().unwrap().dirty);
    assert!(
        PdfDocument::open(&output, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains(value)
    );
    let saved = std::fs::read(&output).unwrap();
    assert!(
        process_with_journal(
            &mut documents,
            Command::Save {
                id: 1,
                path: output.clone()
            },
            Some(&mut journal)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), original);
    if reference {
        use std::process::Command as External;
        let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
        let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
        let check = External::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap();
        assert!(check.status.success());
        let json = External::new(&qpdf)
            .arg("--json")
            .arg(&output)
            .output()
            .unwrap();
        assert!(json.status.success());
        let model: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(
            model["qpdf"][1]["obj:8 0 R"]["value"]["/V"],
            format!("u:{value}")
        );
        let text = External::new(bin.join("pdftotext"))
            .args(["-raw"])
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert!(String::from_utf8(text.stdout).unwrap().contains(value));
        let pixels = |path: &std::path::Path| {
            let result = External::new(bin.join("pdftoppm"))
                .args(["-f", "1", "-singlefile", "-r", "72"])
                .arg(path)
                .output()
                .unwrap();
            assert!(result.status.success());
            result
                .stdout
                .strip_prefix(b"P6\n595 842\n255\n")
                .unwrap()
                .to_vec()
        };
        let old = pixels(&input);
        let new = pixels(&output);
        let mut changed = 0;
        for (index, (a, b)) in old.chunks_exact(3).zip(new.chunks_exact(3)).enumerate() {
            if a != b {
                let x = index % 595;
                let y = index / 595;
                assert!(
                    (72..300).contains(&x) && (90..200).contains(&y),
                    "outside actualwidget{x},{y}"
                );
                changed += 1;
            }
        }
        assert!(changed > 20);
    }
}
#[test]
fn multiline_child_history_font_stale_and_verified_save_are_real() {
    exercise(false);
}

#[test]
#[ignore = "build normal native binary; requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn production_native_worker_multiline_undo_save_and_independent_reference() {
    let current = std::env::current_exe().unwrap();
    let path = current
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-editor-offline-native.exe"
        } else {
            "pdf-editor-offline-native"
        });
    assert!(path.is_file(), "build normal native binary first");
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    assert!(
        String::from_utf8_lossy(
            &std::process::Command::new(qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .contains("12.4.2")
    );
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        String::from_utf8_lossy(
            &std::process::Command::new(bin.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    document::with_test_executor(
        path,
        vec![std::ffi::OsString::from("--pdf-worker")],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
