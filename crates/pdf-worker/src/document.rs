//! Typed, path-free document operations executed by the resource-limited child.
use crate::{PayloadKind, Reply, Request};
use pdf_core::{DocumentInfo, DocumentMetadata, PageInfo, PdfDocument};
use serde::{Deserialize, Serialize};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Info,
    Forms,
    Render {
        page: usize,
        scale: f32,
        logical_zoom: f32,
    },
    Text {
        page: u32,
    },
    Metadata,
    Edit {
        mutation: Mutation,
    },
    PrepareSave,
    ExtractPages {
        pages: Vec<u32>,
    },
    Images {
        lengths: Vec<usize>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "mutation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    FillText {
        changes: Vec<pdf_core::TextChange>,
        #[serde(default)]
        buttons: Vec<pdf_core::ButtonChange>,
        page: u32,
        font_bytes_len: u32,
    },
    FillButtons {
        changes: Vec<pdf_core::ButtonChange>,
        page: u32,
    },
    DuplicatePages {
        pages: Vec<u32>,
        page: u32,
    },
    DeletePages {
        pages: Vec<u32>,
        page: u32,
    },
    Rotate {
        page: u32,
        degrees: i64,
    },
    Move {
        page: u32,
        earlier: bool,
    },
    Reverse {
        page: u32,
    },
    InsertBlank {
        position: u32,
        width: f32,
        height: f32,
    },
    Crop {
        page: u32,
        rectangle: [f32; 4],
    },
    Metadata {
        changes: Vec<MetadataChange>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataChange {
    pub key: pdf_core::MetadataKey,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterResult {
    pub page: usize,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub logical_zoom: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentResult {
    Forms {
        fields: Vec<pdf_core::FormField>,
    },
    Info {
        info: DocumentInfo,
    },
    Render {
        raster: RasterResult,
    },
    Text {
        text: String,
        truncated: bool,
    },
    Metadata {
        metadata: DocumentMetadata,
    },
    Document {
        info: DocumentInfo,
        selected_page: Option<usize>,
    },
}

/// No request contains a filesystem path. Call this only in the bounded child;
/// transport limits do not make direct in-process calls a hostile-file sandbox.
pub fn execute_document(request: Request) -> Result<Reply, String> {
    execute(request).map_err(|message| message.chars().take(8192).collect())
}

fn execute(request: Request) -> Result<Reply, String> {
    if request.payload.len() > crate::MAX_PDF_BYTES {
        return Err("Document input exceeds 128 MiB".into());
    }
    if serde_json::to_vec(&request.control)
        .map_err(|error| error.to_string())?
        .len()
        > crate::MAX_CONTROL_BYTES
    {
        return Err("Document control exceeds 8 MiB".into());
    }
    let field_count = request
        .control
        .as_object()
        .ok_or("Document control must be an object")?
        .len();
    let operation: Operation = serde_json::from_value(request.control)
        .map_err(|error| format!("Invalid document operation: {error}"))?;
    // Serde's internally tagged unit variants can ignore extra fields despite
    // deny_unknown_fields. These operations accept only their tag, never paths.
    if matches!(
        operation,
        Operation::Info | Operation::Forms | Operation::Metadata | Operation::PrepareSave
    ) && field_count != 1
    {
        return Err("This document operation accepts no extra fields or paths".into());
    }
    if let Operation::Images { lengths } = operation {
        return images(&request.payload, &lengths);
    }
    if let Operation::Edit {
        mutation:
            Mutation::FillText {
                changes,
                buttons,
                page,
                font_bytes_len,
            },
    } = &operation
    {
        let font_len = *font_bytes_len as usize;
        if font_len == 0 || font_len > 16 * 1024 * 1024 {
            return Err("Text font must contain 1 byte to 16 MiB".into());
        }
        let split = request
            .payload
            .len()
            .checked_sub(font_len)
            .filter(|split| *split > 0)
            .ok_or("Text font length leaves no PDF input")?;
        let (pdf, font) = request.payload.split_at(split);
        let mut document = PdfDocument::from_bytes(pdf, None).map_err(|error| error.to_string())?;
        let selected = (*page as usize)
            .checked_sub(1)
            .filter(|index| *index < document.page_count())
            .ok_or("Current page is outside the document")?;
        if !buttons.is_empty() {
            document
                .fill_buttons(buttons)
                .map_err(|error| error.to_string())?;
        }
        document
            .fill_text(changes, font)
            .map_err(|error| error.to_string())?;
        return document_reply(document, Some(selected));
    }
    let mut document =
        PdfDocument::from_bytes(&request.payload, None).map_err(|error| error.to_string())?;
    match operation {
        Operation::Forms => reply(
            DocumentResult::Forms {
                fields: document
                    .list_form_fields()
                    .map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
        Operation::Info => reply(
            DocumentResult::Info {
                info: document.info().map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
        Operation::Render {
            page,
            scale,
            logical_zoom,
        } => {
            if !scale.is_finite()
                || scale <= 0.0
                || !logical_zoom.is_finite()
                || logical_zoom <= 0.0
            {
                return Err("Raster scale and logical zoom must be finite and positive".into());
            }
            let info = document.info().map_err(|error| error.to_string())?;
            let geometry = info
                .pages
                .get(page)
                .ok_or("Render page is outside the document")?;
            let expected = dimensions(geometry, scale)?;
            let image = pdf_render::render_view_page(&request.payload, page, scale, logical_zoom)
                .map_err(|error| error.to_string())?;
            let pixels = u64::from(image.width) * u64::from(image.height);
            if [image.width, image.height] != expected
                || pixels > pdf_render::MAX_PIXELS
                || pixels.checked_mul(4) != Some(image.rgba.len() as u64)
                || image.rgba.len() > crate::MAX_RGBA_BYTES
            {
                return Err(
                    "Raster dimensions do not match the inspected page and pixel limits".into(),
                );
            }
            reply(
                DocumentResult::Render {
                    raster: RasterResult {
                        page,
                        width: image.width,
                        height: image.height,
                        scale,
                        logical_zoom,
                    },
                },
                PayloadKind::Rgba,
                image.rgba,
            )
        }
        Operation::Text { page } => {
            let mut text = document
                .extract_text(&[page])
                .map_err(|error| error.to_string())?;
            let truncated = text.len() > MAX_TEXT_BYTES;
            if truncated {
                let mut end = MAX_TEXT_BYTES;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
            }
            reply(
                DocumentResult::Text { text, truncated },
                PayloadKind::None,
                vec![],
            )
        }
        Operation::Metadata => reply(
            DocumentResult::Metadata {
                metadata: document.metadata().map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
        Operation::Edit { mutation } => {
            let selected_page = apply(&mut document, mutation)?;
            document_reply(document, selected_page)
        }
        Operation::PrepareSave => document_reply(document, None),
        Operation::ExtractPages { pages } => {
            if pages.len() > pdf_core::MAX_PAGES {
                return Err("Page selection exceeds the supported page count".into());
            }
            let extracted = document
                .extract_pages(&pages)
                .map_err(|error| error.to_string())?;
            document_reply(extracted, Some(0))
        }
        Operation::Images { .. } => unreachable!("images handled before PDF parsing"),
    }
}

fn reply(result: DocumentResult, kind: PayloadKind, payload: Vec<u8>) -> Result<Reply, String> {
    let control = serde_json::to_value(result).map_err(|error| error.to_string())?;
    if serde_json::to_vec(&control)
        .map_err(|error| error.to_string())?
        .len()
        > crate::MAX_CONTROL_BYTES
    {
        return Err("Document reply exceeds the control limit".into());
    }
    Ok(Reply {
        control,
        payload,
        kind,
    })
}

fn document_reply(document: PdfDocument, selected_page: Option<usize>) -> Result<Reply, String> {
    let payload = document
        .verified_bytes()
        .map_err(|error| error.to_string())?;
    let reopened = PdfDocument::from_bytes(&payload, None).map_err(|error| error.to_string())?;
    let info = reopened.info().map_err(|error| error.to_string())?;
    if selected_page.is_some_and(|page| page >= info.page_count) {
        return Err("Selected page is outside the verified document".into());
    }
    reply(
        DocumentResult::Document {
            info,
            selected_page,
        },
        PayloadKind::Pdf,
        payload,
    )
}

fn apply(document: &mut PdfDocument, mutation: Mutation) -> Result<Option<usize>, String> {
    let selected = match mutation {
        Mutation::FillText { .. } => {
            return Err("Text font payload was not split before PDF parsing".into());
        }
        Mutation::FillButtons { changes, page } => {
            let index = (page as usize)
                .checked_sub(1)
                .filter(|index| *index < document.page_count())
                .ok_or("Current page is outside the document")?;
            document
                .fill_buttons(&changes)
                .map_err(|error| error.to_string())?;
            Some(index)
        }
        Mutation::DuplicatePages { pages, page } => {
            let index = (page as usize)
                .checked_sub(1)
                .filter(|index| *index < document.page_count())
                .ok_or("Current page is outside the document")?;
            if pages.len() > pdf_core::MAX_PAGES {
                return Err("Page selection exceeds the supported page count".into());
            }
            document
                .duplicate_pages(&pages)
                .map_err(|error| error.to_string())?;
            Some(index + pages.iter().filter(|number| **number < page).count())
        }
        Mutation::DeletePages { pages, page } => {
            let index = (page as usize)
                .checked_sub(1)
                .filter(|index| *index < document.page_count())
                .ok_or("Current page is outside the document")?;
            if pages.len() > pdf_core::MAX_PAGES {
                return Err("Page selection exceeds the supported page count".into());
            }
            document
                .delete_pages(&pages)
                .map_err(|error| error.to_string())?;
            let removed_before = pages.iter().filter(|removed| **removed < page).count();
            Some((index - removed_before).min(document.page_count() - 1))
        }
        Mutation::Rotate { page, degrees } => {
            document
                .rotate(&[page], degrees)
                .map_err(|error| error.to_string())?;
            None
        }
        Mutation::Move { page, earlier } => {
            let count = document.page_count();
            let index = (page as usize)
                .checked_sub(1)
                .filter(|index| *index < count)
                .ok_or("Move page is outside the document")?;
            let next = if earlier {
                index.checked_sub(1)
            } else {
                index.checked_add(1).filter(|next| *next < count)
            }
            .ok_or("Page cannot move farther in that direction")?;
            let mut order: Vec<_> = (1..=count as u32).collect();
            order.swap(index, next);
            document
                .reorder(&order)
                .map_err(|error| error.to_string())?;
            Some(next)
        }
        Mutation::Reverse { page } => {
            let index = (page as usize)
                .checked_sub(1)
                .filter(|index| *index < document.page_count())
                .ok_or("Current page is outside the document")?;
            let next = document.page_count() - 1 - index;
            document.reverse().map_err(|error| error.to_string())?;
            Some(next)
        }
        Mutation::InsertBlank {
            position,
            width,
            height,
        } => {
            document
                .insert_blank(position, width, height)
                .map_err(|error| error.to_string())?;
            Some(position as usize - 1)
        }
        Mutation::Crop { page, rectangle } => {
            document
                .crop(&[page], rectangle)
                .map_err(|error| error.to_string())?;
            None
        }
        Mutation::Metadata { changes } => {
            if changes.len() > 6 {
                return Err("At most six standard metadata fields may change".into());
            }
            let updates: Vec<_> = changes
                .iter()
                .map(|change| (change.key, change.value.as_deref()))
                .collect();
            document
                .update_metadata(&updates)
                .map_err(|error| error.to_string())?;
            None
        }
    };
    Ok(selected)
}

fn images(payload: &[u8], lengths: &[usize]) -> Result<Reply, String> {
    if lengths.is_empty() || lengths.len() > pdf_core::MAX_PAGES {
        return Err("Select 1–20,000 image pages".into());
    }
    let mut position = 0usize;
    let mut sources = Vec::with_capacity(lengths.len());
    for length in lengths {
        if *length == 0 || *length > pdf_core::MAX_STREAM_BYTES {
            return Err("Encoded image must contain 1 byte to 32 MiB".into());
        }
        let end = position
            .checked_add(*length)
            .filter(|end| *end <= payload.len())
            .ok_or("Image lengths exceed their binary payload")?;
        sources.push(&payload[position..end]);
        position = end;
    }
    if position != payload.len() {
        return Err("Image payload contains unclaimed bytes".into());
    }
    let converted = pdf_convert::images_to_pdf(&sources).map_err(|error| error.to_string())?;
    document_reply(converted.document, Some(0))
}

fn dimensions(page: &PageInfo, scale: f32) -> Result<[u32; 2], String> {
    if page
        .media_box
        .iter()
        .chain(&page.crop_box)
        .any(|value| !value.is_finite())
        || page.rotation % 90 != 0
    {
        return Err("Page has invalid visible geometry".into());
    }
    let mut width =
        page.media_box[2].min(page.crop_box[2]) - page.media_box[0].max(page.crop_box[0]);
    let mut height =
        page.media_box[3].min(page.crop_box[3]) - page.media_box[1].max(page.crop_box[1]);
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("Page has no valid visible area".into());
    }
    width = width.max(1.0);
    height = height.max(1.0);
    if matches!(page.rotation.rem_euclid(360), 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    let width = (f64::from(width) * f64::from(scale)).ceil();
    let height = (f64::from(height) * f64::from(scale)).ceil();
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(pdf_render::MAX_DIMENSION)
        || height > f64::from(pdf_render::MAX_DIMENSION)
        || width * height > pdf_render::MAX_PIXELS as f64
    {
        return Err("Raster exceeds the dimension or pixel limit".into());
    }
    Ok([width as u32, height as u32])
}
