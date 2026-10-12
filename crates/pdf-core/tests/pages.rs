#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{Error, PdfDocument};

fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let root = doc.new_object_id();
    let branch = doc.new_object_id();
    let font = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for number in 1..=3 {
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 20 30 Td (Page {number}) Tj ET").into_bytes(),
        ));
        let page = doc.add_object(
            dictionary! { "Type" => "Page", "Parent" => branch, "Contents" => content },
        );
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        branch,
        dictionary! { "Type" => "Pages", "Parent" => root,
        "Kids" => kids, "Count" => 3, "Resources" => resources, "Rotate" => 90,
        "CropBox" => vec![10.into(), 10.into(), 590.into(), 780.into()] }
        .into(),
    );
    doc.objects.insert(
        root,
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(branch)],
        "Count" => 3, "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()] }
        .into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    doc.trailer.set("Root", catalog);
    doc
}

fn bytes(mut doc: Document) -> Vec<u8> {
    let mut result = Vec::new();
    doc.save_to(&mut result).unwrap();
    result
}

fn load(doc: Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(doc), None).unwrap()
}

fn parsed(doc: &PdfDocument) -> Document {
    Document::load_mem(&doc.to_bytes().unwrap()).unwrap()
}

#[test]
fn subset_preserves_inert_catalog_info_and_refuses_reference_values() {
    for indirect in [false, true] {
        let mut source = fixture();
        let metadata = dictionary! { "Producer" => Object::string_literal("MuPDF 1.28.2") };
        let value = if indirect {
            Object::Reference(source.add_object(metadata))
        } else {
            Object::Dictionary(metadata)
        };
        source.catalog_mut().unwrap().set("Info", value);
        let expected = source.catalog().unwrap().get(b"Info").unwrap().clone();
        let doc = load(source);
        let subset = doc.extract_pages(&[3, 1]).unwrap();
        let saved = parsed(&subset);
        assert_eq!(saved.catalog().unwrap().get(b"Info").unwrap(), &expected);
        let info = saved.dereference(&expected).unwrap().1.as_dict().unwrap();
        assert_eq!(
            info.get(b"Producer").unwrap().as_str().unwrap(),
            b"MuPDF 1.28.2"
        );
        assert_eq!(subset.page_count(), 2);
    }
    let mut source = fixture();
    let removed = *source.get_pages().get(&2).unwrap();
    source
        .catalog_mut()
        .unwrap()
        .set("Info", dictionary! { "Custom" => removed });
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(matches!(doc.delete_pages(&[2]), Err(Error::Refused(_))));
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn reorder_materializes_nested_inheritance_and_keeps_page_ids() {
    let source = fixture();
    let original_ids = source.get_pages().values().copied().collect::<Vec<_>>();
    let original = bytes(source);
    let mut doc = PdfDocument::from_bytes(&original, None).unwrap();
    doc.reorder(&[3, 1, 2]).unwrap();
    let saved = parsed(&doc);
    assert_eq!(
        saved.get_pages().values().copied().collect::<Vec<_>>(),
        [original_ids[2], original_ids[0], original_ids[1]]
    );
    for id in saved.get_pages().values() {
        let page = saved.get_dictionary(*id).unwrap();
        assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
        assert!(page.has(b"Resources"));
        assert!(page.has(b"MediaBox"));
        assert!(page.has(b"CropBox"));
    }
    assert!(doc.extract_text(&[1]).unwrap().contains("Page 3"));
    assert!(
        PdfDocument::from_bytes(&original, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("Page 1")
    );
    doc.reverse().unwrap();
    assert!(doc.extract_text(&[1]).unwrap().contains("Page 2"));
}

#[test]
fn reorder_preserves_catalog_forms_bookmarks_layers_tags_and_links() {
    let mut source = fixture();
    let ids = source.get_pages().values().copied().collect::<Vec<_>>();
    let link = source.add_object(dictionary! { "Type" => "Annot", "Subtype" => "Link",
    "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
    "Dest" => vec![Object::Reference(ids[2]), Object::Name(b"Fit".to_vec())] });
    source
        .get_dictionary_mut(ids[0])
        .unwrap()
        .set("Annots", vec![Object::Reference(link)]);
    for key in [
        "Outlines",
        "AcroForm",
        "OCProperties",
        "StructTreeRoot",
        "Names",
    ] {
        let object = source.add_object(dictionary! { "Preserved" => true });
        source.catalog_mut().unwrap().set(key, object);
    }
    let expected = source.clone();
    let mut doc = load(source);
    doc.reorder(&[2, 3, 1]).unwrap();
    let saved = parsed(&doc);
    for key in [
        b"Outlines".as_slice(),
        b"AcroForm",
        b"OCProperties",
        b"StructTreeRoot",
        b"Names",
    ] {
        assert_eq!(
            saved.catalog().unwrap().get(key).unwrap(),
            expected.catalog().unwrap().get(key).unwrap()
        );
    }
    assert_eq!(
        saved.get_object(link).unwrap(),
        expected.get_object(link).unwrap()
    );
    assert_eq!(
        saved
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"Annots")
            .unwrap(),
        expected
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"Annots")
            .unwrap()
    );
    assert!(matches!(doc.delete_pages(&[1]), Err(Error::Refused(_))));
}

#[test]
fn crop_and_blank_insert_preserve_content_and_geometry() {
    let mut doc = load(fixture());
    doc.crop(&[1, 3], [20.0, 30.0, 500.0, 700.0]).unwrap();
    assert!(doc.extract_text(&[1]).unwrap().contains("Page 1"));
    let saved = parsed(&doc);
    let first = *saved.get_pages().get(&1).unwrap();
    let crop = saved
        .get_dictionary(first)
        .unwrap()
        .get(b"CropBox")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(
        crop.iter()
            .map(|value| value.as_float().unwrap())
            .collect::<Vec<_>>(),
        [20.0, 30.0, 500.0, 700.0]
    );
    doc.insert_blank(2, 300.0, 400.0).unwrap();
    assert_eq!(doc.page_count(), 4);
    assert_eq!(
        doc.info().unwrap().pages[1].media_box,
        [0.0, 0.0, 300.0, 400.0]
    );
    assert_eq!(doc.info().unwrap().pages[1].rotation, 0);
    assert!(doc.extract_text(&[2]).unwrap().trim().is_empty());
    assert!(doc.extract_text(&[3]).unwrap().contains("Page 2"));
    let dir = tempfile::tempdir().unwrap();
    doc.save_new(dir.path().join("organized.pdf")).unwrap();
}

#[test]
fn invalid_edits_leave_document_unchanged() {
    let mut doc = load(fixture());
    let original = doc.to_bytes().unwrap();
    for order in [
        vec![],
        vec![1, 2],
        vec![1, 1, 3],
        vec![0, 2, 3],
        vec![1, 2, 4],
    ] {
        assert!(doc.reorder(&order).is_err());
        assert_eq!(doc.to_bytes().unwrap(), original);
    }
    for rectangle in [
        [0.0, 0.0, f32::NAN, 20.0],
        [0.0, 0.0, 601.0, 800.0],
        [20.0, 20.0, 10.0, 30.0],
    ] {
        assert!(doc.crop(&[1, 2], rectangle).is_err());
    }
    for (position, width, height) in [
        (0, 100.0, 100.0),
        (5, 100.0, 100.0),
        (2, f32::INFINITY, 100.0),
        (2, 0.0, 100.0),
    ] {
        assert!(doc.insert_blank(position, width, height).is_err());
    }
    assert!(doc.delete_pages(&[1, 2, 3]).is_err());
    assert!(doc.extract_pages(&[1, 1]).is_err());
    assert_eq!(doc.to_bytes().unwrap(), original);
}

#[test]
fn plain_subset_operations_keep_selection_order_and_original() {
    let mut doc = load(fixture());
    let original = doc.to_bytes().unwrap();
    let extracted = doc.extract_pages(&[3, 1]).unwrap();
    assert_eq!(extracted.page_count(), 2);
    assert!(extracted.extract_text(&[1]).unwrap().contains("Page 3"));
    assert_eq!(doc.to_bytes().unwrap(), original);
    doc.delete_pages(&[2]).unwrap();
    assert_eq!(doc.page_count(), 2);
    assert!(doc.extract_text(&[2]).unwrap().contains("Page 3"));
    let dir = tempfile::tempdir().unwrap();
    extracted
        .save_new(dir.path().join("extracted.pdf"))
        .unwrap();
    doc.save_new(dir.path().join("deleted.pdf")).unwrap();
}

#[test]
fn subset_refuses_structures_and_external_page_references() {
    for key in [
        "Outlines",
        "AcroForm",
        "Names",
        "OCProperties",
        "StructTreeRoot",
        "PageLabels",
        "OpenAction",
    ] {
        let mut source = fixture();
        source.catalog_mut().unwrap().set(key, dictionary! {});
        let mut doc = load(source);
        let before = doc.to_bytes().unwrap();
        assert!(matches!(doc.extract_pages(&[1]), Err(Error::Refused(_))));
        assert!(matches!(doc.delete_pages(&[2]), Err(Error::Refused(_))));
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let mut source = fixture();
    let removed = *source.get_pages().get(&2).unwrap();
    source.add_object(dictionary! { "Target" => removed });
    assert!(matches!(
        load(source).extract_pages(&[1]),
        Err(Error::Refused(_))
    ));
}

#[test]
fn indexed_page_labels_and_signatures_block_unsafe_operations() {
    let mut source = fixture();
    source
        .catalog_mut()
        .unwrap()
        .set("PageLabels", dictionary! {});
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(matches!(doc.reorder(&[3, 2, 1]), Err(Error::Refused(_))));
    assert!(matches!(
        doc.insert_blank(1, 100.0, 100.0),
        Err(Error::Refused(_))
    ));
    assert_eq!(doc.to_bytes().unwrap(), before);
    let mut source = fixture();
    source.add_object(dictionary! { "FT" => "Sig" });
    let mut doc = load(source);
    assert!(matches!(doc.reorder(&[3, 2, 1]), Err(Error::Refused(_))));
    assert!(matches!(
        doc.crop(&[1], [0.0, 0.0, 100.0, 100.0]),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        doc.insert_blank(1, 100.0, 100.0),
        Err(Error::Refused(_))
    ));
    assert!(matches!(doc.extract_pages(&[1]), Err(Error::Refused(_))));
}

#[test]
fn crop_failure_on_later_page_rolls_back_earlier_changes() {
    let mut source = fixture();
    let second = *source.get_pages().get(&2).unwrap();
    source
        .get_dictionary_mut(second)
        .unwrap()
        .set("MediaBox", vec![0.into(), 0.into(), 100.into(), 100.into()]);
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(doc.crop(&[1, 2], [0.0, 0.0, 200.0, 200.0]).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn real_corpus_catalog_objects_remain_intact_after_reversal() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1");
    for name in [
        "bookmarks.pdf",
        "forms.pdf",
        "attachment.pdf",
        "layers-transparency.pdf",
    ] {
        let source = Document::load(corpus.join(name)).unwrap();
        let mut doc = load(source.clone());
        doc.reverse().unwrap();
        let saved = parsed(&doc);
        for key in [
            b"Outlines".as_slice(),
            b"AcroForm",
            b"Names",
            b"OCProperties",
            b"StructTreeRoot",
        ] {
            assert_eq!(
                source.catalog().unwrap().get(key).ok(),
                saved.catalog().unwrap().get(key).ok(),
                "{name}"
            );
            if let Ok(id) = source
                .catalog()
                .unwrap()
                .get(key)
                .and_then(Object::as_reference)
            {
                assert_eq!(
                    source.get_object(id).unwrap(),
                    saved.get_object(id).unwrap(),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn direct_inherited_resources_are_moved_once_for_one_hundred_pages() {
    let mut source = fixture();
    let root = source
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    let branch = source
        .get_dictionary(root)
        .unwrap()
        .get(b"Kids")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let resources = source
        .get_dictionary(branch)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_reference()
        .unwrap();
    let font = source
        .get_dictionary(resources)
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"F1")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut fonts = lopdf::Dictionary::new();
    for number in 0..2048 {
        fonts.set(format!("F{number}"), font);
    }
    source
        .get_dictionary_mut(branch)
        .unwrap()
        .set("Resources", dictionary! { "Font" => fonts });
    source.objects.remove(&resources);
    let mut kids = source
        .get_dictionary(branch)
        .unwrap()
        .get(b"Kids")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    for number in 4..=100 {
        let content = source.add_object(Stream::new(
            dictionary! {},
            format!("BT /F2047 12 Tf 20 30 Td (Page {number}) Tj ET").into_bytes(),
        ));
        let page = source.add_object(
            dictionary! { "Type" => "Page", "Parent" => branch, "Contents" => content },
        );
        kids.push(Object::Reference(page));
    }
    source.get_dictionary_mut(branch).unwrap().set("Kids", kids);
    source.get_dictionary_mut(branch).unwrap().set("Count", 100);
    source.get_dictionary_mut(root).unwrap().set("Count", 100);
    let original_ids = source.get_pages().values().copied().collect::<Vec<_>>();
    let input = bytes(source);
    let original_objects = Document::load_mem(&input).unwrap().objects.len();
    let mut doc = PdfDocument::from_bytes(&input, None).unwrap();
    doc.reverse().unwrap();
    let output = doc.to_bytes().unwrap();
    let saved = Document::load_mem(&output).unwrap();
    assert_eq!(saved.objects.len(), original_objects + 1);
    assert_eq!(
        saved.get_pages().values().copied().collect::<Vec<_>>(),
        original_ids.into_iter().rev().collect::<Vec<_>>()
    );
    let shared = saved
        .get_dictionary(branch)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_reference()
        .unwrap();
    for page in saved.get_pages().values() {
        assert_eq!(
            saved
                .get_dictionary(*page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_reference()
                .unwrap(),
            shared
        );
    }
    let fonts = saved
        .get_dictionary(shared)
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(fonts.len(), 2048);
    assert_eq!(fonts.get(b"F2047").unwrap().as_reference().unwrap(), font);
    assert!(doc.extract_text(&[1]).unwrap().contains("Page 100"));
    assert!(output.len() < input.len() + 20_000);
    assert!(
        PdfDocument::from_bytes(&input, None)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("Page 1")
    );
}

#[test]
fn malformed_resources_fail_transactionally_before_normalization_commits() {
    let mut source = fixture();
    let second = *source.get_pages().get(&2).unwrap();
    source
        .get_dictionary_mut(second)
        .unwrap()
        .set("Resources", 42);
    let mut doc = load(source);
    let original = doc.to_bytes().unwrap();
    assert!(doc.reverse().is_err());
    assert_eq!(doc.to_bytes().unwrap(), original);
    assert!(doc.insert_blank(2, 100.0, 100.0).is_err());
    assert_eq!(doc.to_bytes().unwrap(), original);
}
