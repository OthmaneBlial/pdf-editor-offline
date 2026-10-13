//! Independent minimal test PDF: inherited resources/boxes, rotated pages,
//! visible text and an image. No production writer is used.
pub fn fixture(pages: usize) -> Vec<u8> {
    let kids = (0..pages)
        .map(|page| format!("{} 0 R", 6 + 2 * page))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects=vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),format!("<< /Type /Pages /Count {pages} /Kids [{kids}] /MediaBox [0 0 200 300] /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 4 0 R >> >> >>").into_bytes(),b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()];
    let mut image=b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 3 >>\nstream\n".to_vec();
    image.extend([200, 40, 80]);
    image.extend(b"\nendstream");
    objects.push(image);
    for page in 0..pages {
        let content = format!(
            "BT /F1 20 Tf 30 120 Td (PAGE {}) Tj ET q 30 0 0 30 30 20 cm /Im1 Do Q",
            page + 1
        );
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            )
            .into_bytes(),
        );
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Rotate {} >>",
                5 + 2 * page,
                page % 4 * 90
            )
            .into_bytes(),
        );
    }
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n", index + 1).as_bytes());
        bytes.extend(object);
        bytes.extend(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in offsets.iter().skip(1) {
        bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    bytes
}
