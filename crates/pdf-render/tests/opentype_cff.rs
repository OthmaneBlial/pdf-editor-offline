#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    raw::TableProvider,
};
#[path = "../../pdf-core/tests/fixtures/fonts/cff_fixture.rs"]
mod fixture;
const FONT: &[u8] =
    include_bytes!("../../../apps/desktop/assets/fonts/noto-cjk/NotoSansCJKsc-Regular.otf");

fn pdf(bytes: &[u8], raw_cff: bool, shift: u16) -> Vec<u8> {
    let face = FontRef::new(bytes).unwrap();
    let data = if raw_cff {
        face.cff().unwrap().offset_data().as_bytes()
    } else {
        bytes
    };
    let metrics = face.metrics(Size::unscaled(), LocationRef::default());
    let scale = 1000. / f64::from(metrics.units_per_em);
    let bounds = metrics.bounds.unwrap();
    let number = |value: f32| Object::Real((f64::from(value) * scale) as f32);
    let mut doc = Document::with_version("1.6");
    let catalog = doc.new_object_id();
    let pages = doc.new_object_id();
    let page = doc.new_object_id();
    let program = doc.add_object(Stream::new(
        dictionary! {"Subtype"=>if raw_cff { "CIDFontType0C" } else { "OpenType" }},
        data.to_vec(),
    ));
    let descriptor = doc.add_object(dictionary! {
        "Type"=>"FontDescriptor", "FontName"=>"NotoSansCJKsc-Regular", "Flags"=>4,
        "FontBBox"=>vec![number(bounds.x_min),number(bounds.y_min),number(bounds.x_max),number(bounds.y_max)],
        "ItalicAngle"=>0, "Ascent"=>number(metrics.ascent), "Descent"=>number(metrics.descent),
        "CapHeight"=>number(metrics.cap_height.unwrap()), "StemV"=>80, "FontFile3"=>program,
    });
    let widths = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let gids = ['A', 'B'].map(|value| face.charmap().map(value).unwrap().to_u32() as u16);
    assert_eq!(gids, [34, 35]);
    let mut w = Vec::new();
    for gid in gids {
        w.push(Object::Integer(i64::from(gid + shift)));
        w.push(Object::Array(vec![number(
            widths.advance_width(GlyphId::new(u32::from(gid))).unwrap(),
        )]));
    }
    let descendant = doc.add_object(dictionary! {
        "Type"=>"Font", "Subtype"=>"CIDFontType0", "BaseFont"=>"NotoSansCJKsc-Regular",
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor, "DW"=>0, "W"=>Object::Array(w),
    });
    let font = doc.add_object(dictionary!{"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"NotoSansCJKsc-Regular", "Encoding"=>"Identity-H", "DescendantFonts"=>vec![descendant.into()]});
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!(
            "BT /F 30 Tf 1 0 0 1 40 140 Tm <{:04X}{:04X}> Tj ET",
            gids[0] + shift,
            gids[1] + shift
        )
        .into_bytes(),
    ));
    doc.objects.insert(page, Object::Dictionary(dictionary!{"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()], "Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}, "Contents"=>content}));
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}),
    );
    doc.objects.insert(
        catalog,
        Object::Dictionary(dictionary! {"Type"=>"Catalog", "Pages"=>pages}),
    );
    doc.trailer.set("Root", catalog);
    let mut result = Vec::new();
    doc.save_to(&mut result).unwrap();
    result
}

#[test]
fn complete_opentype_cid_charset_matches_raw_cff_and_identity_outlines() {
    let shifted = fixture::shifted_cids_font(FONT);
    let sources = [
        pdf(FONT, false, 0),
        pdf(&shifted, false, 1),
        pdf(FONT, true, 0),
        pdf(&shifted, true, 1),
    ];
    let originals = sources.clone();
    let images = sources
        .iter()
        .map(|bytes| pdf_render::render_page(bytes, 0, 1.).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(images[0].width, 200);
    assert_eq!(images[0].height, 200);
    assert!(
        images[0]
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [255, 255, 255])
    );
    for image in &images[1..] {
        assert!(
            image == &images[0],
            "CID-keyed OpenType must select the original glyph outlines"
        );
    }
    assert_eq!(sources, originals);
}
