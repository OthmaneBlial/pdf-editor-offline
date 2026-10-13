#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, content::Content};
use pdf_core::{PdfDocument, TextChange};
use skrifa::{FontRef, raw::TableProvider};
use std::{collections::HashSet, path::Path, process::Command};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf");
const VALUE: &str = "ABC é 中国 日本語 한글";
fn fill(pdf: &mut PdfDocument, value: &str) {
    pdf.fill_text(
        &[TextChange {
            name: "full_name".into(),
            value: value.into(),
        }],
        FONT,
    )
    .unwrap();
}
fn normal(document: &Document) -> &Stream {
    document
        .get_dictionary((8, 0))
        .unwrap()
        .get_deref(b"AP", document)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"N", document)
        .unwrap()
        .as_stream()
        .unwrap()
}
fn fonts(document: &Document) -> Vec<&lopdf::Dictionary> {
    normal(document)
        .dict
        .get_deref(b"Resources", document)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"Font", document)
        .unwrap()
        .as_dict()
        .unwrap()
        .iter()
        .filter_map(|(_, value)| document.dereference(value).ok()?.1.as_dict().ok())
        .collect()
}
fn cff_font(document: &Document) -> &lopdf::Dictionary {
    fonts(document)
        .into_iter()
        .find(|font| {
            font.get(b"BaseFont").and_then(Object::as_name).ok() == Some(b"NotoSansCJKsc-Regular")
        })
        .unwrap()
}
fn program_ids(document: &Document) -> HashSet<lopdf::ObjectId> {
    document
        .objects
        .values()
        .filter_map(|value| {
            value
                .as_dict()
                .ok()?
                .get(b"FontFile3")
                .ok()?
                .as_reference()
                .ok()
        })
        .collect()
}
#[test]
fn mixed_cjk_latin_fields_embed_cff_cids_preserve_source_and_reuse_whole_program() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, VALUE);
    assert_eq!(pdf.extract_text(&[1]).unwrap().matches(VALUE).count(), 1);
    let saved = pdf.verified_bytes().unwrap();
    let doc = Document::load_mem(&saved).unwrap();
    let original = Document::load_mem(SOURCE).unwrap();
    let catalog = original
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    for (id, value) in &original.objects {
        if *id != (8, 0) && *id != catalog {
            assert_eq!(
                doc.objects.get(id),
                Some(value),
                "changed unrelated source object {id:?}"
            );
        }
    }
    let mut old_widget = original.get_dictionary((8, 0)).unwrap().clone();
    let mut new_widget = doc.get_dictionary((8, 0)).unwrap().clone();
    for key in [b"V".as_slice(), b"DA", b"AP"] {
        old_widget.remove(key);
        new_widget.remove(key);
    }
    assert_eq!(old_widget, new_widget);
    let font = cff_font(&doc);
    let encoding = font
        .get_deref(b"Encoding", &doc)
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(encoding.dict.get(b"WMode").unwrap().as_i64().unwrap(), 0);
    assert!(
        std::str::from_utf8(&encoding.content)
            .unwrap()
            .contains("begincidchar")
    );
    let cid = doc
        .get_dictionary(
            font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]
                .as_reference()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        cid.get(b"Subtype").unwrap().as_name().unwrap(),
        b"CIDFontType0"
    );
    assert!(!cid.has(b"CIDToGIDMap"));
    let descriptor = cid
        .get_deref(b"FontDescriptor", &doc)
        .unwrap()
        .as_dict()
        .unwrap();
    assert!(!descriptor.has(b"FontFile2"));
    let program = descriptor
        .get_deref(b"FontFile3", &doc)
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(
        program.dict.get(b"Subtype").unwrap().as_name().unwrap(),
        b"CIDFontType0C"
    );
    assert_eq!(
        program.get_plain_content().unwrap(),
        FontRef::new(FONT)
            .unwrap()
            .cff()
            .unwrap()
            .offset_data()
            .as_bytes()
    );
    let before_ids = program_ids(&doc);
    assert_eq!(before_ids.len(), 1);
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    fill(&mut reopened, "Second 中国 한글");
    assert_eq!(
        reopened
            .extract_text(&[1])
            .unwrap()
            .matches("Second 中国 한글")
            .count(),
        1
    );
    let after = Document::load_mem(&reopened.verified_bytes().unwrap()).unwrap();
    assert_eq!(program_ids(&after), before_ids);
    let old = pdf_render::render_page(SOURCE, 0, 1.0).unwrap();
    let new = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    assert_eq!((old.width, old.height), (new.width, new.height));
    let rectangle = pdf
        .list_form_fields()
        .unwrap()
        .iter()
        .find(|field| field.name == "full_name")
        .unwrap()
        .widgets[0]
        .rectangle;
    let mut changed = 0;
    for (index, (a, b)) in old
        .rgba
        .chunks_exact(4)
        .zip(new.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            changed += 1;
            let x = (index % old.width as usize) as f32;
            let y = old.height as f32 - (index / old.width as usize) as f32;
            assert!(
                x >= rectangle[0] - 1.0
                    && x <= rectangle[2] + 1.0
                    && y >= rectangle[1] - 1.0
                    && y <= rectangle[3] + 1.0,
                "changed pixel outside field {x},{y}"
            );
        }
    }
    assert!(changed > 40, "CJK appearance was not rendered");
}
#[test]
fn cff_missing_glyph_and_malformed_charset_fail_transactionally() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    assert!(
        pdf.fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "unsupported 😀".into()
            }],
            FONT
        )
        .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    let face = FontRef::new(FONT).unwrap();
    let raw = face.cff().unwrap().offset_data().as_bytes();
    let offset = raw.as_ptr() as usize - FONT.as_ptr() as usize;
    let mut malformed = FONT.to_vec();
    malformed[offset + 13865] = 255;
    assert!(
        pdf.fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "中国".into()
            }],
            &malformed
        )
        .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), before);
}
#[test]
fn cff_tounicode_remains_exact_without_actualtext_replacement() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, "ABC 中国 한글");
    let mut doc = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let ap_id = doc
        .get_dictionary((8, 0))
        .unwrap()
        .get_deref(b"AP", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let stream = doc.get_object_mut(ap_id).unwrap().as_stream_mut().unwrap();
    let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
    for op in &mut content.operations {
        if op.operator == "BDC"
            && let Some(Object::Dictionary(dict)) = op.operands.get_mut(1)
        {
            dict.remove(b"ActualText");
        }
    }
    stream.set_plain_content(content.encode().unwrap());
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert_eq!(
        extracted.matches("ABC 中国 한글").count(),
        1,
        "{extracted:?}"
    );
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10 tools"]
fn independent_qpdf_poppler_confirm_cff_values_text_and_visible_glyphs() {
    let qpdf = std::env::var_os("PDF_FORM_QPDF").unwrap();
    let bin = std::env::var_os("PDF_FORM_POPPLER_BIN").unwrap();
    assert!(
        String::from_utf8(
            Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .unwrap()
        .contains("qpdf version 12.4.2")
    );
    let version = Command::new(Path::new(&bin).join("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(
        String::from_utf8(version.stderr)
            .unwrap()
            .contains("26.10.0")
    );
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, VALUE);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("filled.pdf");
    let original = temp.path().join("original.pdf");
    std::fs::write(&original, SOURCE).unwrap();
    pdf.save_new(&output).unwrap();
    let checked = Command::new(&qpdf)
        .arg("--check")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let json = Command::new(&qpdf)
        .args(["--json=2", "--json-stream-data=none"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(json.status.success());
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert!(json.to_string().contains("NotoSansCJKsc-Regular"));
    let source_json = Command::new(&qpdf)
        .args(["--json=2", "--json-stream-data=none"])
        .arg(&original)
        .output()
        .unwrap();
    assert!(source_json.status.success());
    let source_json: serde_json::Value = serde_json::from_slice(&source_json.stdout).unwrap();
    let catalog = Document::load_mem(SOURCE)
        .unwrap()
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    for (key, before) in source_json["qpdf"][1].as_object().unwrap() {
        if !key.starts_with("obj:") || before["stream"]["dict"]["/Type"] == "/XRef" {
            continue;
        }
        let mut before = before.clone();
        let mut after = json["qpdf"][1][key].clone();
        if key == "obj:8 0 R" {
            for field in ["/V", "/DA", "/AP"] {
                before["value"].as_object_mut().unwrap().remove(field);
                after["value"].as_object_mut().unwrap().remove(field);
            }
        } else if key == &format!("obj:{} {} R", catalog.0, catalog.1) {
            before["value"].as_object_mut().unwrap().remove("/AcroForm");
            after["value"].as_object_mut().unwrap().remove("/AcroForm");
        }
        assert_eq!(
            before, after,
            "qpdf detected unrelated logical object changes in {key}"
        );
    }
    let extracted = Command::new(Path::new(&bin).join("pdftotext"))
        .arg("-enc")
        .arg("UTF-8")
        .arg(&output)
        .arg("-")
        .output()
        .unwrap();
    assert!(extracted.status.success());
    assert!(String::from_utf8(extracted.stdout).unwrap().contains(VALUE));
    let mut images = Vec::new();
    for (index, input) in [&original, &output].into_iter().enumerate() {
        let prefix = temp.path().join(format!("render-{index}"));
        let rendered = Command::new(Path::new(&bin).join("pdftoppm"))
            .args(["-f", "1", "-singlefile", "-r", "72"])
            .arg(input)
            .arg(&prefix)
            .output()
            .unwrap();
        assert!(
            rendered.status.success(),
            "{}",
            String::from_utf8_lossy(&rendered.stderr)
        );
        images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
    }
    fn pixels(bytes: &[u8]) -> (usize, usize, &[u8]) {
        let mut header = bytes.splitn(4, |byte| *byte == b'\n');
        assert_eq!(header.next().unwrap(), b"P6");
        let size = std::str::from_utf8(header.next().unwrap())
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(header.next().unwrap(), b"255");
        let pixels = header.next().unwrap();
        assert_eq!(pixels.len(), size[0] * size[1] * 3);
        (size[0], size[1], pixels)
    }
    let (width, height, before) = pixels(&images[0]);
    let (new_width, new_height, after) = pixels(&images[1]);
    assert_eq!((width, height), (new_width, new_height));
    let mut changed = 0;
    let rectangle = pdf
        .list_form_fields()
        .unwrap()
        .iter()
        .find(|field| field.name == "full_name")
        .unwrap()
        .widgets[0]
        .rectangle;
    for (index, (a, b)) in before
        .chunks_exact(3)
        .zip(after.chunks_exact(3))
        .enumerate()
    {
        if a != b {
            changed += 1;
            let x = (index % width) as f32;
            let y = height as f32 - (index / width) as f32;
            assert!(
                x >= rectangle[0] - 1.0
                    && x <= rectangle[2] + 1.0
                    && y >= rectangle[1] - 1.0
                    && y <= rectangle[3] + 1.0,
                "independent changed pixel outside field {x},{y}"
            );
        }
    }
    assert!(changed > 40, "Poppler did not render changed CJK glyphs");
    let shifted = shifted_cids_font();
    let mut nonidentity = PdfDocument::from_bytes(SOURCE, None).unwrap();
    nonidentity
        .fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: VALUE.into(),
            }],
            &shifted,
        )
        .unwrap();
    let nonidentity_path = temp.path().join("nonidentity.pdf");
    nonidentity.save_new(&nonidentity_path).unwrap();
    let checked = Command::new(&qpdf)
        .arg("--check")
        .arg(&nonidentity_path)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let extracted = Command::new(Path::new(&bin).join("pdftotext"))
        .args(["-enc", "UTF-8"])
        .arg(&nonidentity_path)
        .arg("-")
        .output()
        .unwrap();
    assert!(extracted.status.success());
    assert!(String::from_utf8(extracted.stdout).unwrap().contains(VALUE));
    let prefix = temp.path().join("nonidentity");
    let rendered = Command::new(Path::new(&bin).join("pdftoppm"))
        .args(["-f", "1", "-singlefile", "-r", "72"])
        .arg(&nonidentity_path)
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(rendered.status.success());
    assert_eq!(
        std::fs::read(prefix.with_extension("ppm")).unwrap(),
        images[1],
        "Poppler selected wrong glyphs for nonidentity CIDs"
    );
    assert_eq!(std::fs::read(&original).unwrap(), SOURCE);
}

#[path = "fixtures/fonts/cff_fixture.rs"]
mod cff_fixture;
fn shifted_cids_font() -> Vec<u8> {
    cff_fixture::shifted_cids_font(FONT)
}
#[test]
fn actual_nonidentity_cff_charset_preserves_shaped_glyphs_and_native_pixels() {
    let shifted = shifted_cids_font();
    let mut original = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let mut changed = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut original, VALUE);
    changed
        .fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: VALUE.into(),
            }],
            &shifted,
        )
        .unwrap();
    let left = original.verified_bytes().unwrap();
    let right = changed.verified_bytes().unwrap();
    assert_eq!(
        original.extract_text(&[1]).unwrap(),
        changed.extract_text(&[1]).unwrap()
    );
    assert_eq!(
        pdf_render::render_page(&left, 0, 1.0).unwrap(),
        pdf_render::render_page(&right, 0, 1.0).unwrap(),
        "CID shift selected different glyph outlines"
    );
    let doc = Document::load_mem(&right).unwrap();
    let encoding = cff_font(&doc)
        .get_deref(b"Encoding", &doc)
        .unwrap()
        .as_stream()
        .unwrap()
        .get_plain_content()
        .unwrap();
    assert!(
        std::str::from_utf8(&encoding).unwrap().contains(" 9545\n"),
        "Chinese glyph mapping ignored actual CFF CID"
    );
}
