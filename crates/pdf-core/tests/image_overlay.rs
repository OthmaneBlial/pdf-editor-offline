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
