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
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdfs");
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

#[test]
fn duplicate_pages_use_new_ids_document_order_and_independent_page_geometry() {
    let mut doc = load(fixture());
    let before = doc.info().unwrap();
    let originals = parsed(&doc).get_pages();
    doc.duplicate_pages(&[3, 1]).unwrap();
    assert_eq!(doc.page_count(), 5);
    let copied = parsed(&doc);
    let ids = copied.get_pages();
    assert_eq!(ids[&1], originals[&1]);
    assert_eq!(ids[&3], originals[&2]);
    assert_eq!(ids[&4], originals[&3]);
    let unique: std::collections::BTreeSet<_> = ids.values().copied().collect();
    assert_eq!(unique.len(), 5);
    assert_ne!(ids[&2], ids[&1]);
    assert_ne!(ids[&5], ids[&4]);
    for (new, old) in [(1, 0), (2, 0), (3, 1), (4, 2), (5, 2)] {
        let page = &doc.info().unwrap().pages[new - 1];
        assert_eq!(page.media_box, before.pages[old].media_box);
        assert_eq!(page.crop_box, before.pages[old].crop_box);
        assert_eq!(page.rotation, before.pages[old].rotation);
    }
    let text = doc.extract_text(&[1, 2, 3, 4, 5]).unwrap();
    assert_eq!(text.matches("Page 1").count(), 2);
    assert_eq!(text.matches("Page 2").count(), 1);
    assert_eq!(text.matches("Page 3").count(), 2);
    let original_content = copied
        .get_dictionary(ids[&1])
        .unwrap()
        .get(b"Contents")
        .unwrap();
    assert_eq!(
        original_content,
        copied
            .get_dictionary(ids[&2])
            .unwrap()
            .get(b"Contents")
            .unwrap()
    );
    doc.rotate(&[2], 90).unwrap();
    doc.crop(&[2], [20.0, 20.0, 500.0, 700.0]).unwrap();
    let info = doc.info().unwrap();
    assert_eq!(info.pages[0].rotation, 90);
    assert_eq!(info.pages[1].rotation, 180);
    assert_eq!(info.pages[0].crop_box, [10.0, 10.0, 590.0, 780.0]);
    assert_eq!(info.pages[1].crop_box, [20.0, 20.0, 500.0, 700.0]);
    doc.duplicate_pages(&[2]).unwrap();
    assert_eq!(doc.page_count(), 6);
    let saved = doc.verified_bytes().unwrap();
    let reopened = PdfDocument::from_bytes(&saved, None).unwrap();
    assert_eq!(reopened.info().unwrap().pages[2].rotation, 180);
}

#[test]
fn duplication_normalizes_direct_resources_once_and_preserves_document_metadata() {
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
    let resource = source
        .get_dictionary(branch)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_reference()
        .unwrap();
    let direct = source.get_dictionary(resource).unwrap().clone();
    source
        .get_dictionary_mut(branch)
        .unwrap()
        .set("Resources", direct);
    let info=source.add_object(dictionary!{"Title"=>Object::string_literal("Duplication title"),"Custom"=>Object::string_literal("preserved")});
    source.trailer.set("Info", info);
    let xmp = source.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        b"<metadata>preserved</metadata>".to_vec(),
    ));
    source.catalog_mut().unwrap().set("Metadata", xmp);
    let mut doc = load(source);
    let before = parsed(&doc);
    let count = before.objects.len();
    doc.duplicate_pages(&[1, 2, 3]).unwrap();
    let after = parsed(&doc);
    assert_eq!(
        after.objects.len(),
        count + 4,
        "three leaves plus one shared normalized resource dictionary"
    );
    let resources: std::collections::BTreeSet<_> = after
        .get_pages()
        .values()
        .map(|id| {
            after
                .get_dictionary(*id)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_reference()
                .unwrap()
        })
        .collect();
    assert_eq!(resources.len(), 1);
    assert_eq!(
        after.get_object(info).unwrap(),
        before.get_object(info).unwrap()
    );
    assert_eq!(
        after.get_object(xmp).unwrap(),
        before.get_object(xmp).unwrap()
    );
}

#[test]
fn duplication_refuses_unsafe_graphs_without_modifying_original_bytes() {
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
        assert!(matches!(doc.duplicate_pages(&[1]), Err(Error::Refused(_))));
        assert!(doc.to_bytes().unwrap() == before);
    }
    for kind in 0..6 {
        let mut source = fixture();
        let page = *source.get_pages().get(&1).unwrap();
        match kind {
            0 => {
                source
                    .get_dictionary_mut(page)
                    .unwrap()
                    .set("Annots", vec![Object::Null]);
            }
            1 => {
                source
                    .get_dictionary_mut(page)
                    .unwrap()
                    .set("ID", Object::string_literal("opaque page identity"));
            }
            2 => {
                source.add_object(dictionary! {"Target"=>page});
            }
            3 => {
                source
                    .get_dictionary_mut(page)
                    .unwrap()
                    .set("Group", dictionary! {"S"=>"Transparency","SelfPage"=>page});
            }
            4 => {
                source.trailer.set("CustomPage", page);
            }
            5 => {
                let root = source
                    .catalog()
                    .unwrap()
                    .get(b"Pages")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                source
                    .get_dictionary_mut(root)
                    .unwrap()
                    .set("CustomPage", page);
            }
            _ => {
                source.add_object(dictionary! {"FT"=>"Sig"});
            }
        }
        let mut doc = load(source);
        let before = doc.to_bytes().unwrap();
        assert!(
            matches!(doc.duplicate_pages(&[1]), Err(Error::Refused(_))),
            "graph case {kind}"
        );
        assert!(doc.to_bytes().unwrap() == before);
    }
    let mut doc = load(fixture());
    let before = doc.to_bytes().unwrap();
    for selection in [&[][..], &[0][..], &[4][..], &[1, 1][..]] {
        assert!(doc.duplicate_pages(selection).is_err());
        assert!(doc.to_bytes().unwrap() == before);
    }
}

#[test]
fn duplication_preflights_page_object_and_inline_value_limits_atomically() {
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
    let template = source
        .get_dictionary(*source.get_pages().get(&1).unwrap())
        .unwrap()
        .clone();
    let mut kids = Vec::new();
    for _ in 0..pdf_core::MAX_PAGES {
        kids.push(Object::Reference(source.add_object(template.clone())));
    }
    source.get_dictionary_mut(branch).unwrap().set("Kids", kids);
    source
        .get_dictionary_mut(branch)
        .unwrap()
        .set("Count", pdf_core::MAX_PAGES as i64);
    source
        .get_dictionary_mut(root)
        .unwrap()
        .set("Count", pdf_core::MAX_PAGES as i64);
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(matches!(doc.duplicate_pages(&[1]), Err(Error::Limit(_))));
    assert!(doc.to_bytes().unwrap() == before);
    let mut source = fixture();
    source.objects.insert(
        ((pdf_core::MAX_OBJECTS as u32) - 2, 0),
        dictionary! {"Budget"=>1}.into(),
    );
    source.max_id = (pdf_core::MAX_OBJECTS as u32) - 2;
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(matches!(
        doc.duplicate_pages(&[1, 2, 3]),
        Err(Error::Limit(_))
    ));
    assert!(doc.to_bytes().unwrap() == before);
    let mut source = fixture();
    let page = *source.get_pages().get(&1).unwrap();
    source.get_dictionary_mut(page).unwrap().set("Group",dictionary!{"Type"=>"Group","S"=>"Transparency","Budget"=>vec![Object::Null;pdf_core::MAX_OBJECTS]});
    let mut doc = load(source);
    let before = doc.to_bytes().unwrap();
    assert!(matches!(doc.duplicate_pages(&[1]), Err(Error::Limit(_))));
    assert!(doc.to_bytes().unwrap() == before);
}

#[test]
fn duplication_read_only_guards_preserve_readable_pages_and_refuse_serialization() {
    let check = |mut doc: PdfDocument| {
        let before = doc.info().unwrap();
        let text = doc.extract_text(&doc.all_pages()).unwrap();
        assert!(doc.to_bytes().is_err());
        assert!(matches!(doc.duplicate_pages(&[1]), Err(Error::Refused(_))));
        let after = doc.info().unwrap();
        assert_eq!(after.page_count, before.page_count);
        assert_eq!(after.signature_fields, before.signature_fields);
        assert_eq!(after.encrypted, before.encrypted);
        assert_eq!(doc.extract_text(&doc.all_pages()).unwrap(), text);
        assert!(doc.to_bytes().is_err());
    };
    let mut signed = fixture();
    signed.add_object(dictionary! {"FT"=>"Sig"});
    check(load(signed));
    let mut encrypted = fixture();
    encrypted.trailer.set(
        "ID",
        vec![
            Object::string_literal("duplicate-first!"),
            Object::string_literal("duplicate-last!!"),
        ],
    );
    let encryption = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &encrypted,
        owner_password: "synthetic-owner",
        user_password: "synthetic-user",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    encrypted.encrypt(&encryption).unwrap();
    check(PdfDocument::from_bytes(&bytes(encrypted), Some("synthetic-user")).unwrap());
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn duplicated_pages_match_independent_poppler_pixels_text_geometry_and_qpdf_structure() {
    use std::process::Command;
    let references = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap_or_else(|| "qpdf".into());
    let version = Command::new(references.join("pdfinfo"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stderr).contains("26.10.0"));
    let version = Command::new(&qpdf).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("12.4.2"));
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.pdf");
    let output = directory.path().join("duplicated.pdf");
    let mut doc = load(fixture());
    doc.save_new(&original).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    doc.duplicate_pages(&[3, 1]).unwrap();
    doc.save_new(&output).unwrap();
    assert!(
        Command::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    let info = Command::new(references.join("pdfinfo"))
        .args(["-f", "1", "-l", "5", "-box"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(info.status.success());
    let info = String::from_utf8(info.stdout).unwrap();
    assert!(
        info.lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>() == vec!["Pages:", "5"])
    );
    for (index, (original_page, new_page)) in [(1, 2), (3, 5)].into_iter().enumerate() {
        let render = |path: &std::path::Path, page: u32, label: &str| {
            let prefix = directory.path().join(format!("{label}-{index}"));
            let rendered = Command::new(references.join("pdftoppm"))
                .args([
                    "-f",
                    &page.to_string(),
                    "-l",
                    &page.to_string(),
                    "-r",
                    "72",
                    "-singlefile",
                    "-png",
                ])
                .arg(path)
                .arg(&prefix)
                .output()
                .unwrap();
            assert!(
                rendered.status.success(),
                "{}",
                String::from_utf8_lossy(&rendered.stderr)
            );
            std::fs::read(prefix.with_extension("png")).unwrap()
        };
        assert!(
            render(&original, original_page, "original") == render(&output, new_page, "copy"),
            "independent original/copy PNGs differ"
        );
        let text = |path: &std::path::Path, page: u32| {
            let text = Command::new(references.join("pdftotext"))
                .args(["-f", &page.to_string(), "-l", &page.to_string()])
                .arg(path)
                .arg("-")
                .output()
                .unwrap();
            assert!(text.status.success());
            text.stdout
        };
        assert_eq!(text(&original, original_page), text(&output, new_page));
        let geometry = |path: &std::path::Path, page: u32| {
            let info = Command::new(references.join("pdfinfo"))
                .args(["-f", &page.to_string(), "-l", &page.to_string(), "-box"])
                .arg(path)
                .output()
                .unwrap();
            assert!(info.status.success());
            String::from_utf8(info.stdout)
                .unwrap()
                .lines()
                .filter(|line| {
                    line.contains("MediaBox:") || line.contains("CropBox:") || line.contains("rot:")
                })
                .map(|line| {
                    line.split_whitespace()
                        .skip(2)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            geometry(&original, original_page),
            geometry(&output, new_page)
        );
    }
    assert!(std::fs::read(original).unwrap() == original_bytes);
}
