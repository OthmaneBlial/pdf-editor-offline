#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, content::Content};
use pdf_core::{FormValue, PdfDocument, TextChange};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const PLEX: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const AMIRI: &[u8] = include_bytes!("fixtures/fonts/amiri/Amiri-Regular.ttf");
const CJK: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf");
fn bytes(doc: &mut Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}
fn fixture(width: f32, height: f32, size: f32, q: i64) -> Vec<u8> {
    let mut doc = Document::load_mem(SOURCE).unwrap();
    let field = doc.get_dictionary_mut((8, 0)).unwrap();
    field.set("Ff", 1 << 12);
    field.set(
        "Rect",
        vec![
            72.into(),
            (702.0 - height).into(),
            (72.0 + width).into(),
            702.into(),
        ],
    );
    field.set("DA", Object::string_literal(format!("/Helv {size} Tf 0 g")));
    field.set("Q", q);
    let old = doc.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    dict.set(
        "BBox",
        vec![0.into(), 0.into(), width.into(), height.into()],
    );
    let text = String::from_utf8(old.get_plain_content().unwrap())
        .unwrap()
        .replace("0 0 228 30 re", &format!("0 0 {width} {height} re"));
    doc.objects
        .insert((9, 0), Stream::new(dict, text.into_bytes()).into());
    bytes(&mut doc)
}
fn fill(pdf: &mut PdfDocument, value: &str, font: &[u8]) -> pdf_core::Result<()> {
    pdf.fill_text(
        &[TextChange {
            name: "full_name".into(),
            value: value.into(),
        }],
        font,
    )
}
fn ap(doc: &Document) -> &Stream {
    doc.get_dictionary((8, 0))
        .unwrap()
        .get_deref(b"AP", doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"N", doc)
        .unwrap()
        .as_stream()
        .unwrap()
}
fn assert_value(pdf: &PdfDocument, value: &str) {
    let field = pdf
        .list_form_fields()
        .unwrap()
        .into_iter()
        .find(|f| f.name == "full_name")
        .unwrap();
    assert!(field.writable_text, "{:?}", field.text_refusal_reason);
    assert_eq!(field.value, FormValue::Text(value.into()));
    let extracted = pdf.extract_text(&[1]).unwrap();
    if value.is_empty() {
        assert!(!extracted.contains("Synthetic value"));
    } else {
        assert_eq!(extracted.matches(value).count(), 1);
    }
}
fn lines(doc: &Document) -> BTreeSet<u32> {
    Content::decode(&ap(doc).get_plain_content().unwrap())
        .unwrap()
        .operations
        .iter()
        .filter(|op| op.operator == "Tm")
        .map(|op| op.operands[5].as_float().unwrap().to_bits())
        .collect()
}
#[test]
fn exact_explicit_breaks_blank_lines_unicode_and_repeated_fills_have_one_logical_span() {
    let input = fixture(228.0, 180.0, 12.0, 0);
    let source = Document::load_mem(&input).unwrap();
    let value = "Résumé e\u{301}\r\n\r\nΩЖ\nFinal\u{2028}End\u{2029}";
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    fill(&mut pdf, value, PLEX).unwrap();
    assert_value(&pdf, value);
    let saved = pdf.verified_bytes().unwrap();
    let changed = Document::load_mem(&saved).unwrap();
    let content = Content::decode(&ap(&changed).get_plain_content().unwrap()).unwrap();
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "BT")
            .count(),
        1
    );
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "BDC")
            .count(),
        1
    );
    assert!(lines(&changed).len() >= 4);
    for (id, obj) in &source.objects {
        if !matches!(*id, (1, 0) | (8, 0)) {
            assert_eq!(changed.objects.get(id), Some(obj), "{id:?}");
        }
    }
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    fill(&mut reopened, "First\nSecond", PLEX).unwrap();
    assert_value(&reopened, "First\nSecond");
    fill(&mut reopened, "", PLEX).unwrap();
    assert_value(&reopened, "");
    fill(&mut reopened, "Refilled\nAgain", PLEX).unwrap();
    assert_value(&reopened, "Refilled\nAgain");
}
#[test]
fn uax_wrapping_keeps_combining_graphemes_cjk_and_nonbreaking_spans() {
    for (value, font) in [
        ("Alpha Beta Gamma", PLEX),
        (
            "e\u{301} e\u{301} e\u{301} e\u{301} e\u{301} e\u{301} e\u{301} e\u{301}",
            PLEX,
        ),
        ("中文测试段落", CJK),
    ] {
        let input = fixture(55.0, 180.0, 12.0, 0);
        let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
        fill(&mut pdf, value, font).unwrap();
        assert_value(&pdf, value);
        let doc = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
        assert!(lines(&doc).len() > 1, "{value}");
    }
    for value in [
        "Alpha\u{a0}Beta",
        "Alpha\u{2060}Beta",
        "Alpha\u{ad}Beta",
        "Supercalifragilisticexpialidocious",
    ] {
        let mut pdf = PdfDocument::from_bytes(&fixture(45.0, 180.0, 12.0, 0), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(fill(&mut pdf, value, PLEX).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
}
#[test]
fn mixed_rtl_lines_preserve_paragraph_order_and_alignment_is_real() {
    let value = "ABC e\u{301} مرحبا بالعالم 123 \u{202b}مرحبا 456\u{2028}ثاني 789\u{202c}\nEnd";
    for q in [0, 1, 2] {
        let mut pdf = PdfDocument::from_bytes(&fixture(180.0, 240.0, 12.0, q), None).unwrap();
        fill(&mut pdf, value, AMIRI).unwrap();
        assert_value(&pdf, value);
        let saved = pdf.verified_bytes().unwrap();
        let doc = Document::load_mem(&saved).unwrap();
        assert!(lines(&doc).len() >= 2);
        let raster = pdf_render::render_page(&saved, 0, 1.0).unwrap();
        assert!(
            raster
                .rgba
                .chunks_exact(4)
                .any(|p| p[0] < 100 && p[1] < 100 && p[2] < 100)
        );
    }
}
#[test]
fn inherited_multiline_appearance_alignment_and_character_limit_remain_authoritative() {
    let mut source = Document::load_mem(&fixture(100.0, 100.0, 12.0, 2)).unwrap();
    let old = source.get_dictionary((8, 0)).unwrap().clone();
    let mut parent = lopdf::Dictionary::new();
    parent.set("T", Object::string_literal("Group"));
    parent.set("Kids", vec![Object::Reference((8, 0))]);
    for key in [b"FT".as_slice(), b"Ff", b"DA", b"Q"] {
        parent.set(key, old.get(key).unwrap().clone());
    }
    parent.set("MaxLen", 6);
    let id = source.add_object(parent);
    let field = source.get_dictionary_mut((8, 0)).unwrap();
    field.set("Parent", id);
    for key in [b"FT".as_slice(), b"Ff", b"DA", b"Q"] {
        field.remove(key);
    }
    let root = source.get_dictionary_mut((1, 0)).unwrap();
    let fields = root
        .get_mut(b"AcroForm")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Fields")
        .unwrap()
        .as_array_mut()
        .unwrap();
    for field in fields {
        if *field == Object::Reference((8, 0)) {
            *field = Object::Reference(id);
        }
    }
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let change = |value: &str| TextChange {
        name: "Group.full_name".into(),
        value: value.into(),
    };
    pdf.fill_text(&[change("A\nAAAA")], PLEX).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("A\nAAAA"));
    let saved = pdf.verified_bytes().unwrap();
    let changed = Document::load_mem(&saved).unwrap();
    assert_eq!(
        changed.get_dictionary(id).unwrap(),
        source.get_dictionary(id).unwrap()
    );
    let content = Content::decode(&ap(&changed).get_plain_content().unwrap()).unwrap();
    let mut first_by_line = std::collections::BTreeMap::new();
    for op in &content.operations {
        if op.operator == "Tm" {
            first_by_line
                .entry(op.operands[5].as_float().unwrap().to_bits())
                .or_insert(op.operands[4].as_float().unwrap());
        }
    }
    let xs: Vec<_> = first_by_line.values().copied().collect();
    assert_eq!(xs.len(), 2);
    assert!(
        (xs[0] - xs[1]).abs() > 10.0,
        "inherited right alignment must position unequal lines differently"
    );
    let before = pdf.to_bytes().unwrap();
    assert!(pdf.fill_text(&[change("AA\nAAAA")], PLEX).is_err());
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn generated_operator_budget_refuses_before_making_the_field_uneditable() {
    let mut pdf = PdfDocument::from_bytes(&fixture(100000.0, 100.0, 1.0, 0), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let error = fill(&mut pdf, &"A".repeat(33333), PLEX).unwrap_err();
    assert!(matches!(error, pdf_core::Error::Limit(_)), "{error}");
    assert!(error.to_string().contains("100000 operators"), "{error}");
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn autosize_limits_overflow_maxlen_and_late_failure_are_atomic() {
    let mut pdf = PdfDocument::from_bytes(&fixture(100.0, 100.0, 0.0, 0), None).unwrap();
    let value = "Alpha Beta Gamma Delta Epsilon\nRésumé e\u{301}";
    fill(&mut pdf, value, PLEX).unwrap();
    assert_value(&pdf, value);
    let mut source = Document::load_mem(&fixture(100.0, 20.0, 12.0, 0)).unwrap();
    source.get_dictionary_mut((8, 0)).unwrap().set("MaxLen", 10);
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    for value in [
        "A\nB\nC",
        "Too many characters",
        "\n".repeat(1024).as_str(),
        "a\tbad",
        "中文",
    ] {
        assert!(fill(&mut pdf, value, PLEX).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    assert!(
        pdf.fill_text(
            &[
                TextChange {
                    name: "full_name".into(),
                    value: "A".into()
                },
                TextChange {
                    name: "missing".into(),
                    value: "B".into()
                }
            ],
            PLEX
        )
        .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

fn reference(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap()).join(name)
}
fn pixels(path: &Path) -> Vec<u8> {
    let output = Command::new(reference("pdftoppm"))
        .args(["-f", "1", "-singlefile", "-r", "72"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
        .stdout
        .strip_prefix(b"P6\n595 842\n255\n")
        .unwrap()
        .to_vec()
}
#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn independent_multiline_values_text_pixels_and_original_objects() {
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let version = Command::new(&qpdf).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let version = Command::new(reference("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
    let folder = tempfile::tempdir().unwrap();
    for (case, value, font) in [
        (0, "Résumé e\u{301}\nΩЖ\nThird", PLEX),
        (1, "中文测试\n日本語\n한국어", CJK),
        (2, "ABC مرحبا بالعالم 123 مرحبا 456\nثاني 789", AMIRI),
        (
            3,
            "Alpha Beta Gamma Delta Epsilon Zeta Eta Theta Iota Kappa Lambda Mu Nu Xi Omicron",
            PLEX,
        ),
    ] {
        let width = if case == 2 { 120.0 } else { 228.0 };
        let input = fixture(width, 180.0, 12.0, 0);
        let source = folder.path().join(format!("source{case}.pdf"));
        let output = folder.path().join(format!("filled{case}.pdf"));
        std::fs::write(&source, &input).unwrap();
        let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
        fill(&mut pdf, value, font).unwrap();
        pdf.save_new(&output).unwrap();
        if case >= 2 {
            let saved = Document::load_mem(&std::fs::read(&output).unwrap()).unwrap();
            assert!(lines(&saved).len() >= if case == 2 { 3 } else { 2 });
        }
        let check = Command::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        let json = |path: &Path| {
            let result = Command::new(&qpdf)
                .arg("--json")
                .arg(path)
                .output()
                .unwrap();
            assert!(result.status.success());
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()
        };
        let original = json(&source);
        let changed = json(&output);
        assert_eq!(
            changed["qpdf"][1]["obj:8 0 R"]["value"]["/V"],
            format!("u:{value}")
        );
        for (id, obj) in original["qpdf"][1].as_object().unwrap() {
            if id.starts_with("obj:")
                && id != "obj:1 0 R"
                && id != "obj:8 0 R"
                && obj["stream"]["dict"]["/Type"] != "/XRef"
            {
                assert_eq!(&changed["qpdf"][1][id], obj, "independent preserved{id}");
            }
        }
        let stream = changed["qpdf"][1]["obj:8 0 R"]["value"]["/AP"]["/N"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        let actual = Command::new(&qpdf)
            .arg(format!("--show-object={stream}"))
            .arg("--filtered-stream-data")
            .arg(&output)
            .output()
            .unwrap();
        assert!(actual.status.success());
        let hex = format!(
            "FEFF{}",
            value
                .encode_utf16()
                .map(|u| format!("{u:04X}"))
                .collect::<String>()
        );
        assert!(
            String::from_utf8(actual.stdout)
                .unwrap()
                .to_uppercase()
                .contains(&hex)
        );
        let mut modes = Vec::new();
        for mode in [None, Some("-layout"), Some("-raw")] {
            let mut command = Command::new(reference("pdftotext"));
            if let Some(mode) = mode {
                command.arg(mode);
            }
            let text = command.arg(&output).arg("-").output().unwrap();
            assert!(text.status.success());
            let text = String::from_utf8(text.stdout).unwrap();
            if case != 2 {
                assert!(
                    text.contains(value),
                    "independent case{case} {mode:?}: {text:?}"
                );
            }
            modes.push(text);
        }
        if case == 2 {
            // Poppler 26.10 emits visual Arabic order and directional controls,
            // and removes spaces before European digits. Keep its exact mode
            // behavior visible; logical V/ActualText are independently exact.
            assert_eq!(
                modes,
                vec![
                    "Synthetic form controls\n\nABC \u{202b} ملاعلاب ابحرم\u{202c}123 \u{202b} ابحرم\u{202c}456\n\u{202b} يناث\u{202c}789\n✓\n\nNormal\n2026-08-24\n\n\u{c}",
                    "Synthetic form controls\n\n\n\n\nABC \u{202b} ملاعلاب ابحرم\u{202c}123 \u{202b} ابحرم\u{202c}456\n\u{202b} يناث\u{202c}789\n✓\n\nNormal\n2026-08-24\n\u{c}",
                    "Synthetic form controls\nABC \u{202b} ملاعلاب ابحرم\u{202c}123 \u{202b} ابحرم\u{202c}456\n\u{202b} يناث\u{202c}789 ✓\nNormal\n2026-08-24\u{c}"
                ],
                "RTL mode goldens must be pinned exactly"
            );
        }
        let before = pixels(&source);
        let after = pixels(&output);
        let top_ink = |pixels: &[u8]| {
            (146..160)
                .flat_map(|y| (80..64 + width as usize).map(move |x| (y * 595 + x) * 3))
                .filter(|index| {
                    pixels[*index..*index + 3]
                        .iter()
                        .all(|channel| *channel < 128)
                })
                .count()
        };
        assert!(
            top_ink(&after) > top_ink(&before),
            "independent newly painted first row case{case}"
        );
        let mut changed_pixels = 0;
        for (index, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                let x = index % 595;
                let y = index / 595;
                assert!(
                    (72..72 + width as usize).contains(&x) && (140..320).contains(&y),
                    "independent outside widget{x},{y}"
                );
                changed_pixels += 1;
            }
        }
        assert!(changed_pixels > 20);
        assert_eq!(std::fs::read(&source).unwrap(), input);
    }
}
