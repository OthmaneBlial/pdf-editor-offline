use pdf_render::{RenderError, render_page};

#[test]
fn rejects_oversized_input_before_pdf_parsing() {
    let bytes = vec![0; pdf_render::MAX_INPUT_BYTES + 1];
    assert_eq!(render_page(&bytes, 0, 1.0), Err(RenderError::InputTooLarge));
}

fn fixture(media_box: &str, extra: &str) -> Vec<u8> {
    let content = "1 0 0 rg 0 0 50 80 re f";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [{media_box}] {extra} /Resources << >> /Contents 4 0 R >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in &offsets[1..] {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

#[test]
fn renders_actual_pdf_vectors_in_rgba_with_white_background() {
    let image = render_page(&fixture("0 0 100 80", ""), 0, 1.0).expect("render fixture");
    assert_eq!((image.width, image.height), (100, 80));
    assert_eq!(image.rgba.len(), 100 * 80 * 4);
    assert_eq!(&image.rgba[(40 * 100 + 25) * 4..][..4], &[255, 0, 0, 255]);
    assert_eq!(
        &image.rgba[(40 * 100 + 75) * 4..][..4],
        &[255, 255, 255, 255]
    );
    assert!(image.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
}

#[test]
fn applies_scale_crop_and_page_rotation() {
    let pdf = fixture("0 0 100 80", "/CropBox [0 0 60 40] /Rotate 90");
    let image = render_page(&pdf, 0, 1.5).expect("render rotated crop");
    assert_eq!((image.width, image.height), (60, 90));
    assert!(
        image
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 0, 0, 255])
    );
    let fractional =
        render_page(&fixture("0 0 100.1 80.1", ""), 0, 1.0).expect("render fractional page");
    assert_eq!((fractional.width, fractional.height), (101, 81));
}

#[test]
fn rejects_invalid_pdf_missing_pages_and_oversized_output() {
    assert_eq!(
        render_page(b"not a PDF", 0, 1.0),
        Err(RenderError::InvalidPdf)
    );
    assert_eq!(
        render_page(&fixture("0 0 100 80", ""), 1, 1.0),
        Err(RenderError::PageOutOfRange {
            requested: 1,
            page_count: 1
        })
    );
    assert_eq!(
        render_page(&fixture("0 0 1000000 1000000", ""), 0, 1.0),
        Err(RenderError::OutputTooLarge)
    );
}
