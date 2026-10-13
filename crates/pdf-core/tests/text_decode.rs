#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;

fn fixture(target: &str) -> Vec<u8> {
    let cmap = format!(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /SyntheticBoundedUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n1 beginbfchar\n<0001> <{target}>\nendbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n"
    );
    let mut doc = Document::with_version("1.4");
    let pages = doc.new_object_id();
    let map = doc.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
    let descendant = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>"Synthetic", "CIDSystemInfo"=>dictionary! {"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0}});
    let font = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"Synthetic", "Encoding"=>"Identity-H", "DescendantFonts"=>vec![descendant.into()], "ToUnicode"=>map});
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 12 Tf <{}> Tj ET", "0001".repeat(32)).into_bytes(),
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
