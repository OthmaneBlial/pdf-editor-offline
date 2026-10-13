#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;

fn fixture(content: Vec<u8>, property: Option<Object>, nested: bool) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
    if let Some(property) = property {
        let text = doc.add_object(property);
        let props = doc.add_object(dictionary! {"P"=>text});
        resources.set("Properties", props);
    }
    if nested {
        let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>resources.clone()},b"/Span << /ActualText (INNER) >> BDC BT /F1 12 Tf (VISUAL) Tj ET EMC".to_vec()));
        resources.set("XObject", dictionary! {"Child"=>form});
    }
    let stream = doc.add_object(Stream::new(dictionary! {}, content));
    let pages = doc.new_object_id();
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>resources,"Contents"=>stream});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn text(content: &str) -> pdf_core::Result<String> {
    PdfDocument::from_bytes(&fixture(content.as_bytes().to_vec(), None, false), None)?
        .extract_text(&[1])
}
fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn logical_unicode_replaces_visual_glyphs_once_with_surrounding_text() {
    let value = "שלום ffi 😀";
    let utf16 = std::iter::once(0xfeff)
        .chain(value.encode_utf16())
        .map(|v| format!("{v:04X}"))
        .collect::<String>();
    let actual=text(&format!("BT /F1 12 Tf (before ) Tj /Span << /ActualText <{utf16}> >> BDC (WRONG) Tj EMC ( after) Tj ET")).unwrap();
    assert_eq!(norm(&actual), format!("before {value} after"));
}
#[test]
fn outer_span_overrides_nested_spans_and_ordinary_marked_text_survives() {
    assert_eq!(norm(&text("/Span << /ActualText (OUTER) >> BDC /Tx BMC /Span << /ActualText (INNER) >> BDC BT /F1 12 Tf (wrong) Tj ET EMC EMC EMC BT /F1 12 Tf /Tx BMC (plain) Tj EMC /Span << /Lang (en) >> BDC (kept) Tj EMC ET").unwrap()),"OUTERplainkept");
}
#[test]
fn named_indirect_property_and_nested_form_use_semantic_context() {
    let bytes = fixture(
        b"/Span /P BDC /Child Do EMC".to_vec(),
        Some(dictionary! {"ActualText"=>Object::string_literal("PARENT")}.into()),
        true,
    );
    assert_eq!(
        norm(
            &PdfDocument::from_bytes(&bytes, None)
                .unwrap()
                .extract_text(&[1])
                .unwrap()
        ),
        "PARENT"
    );
}
#[test]
fn invalid_properties_and_marked_nesting_fail_instead_of_losing_glyph_text() {
    for content in [
        "EMC",
        "/Tx BMC",
        "/Tx BMC 1 EMC",
        "/Span /Missing BDC EMC",
        "/Span << /ActualText 7 >> BDC EMC",
        "/Span << /ActualText <FEFF00> >> BDC EMC",
        "/Span << /ActualText <FEFFD800> >> BDC EMC",
        "/Span << /ActualText <EFBBBFFF> >> BDC EMC",
        "/Span << /ActualText <FFFE6100> >> BDC EMC",
        "/Span << /ActualText <00> >> BDC EMC",
    ] {
        assert!(text(content).is_err(), "accepted {content}");
    }
    assert!(text(&format!("{}{}", "/Tx BMC ".repeat(129), "EMC ".repeat(129))).is_err());
}
#[test]
fn font_lookup_deferred_only_while_valid_replacement_is_active() {
    assert_eq!(
        norm(
            &text("/Span << /ActualText (logical) >> BDC BT /Missing 12 Tf (raw) Tj ET EMC")
                .unwrap()
        ),
        "logical"
    );
    assert!(
        text(
            "/Span << /ActualText (logical) >> BDC BT /Missing 12 Tf (raw) Tj EMC (outside) Tj ET"
        )
        .is_err()
    );
}
#[test]
fn repeated_shared_property_is_subject_to_aggregate_decoded_byte_limit() {
    let bytes = fixture(
        "/Span /P BDC EMC\n".repeat(33).into_bytes(),
        Some(dictionary! {"ActualText"=>Object::string_literal("a".repeat(1024*1024))}.into()),
        false,
    );
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(matches!(
        pdf.extract_text(&[1]),
        Err(pdf_core::Error::Limit(_))
    ));
}

fn changed(bytes: &[u8], edit: impl FnOnce(&mut Document)) -> Vec<u8> {
    let mut document = Document::load_mem(bytes).unwrap();
    edit(&mut document);
    let mut result = Vec::new();
    document.save_to(&mut result).unwrap();
    result
}
#[test]
fn indirect_actualtext_string_and_split_content_sequences_are_supported() {
    let original = fixture(
        b"/Span /P BDC".to_vec(),
        Some(dictionary! {"ActualText"=>Object::string_literal("logical")}.into()),
        false,
    );
    let bytes = changed(&original, |doc| {
        let text = doc.add_object(Object::string_literal("indirect"));
        for object in doc.objects.values_mut() {
            if let Ok(dict) = object.as_dict_mut()
                && dict.has(b"ActualText")
            {
                dict.set("ActualText", text);
            }
        }
        let suffix = doc.add_object(Stream::new(
            dictionary! {},
            b"BT /Missing 12 Tf (raw) Tj ET EMC".to_vec(),
        ));
        let page = *doc.get_pages().get(&1).unwrap();
        let dict = doc.get_object_mut(page).unwrap().as_dict_mut().unwrap();
        let initial = dict.get(b"Contents").unwrap().clone();
        dict.set("Contents", vec![initial, suffix.into()]);
    });
    assert_eq!(
        norm(
            &PdfDocument::from_bytes(&bytes, None)
                .unwrap()
                .extract_text(&[1])
                .unwrap()
        ),
        "indirect"
    );
}
#[test]
fn cyclic_named_property_reference_is_rejected_without_mutation() {
    let original = fixture(
        b"/Span /P BDC EMC".to_vec(),
        Some(dictionary! {"ActualText"=>Object::string_literal("logical")}.into()),
        false,
    );
    let bytes = changed(&original, |doc| {
        let id = doc
            .objects
            .iter()
            .find_map(|(id, obj)| {
                obj.as_dict()
                    .ok()
                    .filter(|d| d.has(b"ActualText"))
                    .map(|_| *id)
            })
            .unwrap();
        doc.objects.insert(id, Object::Reference(id));
    });
    let pdf = PdfDocument::from_bytes(&bytes, None).unwrap();
    assert!(matches!(
        pdf.extract_text(&[1]),
        Err(pdf_core::Error::Invalid(
            "cyclic ActualText property reference"
        ))
    ));
    assert!(matches!(
        pdf.extract_text(&[1]),
        Err(pdf_core::Error::Invalid(
            "cyclic ActualText property reference"
        ))
    ));
}

#[test]
fn replacement_does_not_hide_malformed_text_operator_operands() {
    for operation in [
        "Tj",
        "(one) (two) Tj",
        "[(one)] Tj",
        "'",
        "(one) (two) '",
        "TJ",
        "(one) TJ",
        "[] [] TJ",
        "[null] TJ",
        "[[(nested)]] TJ",
        "\"",
        "0 (wrong) (text) \"",
        "0 0 [] \"",
        "0 0 (text) (extra) \"",
    ] {
        let stream =
            format!("/Span << /ActualText (replacement) >> BDC BT /F1 12 Tf {operation} ET EMC");
        assert!(text(&stream).is_err(), "accepted malformed {operation}");
    }
    for operation in [
        "(text) Tj",
        "(text) '",
        "[(text) 0 -20.5] TJ",
        "0 -0.5 (text) \"",
    ] {
        assert_eq!(
            norm(
                &text(&format!(
                    "/Span << /ActualText (replacement) >> BDC BT /F1 12 Tf {operation} ET EMC"
                ))
                .unwrap()
            ),
            "replacement"
        );
    }
}
#[test]
fn replacement_text_array_items_count_toward_work_budget() {
    let content = format!(
        "/Span << /ActualText (replacement) >> BDC BT /F1 12 Tf [{}] TJ ET EMC",
        "0 ".repeat(2_000_000)
    );
    assert!(matches!(text(&content), Err(pdf_core::Error::Limit(_))));
}
