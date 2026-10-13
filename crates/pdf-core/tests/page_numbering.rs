#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{NumberPosition, PageNumbering, PdfDocument, TextAlignment};
use std::process::Command;
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
#[path = "../../../tests/support/split_fixture.rs"]
mod support;

#[test]
fn thousand_page_batch_uses_one_font_and_remains_editable() {
    let source = support::fixture(1000);
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let mut numbering = request();
    numbering.pages = (1..=1000).collect();
    numbering.start = 1;
    numbering.position = NumberPosition::Footer;
    pdf.add_page_numbers(&numbering, FONT).unwrap();
    assert!(pdf.extract_text(&[1000]).unwrap().contains("1000"));
    let output = pdf.verified_bytes().unwrap();
    let parsed = Document::load_mem(&output).unwrap();
    let embedded = parsed
        .objects
        .values()
        .filter(|v| {
            v.as_dict()
                .ok()
                .is_some_and(|d| d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0"))
        })
        .count();
    assert_eq!(embedded, 1);
    numbering.pages = vec![1];
    numbering.start = 2000;
    pdf.add_page_numbers(&numbering, FONT).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("2000"));
}
fn fixture() -> Vec<u8> {
    let mut d = Document::with_version("1.3");
    let tree = d.new_object_id();
    let mut kids = Vec::new();
    for rotation in [0, 90, 180, 270] {
        let stream = d.add_object(Stream::new(
            dictionary! {},
            b"0 0 1 rg 100 180 20 20 re f\n0 0 5 5 re W".to_vec(),
        ));
        kids.push(d.add_object(
            dictionary! {"Type"=>"Page","Parent"=>tree,"Rotate"=>rotation,"Contents"=>stream},
        ));
    }
    d.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>4,"Kids"=>kids.into_iter().map(Object::Reference).collect::<Vec<_>>(),"MediaBox"=>vec![10.into(),20.into(),310.into(),420.into()],"CropBox"=>vec![30.into(),40.into(),290.into(),400.into()],"Resources"=>dictionary!{}}.into());
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    let mut bytes = Vec::new();
    d.save_to(&mut bytes).unwrap();
    bytes
}
fn request() -> PageNumbering {
    PageNumbering {
        pages: vec![4, 2, 1, 3],
        start: 98,
        position: NumberPosition::Header,
        margin: 10.0,
        height: 24.0,
        size: 12.0,
        color: [1.0, 0.0, 0.0],
        alignment: TextAlignment::Center,
    }
}
#[test]
fn repeated_compressed_source_expansion_is_bounded_before_publication() {
    let mut d = Document::load_mem(&fixture()).unwrap();
    let stream = d.add_object(Stream::new(dictionary! {}, vec![b' '; 17 * 1024 * 1024]));
    for page in [1, 2] {
        d.get_dictionary_mut(*d.get_pages().get(&page).unwrap())
            .unwrap()
            .set("Contents", stream);
    }
    d.compress();
    let mut bytes = Vec::new();
    d.save_to(&mut bytes).unwrap();
    assert!(bytes.len() < 100_000);
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    assert!(matches!(
        pdf.add_page_numbers(
            &PageNumbering {
                pages: vec![1, 2],
                ..request()
            },
            FONT
        ),
        Err(pdf_core::Error::Limit(_))
    ));
    assert_eq!(pdf.verified_bytes().unwrap(), before);
}
#[test]
fn footer_placement_is_upright_and_confined_on_every_rotation() {
    let mut pdf = PdfDocument::from_bytes(&fixture(), None).unwrap();
    let mut numbering = request();
    numbering.position = NumberPosition::Footer;
    pdf.add_page_numbers(&numbering, FONT).unwrap();
    let output = pdf.verified_bytes().unwrap();
    for page in 0..4 {
        let image = pdf_render::render_page(&output, page, 1.0).unwrap();
        let mut red = 0;
        for (i, p) in image.rgba.chunks_exact(4).enumerate() {
            if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                red += 1;
                assert!(i / (image.width as usize) >= image.height as usize - 35);
            }
        }
        assert!(red > 10);
    }
    let mut signed = PdfDocument::from_bytes(
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
        None,
    )
    .unwrap();
    let before = signed.info().unwrap().pages;
    assert!(
        signed
            .add_page_numbers(
                &PageNumbering {
                    pages: vec![1],
                    ..numbering
                },
                FONT
            )
            .is_err()
    );
    assert_eq!(signed.info().unwrap().pages, before);
}
#[test]
fn one_atomic_batch_has_shared_font_upright_rotation_and_source_preservation() {
    let source = fixture();
    let original = Document::load_mem(&source).unwrap();
    let mut pdf = PdfDocument::from_bytes(&source, None).unwrap();
    let geometry = pdf.info().unwrap().pages;
    pdf.add_page_numbers(&request(), FONT).unwrap();
    let output = pdf.verified_bytes().unwrap();
    assert_eq!(pdf.info().unwrap().pages, geometry);
    assert_eq!(pdf.info().unwrap().version, "1.4");
    let after = Document::load_mem(&output).unwrap();
    let pages = original.get_pages();
    for (id, value) in &original.objects {
        if !pages.values().any(|p| p == id) && value.type_name().ok() != Some(b"XRef") {
            assert_eq!(after.get_object(*id).unwrap(), value);
        }
    }
    let mut fonts = std::collections::BTreeSet::new();
    for page in 1..=4 {
        let text = pdf.extract_text(&[page]).unwrap();
        assert!(text.contains(&(97 + page).to_string()), "{text}");
        let rendered = pdf_render::render_page(&output, page as usize - 1, 1.0).unwrap();
        let mut red = 0;
        for (index, pixel) in rendered.rgba.chunks_exact(4).enumerate() {
            if pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80 {
                red += 1;
                assert!(
                    index / (rendered.width as usize) < 35,
                    "number outside header"
                );
            }
        }
        assert!(red > 10);
        let page = after.get_dictionary(*pages.get(&page).unwrap()).unwrap();
        let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
        let form = resources
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"NativeTextOverlay")
            .unwrap()
            .as_reference()
            .unwrap();
        let form = after.get_object(form).unwrap().as_stream().unwrap();
        fonts.insert(
            form.dict
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"Font")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"OverlayFont")
                .unwrap()
                .as_reference()
                .unwrap(),
        );
    }
    assert_eq!(fonts.len(), 1, "one font per batch");
}
#[test]
fn failed_last_page_duplicate_overflow_bad_font_and_fit_leave_source_unchanged() {
    let mut pdf = PdfDocument::from_bytes(&fixture(), None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    for bad in [
        PageNumbering {
            pages: vec![1, 1],
            ..request()
        },
        PageNumbering {
            start: u64::MAX,
            ..request()
        },
        PageNumbering {
            height: 500.0,
            ..request()
        },
        PageNumbering {
            size: 1024.0,
            ..request()
        },
        PageNumbering {
            pages: vec![1, 5],
            ..request()
        },
    ] {
        assert!(pdf.add_page_numbers(&bad, FONT).is_err());
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
    assert!(pdf.add_page_numbers(&request(), b"invalid font").is_err());
    assert_eq!(pdf.verified_bytes().unwrap(), before);
    let mut donor = Document::load_mem(&fixture()).unwrap();
    let last = *donor.get_pages().get(&4).unwrap();
    donor
        .get_dictionary_mut(last)
        .unwrap()
        .set("CropBox", vec![30.into(), 40.into(), 40.into(), 50.into()]);
    let mut bytes = Vec::new();
    donor.save_to(&mut bytes).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    assert!(pdf.add_page_numbers(&request(), FONT).is_err());
    assert_eq!(pdf.verified_bytes().unwrap(), before);
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_numbering_pixels_text_and_preserved_artwork() {
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
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("before.pdf");
    let output = dir.path().join("numbered.pdf");
    std::fs::write(&input, fixture()).unwrap();
    let mut pdf = PdfDocument::from_bytes(&fixture(), None).unwrap();
    pdf.add_page_numbers(&request(), FONT).unwrap();
    pdf.save_new(&output).unwrap();
    assert!(
        Command::new(&qpdf)
            .arg("--check")
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    for page in 1..=4 {
        let render = |file: &std::path::Path| {
            let o = Command::new(bin.join("pdftoppm"))
                .args([
                    "-cropbox",
                    "-singlefile",
                    "-r",
                    "72",
                    "-f",
                    &page.to_string(),
                    "-l",
                    &page.to_string(),
                ])
                .arg(file)
                .output()
                .unwrap();
            assert!(o.status.success());
            o.stdout
        };
        let before = render(&input);
        let after = render(&output);
        let start = after
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .nth(2)
            .unwrap()
            .0
            + 1;
        assert_eq!(&before[..start], &after[..start]);
        let width = if page % 2 == 1 { 260 } else { 360 };
        let mut changed = 0;
        for (i, (a, b)) in before[start..]
            .chunks_exact(3)
            .zip(after[start..].chunks_exact(3))
            .enumerate()
        {
            if a != b {
                changed += 1;
                assert!(i / width < 35, "outside header artwork changed");
            }
        }
        assert!(changed > 10);
        let text = Command::new(bin.join("pdftotext"))
            .args(["-f", &page.to_string(), "-l", &page.to_string()])
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        assert_eq!(
            String::from_utf8(text.stdout)
                .unwrap()
                .trim_end_matches(['\n', '\u{c}']),
            (97 + page).to_string()
        );
    }
}
