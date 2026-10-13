#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{PdfDocument, TextAlignment, TextOverlay};
use std::path::Path;
use std::process::Command;
const PLEX: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const CJK: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf");
const AMIRI: &[u8] = include_bytes!("fixtures/fonts/amiri/Amiri-Regular.ttf");
fn source(content: &[u8]) -> Vec<u8> {
    let mut doc = Document::with_version("1.4");
    let root = doc.new_object_id();
    let pages = doc.new_object_id();
    let page = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let stream = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    doc.objects.insert(
        page,
        dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>stream}.into(),
    );
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"Old"=>font}}}.into());
    doc.objects
        .insert(root, dictionary! {"Type"=>"Catalog","Pages"=>pages}.into());
    doc.trailer.set("Root", root);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
const ART: &[u8]=b"0.2 0.4 0.6 rg 1 0 0 1 17 9 cm 0 0 100 100 re W n 0 0 90 90 re f BT /Old 12 Tf 20 30 Td (Original) Tj ET\n";
fn overlay(value: &str) -> TextOverlay {
    TextOverlay {
        page: 1,
        rectangle: [120.0, 120.0, 280.0, 250.0],
        value: value.into(),
        size: 12.0,
        color: [0.7, 0.1, 0.05],
        alignment: TextAlignment::Left,
    }
}
#[test]
fn unicode_overlay_preserves_shared_originals_geometry_text_and_isolation() {
    let input = source(ART);
    let old = Document::load_mem(&input).unwrap();
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    let info = pdf.info().unwrap();
    let value = "Résumé e\u{301}\nSecond line ΩЖ";
    pdf.add_text_overlay(&overlay(value), PLEX).unwrap();
    let saved = pdf.verified_bytes().unwrap();
    let out = Document::load_mem(&saved).unwrap();
    for (id, object) in &old.objects {
        if *id != (3, 0)
            && !matches!(object, Object::Stream(stream) if stream.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|name| name == b"XRef"))
        {
            assert_eq!(out.get_object(*id).unwrap(), object, "source object {id:?}");
        }
    }
    assert_eq!(pdf.info().unwrap().pages, info.pages);
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Original"));
    assert_eq!(text.matches(value).count(), 1);
    let before = pdf_render::render_page(&input, 0, 1.0).unwrap();
    let after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    assert_pixels(&before.rgba, &after.rgba, 4);
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    let mut second = overlay("Again");
    second.rectangle = [120.0, 70.0, 280.0, 110.0];
    reopened.add_text_overlay(&second, PLEX).unwrap();
    assert!(reopened.extract_text(&[1]).unwrap().contains(value));
    assert!(reopened.extract_text(&[1]).unwrap().contains("Again"));
    let repeated = pdf_render::render_page(&reopened.verified_bytes().unwrap(), 0, 1.0).unwrap();
    assert_pixels_in(&after.rgba, &repeated.rgba, 4, [120, 190, 280, 230]);
    assert_eq!(
        Document::load_mem(&reopened.verified_bytes().unwrap())
            .unwrap()
            .objects
            .values()
            .filter(|o| matches!(o,Object::Stream(s) if s.content==PLEX))
            .count(),
        1,
        "reuse exact font program"
    );
    assert_eq!(input, source(ART));
}
fn assert_pixels(before: &[u8], after: &[u8], channels: usize) {
    assert_pixels_in(before, after, channels, [120, 50, 280, 180]);
}
fn assert_pixels_in(before: &[u8], after: &[u8], channels: usize, rectangle: [usize; 4]) {
    assert_eq!(before.len(), after.len());
    let mut changed = 0;
    for (index, (a, b)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if a != b {
            let x = index % 300;
            let y = index / 300;
            assert!(
                (rectangle[0]..rectangle[2]).contains(&x)
                    && (rectangle[1]..rectangle[3]).contains(&y),
                "changed outside actual overlay at{x},{y}"
            );
            changed += 1;
        }
    }
    assert!(
        changed > 20,
        "overlay must actually paint after original clip/CTM/color"
    );
}
#[test]
fn font_geometry_state_and_budget_refusals_are_atomic() {
    let input = source(ART);
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    let original = pdf.to_bytes().unwrap();
    let mut bad = overlay("Text");
    for (value, font) in [
        (overlay("中文"), PLEX),
        (overlay("Text"), b"bad font".as_slice()),
        (overlay(&"A\n".repeat(1024)), PLEX),
    ] {
        assert!(pdf.add_text_overlay(&value, font).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), original);
    }
    for rect in [
        [f64::NAN, 0.0, 20.0, 20.0],
        [-1.0, 0.0, 20.0, 20.0],
        [10.0, 20.0, 10.0, 40.0],
        [0.0, 0.0, 301.0, 300.0],
    ] {
        bad.rectangle = rect;
        assert!(pdf.add_text_overlay(&bad, PLEX).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), original);
    }
    for content in [b"Q\n".as_slice(), b"q\n", b"BT\n", b"/Span BMC\n"] {
        let bytes = source(content);
        let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(pdf.add_text_overlay(&overlay("Text"), PLEX).is_err());
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut signed = PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
        None,
    )
    .unwrap();
    let before = signed.to_bytes().err();
    assert!(signed.add_text_overlay(&overlay("Text"), PLEX).is_err());
    assert!(before.is_some());
}
#[test]
fn cjk_and_mixed_rtl_overlay_use_real_unicode_glyphs_and_exact_logical_text() {
    for (value, font) in [
        ("中文 日本語 한글", CJK),
        ("ABC 123 مرحبا\nثاني 789", AMIRI),
    ] {
        let input = source(ART);
        let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
        pdf.add_text_overlay(&overlay(value), font).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().matches(value).count(), 1);
        let before = pdf_render::render_page(&input, 0, 1.0).unwrap();
        let after = pdf_render::render_page(&pdf.verified_bytes().unwrap(), 0, 1.0).unwrap();
        assert_pixels(&before.rgba, &after.rgba, 4);
    }
}
#[test]
fn overlay_dto_rejects_unknown_paths_and_invalid_enum() {
    let mut value = serde_json::to_value(overlay("Text")).unwrap();
    value["path"] = "secret".into();
    assert!(serde_json::from_value::<TextOverlay>(value).is_err());
    let mut value = serde_json::to_value(overlay("Text")).unwrap();
    value["alignment"] = "justify".into();
    assert!(serde_json::from_value::<TextOverlay>(value).is_err());
}
fn semantic_text(bytes: &[u8]) -> String {
    let doc = Document::load_mem(bytes).unwrap();
    let page = doc.get_dictionary((3, 0)).unwrap();
    let form = page
        .get_deref(b"Resources", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"XObject", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"NativeTextOverlay", &doc)
        .unwrap()
        .as_stream()
        .unwrap();
    lopdf::content::Content::decode(&form.get_plain_content().unwrap())
        .unwrap()
        .operations
        .iter()
        .filter_map(|op| {
            if op.operator != "BDC" {
                return None;
            }
            let bytes = op.operands[1]
                .as_dict()
                .unwrap()
                .get(b"ActualText")
                .unwrap()
                .as_str()
                .unwrap();
            assert_eq!(&bytes[..2], b"\xfe\xff");
            let units: Vec<_> = bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
            Some(String::from_utf16(&units).unwrap())
        })
        .collect()
}
#[test]
fn logical_separators_blank_rows_soft_wrap_and_version_floor_are_exact() {
    for value in [
        "First\r\n\r\nRésumé e\u{301}\u{2028}Last\n",
        "Alpha Beta Gamma Delta Epsilon Zeta Eta Theta Iota Kappa Lambda",
        "ABC 123 مرحبا\nثاني 789",
    ] {
        let mut original = Document::load_mem(&source(ART)).unwrap();
        original.version = "1.3".into();
        original
            .get_dictionary_mut((1, 0))
            .unwrap()
            .set("Version", "1.7");
        let mut input = Vec::new();
        original.save_to(&mut input).unwrap();
        let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
        let mut request = overlay(value);
        request.size = 10.0;
        assert!(pdf.add_text_overlay(&request, b"bad font").is_err());
        assert_eq!(pdf.info().unwrap().version, "1.3");
        pdf.add_text_overlay(&request, AMIRI).unwrap();
        assert_eq!(pdf.info().unwrap().version, "1.4");
        let saved = pdf.verified_bytes().unwrap();
        assert_eq!(semantic_text(&saved), value);
        assert_eq!(pdf.extract_text(&[1]).unwrap().matches(value).count(), 1);
        assert_eq!(
            Document::load_mem(&saved)
                .unwrap()
                .get_dictionary((1, 0))
                .unwrap()
                .get(b"Version")
                .unwrap()
                .as_name()
                .unwrap(),
            b"1.7"
        );
    }
}
#[test]
fn complete_operator_budget_retains_atomicity_and_normal_repeat_eligibility() {
    let input = source("q Q\n".repeat(49_990).as_bytes());
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    assert!(matches!(
        pdf.add_text_overlay(&overlay("Text"), PLEX),
        Err(pdf_core::Error::Limit(_))
    ));
    assert_eq!(pdf.to_bytes().unwrap(), before);
    let mut pdf = PdfDocument::from_bytes(&source(ART), None).unwrap();
    for size in [f64::NAN, f64::INFINITY, 0.0, 1025.0] {
        let mut invalid = overlay("Text");
        invalid.size = size;
        assert!(pdf.add_text_overlay(&invalid, PLEX).is_err());
    }
    let mut invalid = overlay("Text");
    invalid.color = [-0.1, 0.5, 0.5];
    assert!(pdf.add_text_overlay(&invalid, PLEX).is_err());
    pdf.add_text_overlay(&overlay("Valid"), PLEX).unwrap();
    let mut reopened = PdfDocument::from_bytes(&pdf.verified_bytes().unwrap(), None).unwrap();
    reopened
        .add_text_overlay(&overlay("Still valid"), PLEX)
        .unwrap();
    let mut pdf = PdfDocument::from_bytes(
        &source(format!("{}{}", "q\n".repeat(32), "Q\n".repeat(32)).as_bytes()),
        None,
    )
    .unwrap();
    let before = pdf.to_bytes().unwrap();
    assert!(matches!(
        pdf.add_text_overlay(&overlay("Text"), PLEX),
        Err(pdf_core::Error::Limit(_))
    ));
    assert_eq!(pdf.to_bytes().unwrap(), before);
    let mut pdf = PdfDocument::from_bytes(
        &source(format!("{}{}", "q\n".repeat(31), "Q\n".repeat(31)).as_bytes()),
        None,
    )
    .unwrap();
    pdf.add_text_overlay(&overlay("Text"), PLEX).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("Text"));
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_text_overlay_placement_extraction_and_outside_artwork_preservation() {
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        String::from_utf8_lossy(
            &Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .contains("12.4.2")
    );
    assert!(
        String::from_utf8_lossy(
            &Command::new(bin.join("pdftotext"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    std::fs::write(&input, source(ART)).unwrap();
    let pixels = |path: &Path| {
        let output = Command::new(bin.join("pdftoppm"))
            .args(["-singlefile", "-r", "72"])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success());
        output
            .stdout
            .strip_prefix(b"P6\n300 300\n255\n")
            .unwrap()
            .to_vec()
    };
    for (index, value, font, expected) in [
        (
            0,
            "Résumé e\u{301}\nSecond line ΩЖ",
            PLEX,
            "Résumé e\u{301}\nSecond line ΩЖ",
        ),
        (1, "中文 日本語 한글", CJK, "中文 日本語 한글"),
        (
            2,
            "ABC 123 مرحبا\nثاني 789",
            AMIRI,
            "ABC 123 \u{202b}ابحرم\u{202c}\n\u{202b} يناث\u{202c}789",
        ),
    ] {
        let output = temp.path().join(format!("overlay{index}.pdf"));
        let mut pdf = PdfDocument::open(&input, None).unwrap();
        pdf.add_text_overlay(&overlay(value), font).unwrap();
        pdf.save_new(&output).unwrap();
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
            let out = Command::new(&qpdf)
                .arg("--json")
                .arg(path)
                .output()
                .unwrap();
            assert!(out.status.success());
            serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
        };
        let original = json(&input);
        let changed = json(&output);
        for (id, object) in original["qpdf"][1].as_object().unwrap() {
            if id.starts_with("obj:")
                && id != "obj:3 0 R"
                && object["stream"]["dict"]["/Type"] != "/XRef"
            {
                assert_eq!(
                    &changed["qpdf"][1][id], object,
                    "independent unchanged original{id}"
                );
            }
        }
        let text = Command::new(bin.join("pdftotext"))
            .arg("-raw")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let text = String::from_utf8(text.stdout).unwrap();
        assert!(text.contains("Original"));
        assert!(text.contains(expected), "case{index}: {text:?}");
        if index == 2 {
            // Whole-value ActualText previously made this pinned reader reorder
            // Arabic across the explicit break. Row spans retain the intended
            // row order; controls and physical spacing remain reader-specific.
            let modes: Vec<_> = ["", "-layout", "-raw"]
                .into_iter()
                .map(|mode| {
                    let mut cmd = Command::new(bin.join("pdftotext"));
                    if !mode.is_empty() {
                        cmd.arg(mode);
                    }
                    let out = cmd.arg(&output).arg("-").output().unwrap();
                    assert!(out.status.success());
                    String::from_utf8(out.stdout).unwrap()
                })
                .collect();
            assert_eq!(
                modes,
                [
                    "ABC 123 \u{202b}ابحرم\u{202c}\n\u{202b} يناث\u{202c}789\n\nOriginal\n\n\u{c}",
                    "           ABC 123 \u{202b}ابحرم\u{202c}\n\n           \u{202b} يناث\u{202c}789\n\n\n\n\nOriginal\n\u{c}",
                    "Original\nABC 123 \u{202b}ابحرم\u{202c}\n\u{202b} يناث\u{202c}789\u{c}"
                ]
            );
        }
        assert_eq!(semantic_text(&std::fs::read(&output).unwrap()), value);
        assert_pixels(&pixels(&input), &pixels(&output), 3);
    }
    assert_eq!(std::fs::read(input).unwrap(), source(ART));
}
