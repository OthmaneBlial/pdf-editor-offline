#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;
#[path = "../../pdf-worker/tests/support/encryption.rs"]
mod encryption;
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
const PIXELS: &[u8] = b"SECRETSECRETSECRETSECRETSECRETSECRETSECRETSECRET";
const MASK: &[u8] = b"MASK_PRIVATE_123";
fn save(mut document: Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
fn fixture(mask: bool, compressed: bool) -> Vec<u8> {
    let mut doc = Document::load_mem(&support::fixture(2)).unwrap();
    let mut image = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>16,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        PIXELS.to_vec(),
    );
    if mask {
        let mask = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>16,"Height"=>1,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8}, MASK.to_vec()));
        image.dict.set("SMask", mask);
    }
    if compressed {
        image.compress().unwrap();
    }
    doc.objects.insert((4, 0), image.into());
    let public = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>1,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8}, vec![0, 0, 255]));
    doc.get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Public", public);
    doc.get_object_mut((5,0)).unwrap().as_stream_mut().unwrap().set_content(
        b"0.123456789012345678 0.4 0.5 rg 100 210 30 10 re f\nBT /F1 10 Tf 20 240 Td (public text) Tj ET\nq 50 0 0 20 10 30 cm /Im1 Do Q q 30 0 0 10 100 140 cm /Public Do Q\n".to_vec(),
    );
    doc.add_object(Stream::new(dictionary! {}, PIXELS.to_vec()));
    let info = doc.add_object(dictionary! {"Title"=>Object::string_literal(PIXELS)});
    doc.trailer.set("Info", info);
    let xmp = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        [b"<private>".as_slice(), PIXELS, b"</private>"].concat(),
    ));
    doc.get_object_mut((1, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Metadata", xmp);
    save(doc)
}
fn all(pdf: &PdfDocument) -> Vec<pdf_core::ImageSelection> {
    (1..=pdf.page_count() as u32)
        .flat_map(|page| pdf.list_image_instances(page).unwrap())
        .filter(|image| image.selection.image_id == (4, 0))
        .map(|image| image.selection)
        .collect()
}
fn public_id(doc: &Document) -> lopdf::ObjectId {
    doc.get_object((2, 0))
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Public")
        .unwrap()
        .as_reference()
        .unwrap()
}
#[test]
fn complete_shared_image_removal_erases_resources_masks_old_streams_and_orphans() {
    for mask in [false, true] {
        let source = fixture(mask, false);
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        let selections = all(&pdf);
        assert_eq!(selections.len(), 2);
        assert!(pdf.redact_images_copy(&selections[..1]).is_err());
        let (copy, report) = pdf.redact_images_copy(&selections).unwrap();
        assert_eq!(report.removed_draws.len(), 2);
        assert_eq!(report.removed_image_objects, 1);
        assert_eq!(report.removed_mask_objects, usize::from(mask));
        assert!(report.resource_links_removed > 0);
        assert!(report.full_rewrite);
        let output = copy.verified_bytes().unwrap();
        assert!(!output.windows(PIXELS.len()).any(|v| v == PIXELS));
        if mask {
            assert!(!output.windows(MASK.len()).any(|v| v == MASK));
        }
        let doc = Document::load_mem(&output).unwrap();
        assert!(!doc.objects.contains_key(&(4, 0)));
        assert!(!doc.trailer.has(b"Prev"));
        assert!(!doc.trailer.has(b"ID"));
        assert!(
            doc.objects
                .values()
                .filter_map(|v| v.as_stream().ok())
                .all(|s| !s.dict.has(b"Filter"))
        );
        assert_eq!(copy.info().unwrap().pages, pdf.info().unwrap().pages);
        assert_eq!(
            copy.extract_text(&[1, 2]).unwrap(),
            pdf.extract_text(&[1, 2]).unwrap()
        );
        assert_eq!(copy.list_image_instances(1).unwrap().len(), 1);
        assert!(copy.list_image_instances(2).unwrap().is_empty());
        let content = doc.get_page_content(doc.get_pages()[&1]);
        assert!(
            content
                .windows(b"0.123456789012345678".len())
                .any(|v| v == b"0.123456789012345678")
        );
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
}
#[test]
fn stale_duplicate_shared_mask_external_alias_and_duplicate_bytes_refuse_atomically() {
    let base = fixture(true, false);
    for case in 0..6 {
        let mut doc = Document::load_mem(&base).unwrap();
        match case {
            0 => {
                doc.get_object_mut((1, 0))
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("KeepSelected", Object::Reference((4, 0)));
            }
            1 => {
                let id = public_id(&doc);
                let public = doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
                public.dict.set("Width", 16);
                public.set_content([0, 0, 255].repeat(16));
                public.dict.set("SMask", Object::Reference((9, 0)));
            }
            2 => {
                let id = public_id(&doc);
                let mut duplicate = doc.get_object((4, 0)).unwrap().clone();
                duplicate.as_stream_mut().unwrap().dict.remove(b"SMask");
                doc.objects.insert(id, duplicate);
            }
            3 => {
                let alias = doc.add_object(Object::Reference((4, 0)));
                doc.get_object_mut((1, 0))
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("UnclassifiedAlias", alias);
            }
            _ => {}
        }
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        let mut selections = all(&pdf);
        if case == 4 {
            selections[0].source_hash = "0".repeat(64);
        }
        if case == 5 {
            selections.push(selections[0].clone());
        }
        let error = pdf.redact_images_copy(&selections).err().unwrap();
        if case == 1 {
            assert!(error.to_string().contains("surviving shared"), "{error}");
        }
        if case == 2 {
            assert!(error.to_string().contains("duplicate selected"), "{error}");
        }
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
}
#[test]
fn surviving_images_also_require_exact_privacy_decoding() {
    for case in 0..4 {
        let mut doc = Document::load_mem(&fixture(false, false)).unwrap();
        let id = public_id(&doc);
        let public = doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
        match case {
            0 => public.set_content(vec![0, 0]),
            1 => public.set_content(vec![0, 0, 255, 42]),
            2 => public.dict.set("Filter", "DCTDecode"),
            _ => public.dict.set("ColorSpace", "DeviceCMYK"),
        }
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        assert!(pdf.redact_images_copy(&all(&pdf)).is_err(), "case{case}");
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
}

#[test]
fn unused_target_resource_aliases_are_removed_without_touching_surviving_aliases() {
    let mut doc = Document::load_mem(&fixture(false, false)).unwrap();
    let target_alias = doc.add_object(Object::Reference((4, 0)));
    let public = public_id(&doc);
    let public_alias = doc.add_object(Object::Reference(public));
    let resources = doc
        .get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    let objects = resources
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    objects.set("UnusedTargetAlias", target_alias);
    objects.set("Public", public_alias);
    let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
    let original = pdf.verified_bytes().unwrap();
    let (copy, report) = pdf.redact_images_copy(&all(&pdf)).unwrap();
    assert_eq!(report.resource_links_removed, 2);
    let output = Document::load_mem(&copy.verified_bytes().unwrap()).unwrap();
    assert!(!output.objects.contains_key(&target_alias));
    assert!(output.objects.contains_key(&public_alias));
    assert_eq!(copy.list_image_instances(1).unwrap().len(), 1);
    assert_eq!(pdf.verified_bytes().unwrap(), original);
}
#[test]
fn decoder_never_pads_truncates_substitutes_masks_or_accepts_unknown_codecs() {
    for case in 0..8 {
        let mut doc = Document::load_mem(&fixture(true, false)).unwrap();
        let image = doc.get_object_mut((4, 0)).unwrap().as_stream_mut().unwrap();
        match case {
            0 => image.set_content(PIXELS[..PIXELS.len() - 1].to_vec()),
            1 => image.set_content([PIXELS, b"trailing"].concat()),
            2 => image.dict.set("Filter", "DCTDecode"),
            3 => image.dict.set("ColorSpace", "DeviceCMYK"),
            4 => image.dict.set("DecodeParms", dictionary! {"Predictor"=>12}),
            5 => image.dict.set(
                "Mask",
                [0, 255, 0, 255, 0, 255].map(Object::Integer).to_vec(),
            ),
            6 => image.dict.set("ImageMask", true),
            _ => image
                .dict
                .set("Decode", [1, 0, 1, 0, 1, 0].map(Object::Integer).to_vec()),
        }
        let pdf = PdfDocument::from_bytes(&save(doc), None).unwrap();
        let original = pdf.verified_bytes().unwrap();
        assert!(pdf.redact_images_copy(&all(&pdf)).is_err(), "case{case}");
        assert_eq!(pdf.verified_bytes().unwrap(), original);
    }
    let (cipher, _) = encryption::fixture(3, "reader", false);
    let encrypted = PdfDocument::from_bytes(&cipher, Some("reader")).unwrap();
    assert!(encrypted.redact_images_copy(&[]).is_err());
    let mut signed = Document::load_mem(&fixture(false, false)).unwrap();
    signed.add_object(dictionary! {"FT"=>"Sig"});
    let signed = PdfDocument::from_bytes(&save(signed), None).unwrap();
    assert!(signed.redact_images_copy(&all(&signed)).is_err());
}

fn independent_control(source: &[u8]) -> Vec<u8> {
    let mut doc = Document::load_mem(source).unwrap();
    // This deliberately uses the independently authored fixture's exact literal
    // draw, not the production discovery, lexer, selected spans or remover.
    for id in [(5, 0), (7, 0)] {
        let stream = doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
        let program = String::from_utf8(stream.get_plain_content().unwrap()).unwrap();
        assert_eq!(program.matches("/Im1 Do").count(), 1);
        stream.set_content(program.replace("/Im1 Do", "       ").into_bytes());
        stream.dict.remove(b"Filter");
        stream.dict.remove(b"DecodeParms");
    }
    save(doc)
}

#[test]
#[ignore = "requires pinned qpdf 12.4.2 and Poppler 26.10.0 reference binaries"]
fn permanent_image_removal_matches_independent_native_and_poppler_controls() {
    use std::process::Command;
    let qpdf = std::path::PathBuf::from(std::env::var_os("PDF_QPDF_BIN").unwrap());
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    assert!(
        String::from_utf8(
            Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .unwrap()
        .contains("12.4.2")
    );
    let version = Command::new(bin.join("pdftoppm"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(
        String::from_utf8(version.stderr)
            .unwrap()
            .contains("26.10.0")
    );
    for mask in [false, true] {
        for compressed in [false, true] {
            let source = fixture(mask, compressed);
            let original_doc = Document::load_mem(&source).unwrap();
            assert_eq!(
                original_doc
                    .get_object((4, 0))
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .get_plain_content()
                    .unwrap(),
                PIXELS
            );
            if compressed {
                assert!(
                    original_doc
                        .get_object((4, 0))
                        .unwrap()
                        .as_stream()
                        .unwrap()
                        .dict
                        .has(b"Filter")
                );
            }
            let pdf = PdfDocument::from_bytes(&source, None).unwrap();
            let (copy, _) = pdf.redact_images_copy(&all(&pdf)).unwrap();
            let output = copy.verified_bytes().unwrap();
            let control = independent_control(&source);
            let temp = tempfile::tempdir().unwrap();
            let mut files = Vec::new();
            let mut texts = Vec::new();
            for (name, bytes) in [
                ("source", &source),
                ("output", &output),
                ("control", &control),
            ] {
                let path = temp.path().join(format!("{name}.pdf"));
                std::fs::write(&path, bytes).unwrap();
                let check = Command::new(&qpdf)
                    .arg("--check")
                    .arg(&path)
                    .output()
                    .unwrap();
                assert!(
                    check.status.success(),
                    "{}",
                    String::from_utf8_lossy(&check.stderr)
                );
                let text = Command::new(bin.join("pdftotext"))
                    .arg("-layout")
                    .arg(&path)
                    .arg("-")
                    .output()
                    .unwrap();
                assert!(text.status.success());
                texts.push(text.stdout);
                files.push(path);
            }
            assert_eq!(texts[0], texts[1]);
            assert_eq!(texts[1], texts[2]);
            for page in 0..2 {
                let before = pdf_render::render_page(&source, page, 1.0).unwrap().rgba;
                let after = pdf_render::render_page(&output, page, 1.0).unwrap().rgba;
                let expected = pdf_render::render_page(&control, page, 1.0).unwrap().rgba;
                assert!(
                    before != expected,
                    "the selected asset must be visibly present before removal"
                );
                assert!(
                    after == expected,
                    "native pixels differ from the independent no-image control"
                );
                let mut pixels = Vec::new();
                for (index, path) in files.iter().enumerate() {
                    let prefix = temp.path().join(format!("page-{page}-{index}"));
                    let result = Command::new(bin.join("pdftoppm"))
                        .args([
                            "-r",
                            "72",
                            "-f",
                            &(page + 1).to_string(),
                            "-l",
                            &(page + 1).to_string(),
                            "-singlefile",
                        ])
                        .arg(path)
                        .arg(&prefix)
                        .output()
                        .unwrap();
                    assert!(result.status.success());
                    pixels.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
                }
                assert!(
                    pixels[0] != pixels[2],
                    "independent reader must recover the visible source asset"
                );
                // Exact whole-page equality proves every pixel outside this
                // reader's actual before/control difference stayed unchanged.
                // No guessed rectangle padding or edge-coverage tolerance.
                assert!(
                    pixels[1] == pixels[2],
                    "Poppler differs from independent control"
                );
            }
            let qdf = temp.path().join("decoded.qdf");
            let decoded = Command::new(&qpdf)
                .args([
                    "--qdf",
                    "--object-streams=disable",
                    "--stream-data=uncompress",
                ])
                .arg(&files[1])
                .arg(&qdf)
                .output()
                .unwrap();
            assert!(decoded.status.success());
            let decoded = std::fs::read(qdf).unwrap();
            assert!(!decoded.windows(PIXELS.len()).any(|v| v == PIXELS));
            assert!(!decoded.windows(MASK.len()).any(|v| v == MASK));
            assert_eq!(std::fs::read(&files[0]).unwrap(), source);
        }
    }
}
