#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{ImageOverlay, PdfDocument};
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn bytes(mut document: Document) -> Vec<u8> {
    let mut bytes = vec![];
    document.save_to(&mut bytes).unwrap();
    bytes
}
fn donor(mask: bool) -> PdfDocument {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let mut dict = dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Interpolate"=>false};
    if mask {
        let id=document.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8},vec![255,0]));
        dict.set("SMask", id);
    }
    let image = document.add_object(Stream::new(dict, vec![255, 0, 0, 0, 0, 255]));
    let page=document.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),2.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>image}}});
    document.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    document.trailer.set("Root", root);
    PdfDocument::from_bytes(&bytes(document), None).unwrap()
}
fn placement(rotation: u16) -> ImageOverlay {
    ImageOverlay {
        page: 1,
        rectangle: [60.0, 30.0, 100.0, 70.0],
        rotation,
    }
}
#[test]
fn pending_clip_does_not_hide_inserted_image_or_change_original_objects() {
    let mut outputs = Vec::new();
    for clipping in ["", "0 0 5 5 re W", "0 0 5 5 re W*"] {
        let mut source = Document::load_mem(&support::fixture(1)).unwrap();
        let stream = source
            .get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.content.extend(format!("\n{clipping}\n").as_bytes());
        stream.dict.set("Length", stream.content.len() as i64);
        let source_bytes = bytes(source.clone());
        let mut pdf = PdfDocument::from_bytes(&source_bytes, None).unwrap();
        let before = pdf.info().unwrap();
        pdf.add_image_overlay(&placement(0), &donor(false)).unwrap();
        let output = pdf.verified_bytes().unwrap();
        assert_eq!(pdf.info().unwrap().pages, before.pages);
        let parsed = Document::load_mem(&output).unwrap();
        for (id, object) in &source.objects {
            if *id != (6, 0) && object.type_name().ok() != Some(b"XRef") {
                assert_eq!(parsed.get_object(*id).unwrap(), object);
            }
        }
        outputs.push(pdf_render::render_page(&output, 0, 1.0).unwrap());
    }
    assert!(outputs[0].rgba == outputs[1].rgba, "pending W hides image");
    assert!(outputs[0].rgba == outputs[2].rgba, "pending W* hides image");
    assert!(
        outputs[0]
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] > 240 && p[1] < 10)
    );
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_pending_clip_image_pixels() {
    use std::process::Command;
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
            &Command::new(bin.join("pdftoppm"))
                .arg("-v")
                .output()
                .unwrap()
                .stderr
        )
        .contains("26.10.0")
    );
    let temp = tempfile::tempdir().unwrap();
    let mut pixels = Vec::new();
    for (index, clipping) in ["", "0 0 5 5 re W", "0 0 5 5 re W*"].iter().enumerate() {
        let mut source = Document::load_mem(&support::fixture(1)).unwrap();
        let stream = source
            .get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.content.extend(format!("\n{clipping}\n").as_bytes());
        stream.dict.set("Length", stream.content.len() as i64);
        let mut pdf = PdfDocument::from_bytes(&bytes(source), None).unwrap();
        pdf.add_image_overlay(&placement(0), &donor(false)).unwrap();
        let output = temp.path().join(format!("image-{index}.pdf"));
        std::fs::write(&output, pdf.verified_bytes().unwrap()).unwrap();
        assert!(
            Command::new(&qpdf)
                .arg("--check")
                .arg(&output)
                .output()
                .unwrap()
                .status
                .success()
        );
        let render = Command::new(bin.join("pdftoppm"))
            .args(["-singlefile", "-r", "72"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(render.status.success());
        assert!(render.stdout.starts_with(b"P6\n200 300\n255\n"));
        pixels.push(render.stdout);
    }
    assert!(pixels[0] == pixels[1]);
    assert!(pixels[0] == pixels[2]);
}
#[test]
fn image_and_alpha_are_real_resources_and_source_geometry_text_shared_page_are_preserved() {
    let source = support::fixture(2);
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let info = pdf.info().unwrap();
    let text = pdf.extract_text(&[1, 2]).unwrap();
    pdf.add_image_overlay(&placement(90), &donor(true)).unwrap();
    let output = pdf.verified_bytes().unwrap();
    let parsed = Document::load_mem(&output).unwrap();
    let pages = parsed.get_pages();
    let resources = parsed
        .get_object(*pages.get(&1).unwrap())
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap();
    let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
    assert!(xobjects.has(b"Im1"));
    let image = parsed
        .get_object(
            xobjects
                .get(b"NativeImageOverlay")
                .unwrap()
                .as_reference()
                .unwrap(),
        )
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(
        image.dict.get(b"Subtype").unwrap().as_name().unwrap(),
        b"Image"
    );
    assert_eq!(image.content, vec![255, 0, 0, 0, 0, 255]);
    let mask = parsed
        .get_object(image.dict.get(b"SMask").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(mask.content, vec![255, 0]);
    assert_eq!(pdf.info().unwrap().pages, info.pages);
    assert_eq!(pdf.extract_text(&[1, 2]).unwrap(), text);
    let original = Document::load_mem(&source).unwrap();
    let page2 = *pages.get(&2).unwrap();
    assert_eq!(
        parsed.get_object(page2).unwrap(),
        original.get_object(page2).unwrap()
    );
    for (id, value) in &original.objects {
        if *id != *pages.get(&1).unwrap() {
            assert_eq!(parsed.get_object(*id).unwrap(), value);
        }
    }
}
#[test]
fn invalid_geometry_rotation_readonly_and_malformed_assets_are_transactional() {
    let source = support::fixture(2);
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    for bad in [
        ImageOverlay {
            page: 0,
            ..placement(0)
        },
        ImageOverlay {
            rectangle: [0.0, 0.0, f64::NAN, 10.0],
            ..placement(0)
        },
        ImageOverlay {
            rectangle: [0.0, 0.0, 300.0, 10.0],
            ..placement(0)
        },
        placement(45),
    ] {
        assert!(pdf.add_image_overlay(&bad, &donor(false)).is_err());
    }
    let mut document = Document::load_mem(&donor(false).verified_bytes().unwrap()).unwrap();
    let id=*document.objects.iter().find(|(_,object)|matches!(object,Object::Stream(stream) if stream.dict.get(b"Subtype").is_ok_and(|value|value.as_name().is_ok_and(|name|name==b"Image")))).unwrap().0;
    document
        .get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("SMask", id);
    assert!(
        pdf.add_image_overlay(
            &placement(0),
            &PdfDocument::from_bytes(&bytes(document), None).unwrap()
        )
        .is_err()
    );
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    let mut signed = PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
        None,
    )
    .unwrap();
    assert!(
        signed
            .add_image_overlay(&placement(0), &donor(false))
            .is_err()
    );
}
#[test]
fn repeated_insert_uses_fresh_resources_without_overwriting_previous_image() {
    let mut pdf = PdfDocument::from_bytes(&support::fixture(2), None).unwrap();
    pdf.add_image_overlay(&placement(0), &donor(false)).unwrap();
    pdf.add_image_overlay(&placement(180), &donor(true))
        .unwrap();
    let parsed = Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap();
    let page = parsed
        .get_object(parsed.get_pages()[&1])
        .unwrap()
        .as_dict()
        .unwrap();
    let objects = page
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap();
    assert!(
        objects.has(b"Im1")
            && objects.has(b"NativeImageOverlay")
            && objects.has(b"NativeImageOverlay0")
    );
    assert_ne!(
        objects.get(b"NativeImageOverlay").unwrap(),
        objects.get(b"NativeImageOverlay0").unwrap()
    );
}
#[test]
fn unrelated_donor_catalog_actions_metadata_and_orphans_are_never_imported() {
    let mut donor = Document::load_mem(&donor(false).verified_bytes().unwrap()).unwrap();
    let sentinel = "DONOR-PRIVATE-ACTION-AND-METADATA-SENTINEL";
    let script =
        donor.add_object(dictionary! {"S"=>"JavaScript","JS"=>Object::string_literal(sentinel)});
    let root = donor.trailer.get(b"Root").unwrap().as_reference().unwrap();
    donor
        .get_object_mut(root)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("OpenAction", script);
    let info = donor.add_object(dictionary! {"Author"=>Object::string_literal(sentinel)});
    donor.trailer.set("Info", info);
    donor.add_object(Object::string_literal(sentinel));
    let donor = PdfDocument::from_bytes(&bytes(donor), None).unwrap();
    let mut source = PdfDocument::from_bytes(&support::fixture(2), None).unwrap();
    source.add_image_overlay(&placement(0), &donor).unwrap();
    let output = source.verified_bytes().unwrap();
    assert!(
        !output
            .windows(sentinel.len())
            .any(|bytes| bytes == sentinel.as_bytes())
    );
    assert!(source.inspect_security().unwrap().actions.is_empty());
}
#[test]
fn unbalanced_source_and_tagged_page_refusals_preserve_original_objects() {
    for tagged in [false, true] {
        let mut source = Document::load_mem(&support::fixture(2)).unwrap();
        if tagged {
            let root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
            source
                .get_object_mut(root)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("StructTreeRoot", dictionary! {"Type"=>"StructTreeRoot"});
        } else {
            let page = source.get_pages()[&1];
            let content = source
                .get_object(page)
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"Contents")
                .unwrap()
                .as_reference()
                .unwrap();
            source
                .objects
                .insert(content, Stream::new(dictionary! {}, b"Q".to_vec()).into());
        }
        let mut source = PdfDocument::from_bytes(&bytes(source), None).unwrap();
        let before = source.verified_bytes().unwrap();
        assert!(
            source
                .add_image_overlay(&placement(0), &donor(false))
                .is_err()
        );
        assert_eq!(source.verified_bytes().unwrap(), before);
    }
}
#[test]
fn dimensions_that_encode_as_zero_are_refused_without_changing_the_document() {
    for rotation in [0, 90, 180, 270] {
        for rectangle in [[0.0, 0.0, 1e-300, 10.0], [0.0, 0.0, 10.0, 1e-300]] {
            let mut document = PdfDocument::from_bytes(&support::fixture(2), None).unwrap();
            let before = document.verified_bytes().unwrap();
            let placement = ImageOverlay {
                page: 1,
                rectangle,
                rotation,
            };
            assert!(
                document
                    .add_image_overlay(&placement, &donor(false))
                    .is_err(),
                "singular encoded transform accepted: {rectangle:?}/{rotation}"
            );
            assert_eq!(document.verified_bytes().unwrap(), before);
        }
    }
}
#[test]
fn positive_encoded_dimensions_are_not_replaced_by_an_arbitrary_minimum() {
    for rotation in [0, 90, 180, 270] {
        let mut document = PdfDocument::from_bytes(&support::fixture(2), None).unwrap();
        document
            .add_image_overlay(
                &ImageOverlay {
                    page: 1,
                    rectangle: [0.0, 0.0, 1e-20, 10.0],
                    rotation,
                },
                &donor(false),
            )
            .unwrap();
        document.verified_bytes().unwrap();
    }
}
