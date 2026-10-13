#![allow(clippy::unwrap_used, clippy::expect_used)]
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use pdf_core::{Error, PdfDocument};

const SENTINEL: &str = "PRIVATE_SANITIZE_SENTINEL_7f5e";
struct Fixture {
    document: Document,
    page: ObjectId,
    contents: ObjectId,
    root: ObjectId,
    info: ObjectId,
    attachment: ObjectId,
    appearance: ObjectId,
}
fn make_fixture() -> Fixture {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let font = document
        .add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let image = document.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>2,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![255,0,0,0,255,0,0,0,255,255,255,0]));
    let contents = document.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 100 Td (Public page text) Tj ET\nq 20 0 0 20 60 10 cm /Im1 Do Q\n"
            .to_vec(),
    ));
    let page = document.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font},"XObject"=>dictionary! {"Im1"=>image}},"Contents"=>contents});
    document.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let embedded = document.add_object(Stream::new(
        dictionary! {"Type"=>"EmbeddedFile"},
        SENTINEL.as_bytes().to_vec(),
    ));
    let filespec = document.add_object(dictionary! {"Type"=>"Filespec","F"=>Object::string_literal(SENTINEL),"EF"=>dictionary! {"F"=>embedded}});
    let names = document
        .add_object(dictionary! {"Names"=>vec![Object::string_literal(SENTINEL),filespec.into()]});
    let xmp = document.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        SENTINEL.as_bytes().to_vec(),
    ));
    let info = document.add_object(dictionary! {"Title"=>Object::string_literal(SENTINEL)});
    let js = document.add_object(
        dictionary! {"Type"=>"Action","S"=>"JavaScript","JS"=>Object::string_literal(SENTINEL)},
    );
    let appearance = document.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()],"Resources"=>dictionary! {}}, b"q 1 0 0 rg 0 0 20 20 re f Q\n".to_vec()));
    let attachment = document.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"FileAttachment","Rect"=>vec![140.into(),140.into(),160.into(),160.into()],"P"=>page,"FS"=>filespec,"Contents"=>Object::string_literal(SENTINEL),"AP"=>dictionary! {"N"=>appearance}});
    let ordinary = dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),140.into(),40.into(),160.into()],"Contents"=>Object::string_literal("Public note"),"A"=>js};
    let annots = document.add_object(vec![attachment.into(), ordinary.into()]);
    document
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", annots);
    let root = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"Metadata"=>xmp,"AF"=>vec![filespec.into()],"Names"=>dictionary! {"EmbeddedFiles"=>names,"JavaScript"=>dictionary! {"Names"=>vec![Object::string_literal(SENTINEL),js.into()]}},"OpenAction"=>js});
    document.trailer.set("Root", root);
    document.trailer.set("Info", info);
    Fixture {
        document,
        page,
        contents,
        root,
        info,
        attachment,
        appearance,
    }
}
fn bytes(mut document: Document) -> Vec<u8> {
    let mut result = Vec::new();
    document.save_to(&mut result).unwrap();
    result
}
fn load(document: &Document) -> PdfDocument {
    PdfDocument::from_bytes(&bytes(document.clone()), None).unwrap()
}

#[test]
fn full_rewrite_removes_private_graph_preserves_page_and_direct_note() {
    let fixture = make_fixture();
    let source = load(&fixture.document);
    let original = source.verified_bytes().unwrap();
    let (clean, report) = source.sanitized_copy().unwrap();
    assert!(report.full_rewrite);
    assert_eq!(report.attachment_annotations_removed.len(), 1);
    assert_eq!(
        report.attachment_annotations_removed[0].rectangle,
        [140., 140., 160., 160.]
    );
    assert!(report.attachment_annotations_removed[0].visible);
    let result = clean.verified_bytes().unwrap();
    assert!(
        !result
            .windows(SENTINEL.len())
            .any(|value| value == SENTINEL.as_bytes())
    );
    let parsed = Document::load_mem(&result).unwrap();
    assert_eq!(
        parsed.get_object(fixture.contents).unwrap(),
        fixture.document.get_object(fixture.contents).unwrap()
    );
    let page = parsed.get_dictionary(fixture.page).unwrap();
    for key in [b"Resources".as_slice(), b"MediaBox", b"Contents"] {
        assert_eq!(
            page.get(key).unwrap(),
            fixture
                .document
                .get_dictionary(fixture.page)
                .unwrap()
                .get(key)
                .unwrap()
        );
    }
    let image = fixture
        .document
        .get_dictionary(fixture.page)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Im1")
        .unwrap()
        .as_reference()
        .unwrap();
    assert_eq!(
        parsed.objects.get(&image),
        fixture.document.objects.get(&image)
    );
    let annots = page.get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annots.len(), 1);
    let note = annots[0].as_dict().unwrap();
    assert_eq!(
        note.get(b"Contents").unwrap().as_str().unwrap(),
        b"Public note"
    );
    assert!(!note.has(b"A"));
    for id in [fixture.info, fixture.attachment, fixture.appearance] {
        assert!(!parsed.objects.contains_key(&id));
    }
    assert!(!parsed.trailer.has(b"Info"));
    assert_eq!(source.verified_bytes().unwrap(), original);
    assert_eq!(
        source.extract_text(&[1]).unwrap().trim(),
        clean.extract_text(&[1]).unwrap().trim()
    );
}

#[test]
fn unknown_aliases_refuse_without_mutating_source() {
    for target in [0, 1, 2] {
        let mut fixture = make_fixture();
        let id = match target {
            0 => fixture.info,
            1 => fixture.attachment,
            _ => fixture.appearance,
        };
        let alias = fixture.document.add_object(Object::Reference(id));
        fixture
            .document
            .get_dictionary_mut(fixture.root)
            .unwrap()
            .set("CustomAlias", alias);
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
}

#[test]
fn decoder_parameter_content_aliases_cannot_bypass_private_target_refusal() {
    let mut fixture = make_fixture();
    let globals = fixture
        .document
        .add_object(Stream::new(dictionary! {}, SENTINEL.as_bytes().to_vec()));
    let parameters = fixture
        .document
        .add_object(dictionary! {"JBIG2Globals"=>globals});
    let embedded = *fixture
        .document
        .objects
        .iter()
        .find(|(_, object)| matches!(object.type_name(), Ok(b"EmbeddedFile")))
        .unwrap()
        .0;
    let stream = fixture
        .document
        .get_object_mut(embedded)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.compress().unwrap();
    stream.dict.set("DecodeParms", parameters);
    fixture
        .document
        .get_dictionary_mut(fixture.root)
        .unwrap()
        .set("UnrelatedGlobalsAlias", globals);
    let source = load(&fixture.document);
    let before = source.verified_bytes().unwrap();
    assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
    assert_eq!(source.verified_bytes().unwrap(), before);
}

#[test]
fn duplicated_custom_strings_remain_and_report_does_not_claim_privacy_absence() {
    let mut fixture = make_fixture();
    fixture
        .document
        .get_dictionary_mut(fixture.root)
        .unwrap()
        .set("UnrelatedCustomString", Object::string_literal(SENTINEL));
    let (clean, report) = load(&fixture.document).sanitized_copy().unwrap();
    let output = clean.verified_bytes().unwrap();
    assert!(
        output
            .windows(SENTINEL.len())
            .any(|value| value == SENTINEL.as_bytes())
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limit| limit.contains("duplicated strings"))
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limit| limit.contains("not secure text/image redaction"))
    );
}

#[test]
fn linked_or_unprovable_visible_attachments_refuse() {
    for (key, value) in [
        ("StructParent", Object::Integer(0)),
        ("Popup", Object::Null),
        ("F", Object::Integer(8)),
        ("F", Object::Integer(128)),
    ] {
        let mut fixture = make_fixture();
        fixture
            .document
            .get_dictionary_mut(fixture.attachment)
            .unwrap()
            .set(key, value);
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
}

#[test]
fn hidden_attachment_and_nested_duplicate_name_links_are_removed() {
    let mut fixture = make_fixture();
    let annotation = fixture
        .document
        .get_dictionary_mut(fixture.attachment)
        .unwrap();
    annotation.set("F", 2);
    annotation.remove(b"AP");
    let catalog = fixture.document.get_dictionary(fixture.root).unwrap();
    let files = catalog.get(b"AF").unwrap().as_array().unwrap()[0].clone();
    let leaf = fixture.document.add_object(dictionary! {"Names"=>vec![Object::string_literal(SENTINEL),files.clone(),Object::string_literal("duplicate.pdf"),files],"Limits"=>vec![Object::string_literal(SENTINEL),Object::string_literal("duplicate.pdf")]});
    let tree = fixture
        .document
        .add_object(dictionary! {"Kids"=>vec![leaf.into()]});
    fixture
        .document
        .get_dictionary_mut(fixture.root)
        .unwrap()
        .get_mut(b"Names")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("EmbeddedFiles", tree);
    let source = load(&fixture.document);
    let (clean, report) = source.sanitized_copy().unwrap();
    assert!(!report.attachment_annotations_removed[0].visible);
    assert_eq!(report.embedded_file_name_trees_removed, 1);
    let result = clean.verified_bytes().unwrap();
    assert!(
        !result
            .windows(SENTINEL.len())
            .any(|value| value == SENTINEL.as_bytes())
    );
    let document = Document::load_mem(&result).unwrap();
    // A writer may reuse a freed highest ID for a fresh xref carrier.
    assert_ne!(
        document.objects.get(&leaf),
        fixture.document.objects.get(&leaf)
    );
    assert_ne!(
        document.objects.get(&tree),
        fixture.document.objects.get(&tree)
    );
}

#[test]
fn reserved_resource_names_are_preserved_and_local_navigation_does_not_target_pages() {
    let mut fixture = make_fixture();
    let resource = fixture
        .document
        .get_dictionary_mut(fixture.page)
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Font")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    let font = resource.get(b"F1").unwrap().clone();
    for name in ["Metadata", "AA", "JavaScript", "A"] {
        resource.set(name, font.clone());
    }
    let action = fixture.document.add_object(
        dictionary! {"S"=>"GoTo","D"=>vec![fixture.page.into(),Object::Name(b"Fit".to_vec())]},
    );
    fixture
        .document
        .get_dictionary_mut(fixture.root)
        .unwrap()
        .set("OpenAction", action);
    let source = load(&fixture.document);
    let (clean, _) = source.sanitized_copy().unwrap();
    let document = Document::load_mem(&clean.verified_bytes().unwrap()).unwrap();
    assert_eq!(
        document
            .get_dictionary(fixture.page)
            .unwrap()
            .get(b"Resources")
            .unwrap(),
        fixture
            .document
            .get_dictionary(fixture.page)
            .unwrap()
            .get(b"Resources")
            .unwrap()
    );
}

#[test]
fn conflicting_catalog_resource_namespace_cannot_hide_direct_info_metadata() {
    let mut fixture = make_fixture();
    let root = fixture.document.get_dictionary_mut(fixture.root).unwrap();
    for key in [b"Metadata".as_slice(), b"AF", b"Names", b"OpenAction"] {
        root.remove(key);
    }
    root.set(
        "Info",
        dictionary! {"Title"=>Object::string_literal(SENTINEL)},
    );
    fixture.document.trailer.remove(b"Info");
    fixture
        .document
        .get_dictionary_mut(fixture.page)
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Font", fixture.root);
    let source = load(&fixture.document);
    let before = source.verified_bytes().unwrap();
    assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
    assert_eq!(source.verified_bytes().unwrap(), before);
}

#[test]
fn unsupported_inline_payloads_and_security_structures_refuse_atomically() {
    for contents in [
        b"/Span << /AF [(private)] >> BDC EMC".as_slice(),
        b"/Span << /Custom 99 0 R >> BDC EMC",
    ] {
        let mut fixture = make_fixture();
        fixture
            .document
            .get_object_mut(fixture.contents)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(contents.to_vec());
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        assert!(source.sanitized_copy().is_err());
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
    for key in ["XFA", "RichMediaContent"] {
        let mut fixture = make_fixture();
        fixture
            .document
            .get_dictionary_mut(fixture.root)
            .unwrap()
            .set(key, Object::string_literal(SENTINEL));
        let source = load(&fixture.document);
        assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
    }
    let mut fixture = make_fixture();
    fixture.document.add_object(dictionary! {"FT"=>"Sig"});
    let source = load(&fixture.document);
    assert!(matches!(source.sanitized_copy(), Err(Error::Refused(_))));
}

#[test]
fn malformed_or_unbounded_vector_appearances_refuse() {
    for appearance in [
        "Q",
        "q",
        "1 q Q",
        "BT (private) Tj ET",
        "/Other Do",
        "NaN 0 0 rg",
        "[1 [2]] 0 d",
    ] {
        let mut fixture = make_fixture();
        fixture
            .document
            .get_object_mut(fixture.appearance)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(appearance.as_bytes().to_vec());
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        assert!(source.sanitized_copy().is_err(), "accepted {appearance}");
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
    let mut fixture = make_fixture();
    fixture
        .document
        .get_dictionary_mut(fixture.attachment)
        .unwrap()
        .remove(b"AP");
    fixture
        .document
        .get_dictionary_mut(fixture.attachment)
        .unwrap()
        .set("Name", "UnknownCustomIcon");
    assert!(matches!(
        load(&fixture.document).sanitized_copy(),
        Err(Error::Refused(_))
    ));
    let mut fixture = make_fixture();
    fixture
        .document
        .get_object_mut(fixture.appearance)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("Subtype", "Image");
    assert!(matches!(
        load(&fixture.document).sanitized_copy(),
        Err(Error::Refused(_))
    ));
}

#[test]
fn content_cycles_and_large_direct_graphs_return_controlled_errors() {
    let mut fixture = make_fixture();
    let cycle = fixture.document.new_object_id();
    fixture
        .document
        .objects
        .insert(cycle, Object::Array(vec![cycle.into()]));
    fixture
        .document
        .get_dictionary_mut(fixture.page)
        .unwrap()
        .set("Contents", cycle);
    let source = load(&fixture.document);
    assert!(matches!(source.sanitized_copy(), Err(Error::Invalid(_))));
    let mut fixture = make_fixture();
    fixture
        .document
        .get_dictionary_mut(fixture.root)
        .unwrap()
        .set(
            "CustomIntegers",
            Object::Array(vec![Object::Integer(0); 1_000_001]),
        );
    let source = load(&fixture.document);
    assert!(matches!(source.sanitized_copy(), Err(Error::Limit(_))));
}

#[test]
fn new_copy_is_verified_and_never_overwrites() {
    let fixture = make_fixture();
    let source = load(&fixture.document);
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("clean.pdf");
    source.sanitize_new(&destination).unwrap();
    let before = std::fs::read(&destination).unwrap();
    PdfDocument::open(&destination, None).unwrap();
    assert!(source.sanitize_new(&destination).is_err());
    assert_eq!(std::fs::read(destination).unwrap(), before);
}

fn compare_outside_icon(before: &[u8], after: &[u8], channels: usize, minimum_changed: usize) {
    assert_eq!(before.len(), after.len());
    assert_eq!(before.len(), 200 * 200 * channels);
    let mut changed = 0;
    for y in 0..200 {
        for x in 0..200 {
            let offset = (y * 200 + x) * channels;
            let equal = before[offset..offset + channels] == after[offset..offset + channels];
            // Explicit AP and built-in icons occupy Rect [140,140,160,160].
            if !(140..160).contains(&x) || !(40..60).contains(&y) {
                assert!(equal, "pixel changed outside attachment at {x},{y}");
            } else if !equal {
                changed += 1;
            }
        }
    }
    assert!(
        changed > minimum_changed,
        "visible icon removal must alter its supported footprint"
    );
}

#[test]
fn native_pixels_change_only_inside_removed_visible_attachment() {
    let fixture = make_fixture();
    let source = load(&fixture.document);
    let (clean, _) = source.sanitized_copy().unwrap();
    let before = pdf_render::render_page(&source.verified_bytes().unwrap(), 0, 1.0).unwrap();
    let after = pdf_render::render_page(&clean.verified_bytes().unwrap(), 0, 1.0).unwrap();
    assert_eq!((before.width, before.height), (200, 200));
    assert_eq!((after.width, after.height), (200, 200));
    compare_outside_icon(&before.rgba, &after.rgba, 4, 200);
}

#[test]
fn standard_default_icons_render_visibly_and_current_sanitizer_removes_only_their_footprint() {
    for icon in [
        None,
        Some("PushPin"),
        Some("Graph"),
        Some("Paperclip"),
        Some("Tag"),
    ] {
        let mut fixture = make_fixture();
        let annotation = fixture
            .document
            .get_dictionary_mut(fixture.attachment)
            .unwrap();
        annotation.remove(b"AP");
        if let Some(icon) = icon {
            annotation.set("Name", icon);
        }
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        let (clean, report) = source.sanitized_copy().unwrap();
        assert!(
            report.attachment_annotations_removed[0]
                .predefined_icon
                .is_some()
        );
        let output = clean.verified_bytes().unwrap();
        assert!(
            !output
                .windows(SENTINEL.len())
                .any(|value| value == SENTINEL.as_bytes())
        );
        let parsed = Document::load_mem(&output).unwrap();
        assert_eq!(
            parsed.get_object(fixture.contents).unwrap(),
            fixture.document.get_object(fixture.contents).unwrap()
        );
        assert_eq!(
            parsed
                .get_dictionary(fixture.page)
                .unwrap()
                .get(b"Resources")
                .unwrap(),
            fixture
                .document
                .get_dictionary(fixture.page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
        );
        let before_image = pdf_render::render_page(&before, 0, 1.0).unwrap();
        let after_image = pdf_render::render_page(&output, 0, 1.0).unwrap();
        compare_outside_icon(&before_image.rgba, &after_image.rgba, 4, 20);
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
}

#[test]
fn malformed_default_icon_properties_refuse_without_changing_source() {
    for (key, value) in [
        ("Name", Object::string_literal("PushPin")),
        ("Name", Object::Name(b"CustomIcon".to_vec())),
        ("C", Object::Array(vec![Object::Integer(2)])),
        ("C", Object::Array(vec![Object::Integer(0); 2])),
        ("CA", Object::Real(1.5)),
    ] {
        let mut fixture = make_fixture();
        let annotation = fixture
            .document
            .get_dictionary_mut(fixture.attachment)
            .unwrap();
        annotation.remove(b"AP");
        annotation.set(key, value);
        let source = load(&fixture.document);
        let before = source.verified_bytes().unwrap();
        assert!(source.sanitized_copy().is_err());
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
}

#[test]
#[ignore = "requires pinned qpdf 12.4.2 and Poppler 26.10 reference binaries"]
fn independent_decoded_payload_text_and_footprint_gate() {
    use std::{path::Path, process::Command};
    let qpdf = std::env::var_os("PDF_SANITIZE_QPDF").expect("PDF_SANITIZE_QPDF required");
    let poppler =
        std::env::var_os("PDF_SANITIZE_POPPLER_BIN").expect("PDF_SANITIZE_POPPLER_BIN required");
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
    let ppm = Path::new(&poppler).join("pdftoppm");
    assert!(
        String::from_utf8_lossy(&Command::new(&ppm).arg("-v").output().unwrap().stderr)
            .contains("26.10")
    );
    for icon in [
        None,
        Some("PushPin"),
        Some("Graph"),
        Some("Paperclip"),
        Some("Tag"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("source.pdf");
        let clean_path = temp.path().join("clean.pdf");
        let mut fixture = make_fixture();
        if let Some(icon) = icon {
            let annotation = fixture
                .document
                .get_dictionary_mut(fixture.attachment)
                .unwrap();
            annotation.remove(b"AP");
            annotation.set("Name", icon);
            if icon == "Paperclip" {
                annotation.set("C", vec![0.4.into(), 0.2.into(), 0.8.into()]);
                annotation.set("CA", 0.5);
            }
        }
        let original = bytes(fixture.document);
        std::fs::write(&source_path, &original).unwrap();
        // Real compressed object streams retain source metadata/actions in transport bytes.
        // Rewriting must remove those carriers, rather than merely removing logical links.
        let packed = temp.path().join("packed.pdf");
        assert!(
            Command::new(&qpdf)
                .arg("--object-streams=generate")
                .arg(&source_path)
                .arg(&packed)
                .output()
                .unwrap()
                .status
                .success()
        );
        let original = std::fs::read(packed).unwrap();
        std::fs::write(&source_path, &original).unwrap();
        let report = PdfDocument::from_bytes(&original, None)
            .unwrap()
            .sanitize_new(&clean_path)
            .unwrap();
        assert!(report.transport_containers_removed > 0);
        assert!(report.unreachable_objects_removed > report.transport_containers_removed);
        assert!(
            Command::new(&qpdf)
                .arg("--check")
                .arg(&clean_path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let decoded = temp.path().join("decoded.pdf");
        assert!(
            Command::new(&qpdf)
                .args([
                    "--qdf",
                    "--object-streams=disable",
                    "--stream-data=uncompress"
                ])
                .arg(&clean_path)
                .arg(&decoded)
                .output()
                .unwrap()
                .status
                .success()
        );
        for path in [&clean_path, &decoded] {
            let data = std::fs::read(path).unwrap();
            assert!(
                !data
                    .windows(SENTINEL.len())
                    .any(|value| value == SENTINEL.as_bytes())
            );
        }
        let text = |path: &Path| {
            let result = Command::new(Path::new(&poppler).join("pdftotext"))
                .arg(path)
                .arg("-")
                .output()
                .unwrap();
            assert!(result.status.success());
            result.stdout
        };
        assert_eq!(text(&source_path), text(&clean_path));
        let image = |path: &Path, name: &str| {
            let prefix = temp.path().join(name);
            let result = Command::new(&ppm)
                .args(["-r", "72", "-singlefile"])
                .arg(path)
                .arg(&prefix)
                .output()
                .unwrap();
            assert!(result.status.success());
            let bytes = std::fs::read(prefix.with_extension("ppm")).unwrap();
            let mut cursor = 0;
            let mut tokens = Vec::new();
            while tokens.len() < 4 {
                while bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                let start = cursor;
                while !bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                tokens.push(&bytes[start..cursor]);
            }
            assert_eq!(tokens, [b"P6".as_slice(), b"200", b"200", b"255"]);
            cursor += 1;
            bytes[cursor..].to_vec()
        };
        compare_outside_icon(
            &image(&source_path, "before"),
            &image(&clean_path, "after"),
            3,
            if icon.is_some() { 20 } else { 200 },
        );
        assert_eq!(std::fs::read(source_path).unwrap(), original);
    }
}
