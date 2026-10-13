#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, content::Content};
use pdf_core::{PdfDocument, TextChange};
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const PLEX: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const SYMBOLS: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/NotoSansSymbols2-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("fixtures/fonts/NotoSansArabic-Regular.ttf");
const AMIRI: &[u8] = include_bytes!("fixtures/fonts/amiri/Amiri-Regular.ttf");
fn fill(pdf: &mut PdfDocument, value: &str, font: &[u8]) {
    pdf.fill_text(
        &[TextChange {
            name: "full_name".into(),
            value: value.into(),
        }],
        font,
    )
    .unwrap();
}
fn bytes(doc: &mut Document) -> Vec<u8> {
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn generated_text_raises_legacy_version_preserves_newer_versions_and_failed_sources() {
    for (version, expected) in [
        ("1.2", "1.4"),
        ("1.3", "1.4"),
        ("1.4", "1.4"),
        ("1.7", "1.7"),
        ("2.0", "2.0"),
    ] {
        let mut document = Document::load_mem(SOURCE).unwrap();
        document.version = version.into();
        let root = document
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let catalog_version = (version == "1.7").then(|| Object::Name(b"1.7".to_vec()));
        document
            .get_dictionary_mut(root)
            .unwrap()
            .remove(b"Version");
        if let Some(value) = &catalog_version {
            document
                .get_dictionary_mut(root)
                .unwrap()
                .set("Version", value.clone());
        }
        let original = bytes(&mut document);
        let mut failed = PdfDocument::from_bytes(&original, None).unwrap();
        let before = failed.to_bytes().unwrap();
        assert!(
            failed
                .fill_text(
                    &[TextChange {
                        name: "full_name".into(),
                        value: "\0".into()
                    }],
                    PLEX
                )
                .is_err()
        );
        assert_eq!(failed.to_bytes().unwrap(), before);
        assert_eq!(Document::load_mem(&before).unwrap().version, version);
        let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
        fill(&mut pdf, "Ada é", PLEX);
        let saved = pdf.to_bytes().unwrap();
        let reopened = Document::load_mem(&saved).unwrap();
        assert_eq!(reopened.version, expected);
        assert_eq!(
            reopened.get_dictionary(root).unwrap().get(b"Version").ok(),
            catalog_version.as_ref()
        );
        assert_value(&PdfDocument::from_bytes(&saved, None).unwrap(), "Ada é");
        assert_eq!(Document::load_mem(&original).unwrap().version, version);
    }
}
fn appearance(doc: &Document) -> &Stream {
    let widget = doc.get_dictionary((8, 0)).unwrap();
    let ap = widget.get_deref(b"AP", doc).unwrap().as_dict().unwrap();
    ap.get_deref(b"N", doc).unwrap().as_stream().unwrap()
}
fn assert_value(pdf: &PdfDocument, value: &str) {
    let field = pdf
        .list_form_fields()
        .unwrap()
        .into_iter()
        .find(|field| field.name == "full_name")
        .unwrap();
    assert_eq!(field.value, pdf_core::FormValue::Text(value.into()));
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert_eq!(extracted.matches(value).count(), 1, "{extracted:?}");
    assert!(!extracted.contains("Synthetic value"));
}

#[test]
fn generated_operator_limit_includes_retained_artwork_and_preserves_editability() {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let field = source.get_dictionary_mut((8, 0)).unwrap();
    let mut rect = field.get(b"Rect").unwrap().as_array().unwrap().clone();
    rect[2] = 100072.into();
    field.set("Rect", rect);
    field.set("DA", Object::string_literal("/Helv 1 Tf 0 g"));
    let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    dict.set("BBox", vec![0.into(), 0.into(), 100000.into(), 30.into()]);
    let content = String::from_utf8(old.get_plain_content().unwrap())
        .unwrap()
        .replace("0 0 228 30 re", "0 0 100000 30 re");
    source
        .objects
        .insert((9, 0), Stream::new(dict, content.into_bytes()).into());
    let input = bytes(&mut source);
    let mut pdf = PdfDocument::from_bytes(&input, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let error = pdf
        .fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "A".repeat(33333),
            }],
            PLEX,
        )
        .unwrap_err();
    assert!(matches!(error, pdf_core::Error::Limit(_)), "{error}");
    assert!(error.to_string().contains("100000 operators"), "{error}");
    assert_eq!(pdf.to_bytes().unwrap(), before);
    // The same document accepts an appearance within the source parser budget,
    // remains writable after verified reopening, and supports another edit.
    fill(&mut pdf, "Short", PLEX);
    let saved = pdf.verified_bytes().unwrap();
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    assert!(
        reopened
            .list_form_fields()
            .unwrap()
            .into_iter()
            .find(|field| field.name == "full_name")
            .unwrap()
            .writable_text
    );
    fill(&mut reopened, "Again", PLEX);
    assert_value(&reopened, "Again");

    // Retained nonpainting artwork counts too, even when generated glyph
    // operations alone fit. Keep a valid balanced original appearance.
    let mut source = Document::load_mem(&input).unwrap();
    let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut content = Content::decode(&old.get_plain_content().unwrap()).unwrap();
    content.operations.splice(
        0..0,
        (0..500).flat_map(|_| {
            [
                lopdf::content::Operation::new("q", vec![]),
                lopdf::content::Operation::new("Q", vec![]),
            ]
        }),
    );
    let dict = old.dict.clone();
    source
        .objects
        .insert((9, 0), Stream::new(dict, content.encode().unwrap()).into());
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    let error = pdf
        .fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "A".repeat(33000),
            }],
            PLEX,
        )
        .unwrap_err();
    assert!(matches!(error, pdf_core::Error::Limit(_)), "{error}");
    assert!(error.to_string().contains("100000 operators"), "{error}");
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn persistent_text_state_cannot_change_dependent_outside_artwork_or_later_text() {
    for stroke in [false, true] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
        let mut content = Content::decode(&old.get_plain_content().unwrap()).unwrap();
        let et = content
            .operations
            .iter()
            .position(|op| op.operator == "ET")
            .unwrap();
        let op = |name: &str, args| lopdf::content::Operation::new(name, args);
        let color = if stroke { "RG" } else { "rg" };
        content.operations.splice(
            et..et,
            [
                op(color, vec![1.into(), 0.into(), 0.into()]),
                op("w", vec![2.into()]),
            ],
        );
        content.operations.splice(
            et + 3..et + 3,
            [
                op("re", vec![0.into(), 0.into(), 4.into(), 4.into()]),
                op(if stroke { "S" } else { "f" }, vec![]),
            ],
        );
        let mut dict = old.dict.clone();
        dict.remove(b"Filter");
        dict.remove(b"DecodeParms");
        source
            .objects
            .insert((9, 0), Stream::new(dict, content.encode().unwrap()).into());
        let original = bytes(&mut source);
        let raster = pdf_render::render_page(&original, 0, 1.0).unwrap();
        assert!(
            raster
                .rgba
                .chunks_exact(4)
                .any(|pixel| pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80)
        );
        let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        let field = pdf
            .list_form_fields()
            .unwrap()
            .into_iter()
            .find(|f| f.name == "full_name")
            .unwrap();
        assert!(!field.writable_text);
        assert!(
            field
                .text_refusal_reason
                .unwrap()
                .contains("depends on replaced text state")
        );
        assert!(
            pdf.fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: "Changed".into()
                }],
                PLEX
            )
            .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut source = Document::load_mem(SOURCE).unwrap();
    let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut content = Content::decode(&old.get_plain_content().unwrap()).unwrap();
    let et = content
        .operations
        .iter()
        .position(|op| op.operator == "ET")
        .unwrap();
    let op = |name: &str, args| lopdf::content::Operation::new(name, args);
    content.operations.splice(
        et + 1..et + 1,
        [
            op("BT", vec![]),
            op("Td", vec![0.into(), 4.into()]),
            op("Tj", vec![Object::string_literal("Outside")]),
            op("ET", vec![]),
        ],
    );
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    source
        .objects
        .insert((9, 0), Stream::new(dict, content.encode().unwrap()).into());
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    assert!(
        pdf.fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "Changed".into()
            }],
            PLEX
        )
        .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn an_isolated_replacement_preserves_state_used_by_surrounding_artwork() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, "First", PLEX);
    let mut source = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let id = source
        .get_dictionary((8, 0))
        .unwrap()
        .get_deref(b"AP", &source)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let old = source.get_object(id).unwrap().as_stream().unwrap();
    let mut content = Content::decode(&old.get_plain_content().unwrap()).unwrap();
    let start = content
        .operations
        .iter()
        .position(|op| op.operator == "BDC")
        .unwrap()
        - 4;
    let restore = content
        .operations
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, op)| op.operator == "Q")
        .unwrap()
        .0;
    let op = |name: &str, args| lopdf::content::Operation::new(name, args);
    content.operations.splice(
        restore + 1..restore + 1,
        [
            op("re", vec![0.into(), 0.into(), 2.into(), 2.into()]),
            op("f", vec![]),
        ],
    );
    content
        .operations
        .insert(start, op("rg", vec![0.into(), 0.into(), 1.into()]));
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    source
        .objects
        .insert(id, Stream::new(dict, content.encode().unwrap()).into());
    let original = bytes(&mut source);
    let before = pdf_render::render_page(&original, 0, 1.0).unwrap();
    let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
    fill(&mut pdf, "Second", PLEX);
    let after = pdf_render::render_page(&pdf.verified_bytes().unwrap(), 0, 1.0).unwrap();
    for y in 119..120 {
        for x in 72..74 {
            let at = (y * before.width as usize + x) * 4;
            assert_eq!(&before.rgba[at..at + 4], &after.rgba[at..at + 4]);
            assert!(before.rgba[at + 2] > 200 && before.rgba[at] < 80);
        }
    }
    assert_value(&pdf, "Second");
}

#[test]
fn graphics_and_pending_paths_crossing_text_replacement_refuse_atomically() {
    for variant in 0..3 {
        let mut source = Document::load_mem(SOURCE).unwrap();
        let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
        let mut content = Content::decode(&old.get_plain_content().unwrap()).unwrap();
        let bt = content
            .operations
            .iter()
            .position(|op| op.operator == "BT")
            .unwrap();
        let et = content
            .operations
            .iter()
            .position(|op| op.operator == "ET")
            .unwrap();
        let operation = |name: &str| lopdf::content::Operation::new(name, vec![]);
        match variant {
            // Balanced overall, but removing BT/ET also removes this restore.
            0 => {
                content.operations.insert(et + 1, operation("q"));
                content.operations.insert(bt + 1, operation("Q"));
            }
            // The new clip would erase a path painted after the text object.
            1 => {
                content.operations.insert(et + 1, operation("f"));
                content.operations.insert(
                    bt,
                    lopdf::content::Operation::new(
                        "re",
                        vec![0.into(), 0.into(), 4.into(), 4.into()],
                    ),
                );
            }
            // A removed save leaves its matching restore in surrounding artwork.
            _ => {
                content.operations.insert(et + 1, operation("Q"));
                content.operations.insert(bt + 1, operation("q"));
            }
        }
        let mut dict = old.dict.clone();
        dict.remove(b"Filter");
        dict.remove(b"DecodeParms");
        source
            .objects
            .insert((9, 0), Stream::new(dict, content.encode().unwrap()).into());
        let original = bytes(&mut source);
        let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        let field = pdf
            .list_form_fields()
            .unwrap()
            .into_iter()
            .find(|field| field.name == "full_name")
            .unwrap();
        assert!(!field.writable_text);
        assert!(
            field
                .text_refusal_reason
                .unwrap()
                .contains("replacement boundary")
        );
        let error = pdf
            .fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: "Replacement".into(),
                }],
                PLEX,
            )
            .unwrap_err();
        assert!(matches!(error, pdf_core::Error::Refused(_)), "{error}");
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
}

#[test]
fn clearing_text_generates_a_real_empty_appearance_and_can_be_refilled() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, "", PLEX);
    let saved = pdf.verified_bytes().unwrap();
    let doc = Document::load_mem(&saved).unwrap();
    let content = Content::decode(&appearance(&doc).get_plain_content().unwrap()).unwrap();
    assert!(!content.operations.iter().any(|op| op.operator == "Tj"));
    assert!(!pdf.extract_text(&[1]).unwrap().contains("Synthetic value"));
    let field = pdf
        .list_form_fields()
        .unwrap()
        .into_iter()
        .find(|f| f.name == "full_name")
        .unwrap();
    assert_eq!(field.value, pdf_core::FormValue::Text(String::new()));
    let mut reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    fill(&mut reopened, "Restored", PLEX);
    assert_value(&reopened, "Restored");
}

#[test]
fn invisible_letters_and_color_fonts_refuse_atomically_but_formatting_is_preserved() {
    fn table(font: &[u8], tag: &[u8]) -> usize {
        let count = u16::from_be_bytes(font[4..6].try_into().unwrap()) as usize;
        (0..count)
            .find_map(|i| {
                let entry = 12 + i * 16;
                (&font[entry..entry + 4] == tag).then(|| {
                    u32::from_be_bytes(font[entry + 8..entry + 12].try_into().unwrap()) as usize
                })
            })
            .unwrap()
    }
    let mut invisible = PLEX.to_vec();
    let gid = FontRef::new(PLEX)
        .unwrap()
        .charmap()
        .map('A')
        .unwrap()
        .to_u32() as usize;
    let head = table(PLEX, b"head");
    let loca = table(PLEX, b"loca");
    let offset = if i16::from_be_bytes(PLEX[head + 50..head + 52].try_into().unwrap()) == 0 {
        usize::from(u16::from_be_bytes(
            PLEX[loca + gid * 2..loca + gid * 2 + 2].try_into().unwrap(),
        )) * 2
    } else {
        u32::from_be_bytes(PLEX[loca + gid * 4..loca + gid * 4 + 4].try_into().unwrap()) as usize
    };
    let start = table(PLEX, b"glyf") + offset;
    // Keep the original nonzero declared bounding box: inspect real outline
    // commands rather than trusting a font's header to prove visible ink.
    invisible[start..start + 2].fill(0);
    let count = u16::from_be_bytes(PLEX[4..6].try_into().unwrap()) as usize;
    let end = 12 + count * 16;
    let mut color = PLEX.to_vec();
    color.splice(end..end, [0u8; 16]);
    color[4..6].copy_from_slice(&((count + 1) as u16).to_be_bytes());
    for i in 0..count {
        let at = 12 + i * 16 + 8;
        let old = u32::from_be_bytes(color[at..at + 4].try_into().unwrap());
        color[at..at + 4].copy_from_slice(&(old + 16).to_be_bytes());
    }
    color[end..end + 4].copy_from_slice(b"COLR");
    let length = color.len() as u32;
    color[end + 8..end + 12].copy_from_slice(&length.to_be_bytes());
    for (font, message) in [(&invisible, "visible outline"), (&color, "color, bitmap")] {
        let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        let error = pdf
            .fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: "A".into(),
                }],
                font,
            )
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    for value in ["A  B\u{a0}C", "ABC \u{202b}مرحبا\u{202c} 123 e\u{301}"] {
        let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
        fill(&mut pdf, value, AMIRI);
        assert_value(&pdf, value);
        let saved = pdf.verified_bytes().unwrap();
        if value.contains('\u{202b}') {
            let doc = Document::load_mem(&saved).unwrap();
            let font = appearance(&doc)
                .dict
                .get_deref(b"Resources", &doc)
                .unwrap()
                .as_dict()
                .unwrap()
                .get_deref(b"Font", &doc)
                .unwrap()
                .as_dict()
                .unwrap()
                .get_deref(b"NativeFormText", &doc)
                .unwrap()
                .as_dict()
                .unwrap();
            let cmap = font
                .get_deref(b"ToUnicode", &doc)
                .unwrap()
                .as_stream()
                .unwrap();
            let decoded = cmap.get_plain_content().unwrap();
            let mapping = String::from_utf8(decoded).unwrap();
            assert!(mapping.contains("202B"), "{mapping}");
            assert!(mapping.contains("202C"), "{mapping}");
        }
        let reopened = PdfDocument::from_bytes(&saved, None).unwrap();
        assert_value(&reopened, value);
    }
}

#[test]
fn accents_combining_ligatures_and_repeated_fills_embed_real_unicode_appearances() {
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, "Résumé e\u{301} office ΩЖ", PLEX);
    assert_value(&pdf, "Résumé e\u{301} office ΩЖ");
    let saved = pdf.verified_bytes().unwrap();
    let doc = Document::load_mem(&saved).unwrap();
    let ap = appearance(&doc);
    let fonts = ap
        .dict
        .get_deref(b"Resources", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"Font", &doc)
        .unwrap()
        .as_dict()
        .unwrap();
    let font = fonts
        .get_deref(b"NativeFormText", &doc)
        .unwrap()
        .as_dict()
        .unwrap();
    let cmap = font
        .get_deref(b"ToUnicode", &doc)
        .unwrap()
        .as_stream()
        .unwrap()
        .get_plain_content()
        .unwrap();
    let cmap = String::from_utf8(cmap).unwrap();
    assert!(
        cmap.contains("00650301"),
        "combining cluster mapping: {cmap}"
    );
    assert!(
        cmap.contains("006600660069") || cmap.contains("00660069"),
        "ligature mapping: {cmap}"
    );
    let descendant = font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]
        .as_reference()
        .unwrap();
    let descendant = doc.get_dictionary(descendant).unwrap();
    assert_eq!(
        descendant.get(b"Subtype").unwrap().as_name().unwrap(),
        b"CIDFontType2"
    );
    let map = descendant
        .get_deref(b"CIDToGIDMap", &doc)
        .unwrap()
        .as_stream()
        .unwrap()
        .get_plain_content()
        .unwrap();
    assert!(map.len() > 2 && map.len().is_multiple_of(2));
    let descriptor = descendant
        .get_deref(b"FontDescriptor", &doc)
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(
        descriptor
            .get_deref(b"FontFile2", &doc)
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap(),
        PLEX
    );
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    pdf.save_new(&first).unwrap();
    let mut reopened = PdfDocument::open(&first, None).unwrap();
    fill(&mut reopened, "AAAAA", PLEX);
    let middle = Document::load_mem(&reopened.to_bytes().unwrap()).unwrap();
    let count = Content::decode_strict(&appearance(&middle).get_plain_content().unwrap())
        .unwrap()
        .operations
        .len();
    for value in ["BBBBB", "CCCCC", "DDDDD"] {
        fill(&mut reopened, value, PLEX);
        assert_value(&reopened, value);
        let saved = reopened.verified_bytes().unwrap();
        let doc = Document::load_mem(&saved).unwrap();
        assert_eq!(
            Content::decode_strict(&appearance(&doc).get_plain_content().unwrap())
                .unwrap()
                .operations
                .len(),
            count
        );
        reopened = PdfDocument::from_bytes(&saved, None).unwrap();
        assert_eq!(
            doc.objects
                .values()
                .filter_map(|object| object.as_dict().ok())
                .filter_map(|dict| dict.get(b"FontFile2").ok())
                .filter_map(|object| object.as_reference().ok())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1,
            "font program reused"
        );
    }
}

#[test]
fn supplementary_and_mixed_arabic_latin_text_have_real_glyphs_and_logical_extraction() {
    let face = FontRef::new(SYMBOLS).unwrap();
    let character = face
        .charmap()
        .mappings()
        .find_map(|(code, glyph)| {
            if code > 0xffff && glyph.to_u32() != 0 {
                char::from_u32(code)
            } else {
                None
            }
        })
        .unwrap();
    let value = character.to_string();
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, &value, SYMBOLS);
    assert_value(&pdf, &value);
    let saved = pdf.verified_bytes().unwrap();
    let doc = Document::load_mem(&saved).unwrap();
    let font = appearance(&doc)
        .dict
        .get_deref(b"Resources", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"Font", &doc)
        .unwrap()
        .as_dict()
        .unwrap()
        .get_deref(b"NativeFormText", &doc)
        .unwrap()
        .as_dict()
        .unwrap();
    let cmap = String::from_utf8(
        font.get_deref(b"ToUnicode", &doc)
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap(),
    )
    .unwrap();
    let hex: String = value.encode_utf16().map(|n| format!("{n:04X}")).collect();
    assert!(cmap.contains(&hex));
    let face = FontRef::new(ARABIC).unwrap();
    assert!(face.glyf().is_ok());
    let value = "مرحبا 123";
    assert!(
        value.chars().all(|c| face.charmap().map(c).is_some()),
        "test font coverage"
    );
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, value, ARABIC);
    assert_value(&pdf, value);
    let saved = pdf.verified_bytes().unwrap();
    assert_value(&PdfDocument::from_bytes(&saved, None).unwrap(), value);
    let raster = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    assert_ne!(raster, pdf_render::render_page(SOURCE, 0, 1.0).unwrap());
    let mixed = "ABC e\u{301} مرحبا بالعالم 123";
    let face = FontRef::new(AMIRI).unwrap();
    assert!(face.glyf().is_ok());
    assert!(
        mixed.chars().all(|c| face.charmap().map(c).is_some()),
        "missing {:?}",
        mixed
            .chars()
            .filter(|c| face.charmap().map(*c).is_none())
            .collect::<Vec<_>>()
    );
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    fill(&mut pdf, mixed, AMIRI);
    assert_value(&pdf, mixed);
    assert_value(
        &PdfDocument::from_bytes(&pdf.verified_bytes().unwrap(), None).unwrap(),
        mixed,
    );
    let before = pdf.to_bytes().unwrap();
    assert!(
        pdf.fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "ABC مرحبا".into()
            }],
            ARABIC
        )
        .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn shared_appearance_and_unrelated_artwork_survive_widget_local_replacement() {
    let mut source = Document::load_mem(SOURCE).unwrap();
    let mut other = source.get_dictionary((8, 0)).unwrap().clone();
    other.set("T", Object::string_literal("other"));
    other.set("Rect", vec![72.into(), 632.into(), 300.into(), 662.into()]);
    let other = source.add_object(other);
    source
        .get_dictionary_mut((5, 0))
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(other.into());
    let root = source.get_dictionary_mut((1, 0)).unwrap();
    root.get_mut(b"AcroForm")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Fields")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(other.into());
    let old = source.get_object((9, 0)).unwrap().as_stream().unwrap();
    let mut content = b"q 1 0 0 rg 0 0 4 4 re f Q\n".to_vec();
    content.extend(old.get_plain_content().unwrap());
    let mut dict = old.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    source
        .objects
        .insert((9, 0), Stream::new(dict, content).into());
    let original = bytes(&mut source);
    let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
    fill(&mut pdf, "AAAAA", PLEX);
    let saved = pdf.verified_bytes().unwrap();
    let changed = Document::load_mem(&saved).unwrap();
    assert_eq!(
        source.get_object((9, 0)).unwrap(),
        changed.get_object((9, 0)).unwrap()
    );
    assert_eq!(
        source.get_object(other).unwrap(),
        changed.get_object(other).unwrap()
    );
    let before = pdf_render::render_page(&original, 0, 1.0).unwrap();
    let after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
    let mut count = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let x = index % before.width as usize;
            let y = index / before.width as usize;
            assert!(
                (72..300).contains(&x) && (90..120).contains(&y),
                "outside widget {x},{y}"
            );
            count += 1;
        }
    }
    assert!(count > 0);
    for y in 117..120 {
        for x in 72..75 {
            let index = (y * before.width as usize + x) * 4;
            assert_eq!(
                &before.rgba[index..index + 4],
                &after.rgba[index..index + 4]
            );
        }
    }
}

#[test]
fn unsupported_layout_fonts_values_and_late_transaction_errors_are_atomic() {
    for flags in [1, 1 << 13, 1 << 20, 1 << 24, 1 << 25] {
        let mut source = Document::load_mem(SOURCE).unwrap();
        source.get_dictionary_mut((8, 0)).unwrap().set("Ff", flags);
        let source = bytes(&mut source);
        let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(
            pdf.fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: "new".into()
                }],
                PLEX
            )
            .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    for value in ["line\nbreak", "中文", "a".repeat(65537).as_str()] {
        let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(
            pdf.fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: value.into()
                }],
                PLEX
            )
            .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut source = Document::load_mem(SOURCE).unwrap();
    source
        .get_dictionary_mut((8, 0))
        .unwrap()
        .set("DA", Object::string_literal("/Helv 100 Tf 0 g"));
    source.get_dictionary_mut((8, 0)).unwrap().set("MaxLen", 3);
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut source), None).unwrap();
    let before = pdf.to_bytes().unwrap();
    for value in ["AAAA", "AAA"] {
        assert!(
            pdf.fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: value.into()
                }],
                PLEX
            )
            .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    let before = pdf.to_bytes().unwrap();
    assert!(
        pdf.fill_text(
            &[
                TextChange {
                    name: "full_name".into(),
                    value: "planned".into()
                },
                TextChange {
                    name: "missing".into(),
                    value: "error".into()
                }
            ],
            PLEX
        )
        .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
    assert!(
        pdf.fill_text(
            &[TextChange {
                name: "full_name".into(),
                value: "new".into()
            }],
            b"not a font"
        )
        .is_err()
    );
    assert_eq!(pdf.to_bytes().unwrap(), before);
}

#[test]
fn quadrant_rotations_alignment_and_embedding_permissions_are_checked() {
    for rotation in [0, 90, 180, 270] {
        for q in [0, 1, 2] {
            let mut source = Document::load_mem(SOURCE).unwrap();
            let widget = source.get_dictionary_mut((8, 0)).unwrap();
            widget.set("Q", q);
            widget.set("MK", lopdf::dictionary! {"R"=>rotation});
            let (w, h, matrix) = match rotation {
                0 => (228, 30, vec![1, 0, 0, 1, 0, 0]),
                90 => (30, 228, vec![0, 1, -1, 0, 228, 0]),
                180 => (228, 30, vec![-1, 0, 0, -1, 228, 30]),
                _ => (30, 228, vec![0, -1, 1, 0, 0, 30]),
            };
            let stream = source
                .get_object_mut((9, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap();
            stream
                .dict
                .set("BBox", vec![0.into(), 0.into(), w.into(), h.into()]);
            stream.dict.set(
                "Matrix",
                matrix.into_iter().map(Object::Integer).collect::<Vec<_>>(),
            );
            let old = String::from_utf8(stream.get_plain_content().unwrap()).unwrap();
            stream.set_plain_content(
                old.replace("0 0 228 30 re", &format!("0 0 {w} {h} re"))
                    .into_bytes(),
            );
            let original = bytes(&mut source);
            let mut pdf = PdfDocument::from_bytes(&original, None).unwrap();
            fill(&mut pdf, "abc", PLEX);
            assert_value(&pdf, "abc");
            let saved = pdf.verified_bytes().unwrap();
            assert_eq!(
                pdf.info().unwrap().pages,
                PdfDocument::from_bytes(&original, None)
                    .unwrap()
                    .info()
                    .unwrap()
                    .pages
            );
            let rendered = pdf_render::render_page(&saved, 0, 1.0).unwrap();
            assert!(rendered.rgba.len() > 100);
        }
    }
    let tables = u16::from_be_bytes([PLEX[4], PLEX[5]]) as usize;
    let os2 = (0..tables)
        .find_map(|index| {
            let entry = 12 + index * 16;
            if &PLEX[entry..entry + 4] == b"OS/2" {
                Some(u32::from_be_bytes(PLEX[entry + 8..entry + 12].try_into().unwrap()) as usize)
            } else {
                None
            }
        })
        .unwrap();
    for fs_type in [0x0002u16, 0x0004, 0x0200] {
        let mut restricted = PLEX.to_vec();
        restricted[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
        let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(
            pdf.fill_text(
                &[TextChange {
                    name: "full_name".into(),
                    value: "text".into()
                }],
                &restricted
            )
            .is_err()
        );
        assert_eq!(pdf.to_bytes().unwrap(), before);
    }
    let cff =
        include_bytes!("../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf");
    let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
    pdf.fill_text(
        &[TextChange {
            name: "full_name".into(),
            value: "中文".into(),
        }],
        cff,
    )
    .unwrap();
    assert_value(&pdf, "中文");
}

fn reference(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap()).join(name)
}
fn text(path: &Path) -> String {
    let output = Command::new(reference("pdftotext"))
        .arg("-raw")
        .arg(path)
        .arg("-")
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
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
// Pinned Poppler 26.10.0 emits visual Arabic order and bidi controls and
// removes the space before European digits. Exact mode-specific outputs keep
// this limitation visible. Logical V/AP ActualText are checked independently.
const RTL_GOLDENS: [[&str; 3]; 2] = [
    [
        "Synthetic form controls\n\u{202b} ابحرم\u{202c}123\n\n✓\n\nNormal\n2026-08-24\n\n\u{c}",
        "Synthetic form controls\n\n\n\u{202b} ابحرم\u{202c}123\n\n\n\n✓\n\nNormal\n2026-08-24\n\u{c}",
        "Synthetic form controls\n\u{202b} ابحرم\u{202c}123\n✓\nNormal\n2026-08-24\u{c}",
    ],
    [
        "Synthetic form controls\nABC e\u{301} \u{202b} ملاعلاب ابحرم\u{202c}123\n\n✓\n\nNormal\n2026-08-24\n\n\u{c}",
        "Synthetic form controls\n\n\nABC e\u{301} \u{202b} ملاعلاب ابحرم\u{202c}123\n\n\n✓\n\nNormal\n2026-08-24\n\u{c}",
        "Synthetic form controls\nABC e\u{301} \u{202b} ملاعلاب ابحرم\u{202c}123\n✓\nNormal\n2026-08-24\u{c}",
    ],
];
#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn unicode_appearances_pass_independent_poppler_and_qpdf_checks() {
    let version = Command::new(reference("pdftoppm"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let version = Command::new(&qpdf).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    std::fs::write(&source, SOURCE).unwrap();
    let before = pixels(&source);
    let supplementary = FontRef::new(SYMBOLS)
        .unwrap()
        .charmap()
        .mappings()
        .find_map(|(code, glyph)| {
            if code > 0xffff && glyph.to_u32() != 0 {
                char::from_u32(code)
            } else {
                None
            }
        })
        .unwrap()
        .to_string();
    for (index, (value, font)) in [
        ("Résumé e\u{301} office ΩЖ", PLEX),
        ("مرحبا 123", ARABIC),
        ("ABC e\u{301} مرحبا بالعالم 123", AMIRI),
        (supplementary.as_str(), SYMBOLS),
    ]
    .into_iter()
    .enumerate()
    {
        let mut pdf = PdfDocument::from_bytes(SOURCE, None).unwrap();
        fill(&mut pdf, value, font);
        let output = temp.path().join(format!("text{index}.pdf"));
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
        let extracted = text(&output);
        if matches!(index, 1 | 2) {
            for (mode_index, mode) in [None, Some("-layout"), Some("-raw")]
                .into_iter()
                .enumerate()
            {
                let mut command = Command::new(reference("pdftotext"));
                if let Some(mode) = mode {
                    command.arg(mode);
                }
                let result = command.arg(&output).arg("-").output().unwrap();
                assert!(result.status.success());
                assert_eq!(
                    String::from_utf8(result.stdout).unwrap(),
                    RTL_GOLDENS[index - 1][mode_index],
                    "pinned RTL output case{index} mode{mode:?}"
                );
            }
        }
        if matches!(index, 0 | 3) {
            assert!(
                extracted.contains(value),
                "independent LTR extraction {extracted:?}"
            );
        }
        let model = Command::new(&qpdf)
            .arg("--json")
            .arg(&output)
            .output()
            .unwrap();
        assert!(model.status.success());
        let model: serde_json::Value = serde_json::from_slice(&model.stdout).unwrap();
        assert_eq!(
            model["qpdf"][1]["obj:8 0 R"]["value"]["/V"],
            format!("u:{value}")
        );
        let normal = model["qpdf"][1]["obj:8 0 R"]["value"]["/AP"]["/N"]
            .as_str()
            .unwrap();
        let object = normal.split_whitespace().next().unwrap();
        let ap = Command::new(&qpdf)
            .arg(format!("--show-object={object}"))
            .arg("--filtered-stream-data")
            .arg(&output)
            .output()
            .unwrap();
        assert!(ap.status.success());
        let actual = format!(
            "FEFF{}",
            value
                .encode_utf16()
                .map(|unit| format!("{unit:04X}"))
                .collect::<String>()
        );
        assert!(
            String::from_utf8(ap.stdout)
                .unwrap()
                .to_ascii_uppercase()
                .contains(&actual),
            "independent logical AP ActualText"
        );
        let after = pixels(&output);
        assert_eq!(before.len(), after.len());
        let mut changed = 0;
        for (pixel, (a, b)) in before
            .chunks_exact(3)
            .zip(after.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                let x = pixel % 595;
                let y = pixel / 595;
                assert!(
                    (72..300).contains(&x) && (90..120).contains(&y),
                    "independent outsidewidget {x},{y}"
                );
                changed += 1;
            }
        }
        assert!(changed > 0);
        let raster = Command::new(reference("pdftoppm"))
            .args(["-f", "1", "-singlefile", "-r", "72", "-png"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(raster.status.success());
        assert!(!raster.stdout.is_empty());
    }
    assert_eq!(std::fs::read(source).unwrap(), SOURCE);
}
