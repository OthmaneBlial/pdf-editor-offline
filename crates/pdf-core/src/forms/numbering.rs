//! Page numbers are new text overlays, not edits to existing artwork.
use super::{TextAlignment, TextOverlay};
use crate::{Error, PdfDocument, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberPosition {
    Header,
    Footer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageNumbering {
    pub pages: Vec<u32>,
    pub start: u64,
    pub position: NumberPosition,
    /// Distance from the visible page edges, in PDF user units.
    pub margin: f64,
    pub height: f64,
    pub size: f64,
    pub color: [f64; 3],
    pub alignment: TextAlignment,
}
impl PdfDocument {
    /// Number unique selected pages in document order, upright as viewed.
    /// Every page is planned before one candidate is published; errors leave
    /// the original untouched. An explicit font must cover every digit.
    pub fn add_page_numbers(&mut self, numbering: &PageNumbering, font: &[u8]) -> Result<()> {
        if numbering.pages.len() > crate::MAX_PAGES {
            return Err(Error::Limit("numbering selection exceeds 20000 pages"));
        }
        self.validate_selection(&numbering.pages)?;
        let mut pages = numbering.pages.clone();
        pages.sort_unstable();
        if pages.windows(2).any(|p| p[0] == p[1]) {
            return Err(Error::Invalid("numbering pages must be unique"));
        }
        numbering
            .start
            .checked_add(pages.len() as u64 - 1)
            .ok_or(Error::Invalid("page number sequence overflows"))?;
        if !numbering.margin.is_finite()
            || numbering.margin < 0.0
            || !numbering.height.is_finite()
            || numbering.height <= 0.0
        {
            return Err(Error::Invalid("numbering margin or height is invalid"));
        }
        let info = self.info()?;
        let mut overlays = Vec::with_capacity(pages.len());
        for (index, page) in pages.into_iter().enumerate() {
            let p = &info.pages[page as usize - 1];
            let [l, b, r, t] = [
                p.media_box[0].max(p.crop_box[0]),
                p.media_box[1].max(p.crop_box[1]),
                p.media_box[2].min(p.crop_box[2]),
                p.media_box[3].min(p.crop_box[3]),
            ]
            .map(f64::from);
            let (w, h) = (r - l, t - b);
            let rotation = p.rotation.rem_euclid(360);
            if !matches!(rotation, 0 | 90 | 180 | 270) {
                return Err(Error::Refused(
                    "numbering requires quarter-turn page rotation",
                ));
            }
            let (view_w, view_h) = if matches!(rotation, 90 | 270) {
                (h, w)
            } else {
                (w, h)
            };
            let x0 = numbering.margin;
            let x1 = view_w - numbering.margin;
            let y0 = match numbering.position {
                NumberPosition::Footer => numbering.margin,
                NumberPosition::Header => view_h - numbering.margin - numbering.height,
            };
            let y1 = y0 + numbering.height;
            if x0 >= x1
                || y0 < 0.0
                || y1 > view_h
                || ![l, b, r, t, w, h, view_w, view_h]
                    .iter()
                    .all(|n| n.is_finite())
            {
                return Err(Error::Refused(
                    "numbering box does not fit the visible page",
                ));
            }
            let rectangle = match rotation {
                0 => [l + x0, b + y0, l + x1, b + y1],
                90 => [l + w - y1, b + x0, l + w - y0, b + x1],
                180 => [l + w - x1, b + h - y1, l + w - x0, b + h - y0],
                _ => [l + y0, b + h - x1, l + y1, b + h - x0],
            };
            overlays.push((
                TextOverlay {
                    page,
                    rectangle,
                    value: (numbering.start + index as u64).to_string(),
                    size: numbering.size,
                    color: numbering.color,
                    alignment: numbering.alignment,
                },
                rotation as u16,
            ));
        }
        self.add_text_overlay_batch(&overlays, font)
    }
}
