#![allow(clippy::unwrap_used)]
use lopdf::{Document, Object};
use pdf_core::{Error, FormValue, PdfDocument};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
fn encode(document: &mut Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn undefined_pdfdocencoding_in_names_values_and_choice_arrays_is_refused_without_changes() {
    for mode in ["name", "text-value", "choice-value", "choice-array"] {
        let mut document = Document::load_mem(SOURCE).unwrap();
        let bad = Object::string_literal(b"valid\x00lost".to_vec());
        match mode {
            "name" => document.get_dictionary_mut((8, 0)).unwrap().set("T", bad),
            "text-value" => document.get_dictionary_mut((8, 0)).unwrap().set("V", bad),
            "choice-value" => document.get_dictionary_mut((14, 0)).unwrap().set("V", bad),
            "choice-array" => {
                let reference = document.add_object(bad);
                document
                    .get_dictionary_mut((14, 0))
                    .unwrap()
                    .set("V", vec![Object::string_literal("valid"), reference.into()]);
            }
            _ => unreachable!(),
        }
        let pdf = PdfDocument::from_bytes(&encode(&mut document), None).unwrap();
        let before = pdf.to_bytes().unwrap();
        assert!(
            matches!(
                pdf.list_form_fields(),
                Err(Error::Refused("undefined PDFDocEncoding character"))
            ),
            "accepted {mode}"
        );
        assert_eq!(
            pdf.to_bytes().unwrap(),
            before,
            "inspection changed source for {mode}"
        );
    }
}
#[test]
fn valid_pdfdocencoding_names_and_values_keep_their_unicode_characters() {
    let mut document = Document::load_mem(SOURCE).unwrap();
    let field = document.get_dictionary_mut((8, 0)).unwrap();
    field.set("T", Object::string_literal(b"caf\xe9".to_vec()));
    field.set("V", Object::string_literal(b"\x18 caf\xe9".to_vec()));
    let pdf = PdfDocument::from_bytes(&encode(&mut document), None).unwrap();
    let field = pdf
        .list_form_fields()
        .unwrap()
        .into_iter()
        .find(|field| field.name == "café")
        .unwrap();
    assert_eq!(field.value, FormValue::Text("˘ café".into()));
}
