#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use pdf_core::{ButtonChange, ButtonSelection, FormFieldKind, FormValue, PdfDocument};
use std::path::Path;
use std::process::Command;

fn change(name: &str, widget: ObjectId) -> ButtonChange {
    ButtonChange {
        name: name.into(),
        selection: ButtonSelection::On { widget },
    }
}
fn bytes(document: &mut Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
fn source(pdf: &PdfDocument) -> Document {
    Document::load_mem(&pdf.verified_bytes().unwrap()).unwrap()
}

fn radio(flags: u32, duplicate: bool) -> (PdfDocument, Document, ObjectId, [ObjectId; 2]) {
    let mut doc = Document::with_version("1.7");
    // Plain xref keeps transport carriers out of exact logical-object assertions.
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();
    let page = doc.new_object_id();
    let root_field = doc.new_object_id();
    let terminal = doc.new_object_id();
    let off=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),30.into(),30.into()],"Resources"=>dictionary! {}},b"q Q".to_vec()));
    let on=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),30.into(),30.into()],"Resources"=>dictionary! {}},b"q 1 0 0 rg 0 0 30 30 re f Q".to_vec()));
    let mut widgets = Vec::new();
    for (index, name) in ["Mail", if duplicate { "Mail" } else { "Email" }]
        .into_iter()
        .enumerate()
    {
        let x = 10 + index as i64 * 50;
        let normal = doc.add_object(dictionary! {"Off"=>off,name=>on});
        let widget=doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","Parent"=>terminal,"P"=>page,"Rect"=>vec![x.into(),10.into(),(x+30).into(),40.into()],"F"=>4,"AS"=>"Off","AP"=>dictionary! {"N"=>normal},"CustomWidget"=>"retained"});
        widgets.push(widget);
    }
    let annots = doc.add_object(
        widgets
            .iter()
            .copied()
            .map(Object::Reference)
            .collect::<Vec<_>>(),
    );
    doc.objects.insert(root_field,dictionary! {"T"=>Object::string_literal("account"),"FT"=>"Btn","Ff"=>flags,"Kids"=>vec![terminal.into()]}.into());
    doc.objects.insert(terminal,dictionary! {"Parent"=>root_field,"T"=>Object::string_literal("delivery"),"V"=>"Off","Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>(),"Keep"=>123}.into());
    doc.objects.insert(page,dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),120.into(),80.into()],"Resources"=>dictionary! {},"Annots"=>annots}.into());
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let root_alias = doc.add_object(Object::Reference(root_field));
    let catalog=doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary! {"Fields"=>vec![root_alias.into()],"CustomForm"=>Object::string_literal("retained")}});
    doc.trailer.set("Root", catalog);
    let data = bytes(&mut doc);
    (
        PdfDocument::from_bytes(&data, None).unwrap(),
        Document::load_mem(&data).unwrap(),
        terminal,
        [widgets[0], widgets[1]],
    )
}

fn assert_only_states_changed(
    before: &Document,
    after: &Document,
    field: ObjectId,
    widgets: &[ObjectId],
) {
    assert_eq!(before.objects.len(), after.objects.len());
    for (id, object) in &before.objects {
        let mut old = object.clone();
        let mut new = after.objects[id].clone();
        if *id == field {
            old.as_dict_mut().unwrap().remove(b"V");
            new.as_dict_mut().unwrap().remove(b"V");
        }
        if widgets.contains(id) {
            old.as_dict_mut().unwrap().remove(b"AS");
            new.as_dict_mut().unwrap().remove(b"AS");
        }
        assert_eq!(old, new, "unrelated object or key changed: {id:?}");
    }
}

#[test]
fn actual_forms_fixture_fills_checkbox_and_radio_preserving_every_other_object() {
    let input = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
    let mut pdf = PdfDocument::from_bytes(input, None).unwrap();
    let fields = pdf.list_form_fields().unwrap();
    assert_eq!(fields.len(), 5);
    let checkbox = fields
        .iter()
        .find(|field| field.name == "approved")
        .unwrap();
    assert_eq!(checkbox.kind, FormFieldKind::CheckBox);
    assert!(checkbox.writable_button);
    let radio = fields
        .iter()
        .find(|field| field.name == "delivery_method")
        .unwrap();
    assert_eq!(radio.kind, FormFieldKind::RadioButton);
    assert!(radio.writable_button);
    assert!(
        fields
            .iter()
            .filter(|field| matches!(field.kind, FormFieldKind::Text | FormFieldKind::Choice))
            .all(|field| !field.writable_button)
    );
    let before = Document::load_mem(input).unwrap();
    pdf.fill_buttons(&[
        ButtonChange {
            name: "approved".into(),
            selection: ButtonSelection::Off,
        },
        change("delivery_method", radio.widgets[0].object_id),
    ])
    .unwrap();
    let after = source(&pdf);
    let mut expected = before.clone();
    expected
        .get_dictionary_mut(checkbox.object_id)
        .unwrap()
        .set("V", "Off");
    expected
        .get_dictionary_mut(checkbox.object_id)
        .unwrap()
        .set("AS", "Off");
    expected
        .get_dictionary_mut(radio.object_id)
        .unwrap()
        .set("V", "Yes");
    expected
        .get_dictionary_mut(radio.object_id)
        .unwrap()
        .set("AS", "Yes");
    assert_eq!(after.objects, expected.objects);
    let current = pdf.list_form_fields().unwrap();
    assert_eq!(
        current
            .iter()
            .find(|field| field.name == "approved")
            .unwrap()
            .value,
        FormValue::Name(b"Off".to_vec())
    );
    assert_eq!(
        current
            .iter()
            .find(|field| field.name == "delivery_method")
            .unwrap()
            .value,
        FormValue::Name(b"Yes".to_vec())
    );
    let old = pdf_render::render_page(input, 0, 1.0).unwrap();
    let new = pdf_render::render_page(&pdf.verified_bytes().unwrap(), 0, 1.0).unwrap();
    assert_eq!([old.width, old.height], [new.width, new.height]);
    assert_ne!(
        old.rgba, new.rgba,
        "button filling must change visible appearance"
    );
    // Other text/choice widget rows and all page content outside the buttons are identical.
    let mut changed_checkbox = 0;
    let mut changed_radio = 0;
    for y in 0..old.height {
        for x in 0..old.width {
            let checkbox = (72..92).contains(&x) && (145..165).contains(&y);
            let radio = (72..92).contains(&x) && (290..310).contains(&y);
            let offset = (y as usize * old.width as usize + x as usize) * 4;
            if old.rgba[offset..offset + 4] != new.rgba[offset..offset + 4] {
                changed_checkbox += usize::from(checkbox);
                changed_radio += usize::from(radio);
            }
            if !checkbox && !radio {
                let offset = (y as usize * old.width as usize + x as usize) * 4;
                assert_eq!(
                    &old.rgba[offset..offset + 4],
                    &new.rgba[offset..offset + 4],
                    "unexpected pixel change {x},{y}"
                );
            }
        }
    }
    assert!(
        changed_checkbox > 0 && changed_radio > 0,
        "each button must visibly change"
    );
}

#[test]
fn inherited_radio_flags_names_aliases_and_exclusivity_are_preserved() {
    let (mut pdf, before, field, widgets) = radio(1 << 15, false);
    let discovered = pdf.list_form_fields().unwrap();
    assert_eq!(discovered[0].name, "account.delivery");
    assert_eq!(discovered[0].flags, 1 << 15);
    pdf.fill_buttons(&[change("account.delivery", widgets[0])])
        .unwrap();
    pdf.fill_buttons(&[change("account.delivery", widgets[1])])
        .unwrap();
    let current = pdf.list_form_fields().unwrap();
    assert_eq!(current[0].value, FormValue::Name(b"Email".to_vec()));
    assert_eq!(
        current[0].widgets[0].appearance_state.as_deref(),
        Some(b"Off".as_slice())
    );
    assert_eq!(
        current[0].widgets[1].appearance_state.as_deref(),
        Some(b"Email".as_slice())
    );
    assert_only_states_changed(&before, &source(&pdf), field, &widgets);
}

#[test]
fn inherited_value_is_overridden_on_the_terminal_without_changing_ancestor_defaults() {
    let (_, mut document, field, widgets) = radio(1 << 15, false);
    let parent = document
        .get_dictionary(field)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_dictionary_mut(parent)
        .unwrap()
        .set("V", "Mail");
    document.get_dictionary_mut(field).unwrap().remove(b"V");
    document
        .get_dictionary_mut(widgets[0])
        .unwrap()
        .set("AS", "Mail");
    let raw = bytes(&mut document);
    let before = Document::load_mem(&raw).unwrap();
    let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
    assert_eq!(
        pdf.list_form_fields().unwrap()[0].value,
        FormValue::Name(b"Mail".to_vec())
    );
    pdf.fill_buttons(&[change("account.delivery", widgets[1])])
        .unwrap();
    let after = source(&pdf);
    assert_eq!(
        after
            .get_dictionary(parent)
            .unwrap()
            .get(b"V")
            .unwrap()
            .as_name()
            .unwrap(),
        b"Mail"
    );
    assert_only_states_changed(&before, &after, field, &widgets);
}

#[test]
fn equal_export_radio_widgets_are_exclusive_or_unison_as_the_flags_require() {
    for unison in [false, true] {
        let flags = (1 << 15) | if unison { 1 << 25 } else { 0 };
        let (mut pdf, _, _, widgets) = radio(flags, true);
        pdf.fill_buttons(&[change("account.delivery", widgets[1])])
            .unwrap();
        let fields = pdf.list_form_fields().unwrap();
        assert_eq!(
            fields[0]
                .widgets
                .iter()
                .filter(|widget| widget.appearance_state.as_deref() == Some(b"Mail".as_slice()))
                .count(),
            if unison { 2 } else { 1 }
        );
        assert_eq!(
            fields[0].widgets[1].appearance_state.as_deref(),
            Some(b"Mail".as_slice())
        );
        pdf.fill_buttons(&[ButtonChange {
            name: "account.delivery".into(),
            selection: ButtonSelection::Off,
        }])
        .unwrap();
        assert!(
            pdf.list_form_fields().unwrap()[0]
                .widgets
                .iter()
                .all(|widget| widget.appearance_state.as_deref() == Some(b"Off".as_slice()))
        );
    }
}

#[test]
fn no_toggle_required_readonly_and_failed_batch_keep_original_intact() {
    for flags in [(1 << 15) | (1 << 14), (1 << 15) | 2, (1 << 15) | 1] {
        let (mut pdf, _, _, widgets) = radio(flags, false);
        if flags & 1 == 0 {
            pdf.fill_buttons(&[change("account.delivery", widgets[0])])
                .unwrap();
        }
        let before = source(&pdf);
        assert!(
            pdf.fill_buttons(&[ButtonChange {
                name: "account.delivery".into(),
                selection: ButtonSelection::Off
            }])
            .is_err()
        );
        assert_eq!(source(&pdf).objects, before.objects);
    }
    let (mut pdf, before, _, widgets) = radio(1 << 15, false);
    assert!(
        pdf.fill_buttons(&[
            change("account.delivery", widgets[0]),
            change("absent", widgets[1])
        ])
        .is_err()
    );
    assert!(
        pdf.fill_buttons(&[change("account.delivery", (999, 0))])
            .is_err()
    );
    assert_eq!(source(&pdf).objects, before.objects);
}

#[test]
fn xfa_actions_needappearances_malformed_states_and_graphs_are_refused_without_partial_changes() {
    for mode in [
        "xfa",
        "need",
        "co",
        "action",
        "state",
        "bbox",
        "cycle",
        "missing",
        "readonly-widget",
        "child-value",
    ] {
        let (_, mut document, field, widgets) = radio(1 << 15, false);
        let catalog = document
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        match mode {
            "xfa"=>document.get_dictionary_mut(catalog).unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("XFA",Object::Array(vec![])),
            "need"=>document.get_dictionary_mut(catalog).unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("NeedAppearances",true),
            "co"=>document.get_dictionary_mut(catalog).unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("CO",Object::Array(vec![])),
            "action"=>document.get_dictionary_mut(field).unwrap().set("AA",dictionary! {"V"=>dictionary! {"S"=>"JavaScript","JS"=>Object::string_literal("unchanged")}}),
            "state"=>document.get_dictionary_mut(widgets[0]).unwrap().set("AS","Mail"),
            "bbox"=> {let appearance=document.objects.values_mut().find_map(|object|object.as_stream_mut().ok()).unwrap();appearance.dict.set("BBox",vec![0.into(),0.into(),0.into(),0.into()]);},
            "cycle"=>document.get_dictionary_mut(field).unwrap().set("Kids",vec![field.into()]),
            "missing"=>document.get_dictionary_mut(field).unwrap().set("Parent",Object::Reference((999,0))),
            "readonly-widget"=>document.get_dictionary_mut(widgets[0]).unwrap().set("F",128),
            "child-value"=>document.get_dictionary_mut(widgets[0]).unwrap().set("V","Off"),
            _=>unreachable!(),
        }
        let raw = bytes(&mut document);
        let before = Document::load_mem(&raw).unwrap();
        let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
        assert!(
            pdf.fill_buttons(&[change("account.delivery", widgets[0])])
                .is_err(),
            "{mode}"
        );
        assert_eq!(source(&pdf).objects, before.objects, "{mode}");
    }
}

#[test]
fn unicode_names_and_password_field_discovery_are_bounded_and_private() {
    let (_, mut doc, field, _) = radio(1 << 15, false);
    let parent = doc
        .get_dictionary(field)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut utf16 = Vec::new();
    lopdf::encode_utf16_be("客户", &mut utf16);
    doc.get_dictionary_mut(parent)
        .unwrap()
        .set("T", Object::String(utf16, lopdf::StringFormat::Hexadecimal));
    let pdf = PdfDocument::from_bytes(&bytes(&mut doc), None).unwrap();
    assert_eq!(pdf.list_form_fields().unwrap()[0].name, "客户.delivery");
    doc.get_dictionary_mut(field).unwrap().set("FT", "Tx");
    doc.get_dictionary_mut(field).unwrap().set("Ff", 1 << 13);
    doc.get_dictionary_mut(field)
        .unwrap()
        .set("V", Object::string_literal("private test value"));
    let pdf = PdfDocument::from_bytes(&bytes(&mut doc), None).unwrap();
    assert_eq!(
        pdf.list_form_fields().unwrap()[0].value,
        FormValue::PasswordOmitted
    );
    assert!(
        !serde_json::to_string(&pdf.list_form_fields().unwrap())
            .unwrap()
            .contains("private test value")
    );
}

#[test]
fn signed_and_decrypted_encrypted_forms_are_readable_but_not_writable() {
    let (_, mut document, _, widgets) = radio(1 << 15, false);
    let signature = document.add_object(dictionary! {"Type"=>"Sig"});
    document
        .trailer
        .set("UnreachableSignatureEvidence", signature);
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut document), None).unwrap();
    assert!(!pdf.list_form_fields().unwrap()[0].writable_button);
    let before = pdf.list_form_fields().unwrap();
    assert!(
        pdf.fill_buttons(&[change("account.delivery", widgets[0])])
            .is_err()
    );
    assert_eq!(pdf.list_form_fields().unwrap(), before);
    let (_, mut document, _, widgets) = radio(1 << 15, false);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("forms-test-id-001"),
            Object::string_literal("forms-test-id-001"),
        ],
    );
    let encryption = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &document,
        owner_password: "owner",
        user_password: "reader",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    document.encrypt(&encryption).unwrap();
    let mut pdf = PdfDocument::from_bytes(&bytes(&mut document), Some("reader")).unwrap();
    assert_eq!(pdf.list_form_fields().unwrap()[0].name, "account.delivery");
    assert!(!pdf.list_form_fields().unwrap()[0].writable_button);
    let before = pdf.info().unwrap();
    assert!(
        pdf.fill_buttons(&[change("account.delivery", widgets[0])])
            .is_err()
    );
    assert_eq!(pdf.info().unwrap().pages, before.pages);
}

#[test]
fn aliases_cycles_duplicate_placements_and_inheritance_depth_fail_with_controlled_errors() {
    for mode in [
        "alias-cycle",
        "duplicate-placement",
        "deep",
        "duplicate-name",
    ] {
        let (_, mut document, field, widgets) = radio(1 << 15, false);
        let catalog = document
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        match mode {
            "alias-cycle" => {
                let alias = document.new_object_id();
                document.objects.insert(alias, Object::Reference(alias));
                document
                    .get_dictionary_mut(catalog)
                    .unwrap()
                    .get_mut(b"AcroForm")
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("Fields", vec![alias.into()]);
            }
            "duplicate-placement" => {
                let pages = document
                    .get_dictionary(catalog)
                    .unwrap()
                    .get(b"Pages")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let page = document
                    .get_dictionary(pages)
                    .unwrap()
                    .get(b"Kids")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .as_reference()
                    .unwrap();
                document.get_dictionary_mut(page).unwrap().set(
                    "Annots",
                    vec![widgets[0].into(), widgets[0].into(), widgets[1].into()],
                );
            }
            "deep" => {
                let mut root = document
                    .get_dictionary(field)
                    .unwrap()
                    .get(b"Parent")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                for _ in 0..129 {
                    let parent = document.add_object(dictionary! {"Kids"=>vec![root.into()]});
                    document
                        .get_dictionary_mut(root)
                        .unwrap()
                        .set("Parent", parent);
                    root = parent;
                }
                document
                    .get_dictionary_mut(catalog)
                    .unwrap()
                    .get_mut(b"AcroForm")
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("Fields", vec![root.into()]);
            }
            "duplicate-name" => {
                let root = document
                    .get_dictionary(field)
                    .unwrap()
                    .get(b"Parent")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let mut sibling = document.get_dictionary(field).unwrap().clone();
                sibling.remove(b"Kids");
                let sibling = document.add_object(sibling);
                document
                    .get_dictionary_mut(root)
                    .unwrap()
                    .set("Kids", vec![field.into(), sibling.into()]);
            }
            _ => unreachable!(),
        }
        let raw = bytes(&mut document);
        let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
        if mode == "duplicate-name" {
            assert!(
                pdf.list_form_fields()
                    .unwrap()
                    .iter()
                    .all(|field| !field.writable_button)
            );
        } else {
            assert!(pdf.list_form_fields().is_err(), "{mode}");
        }
        assert!(
            pdf.fill_buttons(&[change("account.delivery", widgets[0])])
                .is_err()
        );
        assert_eq!(
            source(&pdf).objects,
            Document::load_mem(&raw).unwrap().objects
        );
    }
}

#[test]
fn shared_large_values_and_appearance_decoding_limits_fail_before_any_button_changes() {
    let (_, mut document, field, widgets) = radio(1 << 15, false);
    let parent = document
        .get_dictionary(field)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let value = document.add_object(Object::string_literal("x".repeat(64 * 1024)));
    let mut kids = vec![Object::Reference(field)];
    for index in 0..65 {
        let child=document.add_object(dictionary! {"Parent"=>parent,"FT"=>"Tx","T"=>Object::string_literal(format!("text{index}")),"V"=>value});
        kids.push(child.into());
    }
    document
        .get_dictionary_mut(parent)
        .unwrap()
        .set("Kids", kids);
    let raw = bytes(&mut document);
    let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
    assert!(matches!(
        pdf.list_form_fields(),
        Err(pdf_core::Error::Limit(_))
    ));
    assert!(matches!(
        pdf.fill_buttons(&[change("account.delivery", widgets[0])]),
        Err(pdf_core::Error::Limit(_))
    ));
    assert_eq!(
        source(&pdf).objects,
        Document::load_mem(&raw).unwrap().objects
    );
    let (_, mut document, _, widgets) = radio(1 << 15, false);
    let stream = document
        .objects
        .values_mut()
        .find_map(|object| object.as_stream_mut().ok())
        .unwrap();
    stream.content = vec![b' '; pdf_core::MAX_STREAM_BYTES + 1];
    stream.dict.set("Length", stream.content.len() as i64);
    stream.compress().unwrap();
    let raw = bytes(&mut document);
    let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
    assert!(matches!(
        pdf.list_form_fields(),
        Err(pdf_core::Error::Limit(_))
    ));
    assert!(
        pdf.fill_buttons(&[change("account.delivery", widgets[0])])
            .is_err()
    );
    assert_eq!(
        source(&pdf).objects,
        Document::load_mem(&raw).unwrap().objects
    );
}

#[test]
fn button_wire_changes_reject_unknown_fields_and_malformed_unit_selection() {
    for invalid in [
        serde_json::json!({"name":"approved","selection":"Off","path":"not-allowed"}),
        serde_json::json!({"name":"approved","selection":{"On":{"widget":[10,0],"path":"not-allowed"}}}),
        serde_json::json!({"name":"approved","selection":{"Off":{"path":"not-allowed"}}}),
        serde_json::json!({"name":"approved","selection":{"On":{"widget":[10,0]},"Off":null}}),
    ] {
        assert!(serde_json::from_value::<ButtonChange>(invalid).is_err());
    }
    for selection in [
        ButtonSelection::Off,
        ButtonSelection::On { widget: (10, 0) },
    ] {
        let change = ButtonChange {
            name: "approved".into(),
            selection,
        };
        let encoded = serde_json::to_vec(&change).unwrap();
        assert_eq!(
            serde_json::from_slice::<ButtonChange>(&encoded).unwrap(),
            change
        );
    }
}

#[test]
fn page_widgets_missing_from_kids_and_widget_ancestors_cannot_leave_stale_appearances() {
    for ancestor in [false, true] {
        let (_, mut document, field, widgets) = radio(1 << 15, false);
        let page = document
            .get_dictionary(widgets[0])
            .unwrap()
            .get(b"P")
            .unwrap()
            .as_reference()
            .unwrap();
        let annots = document
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_reference()
            .unwrap();
        let unexpected = if ancestor {
            let parent = document
                .get_dictionary(field)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            let rectangle = document
                .get_dictionary(widgets[0])
                .unwrap()
                .get(b"Rect")
                .unwrap()
                .clone();
            document
                .get_dictionary_mut(parent)
                .unwrap()
                .set("Subtype", "Widget");
            document
                .get_dictionary_mut(parent)
                .unwrap()
                .set("Rect", rectangle);
            parent
        } else {
            let widget = document.get_dictionary(widgets[0]).unwrap().clone();
            document.add_object(widget)
        };
        document
            .objects
            .get_mut(&annots)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(unexpected.into());
        let raw = bytes(&mut document);
        let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
        assert!(pdf.list_form_fields().is_err());
        assert!(
            pdf.fill_buttons(&[change("account.delivery", widgets[0])])
                .is_err()
        );
        assert_eq!(
            source(&pdf).objects,
            Document::load_mem(&raw).unwrap().objects
        );
    }
}

#[test]
fn aliased_widget_placements_and_choice_value_strings_resolve_without_losing_identity() {
    let (_, mut document, field, widgets) = radio(1 << 15, false);
    let page = document
        .get_dictionary(widgets[0])
        .unwrap()
        .get(b"P")
        .unwrap()
        .as_reference()
        .unwrap();
    let annots = document
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_reference()
        .unwrap();
    let alias = document.add_object(Object::Reference(widgets[0]));
    document
        .objects
        .get_mut(&annots)
        .unwrap()
        .as_array_mut()
        .unwrap()[0] = alias.into();
    let raw = bytes(&mut document);
    let before = Document::load_mem(&raw).unwrap();
    let mut pdf = PdfDocument::from_bytes(&raw, None).unwrap();
    assert_eq!(
        pdf.list_form_fields().unwrap()[0].widgets[0].object_id,
        widgets[0]
    );
    pdf.fill_buttons(&[change("account.delivery", widgets[0])])
        .unwrap();
    assert_only_states_changed(&before, &source(&pdf), field, &widgets);
    let first = document.add_object(Object::string_literal("First"));
    let second = document.add_object(Object::string_literal("Second"));
    document.get_dictionary_mut(field).unwrap().set("FT", "Ch");
    document.get_dictionary_mut(field).unwrap().set(
        "V",
        vec![Object::Reference(first), Object::Reference(second)],
    );
    let pdf = PdfDocument::from_bytes(&bytes(&mut document), None).unwrap();
    assert_eq!(
        pdf.list_form_fields().unwrap()[0].value,
        FormValue::TextArray(vec!["First".into(), "Second".into()])
    );
}

#[test]
#[ignore = "requires explicit pinned qpdf12.4.2 and Poppler26.10 reference tools"]
fn independent_qpdf_values_and_poppler_widget_pixels_verify_actual_fixture() {
    let qpdf = std::env::var_os("PDF_FORM_QPDF")
        .ok_or("set PDF_FORM_QPDF to pinned qpdf12.4.2")
        .unwrap();
    let poppler = std::env::var_os("PDF_FORM_POPPLER_BIN")
        .ok_or("set PDF_FORM_POPPLER_BIN to pinned Poppler26.10 bin directory")
        .unwrap();
    assert!(
        String::from_utf8(
            Command::new(&qpdf)
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .unwrap()
        .contains("qpdf version 12.4.2")
    );
    let temp = tempfile::tempdir().unwrap();
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdfs/forms.pdf");
    let mut pdf = PdfDocument::open(&input, None).unwrap();
    let fields = pdf.list_form_fields().unwrap();
    let radio = fields
        .iter()
        .find(|field| field.name == "delivery_method")
        .unwrap();
    pdf.fill_buttons(&[
        ButtonChange {
            name: "approved".into(),
            selection: ButtonSelection::Off,
        },
        change("delivery_method", radio.widgets[0].object_id),
    ])
    .unwrap();
    let output = temp.path().join("filled.pdf");
    pdf.save_new(&output).unwrap();
    let result = Command::new(&qpdf)
        .args(["--json", "--json-stream-data=inline"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success());
    let model: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let fields = model["acroform"]["fields"].as_array().unwrap();
    assert_eq!(
        fields
            .iter()
            .find(|field| field["fullname"] == "approved")
            .unwrap()["value"],
        "/Off"
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field["fullname"] == "delivery_method")
            .unwrap()["value"],
        "/Yes"
    );
    assert_eq!(model["acroform"]["needappearances"], false);
    let before = Command::new(&qpdf)
        .args(["--json", "--json-stream-data=inline"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(before.status.success());
    let original: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
    let mut old = original["qpdf"][1].as_object().unwrap().clone();
    let mut new = model["qpdf"][1].as_object().unwrap().clone();
    // Transport trailer/xref data can differ; every logical object, including
    // orphaned objects and decoded appearance streams, remains checked.
    old.remove("trailer");
    new.remove("trailer");
    old.retain(|_, value| value["stream"]["dict"]["/Type"] != "/XRef");
    new.retain(|_, value| value["stream"]["dict"]["/Type"] != "/XRef");
    for id in ["obj:10 0 R", "obj:18 0 R"] {
        for objects in [&mut old, &mut new] {
            let dictionary = objects[id]["value"].as_object_mut().unwrap();
            dictionary.remove("/V");
            dictionary.remove("/AS");
        }
    }
    assert_eq!(old, new, "independent logical-object preservation failed");
    let renderer = Path::new(&poppler).join("pdftoppm");
    let version = Command::new(&renderer).arg("-v").output().unwrap();
    assert!(
        String::from_utf8(version.stderr)
            .unwrap()
            .contains("26.10.0")
    );
    let mut images = Vec::new();
    for (name, path) in [("before", &input), ("after", &output)] {
        let prefix = temp.path().join(name);
        assert!(
            Command::new(&renderer)
                .args(["-r", "72", "-f", "1", "-singlefile"])
                .arg(path)
                .arg(&prefix)
                .status()
                .unwrap()
                .success()
        );
        images.push(std::fs::read(prefix.with_extension("ppm")).unwrap());
    }
    assert_ne!(
        images[0], images[1],
        "independent renderer did not display changed button states"
    );
    assert_eq!(
        std::fs::read(&input).unwrap(),
        include_bytes!("../../../tests/fixtures/pdfs/forms.pdf")
    );
}
