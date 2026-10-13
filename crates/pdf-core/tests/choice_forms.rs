#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object};
use pdf_core::{ChoiceChange, FormValue, PdfDocument};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn bytes(document: &mut Document) -> Vec<u8> {
    let mut bytes = vec![];
    document.save_to(&mut bytes).unwrap();
    bytes
}
fn string(value: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for unit in value.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

#[test]
fn real_priority_combo_reports_options_selection_and_preserves_source() {
    let pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let fields = pdf.list_choice_fields().unwrap();
    assert_eq!(fields.len(), 1);
    let priority = &fields[0];
    assert_eq!(priority.field.name, "priority");
    assert!(priority.combo && !priority.multi_select);
    assert_eq!(
        priority
            .options
            .iter()
            .map(|option| option.display.as_str())
            .collect::<Vec<_>>(),
        ["Low", "Normal", "High"]
    );
    assert_eq!(priority.selected_indices, Some(vec![1]));
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn paired_duplicate_exports_require_consistent_authoritative_indices() {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let field = source.get_dictionary_mut((14, 0)).unwrap();
    field.set(
        "Opt",
        vec![
            Object::Array(vec![string("export"), string("First label")]),
            Object::Array(vec![string("export"), string("Second label")]),
        ],
    );
    field.set("V", string("export"));
    let pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    assert_eq!(pdf.list_choice_fields().unwrap()[0].selected_indices, None);
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("I", vec![Object::Integer(1)]);
    let pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    assert_eq!(
        pdf.list_choice_fields().unwrap()[0].selected_indices,
        Some(vec![1])
    );
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("V", string("unknown"));
    let pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].selected_indices, None);
    assert!(!report[0].writable);
}

#[test]
fn malformed_options_indices_and_top_indices_are_explicit_errors() {
    for (key, value) in [
        (
            "Opt",
            Object::Array(vec![Object::Array(vec![string("short")])]),
        ),
        ("Opt", Object::Array(vec![Object::Integer(4)])),
        ("I", Object::Array(vec![2.into(), 1.into()])),
        ("I", Object::Array(vec![1.into(), 1.into()])),
        ("I", Object::Array(vec![3.into()])),
        ("I", Object::Array(vec![(-1).into()])),
        ("TI", Object::Integer(-1)),
        ("TI", Object::Integer(3)),
    ] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        source.get_dictionary_mut((14, 0)).unwrap().set(key, value);
        let pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        assert!(pdf.list_choice_fields().is_err(), "{key}");
    }
}

fn change(indices: &[u32], top_index: Option<u32>) -> ChoiceChange {
    ChoiceChange {
        name: "priority".into(),
        indices: indices.to_vec(),
        top_index,
    }
}
fn list_source(multi: bool) -> Vec<u8> {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let field = source.get_dictionary_mut((14, 0)).unwrap();
    field.set("Ff", if multi { 1 << 21 } else { 0 });
    field.set("Rect", vec![72.into(), 550.into(), 220.into(), 650.into()]);
    field.set(
        "Opt",
        vec![
            Object::Array(vec![string("a"), string("Alpha")]),
            Object::Array(vec![string("b"), string("Beta")]),
            Object::Array(vec![string("c"), string("Gamma")]),
        ],
    );
    field.set("V", string("a"));
    field.set("I", vec![Object::Integer(0)]);
    source
        .get_object_mut((15, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("BBox", vec![0.into(), 0.into(), 148.into(), 100.into()]);
    bytes(&mut source)
}

#[test]
fn combo_has_real_unicode_appearance_repeatable_indices_and_unrelated_objects() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let source = Document::load_mem(SOURCE).unwrap();
    let original_pages = pdf.info().unwrap().pages;
    for (index, label) in [(2, "High"), (0, "Low")] {
        pdf.fill_choice(&[change(&[index], None)], FONT).unwrap();
        let fields = pdf.list_choice_fields().unwrap();
        assert!(fields[0].writable);
        assert_eq!(fields[0].selected_indices, Some(vec![index]));
        assert_eq!(fields[0].field.value, FormValue::Text(label.into()));
        assert!(pdf.extract_text(&[1]).unwrap().contains(label));
        let changed = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
        for (id, object) in &source.objects {
            if !matches!(*id, (1, 0) | (14, 0)) {
                assert_eq!(
                    changed.objects.get(id),
                    Some(object),
                    "unrelated object{id:?}"
                );
            }
        }
        assert_eq!(pdf.info().unwrap().pages, original_pages);
    }
    pdf.fill_choice(&[change(&[], None)], FONT).unwrap();
    assert_eq!(
        pdf.list_choice_fields().unwrap()[0].selected_indices,
        Some(vec![])
    );
}

#[test]
fn list_multiselection_is_visible_scrollable_and_repeatable() {
    let original = list_source(true);
    let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
    let before = pdf_render::render_page(&original, 0, 1.0).unwrap();
    pdf.fill_choice(&[change(&[1, 2], Some(1))], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert!(report[0].writable && report[0].multi_select && !report[0].combo);
    assert_eq!(report[0].selected_indices, Some(vec![1, 2]));
    assert_eq!(report[0].top_index, 1);
    assert_eq!(
        report[0].field.value,
        FormValue::TextArray(vec!["b".into(), "c".into()])
    );
    let saved = pdf.verified_bytes().unwrap();
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(
        text.contains("Beta\nGamma"),
        "logical visible rows {text:?}"
    );
    assert!(!text.contains("Alpha"));
    let after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    let mut changes = 0;
    let mut blue = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (index % 595, index / 595);
            assert!(
                (72..220).contains(&x) && (192..292).contains(&y),
                "outside widget{x},{y}"
            );
            changes += 1;
        }
        if b[2] > 160 && b[0] < 80 && b[1] < 130 {
            blue += 1;
        }
    }
    assert!(changes > 0 && blue > 100, "real selected-row highlight");
    pdf.fill_choice(&[change(&[0], Some(0))], FONT).unwrap();
    assert_eq!(
        pdf.list_choice_fields().unwrap()[0].selected_indices,
        Some(vec![0])
    );
    assert!(pdf.extract_text(&[1]).unwrap().contains("Alpha"));
}

#[test]
fn invalid_late_requests_fonts_flags_and_layouts_are_atomic() {
    for mut source in [
        Document::load_mem(SOURCE).unwrap(),
        Document::load_mem(&list_source(false)).unwrap(),
    ] {
        for invalid in [
            change(&[3], None),
            change(&[1, 1], None),
            change(&[2, 1], None),
            change(&[0, 1], None),
            change(&[0], Some(99)),
        ] {
            let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
            let before = pdf.to_bytes().unwrap();
            assert!(pdf.fill_choice(&[invalid], FONT).is_err());
            assert_eq!(pdf.to_bytes().unwrap(), before);
        }
    }
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let missing = ChoiceChange {
        name: "missing".into(),
        indices: vec![0],
        top_index: None,
    };
    assert!(
        pdf.fill_choice(&[change(&[2], None), missing], FONT)
            .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
    assert!(
        pdf.fill_choice(&[change(&[2], None)], b"invalid font")
            .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
    for flag in [1, 1 << 18, (1 << 17) | (1 << 21)] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        source.get_dictionary_mut((14, 0)).unwrap().set("Ff", flag);
        let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(!pdf.list_choice_fields().unwrap()[0].writable);
        assert!(pdf.fill_choice(&[change(&[0], None)], FONT).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
}

#[test]
fn small_list_automatically_reveals_new_selection_but_explicit_scroll_wins() {
    let mut source = Document::load_mem(&list_source(true)).unwrap();
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("Rect", vec![72.into(), 622.into(), 220.into(), 652.into()]);
    source.get_dictionary_mut((14, 0)).unwrap().set("TI", 1);
    source
        .get_object_mut((15, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("BBox", vec![0.into(), 0.into(), 148.into(), 30.into()]);
    let original = bytes(&mut source);
    let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
    pdf.fill_choice(&[change(&[2], None)], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].top_index, 2);
    assert_eq!(report[0].selected_indices, Some(vec![2]));
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Gamma") && !text.contains("Alpha") && !text.contains("Beta"));
    pdf.fill_choice(&[change(&[0, 2], None)], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].top_index, 2, "one selected row already visible");
    assert_eq!(report[0].selected_indices, Some(vec![0, 2]));
    pdf.fill_choice(&[change(&[0, 2], Some(1))], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].top_index, 1);
    assert_eq!(report[0].selected_indices, Some(vec![0, 2]));
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(
        text.contains("Beta") && !text.contains("Alpha") && !text.contains("Gamma"),
        "explicit scroll can intentionally show an unselected row"
    );
    pdf.fill_choice(&[change(&[0, 2], None)], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].top_index, 0);
    assert_eq!(report[0].selected_indices, Some(vec![0, 2]));
    let changed = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    for (id, object) in &source.objects {
        if !matches!(*id, (1, 0) | (14, 0)) {
            assert_eq!(
                changed.objects.get(id),
                Some(object),
                "unrelated object{id:?}"
            );
        }
    }
}

#[test]
fn paired_exports_change_logical_value_and_duplicate_option_appearance_by_index() {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let field = source.get_dictionary_mut((14, 0)).unwrap();
    field.set(
        "Opt",
        vec![
            Object::Array(vec![string("same"), string("Première")]),
            Object::Array(vec![string("same"), string("Deuxième")]),
        ],
    );
    field.set("V", string("same"));
    field.set("I", vec![Object::Integer(0)]);
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    pdf.fill_choice(&[change(&[1], None)], FONT).unwrap();
    let report = pdf.list_choice_fields().unwrap();
    assert_eq!(report[0].field.value, FormValue::Text("same".into()));
    assert_eq!(report[0].selected_indices, Some(vec![1]));
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(
        text.contains("Deuxième") && !text.contains("Première"),
        "actual text {text:?}"
    );
}

#[test]
fn inherited_options_flags_values_defaults_and_empty_legacy_value_are_preserved() {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let original = source.get_dictionary((14, 0)).unwrap().clone();
    let mut parent = lopdf::Dictionary::new();
    parent.set("T", string("Group"));
    parent.set("Kids", vec![Object::Reference((14, 0))]);
    for key in [b"FT".as_slice(), b"Ff", b"Opt", b"V", b"DA"] {
        parent.set(key, original.get(key).unwrap().clone());
    }
    parent.set("DV", string("Low"));
    let id = source.add_object(parent);
    let field = source.get_dictionary_mut((14, 0)).unwrap();
    field.set("Parent", id);
    for key in [b"FT".as_slice(), b"Ff", b"Opt", b"V", b"DA"] {
        field.remove(key);
    }
    let form = source
        .get_dictionary_mut((1, 0))
        .unwrap()
        .get_mut(b"AcroForm")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    for field in form.get_mut(b"Fields").unwrap().as_array_mut().unwrap() {
        if *field == Object::Reference((14, 0)) {
            *field = Object::Reference(id);
        }
    }
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let field = &pdf.list_choice_fields().unwrap()[0];
    assert_eq!(field.field.name, "Group.priority");
    assert_eq!(field.default_value, FormValue::Text("Low".into()));
    assert_eq!(field.selected_indices, Some(vec![1]));
    assert!(field.writable);
    pdf.fill_choice(
        &[ChoiceChange {
            name: "Group.priority".into(),
            indices: vec![2],
            top_index: None,
        }],
        FONT,
    )
    .unwrap();
    assert_eq!(
        pdf.list_choice_fields().unwrap()[0].default_value,
        FormValue::Text("Low".into())
    );
    let changed = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    assert_eq!(
        changed.get_dictionary(id).unwrap(),
        source.get_dictionary(id).unwrap()
    );
    pdf.fill_choice(
        &[ChoiceChange {
            name: "Group.priority".into(),
            indices: vec![],
            top_index: None,
        }],
        FONT,
    )
    .unwrap();
    let cleared = pdf.list_choice_fields().unwrap();
    assert_eq!(cleared[0].selected_indices, Some(vec![]));
    assert_eq!(cleared[0].field.value, FormValue::Empty);
    let saved = pdf.verified_bytes().unwrap();
    let reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(
        reopened.list_choice_fields().unwrap()[0].selected_indices,
        Some(vec![])
    );
    let changed = Document::load_mem(&saved).unwrap();
    assert!(matches!(
        changed.get_dictionary((14, 0)).unwrap().get(b"V").unwrap(),
        Object::Null
    ));
    assert_eq!(
        changed.get_dictionary(id).unwrap(),
        source.get_dictionary(id).unwrap()
    );
    let mut source = Document::load_mem(SOURCE).unwrap();
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("V", string(""));
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    assert_eq!(
        pdf.list_choice_fields().unwrap()[0].selected_indices,
        Some(vec![])
    );
    pdf.fill_choice(&[change(&[0], None)], FONT).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("Low"));
}

#[test]
fn actions_hidden_tagged_signed_xfa_missing_glyph_and_oversized_options_refuse() {
    for (key, value) in [
        (
            "A",
            Object::Dictionary(lopdf::dictionary! {"S"=>"JavaScript","JS"=>string("ignored")}),
        ),
        ("F", Object::Integer(2)),
        ("F", Object::Integer(32)),
        ("StructParent", Object::Integer(0)),
    ] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        source.get_dictionary_mut((14, 0)).unwrap().set(key, value);
        let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(!pdf.list_choice_fields().unwrap()[0].writable);
        assert!(pdf.fill_choice(&[change(&[0], None)], FONT).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    for signed in [false, true] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        if signed {
            source.add_object(lopdf::dictionary! {"FT"=>"Sig"});
        } else {
            source
                .get_dictionary_mut((1, 0))
                .unwrap()
                .get_mut(b"AcroForm")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("XFA", string("unsupported packet"));
        }
        let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        let before = pdf.list_choice_fields().unwrap();
        assert!(!before[0].writable);
        assert!(pdf.fill_choice(&[change(&[0], None)], FONT).is_err());
        assert_eq!(pdf.list_choice_fields().unwrap(), before);
    }
    let mut source = Document::load_mem(SOURCE).unwrap();
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("Opt", vec![string("Normal"), string("🦊")]);
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    assert!(pdf.fill_choice(&[change(&[1], None)], FONT).is_err());
    assert_eq!(pdf.to_bytes().unwrap(), before);
    source
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("Opt", vec![string("small"); 10_001]);
    let pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    assert!(pdf.list_choice_fields().is_err());
}

#[test]
fn cross_boundary_graphics_and_pending_artwork_paths_are_refused_atomically() {
    for content in [
        "q /Tx BMC Q BT /Helv 12 Tf (Normal) Tj ET q EMC Q",
        "/Tx BMC q BT /Helv 12 Tf (Normal) Tj ET EMC Q",
        "0 0 10 10 re /Tx BMC BT /Helv 12 Tf (Normal) Tj ET EMC f",
        "/Tx BMC BT /Helv 12 Tf (Normal) Tj ET 0 0 10 10 re EMC f",
    ] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        let stream = source
            .get_object_mut((15, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.dict.remove(b"Filter");
        stream.dict.remove(b"DecodeParms");
        stream.set_content(content.as_bytes().to_vec());
        let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        let field = &pdf.list_choice_fields().unwrap()[0];
        assert!(!field.writable);
        assert!(
            field
                .refusal_reason
                .as_ref()
                .unwrap()
                .contains("replacement boundary"),
            "{:?}",
            field.refusal_reason
        );
        assert!(pdf.fill_choice(&[change(&[2], None)], FONT).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
}

#[test]
fn unisolated_choice_state_cannot_feed_later_artwork_but_isolated_art_survives() {
    for content in [
        "/Tx BMC BT /Helv 12 Tf 1 0 0 rg (Normal) Tj ET EMC 0 0 10 10 re f",
        "/Tx BMC BT /Helv 12 Tf 1 0 0 RG (Normal) Tj ET EMC 0 0 10 10 re S",
    ] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        let stream = source
            .get_object_mut((15, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.dict.remove(b"Filter");
        stream.dict.remove(b"DecodeParms");
        stream.set_content(content.as_bytes().to_vec());
        let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        let report = pdf.list_choice_fields().unwrap();
        assert!(!report[0].writable);
        assert!(
            report[0]
                .refusal_reason
                .as_ref()
                .unwrap()
                .contains("depends on replaced text state")
        );
        assert!(pdf.fill_choice(&[change(&[2], None)], FONT).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut source = Document::load_mem(SOURCE).unwrap();
    let stream = source
        .get_object_mut((15, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.dict.remove(b"Filter");
    stream.dict.remove(b"DecodeParms");
    stream.set_content(
        b"/Tx BMC q BT /Helv 12 Tf 1 0 0 rg (Normal) Tj ET Q EMC 0 0 1 rg 0 0 10 10 re f".to_vec(),
    );
    let original = bytes(&mut source);
    let before = pdf_render::render_page(&original, 0, 1.0).unwrap();
    let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
    pdf.fill_choice(&[change(&[2], None)], FONT).unwrap();
    pdf.fill_choice(&[change(&[0], None)], FONT).unwrap();
    let after = pdf_render::render_page(&pdf.verified_bytes().unwrap(), 0, 1.0).unwrap();
    for y in 212..220 {
        for x in 72..80 {
            let pixel = (y * 595 + x) * 4;
            assert_eq!(
                &after.rgba[pixel..pixel + 4],
                &before.rgba[pixel..pixel + 4],
                "preserved blue artwork{x},{y}"
            );
        }
    }
}

fn reference(name: &str, args: &[&str]) -> std::process::Output {
    let binary = if name == "qpdf" {
        std::path::PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap())
    } else {
        std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap()).join(name)
    };
    let output = std::process::Command::new(binary)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn reference_pixels(path: &std::path::Path) -> Vec<u8> {
    let output = reference(
        "pdftoppm",
        &["-f", "1", "-singlefile", "-r", "72", path.to_str().unwrap()],
    );
    output
        .stdout
        .strip_prefix(b"P6\n595 842\n255\n")
        .unwrap()
        .to_vec()
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn dropdown_list_export_indices_unicode_and_real_pixels_pass_independent_readers() {
    assert!(
        String::from_utf8_lossy(&reference("qpdf", &["--version"]).stdout)
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    assert!(
        String::from_utf8_lossy(&reference("pdftoppm", &["-v"]).stderr)
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    let directory = tempfile::tempdir().unwrap();
    let mut paired = Document::load_mem(SOURCE).unwrap();
    paired.get_dictionary_mut((14, 0)).unwrap().set(
        "Opt",
        vec![
            Object::Array(vec![string("same"), string("Première")]),
            Object::Array(vec![string("same"), string("Deuxième")]),
        ],
    );
    paired
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("V", string("same"));
    paired
        .get_dictionary_mut((14, 0))
        .unwrap()
        .set("I", vec![Object::Integer(0)]);
    let mut unicode = Document::load_mem(&list_source(true)).unwrap();
    unicode.get_dictionary_mut((14, 0)).unwrap().set(
        "Opt",
        vec![
            Object::Array(vec![string("a"), string("e\u{301}cole")]),
            Object::Array(vec![string("b"), string("Déjà")]),
            Object::Array(vec![string("c"), string("Ωmega")]),
        ],
    );
    for (index, (source, selection, replacement, expected_v, expected_i, top, mask)) in [
        (
            SOURCE.to_vec(),
            change(&[2], None),
            "High",
            serde_json::json!("u:High"),
            serde_json::json!([2]),
            None,
            [72, 190, 220, 220],
        ),
        (
            bytes(&mut paired),
            change(&[1], None),
            "Deuxième",
            serde_json::json!("u:same"),
            serde_json::json!([1]),
            None,
            [72, 190, 220, 220],
        ),
        (
            list_source(true),
            change(&[1, 2], Some(1)),
            "Beta\nGamma",
            serde_json::json!(["u:b", "u:c"]),
            serde_json::json!([1, 2]),
            Some(1),
            [72, 192, 220, 292],
        ),
        (
            bytes(&mut unicode),
            change(&[0, 1], Some(0)),
            "e\u{301}cole\nDéjà\nΩmega",
            serde_json::json!(["u:a", "u:b"]),
            serde_json::json!([0, 1]),
            Some(0),
            [72, 192, 220, 292],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let input = directory.path().join(format!("source-{index}.pdf"));
        let output = directory.path().join(format!("filled-{index}.pdf"));
        std::fs::write(&input, &source).unwrap();
        let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
        pdf.fill_choice(&[selection], FONT).unwrap();
        if index == 3 {
            assert!(
                pdf.extract_text(&[1])
                    .unwrap()
                    .contains("e\u{301}cole\nDéjà\nΩmega")
            );
        }
        pdf.save_new(&output).unwrap();
        reference("qpdf", &["--check", output.to_str().unwrap()]);
        let model: serde_json::Value = serde_json::from_slice(
            &reference("qpdf", &["--json", output.to_str().unwrap()]).stdout,
        )
        .unwrap();
        let field = &model["qpdf"][1]["obj:14 0 R"]["value"];
        assert_eq!(field["/V"], expected_v);
        assert_eq!(field["/I"], expected_i);
        if let Some(top) = top {
            assert_eq!(field["/TI"], top);
        } else {
            assert!(field.get("/TI").is_none());
        }
        let before_text = String::from_utf8(
            reference("pdftotext", &["-raw", input.to_str().unwrap(), "-"]).stdout,
        )
        .unwrap();
        let after_text = String::from_utf8(
            reference("pdftotext", &["-raw", output.to_str().unwrap(), "-"]).stdout,
        )
        .unwrap();
        assert_eq!(before_text.matches("Normal").count(), 1);
        assert_eq!(
            after_text,
            before_text.replacen("Normal", replacement, 1),
            "exact independent case{index}"
        );
        let before = reference_pixels(&input);
        let after = reference_pixels(&output);
        let mut changed = 0;
        for (pixel, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                let (x, y) = (pixel % 595, pixel / 595);
                assert!(
                    (mask[0]..mask[2]).contains(&x) && (mask[1]..mask[3]).contains(&y),
                    "case{index} outside widget{x},{y}"
                );
                changed += 1;
            }
        }
        assert!(changed > 0);
        if index == 2 {
            assert!(
                after
                    .chunks_exact(3)
                    .filter(|pixel| pixel[2] > 160 && pixel[0] < 80 && pixel[1] < 130)
                    .count()
                    > 100,
                "independent selected blue highlight"
            );
        }
        assert_eq!(std::fs::read(input).unwrap(), source);
        assert_eq!(
            pdf.info().unwrap().pages,
            PdfDocument::from_bytes(&source, None)
                .unwrap()
                .info()
                .unwrap()
                .pages
        );
    }
}
