#![allow(clippy::unwrap_used)]

use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use pdf_core::{Error, PdfDocument};

fn fixture(text: &str, font: &str, color: [u8; 3], rotation: i64) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => font });
    let image = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Image",
        "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8 },
        color.to_vec(),
    ));
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 20 Tf 30 220 Td ({text}) Tj ET q 50 0 0 50 30 120 cm /Im1 Do Q")
            .into_bytes(),
    ));
    let page =
        doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content });
    doc.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)],
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 300.into()], "Rotate" => rotation,
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font }, "XObject" => dictionary! { "Im1" => image } } }.into());
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages,
    "Info" => dictionary! { "Producer" => Object::string_literal("synthetic-producer") } });
    doc.trailer.set("Root", catalog);
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal("synthetic-first!"),
            Object::string_literal("synthetic-next!!"),
        ],
    );
    doc
}

fn bytes(mut doc: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn load(doc: Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(doc), None).unwrap()
}

fn left() -> Document {
    fixture("ALPHA", "Helvetica", [255, 0, 0], 0)
}
fn right() -> Document {
    fixture("BETA", "Times-Roman", [0, 0, 255], 90)
}

#[test]
fn colliding_ids_fonts_images_and_orphan_cycles_remap_in_both_namespaces() {
    let mut source = right();
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
    let orphan = source.new_object_id();
    let peer = source.new_object_id();
    source.objects.insert(orphan, dictionary! { "Witness" => Object::string_literal("incoming-orphan"), "Font" => font, "Peer" => peer }.into());
    source
        .objects
        .insert(peer, dictionary! { "Peer" => orphan }.into());
    let incoming = load(source);
    let incoming_before = incoming.to_bytes().unwrap();
    let mut merged = load(left());
    merged.append(&incoming).unwrap();
    assert_eq!(merged.page_count(), 2);
    assert!(merged.extract_text(&[1]).unwrap().contains("ALPHA"));
    assert!(merged.extract_text(&[2]).unwrap().contains("BETA"));
    assert_eq!(merged.info().unwrap().pages[1].rotation, 90);
    let output = Document::load_mem(&merged.to_bytes().unwrap()).unwrap();
    let (orphan_id, witness) = output
        .objects
        .iter()
        .find_map(|(id, object)| {
            let dict = object.as_dict().ok()?;
            (dict.get(b"Witness").ok()?.as_str().ok()? == b"incoming-orphan").then_some((*id, dict))
        })
        .unwrap();
    let imported_font = witness.get(b"Font").unwrap().as_reference().unwrap();
    assert_eq!(
        output
            .get_dictionary(imported_font)
            .unwrap()
            .get(b"BaseFont")
            .unwrap()
            .as_name()
            .unwrap(),
        b"Times-Roman"
    );
    let peer = witness.get(b"Peer").unwrap().as_reference().unwrap();
    assert_eq!(
        output
            .get_dictionary(peer)
            .unwrap()
            .get(b"Peer")
            .unwrap()
            .as_reference()
            .unwrap(),
        orphan_id
    );
    let image_colors = output
        .objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|stream| {
            stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")
        })
        .map(|stream| stream.content.clone())
        .collect::<Vec<_>>();
    assert!(image_colors.contains(&vec![255, 0, 0]) && image_colors.contains(&vec![0, 0, 255]));
    assert!(!output.trailer.has(b"ID") && !output.trailer.has(b"Prev"));
    assert_eq!(
        output
            .objects
            .values()
            .filter(|object| object.as_dict().is_ok_and(|dict| dict.has_type(b"Catalog")))
            .count(),
        2
    );
    assert_eq!(incoming.to_bytes().unwrap(), incoming_before);
    let directory = tempfile::tempdir().unwrap();
    merged
        .save_new(directory.path().join("merged.pdf"))
        .unwrap();
}

#[test]
fn matching_string_metadata_and_effective_versions_are_preserved() {
    let mut primary = left();
    let info =
        primary.add_object(dictionary! { "Author" => Object::string_literal("fixture-author") });
    primary.trailer.set("Info", info);
    primary
        .catalog_mut()
        .unwrap()
        .set("Version", Object::Name(b"1.5".to_vec()));
    let mut incoming = right();
    let info =
        incoming.add_object(dictionary! { "Author" => Object::string_literal("fixture-author") });
    incoming.trailer.set("Info", info);
    incoming
        .catalog_mut()
        .unwrap()
        .set("Version", Object::Name(b"2.0".to_vec()));
    let mut output = load(primary);
    output.append(&load(incoming)).unwrap();
    assert_eq!(output.info().unwrap().version, "2.0");
    let parsed = Document::load_mem(&output.to_bytes().unwrap()).unwrap();
    assert_eq!(
        parsed
            .catalog()
            .unwrap()
            .get(b"Version")
            .unwrap()
            .as_name()
            .unwrap(),
        b"2.0"
    );
    let info = parsed.trailer.get(b"Info").unwrap().as_reference().unwrap();
    assert_eq!(
        parsed
            .get_dictionary(info)
            .unwrap()
            .get(b"Author")
            .unwrap()
            .as_str()
            .unwrap(),
        b"fixture-author"
    );
}

#[test]
fn unsupported_catalogs_metadata_trailers_and_page_structures_roll_back() {
    for key in [
        "Outlines",
        "AcroForm",
        "Names",
        "PageLabels",
        "Metadata",
        "OCProperties",
        "StructTreeRoot",
        "OpenAction",
        "OutputIntents",
    ] {
        let mut incoming = right();
        incoming.catalog_mut().unwrap().set(key, dictionary! {});
        assert_unchanged_refusal(load(incoming));
    }
    for key in ["Annots", "AA", "StructParents", "TemplateInstantiated"] {
        let mut incoming = right();
        let page = *incoming.get_pages().get(&1).unwrap();
        incoming
            .get_dictionary_mut(page)
            .unwrap()
            .set(key, dictionary! {});
        assert_unchanged_refusal(load(incoming));
    }
    let mut incoming = right();
    incoming.catalog_mut().unwrap().set(
        "Info",
        dictionary! { "Producer" => Object::string_literal("different") },
    );
    assert_unchanged_refusal(load(incoming));
    let mut incoming = right();
    incoming.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    incoming.trailer.set(
        "CustomMeaning",
        Object::string_literal("must-not-be-discarded"),
    );
    let input = load(incoming);
    assert!(
        Document::load_mem(&input.to_bytes().unwrap())
            .unwrap()
            .trailer
            .has(b"CustomMeaning")
    );
    assert_unchanged_refusal(input);
    let mut incoming = right();
    // The ordinary writer omits true linearization carriers; retain the marker for this gate fixture.
    incoming.add_object(dictionary! { "Type" => "UnsupportedHint", "Linearized" => 1 });
    assert_unchanged_refusal(load(incoming));
    let mut incoming = right();
    incoming
        .catalog_mut()
        .unwrap()
        .set("Version", Object::Name(b"2.1".to_vec()));
    assert_unchanged_refusal(load(incoming));
}

fn assert_unchanged_refusal(incoming: PdfDocument) {
    let mut primary = load(left());
    let before = primary.to_bytes().unwrap();
    let result = primary.append(&incoming);
    assert!(matches!(result, Err(Error::Refused(_))), "{result:?}");
    assert_eq!(primary.to_bytes().unwrap(), before);
}

#[test]
fn dangling_orphan_references_refuse_instead_of_colliding_with_another_document() {
    let mut source = right();
    source.add_object(dictionary! { "OrphanTarget" => Object::Reference((99999, 0)) });
    assert_unchanged_refusal(load(source));
}

#[test]
fn signed_and_encrypted_inputs_are_read_only_even_for_append() {
    let mut source = right();
    source.add_object(dictionary! { "FT" => "Sig" });
    assert_unchanged_refusal(load(source));
    let mut source = right();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &source,
        owner_password: "owner",
        user_password: "user",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    let encrypted = PdfDocument::from_bytes(&bytes(source), Some("user")).unwrap();
    assert_unchanged_refusal(encrypted);
}

#[test]
fn direct_shared_resources_and_nonascending_page_order_do_not_amplify() {
    let mut source = left();
    let root = source
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    let font = source
        .get_dictionary(root)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
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
        .get_dictionary_mut(root)
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Font", fonts);
    let original = *source.get_pages().get(&1).unwrap();
    let mut kids = vec![Object::Reference(original)];
    for number in 2..=100 {
        let content = source.add_object(Stream::new(
            dictionary! {},
            format!("BT /F2047 12 Tf 30 220 Td (Page {number}) Tj ET").into_bytes(),
        ));
        let page = source
            .add_object(dictionary! { "Type" => "Page", "Parent" => root, "Contents" => content });
        kids.push(Object::Reference(page));
    }
    kids.reverse();
    source.get_dictionary_mut(root).unwrap().set("Kids", kids);
    source.get_dictionary_mut(root).unwrap().set("Count", 100);
    let mut merged = load(source);
    let before = merged.to_bytes().unwrap().len();
    merged.append(&load(right())).unwrap();
    assert!(merged.extract_text(&[1]).unwrap().contains("Page 100"));
    assert!(merged.extract_text(&[100]).unwrap().contains("ALPHA"));
    assert_eq!(merged.page_count(), 101);
    assert!(merged.to_bytes().unwrap().len() < before + 20_000);
}

#[test]
fn aggregate_payload_is_refused_before_cloning_or_publishing() {
    let mut first = left();
    first.add_object(Object::string_literal(vec![b'a'; 65 * 1024 * 1024]));
    let mut second = right();
    second.add_object(Object::string_literal(vec![b'b'; 65 * 1024 * 1024]));
    let mut first = load(first);
    let second = load(second);
    let before = first.info().unwrap();
    assert!(matches!(first.append(&second), Err(Error::Limit(_))));
    assert_eq!(first.info().unwrap().object_count, before.object_count);
    assert_eq!(first.page_count(), 1);
    assert!(first.extract_text(&[1]).unwrap().contains("ALPHA"));
}

#[test]
fn object_streams_and_logical_xref_references_are_explicitly_refused() {
    let source = right();
    let root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let mut input = bytes(source);
    let previous = Document::load_mem(&input).unwrap().xref_start;
    let offset = input.len();
    input.extend_from_slice(
        b"9000 0 obj\n<< /Type /ObjStm /N 0 /First 0 /Length 0 >>\nstream\n\nendstream\nendobj\n",
    );
    let xref = input.len();
    input.extend_from_slice(format!("xref\n9000 1\n{offset:010} 00000 n \ntrailer\n<< /Size 9001 /Root {} {} R /Prev {previous} >>\nstartxref\n{xref}\n%%EOF\n", root.0, root.1).as_bytes());
    assert!(
        Document::load_mem(&input)
            .unwrap()
            .objects
            .contains_key(&(9000, 0))
    );
    assert_unchanged_refusal(PdfDocument::from_bytes(&input, None).unwrap());

    let mut source = right();
    let witness = source.add_object(dictionary! {});
    let next_xref = source.max_id + 1;
    source
        .get_dictionary_mut(witness)
        .unwrap()
        .set("Carrier", Object::Reference((next_xref, 0)));
    assert_unchanged_refusal(load(source));
}

#[test]
fn added_page_tree_level_overflow_rolls_back_after_candidate_construction() {
    let mut source = left();
    let root = source
        .catalog()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    let page = *source.get_pages().get(&1).unwrap();
    let mut parent = root;
    for _ in 0..127 {
        let child =
            source.add_object(dictionary! { "Type" => "Pages", "Parent" => parent, "Count" => 1 });
        source
            .get_dictionary_mut(parent)
            .unwrap()
            .set("Kids", vec![Object::Reference(child)]);
        parent = child;
    }
    source
        .get_dictionary_mut(parent)
        .unwrap()
        .set("Kids", vec![Object::Reference(page)]);
    source
        .get_dictionary_mut(page)
        .unwrap()
        .set("Parent", parent);
    let mut primary = load(source);
    let before = primary.to_bytes().unwrap();
    assert!(matches!(
        primary.append(&load(right())),
        Err(Error::Limit(_))
    ));
    assert_eq!(primary.to_bytes().unwrap(), before);
}

#[test]
fn aggregate_page_budget_is_checked_before_import() {
    let expand = |mut source: Document| {
        let root = source
            .catalog()
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        let mut kids = source
            .get_dictionary(root)
            .unwrap()
            .get(b"Kids")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        for _ in 1..10_001 {
            let page = source.add_object(dictionary! { "Type" => "Page", "Parent" => root });
            kids.push(Object::Reference(page));
        }
        source.get_dictionary_mut(root).unwrap().set("Kids", kids);
        source
            .get_dictionary_mut(root)
            .unwrap()
            .set("Count", 10_001);
        load(source)
    };
    let mut first = expand(left());
    let second = expand(right());
    assert!(matches!(
        first.append(&second),
        Err(Error::Limit("merged document exceeds 20000 pages"))
    ));
    assert_eq!(first.page_count(), 10_001);
}

#[test]
#[ignore = "independent dev gate requires Poppler CLI tools; run with --include-ignored"]
fn independent_poppler_verifies_text_images_rotation_and_geometry() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("a.pdf");
    let second = directory.path().join("b.pdf");
    let output = directory.path().join("merged.pdf");
    std::fs::write(&first, bytes(left())).unwrap();
    std::fs::write(&second, bytes(right())).unwrap();
    let mut document = PdfDocument::open(&first, None).unwrap();
    document
        .append(&PdfDocument::open(&second, None).unwrap())
        .unwrap();
    document.save_new(&output).unwrap();
    for (index, source) in [&first, &second].into_iter().enumerate() {
        let page = (index + 1).to_string();
        let text = |path: &std::path::Path, page: &str| {
            reader(
                std::process::Command::new("pdftotext")
                    .args(["-f", page, "-l", page, "-nopgbrk"])
                    .arg(path)
                    .arg("-"),
            )
        };
        assert_eq!(text(source, "1"), text(&output, &page));
        let geometry = |path: &std::path::Path, page: &str| {
            let report = reader(
                std::process::Command::new("pdfinfo")
                    .args(["-f", page, "-l", page, "-box"])
                    .arg(path),
            );
            String::from_utf8(report)
                .unwrap()
                .lines()
                .filter(|line| {
                    line.trim_start().starts_with("Page")
                        && (line.contains(" size:")
                            || line.contains(" rot:")
                            || line.contains("Box:"))
                })
                .map(|line| line.split_once(':').unwrap().1.trim().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(geometry(source, "1"), geometry(&output, &page));
        let prior = directory.path().join(format!("prior-{index}"));
        let after = directory.path().join(format!("after-{index}"));
        reader(
            std::process::Command::new("pdftoppm")
                .args(["-f", "1", "-l", "1", "-singlefile", "-r", "72"])
                .arg(source)
                .arg(&prior),
        );
        reader(
            std::process::Command::new("pdftoppm")
                .args(["-f", &page, "-l", &page, "-singlefile", "-r", "72"])
                .arg(&output)
                .arg(&after),
        );
        assert_eq!(
            std::fs::read(prior.with_extension("ppm")).unwrap(),
            std::fs::read(after.with_extension("ppm")).unwrap()
        );
    }
}

fn reader(command: &mut std::process::Command) -> Vec<u8> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
