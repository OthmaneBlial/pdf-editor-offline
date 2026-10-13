#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;

fn fixture(target: &str) -> Vec<u8> {
    fixture_body(&format!("1 beginbfchar\n<0001> <{target}>\nendbfchar\n"))
}
fn fixture_body(body: &str) -> Vec<u8> {
    fixture_code(body, "0001")
}
fn fixture_code(body: &str, code: &str) -> Vec<u8> {
    let cmap = format!(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /SyntheticBoundedUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n{body}endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n"
    );
    let mut doc = Document::with_version("1.4");
    let pages = doc.new_object_id();
    let map = doc.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
    let descendant = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>"Synthetic", "CIDSystemInfo"=>dictionary! {"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0}});
    let font = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"Synthetic", "Encoding"=>"Identity-H", "DescendantFonts"=>vec![descendant.into()], "ToUnicode"=>map});
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 12 Tf <{}> Tj ET", code.repeat(32)).into_bytes(),
    ));
    let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()], "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}, "Contents"=>content});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    doc.trailer.set("Root", root);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn actual_pdf_expansion_is_exact_and_read_only_without_large_allocations() {
    let bytes = fixture(&"0041".repeat(256));
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    let before = pdf.verified_bytes().unwrap();
    assert_eq!(
        pdf.extract_text(&[1]).unwrap(),
        format!("{}\n", "A".repeat(8192))
    );
    assert_eq!(pdf.verified_bytes().unwrap(), before);
}

#[test]
fn empty_invalid_and_oversized_pdf_mappings_refuse_without_mutation() {
    for target in [String::new(), "not-hex".into(), "0041".repeat(257)] {
        let pdf = PdfDocument::from_bytes(&fixture(&target), None).unwrap();
        let before = pdf.verified_bytes().unwrap();
        assert!(
            pdf.extract_text(&[1]).is_err(),
            "target length {}",
            target.len()
        );
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
}

#[test]
fn valid_scalar_and_array_ranges_keep_exact_strings_and_last_byte_semantics() {
    let large_array = format!("<0000> <0100> [{}]", "<0041> ".repeat(257));
    for (entry, expected) in [
        ("<0000> <0001> <0041>", "B"),
        ("<0000> <0001> <00410042>", "AC"),
        ("<0000> <0001> <00FE>", "ÿ"),
        ("<0000> <0001> [<0041FFFF> <03A9>]", "Ω"),
        (large_array.as_str(), "A"),
    ] {
        let source = fixture_body(&format!("1 beginbfrange\n{entry}\nendbfrange\n"));
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let before = pdf.verified_bytes().unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap(),
            format!("{}\n", expected.repeat(32))
        );
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
}

#[test]
fn undefined_range_arithmetic_and_mismatched_arrays_refuse_before_expansion() {
    for entry in [
        "<0000> <0001> <0041FFFF>",
        "<0000> <0001> <00FF>",
        "<0000> <0100> <0000>",
        "<00000000> <FFFFFFFF> <0000>",
        "<0000> <0002> [<0041> <0042>]",
    ] {
        let source = fixture_body(&format!("1 beginbfrange\n{entry}\nendbfrange\n"));
        let pdf = PdfDocument::from_bytes(&source, None).unwrap();
        let before = pdf.verified_bytes().unwrap();
        assert!(pdf.extract_text(&[1]).is_err(), "{entry}");
        assert_eq!(pdf.verified_bytes().unwrap(), before);
    }
}

#[test]
fn valid_characters_and_split_ranges_preserve_original_mapping_origins() {
    for (body, code, expected) in [
        (
            "2 beginbfchar\n<0000> <00FF>\n<0001> <0100>\nendbfchar\n",
            "0001",
            "Ā",
        ),
        (
            "2 beginbfchar\n<0000> <00660069>\n<0001> <00660069>\nendbfchar\n",
            "0001",
            "fi",
        ),
        (
            "2 beginbfchar\n<0000> <0041FFFF>\n<0001> <0041FFFF>\nendbfchar\n",
            "0001",
            "A\u{ffff}",
        ),
        (
            "1 beginbfrange\n<0000> <0002> <00660069>\nendbfrange\n1 beginbfchar\n<0001> <0058>\nendbfchar\n",
            "0002",
            "fk",
        ),
        (
            "1 beginbfrange\n<0000> <0002> [<0041> <0042> <0043>]\nendbfrange\n1 beginbfchar\n<0001> <0058>\nendbfchar\n",
            "0002",
            "C",
        ),
    ] {
        let pdf = PdfDocument::from_bytes(&fixture_code(body, code), None).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap(),
            format!("{}\n", expected.repeat(32)),
            "{body}"
        );
    }
}
