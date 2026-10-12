#![allow(clippy::unwrap_used)]

use lopdf::{DecompressError, Document, Object, Stream, dictionary};
use pdf_core::{Error, MAX_STREAM_BYTES, PdfDocument};
use std::sync::OnceLock;

fn fixture() -> Document {
    let mut doc = Document::with_version("1.7");
    let root = doc.new_object_id();
    let branch = doc.new_object_id();
    let font = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for number in 1..=2 {
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
        dictionary! { "Type" => "Pages", "Parent" => root, "Kids" => kids, "Count" => 2 }.into(),
    );
    doc.objects.insert(root, dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(branch)],
        "Count" => 2, "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()], "Resources" => resources }.into());
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    doc.trailer.set("Root", catalog);
    doc
}

fn bytes(mut doc: Document) -> Vec<u8> {
    let mut result = Vec::new();
    doc.save_to(&mut result).unwrap();
    result
}

fn root_and_branch(doc: &Document) -> (lopdf::ObjectId, lopdf::ObjectId) {
    let root = doc
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    let branch = doc
        .get_dictionary(root)
        .unwrap()
        .get(b"Kids")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    (root, branch)
}

fn bomb() -> Stream {
    static COMPRESSED: OnceLock<Stream> = OnceLock::new();
    COMPRESSED
        .get_or_init(|| {
            let mut stream = Stream::new(dictionary! {}, vec![b' '; MAX_STREAM_BYTES + 1]);
            stream.compress().unwrap();
            assert!(stream.content.len() < 128 * 1024);
            stream
        })
        .clone()
}

fn is_decompression_limit(error: Error) -> bool {
    matches!(error, Error::Pdf(lopdf::Error::Decompress(DecompressError::MemoryLimitExceeded { limit }))
        if limit == MAX_STREAM_BYTES)
}

#[test]
fn every_nested_page_count_must_agree_with_descendant_leaves() {
    for count in [-1, 0, 1, 3] {
        let mut doc = fixture();
        let (_, branch) = root_and_branch(&doc);
        doc.get_dictionary_mut(branch).unwrap().set("Count", count);
        assert!(matches!(
            PdfDocument::from_bytes(&bytes(doc), None),
            Err(Error::Invalid(_))
        ));
    }
    let valid = PdfDocument::from_bytes(&bytes(fixture()), None).unwrap();
    assert_eq!(valid.page_count(), 2);
    assert!(valid.extract_text(&[2]).unwrap().contains("Page 2"));
}

#[test]
fn root_parent_shared_nodes_and_cycles_are_rejected() {
    let mut doc = fixture();
    let (root, _) = root_and_branch(&doc);
    doc.get_dictionary_mut(root).unwrap().set("Parent", root);
    assert!(matches!(
        PdfDocument::from_bytes(&bytes(doc), None),
        Err(Error::Invalid(_))
    ));
    let mut doc = fixture();
    let (root, branch) = root_and_branch(&doc);
    doc.get_dictionary_mut(root).unwrap().set(
        "Kids",
        vec![Object::Reference(branch), Object::Reference(branch)],
    );
    assert!(PdfDocument::from_bytes(&bytes(doc), None).is_err());
    let mut doc = fixture();
    let (root, branch) = root_and_branch(&doc);
    doc.get_dictionary_mut(branch)
        .unwrap()
        .set("Kids", vec![Object::Reference(root)]);
    assert!(PdfDocument::from_bytes(&bytes(doc), None).is_err());
}

#[test]
fn nested_signature_dictionaries_are_counted_once_and_block_rewriting() {
    let mut source = fixture();
    source.catalog_mut().unwrap().set(
        "AcroForm",
        dictionary! {
            "Fields" => vec![Object::Dictionary(dictionary! { "FT" => "Sig", "Type" => "Sig" })]
        },
    );
    let indirect_name = source.add_object(Object::Name(b"Sig".to_vec()));
    source.add_object(Object::Array(vec![Object::Dictionary(
        dictionary! { "FT" => indirect_name },
    )]));
    source.add_object(Stream::new(
        dictionary! { "Nested" => dictionary! { "Type" => "Sig" } },
        Vec::new(),
    ));
    let mut doc = PdfDocument::from_bytes(&bytes(source), None).unwrap();
    assert_eq!(doc.info().unwrap().signature_fields, 3);
    assert!(matches!(doc.rotate(&[1], 90), Err(Error::Refused(_))));
    assert!(matches!(doc.reorder(&[2, 1]), Err(Error::Refused(_))));
    assert!(matches!(doc.to_bytes(), Err(Error::Refused(_))));
}

#[test]
fn indirect_signature_name_cycles_fail_with_a_controlled_error() {
    let mut source = fixture();
    let first = source.new_object_id();
    let second = source.new_object_id();
    source.objects.insert(first, Object::Reference(second));
    source.objects.insert(second, Object::Reference(first));
    source.add_object(dictionary! { "FT" => first });
    assert!(PdfDocument::from_bytes(&bytes(source), None).is_err());
}

#[test]
fn excessive_direct_nesting_is_rejected_by_the_parser() {
    let mut source = fixture();
    let mut nested = Object::Null;
    for _ in 0..150 {
        nested = Object::Array(vec![nested]);
    }
    source.add_object(nested);
    assert!(PdfDocument::from_bytes(&bytes(source), None).is_err());
}

#[test]
fn total_structural_values_are_bounded() {
    let mut source = fixture();
    source.add_object(Object::Array(vec![Object::Null; 2_000_000]));
    assert!(matches!(
        PdfDocument::from_bytes(&bytes(source), None),
        Err(Error::Limit(_))
    ));
}

#[test]
fn compressed_content_and_tounicode_obey_extraction_limit() {
    let mut source = fixture();
    let first = *source.get_pages().get(&1).unwrap();
    let content = source.get_page_contents(first)[0];
    source.objects.insert(content, Object::Stream(bomb()));
    let doc = PdfDocument::from_bytes(&bytes(source), None).unwrap();
    assert!(is_decompression_limit(doc.extract_text(&[1]).unwrap_err()));

    let mut source = fixture();
    let unicode = source.add_object(bomb());
    let font = source
        .objects
        .iter()
        .find_map(|(id, object)| {
            object
                .as_dict()
                .ok()
                .filter(|dict| dict.has_type(b"Font"))
                .map(|_| *id)
        })
        .unwrap();
    source
        .get_dictionary_mut(font)
        .unwrap()
        .set("ToUnicode", unicode);
    let doc = PdfDocument::from_bytes(&bytes(source), None).unwrap();
    assert!(is_decompression_limit(doc.extract_text(&[1]).unwrap_err()));
}

#[test]
fn compressed_object_stream_obeys_load_limit() {
    let source = fixture();
    let root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let mut input = bytes(source);
    let previous = Document::load_mem(&input).unwrap().xref_start;
    let offset = input.len();
    let compressed = bomb().content;
    // lopdf's normal writer omits old ObjStm objects; append a real stream and xref entry.
    input.extend_from_slice(format!(
        "999 0 obj\n<< /Type /ObjStm /N 1 /First 0 /Length {} /Filter /FlateDecode >>\nstream\n",
        compressed.len()).as_bytes());
    input.extend_from_slice(&compressed);
    input.extend_from_slice(b"\nendstream\nendobj\n");
    let xref = input.len();
    input.extend_from_slice(format!(
        "xref\n999 1\n{offset:010} 00000 n \ntrailer\n<< /Size 1000 /Root {} {} R /Prev {previous} >>\nstartxref\n{xref}\n%%EOF\n",
        root.0, root.1).as_bytes());
    let result = PdfDocument::from_bytes(&input, None);
    assert!(is_decompression_limit(result.err().unwrap()));
}

#[test]
fn compressed_xref_stream_obeys_load_limit() {
    let source = fixture();
    let root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let mut input = bytes(source);
    let offset = input.len();
    let compressed = bomb().content;
    input.extend_from_slice(format!(
        "999 0 obj\n<< /Type /XRef /Size 1000 /W [1 4 2] /Root {} {} R /Length {} /Filter /FlateDecode >>\nstream\n",
        root.0, root.1, compressed.len()).as_bytes());
    input.extend_from_slice(&compressed);
    input
        .extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{offset}\n%%EOF\n").as_bytes());
    let result = PdfDocument::from_bytes(&input, None);
    assert!(is_decompression_limit(result.err().unwrap()));
}

#[test]
fn sibling_subtree_counts_are_checked_against_their_own_leaves() {
    let mut source = fixture();
    let (root, first_branch) = root_and_branch(&source);
    let sibling = source.new_object_id();
    let content = source.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 30 Td (Sibling) Tj ET".to_vec(),
    ));
    let page = source
        .add_object(dictionary! { "Type" => "Page", "Parent" => sibling, "Contents" => content });
    source.objects.insert(
        sibling,
        dictionary! { "Type" => "Pages", "Parent" => root,
        "Kids" => vec![Object::Reference(page)], "Count" => 1 }
        .into(),
    );
    source.get_dictionary_mut(root).unwrap().set(
        "Kids",
        vec![Object::Reference(first_branch), Object::Reference(sibling)],
    );
    source.get_dictionary_mut(root).unwrap().set("Count", 3);
    let valid = PdfDocument::from_bytes(&bytes(source.clone()), None).unwrap();
    assert_eq!(valid.page_count(), 3);
    assert!(valid.extract_text(&[3]).unwrap().contains("Sibling"));
    source.get_dictionary_mut(sibling).unwrap().set("Count", 3);
    assert!(matches!(
        PdfDocument::from_bytes(&bytes(source), None),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn reorder_preserves_shared_resource_references_instead_of_copying_dictionaries() {
    let mut source = fixture();
    let (root, _) = root_and_branch(&source);
    let resources = source
        .get_dictionary(root)
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
        .get_dictionary_mut(resources)
        .unwrap()
        .set("Font", fonts);
    let mut doc = PdfDocument::from_bytes(&bytes(source), None).unwrap();
    doc.reverse().unwrap();
    let output = Document::load_mem(&doc.to_bytes().unwrap()).unwrap();
    for page in output.get_pages().values() {
        assert_eq!(
            output
                .get_dictionary(*page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_reference()
                .unwrap(),
            resources
        );
    }
    assert!(doc.extract_text(&[1]).unwrap().contains("Page 2"));
}
