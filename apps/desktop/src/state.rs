use crate::search::{PageText, SearchState};
use eframe::egui::TextureHandle;
use pdf_core::DocumentInfo;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextKey {
    pub tab: u64,
    pub revision: u64,
    pub page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderKey {
    pub tab: u64,
    pub revision: u64,
    pub page: usize,
    pub scale_bits: u32,
    pub zoom_bits: u32,
    pub device_ratio_bits: u32,
    pub pixel_size: [u32; 2],
}

pub struct DocumentStatus {
    pub info: DocumentInfo,
    pub revision: u64,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

pub struct Tab {
    pub id: u64,
    pub path: PathBuf,
    pub status: DocumentStatus,
    pub page: usize,
    pub scale: f32,
    pub device_ratio: f32,
    pub texture: Option<TextureHandle>,
    pub needs_render: bool,
    pub error: Option<String>,
    pub text: Option<PageText>,
    pub text_error: Option<String>,
    pub needs_text: bool,
    pub search: SearchState,
}

impl Tab {
    pub fn new(id: u64, path: PathBuf, status: DocumentStatus) -> Self {
        Self {
            id,
            path,
            status,
            page: 0,
            scale: 1.0,
            device_ratio: 1.0,
            texture: None,
            needs_render: true,
            error: None,
            text: None,
            text_error: None,
            needs_text: true,
            search: SearchState::default(),
        }
    }

    pub fn title(&self) -> String {
        self.path.file_name().map_or_else(
            || "PDF".to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub fn raster_plan(&self) -> Result<crate::raster::RasterPlan, &'static str> {
        let page = self
            .status
            .info
            .pages
            .get(self.page)
            .ok_or("Current page is outside the document.")?;
        crate::raster::plan(page, self.scale, self.device_ratio)
    }

    pub fn key(&self) -> Result<RenderKey, &'static str> {
        let plan = self.raster_plan()?;
        Ok(RenderKey {
            tab: self.id,
            revision: self.status.revision,
            page: self.page,
            scale_bits: plan.scale.to_bits(),
            zoom_bits: self.scale.to_bits(),
            device_ratio_bits: self.device_ratio.to_bits(),
            pixel_size: plan.pixel_size,
        })
    }

    pub fn accepts(&self, key: RenderKey) -> bool {
        self.key().is_ok_and(|current| current == key)
    }

    pub fn text_key(&self) -> TextKey {
        TextKey {
            tab: self.id,
            revision: self.status.revision,
            page: self.page,
        }
    }

    pub fn editable(&self) -> bool {
        !self.status.info.encrypted && self.status.info.signature_fields == 0
    }

    pub fn update(&mut self, status: DocumentStatus) {
        if self.status.revision != status.revision {
            self.texture = None;
            self.needs_render = true;
            self.text = None;
            self.text_error = None;
            self.needs_text = true;
            self.search.cancel();
            self.search.hits.clear();
            self.page = self.page.min(status.info.page_count.saturating_sub(1));
        }
        self.status = status;
    }

    pub fn set_page(&mut self, page: usize) {
        if page < self.status.info.page_count && page != self.page {
            self.page = page;
            self.texture = None;
            self.needs_render = true;
            self.text = None;
            self.text_error = None;
            self.needs_text = true;
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        if !scale.is_finite() {
            return;
        }
        let scale = scale.clamp(0.25, 4.0);
        if self.scale != scale {
            self.scale = scale;
            self.texture = None;
            self.needs_render = true;
            self.error = None;
        }
    }

    pub fn set_device_ratio(&mut self, ratio: f32) {
        if ratio.to_bits() != self.device_ratio.to_bits() {
            self.device_ratio = ratio;
            self.texture = None;
            self.needs_render = true;
            self.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab() -> Tab {
        Tab::new(
            1,
            PathBuf::from("original.pdf"),
            DocumentStatus {
                info: DocumentInfo {
                    version: "1.7".into(),
                    page_count: 3,
                    object_count: 5,
                    encrypted: false,
                    signature_fields: 0,
                    pages: (1..=3)
                        .map(|number| pdf_core::PageInfo {
                            number,
                            media_box: [0.0, 0.0, 100.0, 80.0],
                            crop_box: [0.0, 0.0, 100.0, 80.0],
                            rotation: 0,
                        })
                        .collect(),
                },
                revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        )
    }

    #[test]
    fn stale_render_results_cannot_replace_new_page_zoom_revision_or_tab() {
        let mut tab = tab();
        let key = tab.key().expect("render key");
        assert!(tab.accepts(key));
        tab.set_page(1);
        assert!(!tab.accepts(key));
        let key = tab.key().expect("render key");
        tab.set_scale(2.0);
        assert!(!tab.accepts(key));
        let key = tab.key().expect("render key");
        tab.status.revision = 1;
        assert!(!tab.accepts(key));
        let key = RenderKey {
            tab: 2,
            ..tab.key().expect("render key")
        };
        assert!(!tab.accepts(key));
    }

    #[test]
    fn navigation_stays_in_document_and_zoom_is_bounded() {
        let mut tab = tab();
        tab.set_page(3);
        assert_eq!(tab.page, 0);
        tab.set_scale(0.01);
        assert_eq!(tab.scale, 0.25);
        tab.set_scale(20.0);
        assert_eq!(tab.scale, 4.0);
    }

    #[test]
    fn moving_between_display_scales_invalidates_old_physical_render_keys() {
        let mut tab = tab();
        let old = tab.key().expect("ordinary key");
        tab.set_device_ratio(2.0);
        let retina = tab.key().expect("retina key");
        assert_eq!(f32::from_bits(retina.scale_bits), 2.0);
        assert_eq!(f32::from_bits(retina.zoom_bits), 1.0);
        assert_eq!(f32::from_bits(retina.device_ratio_bits), 2.0);
        assert!(!tab.accepts(old));
        assert!(tab.accepts(retina));
        assert!(tab.needs_render);
        tab.set_device_ratio(f32::NAN);
        assert!(tab.key().is_err());
        assert!(!tab.accepts(retina));
    }
}
