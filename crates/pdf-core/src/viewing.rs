use crate::{Error, MAX_INPUT_BYTES, PdfDocument, Result, save};

impl PdfDocument {
    /// Rasterize an authenticated encrypted document without exposing a plaintext PDF.
    ///
    /// Call in a resource-limited child. The bounded plaintext serialization and
    /// verification copies exist only in memory; saving and editing remain refused.
    /// Ordinary and signed unencrypted inputs must use the renderer's original-byte
    /// API so this transient adapter never rewrites their rendering input.
    pub fn render_decrypted_page(
        &self,
        page: usize,
        scale: f32,
        logical_zoom: f32,
    ) -> Result<pdf_render::RenderedPage> {
        if !self.encrypted_input {
            return Err(Error::Refused(
                "decrypted viewing requires an encrypted input",
            ));
        }
        if page >= self.pages.len() {
            return Err(Error::Pages("render page is outside the document"));
        }
        if !scale.is_finite() || scale <= 0.0 || !logical_zoom.is_finite() || logical_zoom <= 0.0 {
            return Err(Error::Invalid(
                "encrypted raster scale and zoom must be finite and positive",
            ));
        }
        let bytes = save::serialize_bounded(&self.document, MAX_INPUT_BYTES)?;
        let reopened = Self::from_bytes(&bytes, None)?;
        if reopened.encrypted_input
            || reopened.info()?.pages != self.info()?.pages
            || reopened.pages != self.pages
        {
            return Err(Error::Verification(
                "decrypted view changed the page structure",
            ));
        }
        save::verify_objects(&self.document, &reopened.document)?;
        pdf_render::render_view_page(&bytes, page, scale, logical_zoom)
            .map_err(|_| Error::Refused("decrypted page rasterization failed"))
    }
}
