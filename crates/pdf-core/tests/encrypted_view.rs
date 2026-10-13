#![allow(clippy::unwrap_used)]
#[path = "../../pdf-worker/tests/support/encryption.rs"]
mod encryption;

#[test]
fn authenticated_read_only_rasters_match_original_across_verified_encryption_tiers() {
    for revision in [2, 3, 4, 6] {
        let (cipher, plain) = encryption::fixture(revision, "synthetic-reader", false);
        let before = cipher.clone();
        let mut document =
            pdf_core::PdfDocument::from_bytes(&cipher, Some("synthetic-reader")).unwrap();
        assert!(document.info().unwrap().encrypted);
        assert_eq!(
            document.render_decrypted_page(0, 1.0, 1.0).unwrap(),
            pdf_render::render_page(&plain, 0, 1.0).unwrap()
        );
        assert!(document.to_bytes().is_err());
        assert!(document.rotate(&[1], 90).is_err());
        assert!(document.render_decrypted_page(1, 1.0, 1.0).is_err());
        assert!(document.render_decrypted_page(0, 0.0, 1.0).is_err());
        assert_eq!(cipher, before);
    }
    let (_, plain) = encryption::fixture(3, "", false);
    let plain = pdf_core::PdfDocument::from_bytes(&plain, None).unwrap();
    assert!(plain.render_decrypted_page(0, 1.0, 1.0).is_err());
}
