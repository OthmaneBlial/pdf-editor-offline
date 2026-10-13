//! Independent two-page plaintext PDF with one shared RGB image asset.
pub const PIXELS: &[u8] = &[200, 40, 80];
pub fn selections(bytes: &[u8]) -> Vec<pdf_core::ImageSelection> {
    let pdf = pdf_core::PdfDocument::from_bytes(bytes, None).unwrap();
    (1..=pdf.page_count() as u32)
        .flat_map(|p| pdf.list_image_instances(p).unwrap())
        .map(|i| i.selection)
        .collect()
}
pub fn control(source: &[u8]) -> Vec<u8> {
    let mut doc = lopdf::Document::load_mem(source).unwrap();
    for id in [(5, 0), (7, 0)] {
        let stream = doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
        let program = String::from_utf8(stream.get_plain_content().unwrap()).unwrap();
        assert_eq!(program.matches("/Im1 Do").count(), 1);
        stream.set_content(program.replace("/Im1 Do", "       ").into_bytes());
    }
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
