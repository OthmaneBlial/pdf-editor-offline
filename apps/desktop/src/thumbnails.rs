use crate::state::RenderKey;
use eframe::egui;
use pdf_core::PageInfo;
use std::collections::{HashMap, VecDeque};

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 128;
pub const MAX_VISIBLE: usize = 16;
pub const MAX_SIDE: u32 = 512;
const LOGICAL_SIDE: f32 = 72.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThumbnailKey {
    pub tab: u64,
    pub revision: u64,
    pub page: usize,
    pub device_ratio_bits: u32,
    pub scale_bits: u32,
    pub pixel_size: [u32; 2],
}

impl ThumbnailKey {
    pub fn new(
        tab: u64,
        revision: u64,
        page: usize,
        info: &PageInfo,
        ratio: f32,
    ) -> Result<Self, &'static str> {
        if !ratio.is_finite() || ratio <= 0.0 {
            return Err("Invalid thumbnail display scale.");
        }
        let geometry = crate::raster::plan(info, 1.0, 1.0)?;
        let zoom = LOGICAL_SIDE / geometry.logical_size[0].max(geometry.logical_size[1]);
        let plan =
            crate::raster::plan(info, zoom, ratio.min((MAX_SIDE - 1) as f32 / LOGICAL_SIDE))?;
        if plan.pixel_size.iter().any(|side| *side > MAX_SIDE) {
            return Err("Thumbnail exceeds its pixel limit.");
        }
        Ok(Self {
            tab,
            revision,
            page,
            device_ratio_bits: ratio.to_bits(),
            scale_bits: plan.scale.to_bits(),
            pixel_size: plan.pixel_size,
        })
    }

    pub fn render_key(self) -> RenderKey {
        RenderKey {
            tab: self.tab,
            revision: self.revision,
            page: self.page,
            scale_bits: self.scale_bits,
            zoom_bits: 0,
            device_ratio_bits: self.device_ratio_bits,
            pixel_size: self.pixel_size,
        }
    }

    pub fn display_size(self) -> egui::Vec2 {
        let size = egui::vec2(self.pixel_size[0] as f32, self.pixel_size[1] as f32);
        size * (LOGICAL_SIDE / size.x.max(size.y))
    }
}

pub struct Entry {
    pub texture: Option<egui::TextureHandle>,
    pub error: Option<String>,
    bytes: usize,
}

#[derive(Default)]
pub struct Cache {
    entries: HashMap<ThumbnailKey, Entry>,
    lru: VecDeque<ThumbnailKey>,
    bytes: usize,
}

impl Cache {
    pub fn contains(&self, key: ThumbnailKey) -> bool {
        self.entries.contains_key(&key)
    }

    pub fn get(&mut self, key: ThumbnailKey) -> Option<&Entry> {
        if self.entries.contains_key(&key) {
            self.lru.retain(|old| *old != key);
            self.lru.push_back(key);
        }
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        key: ThumbnailKey,
        texture: Option<egui::TextureHandle>,
        error: Option<String>,
    ) {
        let bytes = texture.as_ref().map_or(0, |texture| {
            let size = texture.size();
            size[0] * size[1] * 4
        });
        if bytes > MAX_BYTES {
            return;
        }
        self.remove(key);
        while self.bytes + bytes > MAX_BYTES || self.entries.len() >= MAX_ENTRIES {
            let Some(old) = self.lru.front().copied() else {
                break;
            };
            self.remove(old);
        }
        self.bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                texture,
                error,
                bytes,
            },
        );
        self.lru.push_back(key);
    }

    fn remove(&mut self, key: ThumbnailKey) {
        if let Some(entry) = self.entries.remove(&key) {
            self.bytes -= entry.bytes;
        }
        self.lru.retain(|old| *old != key);
    }

    pub fn retain(&mut self, mut valid: impl FnMut(ThumbnailKey) -> bool) {
        let stale: Vec<_> = self
            .entries
            .keys()
            .copied()
            .filter(|key| !valid(*key))
            .collect();
        for key in stale {
            self.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(page: usize) -> ThumbnailKey {
        ThumbnailKey::new(
            1,
            0,
            page,
            &PageInfo {
                number: page as u32 + 1,
                media_box: [0.0, 0.0, 100.0, 80.0],
                crop_box: [0.0, 0.0, 100.0, 80.0],
                rotation: 0,
            },
            2.0,
        )
        .expect("key")
    }

    #[test]
    fn thumbnails_are_dpi_aware_finite_and_physically_bounded() {
        let base = key(0);
        assert_eq!(base.pixel_size, [145, 116]);
        assert_eq!(base.display_size().x, LOGICAL_SIDE);
        let info = PageInfo {
            number: 1,
            media_box: [0.0, 0.0, 10000.0, 20000.0],
            crop_box: [0.0, 0.0, 10000.0, 20000.0],
            rotation: 90,
        };
        let capped = ThumbnailKey::new(1, 0, 0, &info, f32::MAX).expect("bounded");
        assert!(capped.pixel_size.iter().all(|side| *side <= MAX_SIDE));
        assert!(ThumbnailKey::new(1, 0, 0, &info, f32::NAN).is_err());
        assert!(ThumbnailKey::new(1, 0, 0, &info, 0.0).is_err());
        assert_ne!(
            base,
            ThumbnailKey::new(1, 0, 0, &info, 1.0).expect("ordinary")
        );
    }

    #[test]
    fn lru_bounds_real_pixel_bytes_and_error_entries() {
        let context = egui::Context::default();
        let mut cache = Cache::default();
        for page in 0..9 {
            let texture = context.load_texture(
                format!("thumb-{page}"),
                egui::ColorImage::filled([512, 512], egui::Color32::WHITE),
                Default::default(),
            );
            cache.insert(key(page), Some(texture), None);
        }
        assert_eq!(cache.bytes, MAX_BYTES);
        assert_eq!(cache.entries.len(), 8);
        assert!(!cache.contains(key(0)));
        cache.get(key(1));
        let texture = context.load_texture(
            "next",
            egui::ColorImage::filled([512, 512], egui::Color32::WHITE),
            Default::default(),
        );
        cache.insert(key(9), Some(texture), None);
        assert!(cache.contains(key(1)));
        assert!(!cache.contains(key(2)));
        for page in 10..200 {
            cache.insert(key(page), None, Some("Unavailable".into()));
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        cache.retain(|key| key.tab != 1);
        assert_eq!(cache.bytes, 0);
        assert!(cache.entries.is_empty());
    }
}
