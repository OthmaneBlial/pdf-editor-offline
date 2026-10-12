use eframe::egui::TextureHandle;
use pdf_core::DocumentInfo;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderKey {
    pub tab: u64,
    pub revision: u64,
    pub page: usize,
    pub scale_bits: u32,
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
    pub texture: Option<TextureHandle>,
    pub needs_render: bool,
    pub error: Option<String>,
}

impl Tab {
    pub fn new(id: u64, path: PathBuf, status: DocumentStatus) -> Self {
        Self {
            id,
            path,
            status,
            page: 0,
            scale: 1.0,
            texture: None,
            needs_render: true,
            error: None,
        }
    }

    pub fn title(&self) -> String {
        self.path.file_name().map_or_else(
            || "PDF".to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub fn key(&self) -> RenderKey {
        RenderKey {
            tab: self.id,
            revision: self.status.revision,
            page: self.page,
            scale_bits: self.scale.to_bits(),
        }
    }

    pub fn accepts(&self, key: RenderKey) -> bool {
        self.key() == key
    }

    pub fn editable(&self) -> bool {
        !self.status.info.encrypted && self.status.info.signature_fields == 0
    }

    pub fn update(&mut self, status: DocumentStatus) {
        if self.status.revision != status.revision {
            self.texture = None;
            self.needs_render = true;
        }
        self.status = status;
    }

    pub fn set_page(&mut self, page: usize) {
        if page < self.status.info.page_count && page != self.page {
            self.page = page;
            self.texture = None;
            self.needs_render = true;
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        let scale = scale.clamp(0.25, 4.0);
        if self.scale != scale {
            self.scale = scale;
            self.texture = None;
            self.needs_render = true;
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
                    pages: vec![],
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
        let key = tab.key();
        assert!(tab.accepts(key));
        tab.set_page(1);
        assert!(!tab.accepts(key));
        let key = tab.key();
        tab.set_scale(2.0);
        assert!(!tab.accepts(key));
        let key = tab.key();
        tab.status.revision = 1;
        assert!(!tab.accepts(key));
        let key = RenderKey {
            tab: 2,
            ..tab.key()
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
}
