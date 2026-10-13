//! Typed, path-free document operations executed by the resource-limited child.
use crate::{PayloadKind, Reply, Request};
use pdf_core::{DocumentInfo, DocumentMetadata, PageInfo, PdfDocument};
use serde::{Deserialize, Serialize};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_PASSWORD_BYTES: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadOperation {
    Notes {
        page: u32,
        offset: u32,
    },
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Open,
    Unlock {
        password_bytes_len: u32,
        read: ReadOperation,
    },
    Info,
    Forms,
    Split {
        every: u32,
    },
    Notes {
        page: u32,
        offset: u32,
    },
    Render {
        page: usize,
        scale: f32,
        logical_zoom: f32,
    },
    Text {
        page: u32,
    },
    Metadata,
    Sanitize,
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
    Merge {
        lengths: Vec<usize>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "mutation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    VectorOverlay {
        overlay: pdf_core::VectorOverlay,
    },
    ImageOverlay {
        overlay: pdf_core::ImageOverlay,
        image_bytes_len: u32,
    },
    TextOverlay {
        overlay: pdf_core::TextOverlay,
        font_bytes_len: u32,
    },
    Note {
        change: NoteChange,
    },
    FillText {
        changes: Vec<pdf_core::TextChange>,
        #[serde(default)]
        buttons: Vec<pdf_core::ButtonChange>,
        #[serde(default)]
        choices: Vec<pdf_core::ChoiceChange>,
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
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub enum NoteChange {
    Add {
        page: u32,
        rectangle: [f32; 4],
        contents: String,
        author: String,
    },
    Edit {
        page: u32,
        annotation_index: u32,
        expected_id: Option<(u32, u16)>,
        contents: Option<String>,
        author: Option<String>,
    },
    Remove {
        page: u32,
        annotation_index: u32,
        expected_id: Option<(u32, u16)>,
    },
}
impl NoteChange {
    pub fn page(&self) -> u32 {
        match self {
            Self::Add { page, .. } | Self::Edit { page, .. } | Self::Remove { page, .. } => *page,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteEntry {
    pub note: pdf_core::Note,
    pub editable: bool,
    pub refusal_reason: Option<String>,
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
    Split {
        parts: Vec<SplitFrame>,
    },
    Notes {
        entries: Vec<NoteEntry>,
        next_offset: Option<u32>,
    },
    PasswordRequired {
        incorrect: bool,
    },
    Sanitized {
        info: DocumentInfo,
        report: pdf_core::SanitizationReport,
    },
    Forms {
        fields: Vec<pdf_core::FormField>,
        choices: Vec<pdf_core::ChoiceField>,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitFrame {
    pub first_page: u32,
    pub last_page: u32,
    pub length: usize,
    pub info: DocumentInfo,
}
/// Validate the complete path-free batch before accepting any child result.
/// This checks framing and inspected geometry; it does not parse PDF data in the parent.
pub fn split_slices<'a>(
    parts: &[SplitFrame],
    payload: &'a [u8],
    source: &DocumentInfo,
    every: u32,
) -> Result<Vec<&'a [u8]>, String> {
    if every == 0
        || source.page_count == 0
        || source.page_count > pdf_core::MAX_PAGES
        || source.pages.len() != source.page_count
        || source.encrypted
        || source.signature_fields != 0
        || parts.len() != source.page_count.div_ceil(every as usize)
        || parts.len() > pdf_core::MAX_SPLIT_PARTS
        || payload.len() > crate::MAX_PDF_BYTES
    {
        return Err("Invalid split batch count or readonly source".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(parts.len())
        .map_err(|_| "Cannot allocate bounded split frames")?;
    let mut offset = 0usize;
    let mut first = 1u32;
    for part in parts {
        let last = first
            .saturating_sub(1)
            .saturating_add(every)
            .min(source.page_count as u32);
        let count = (last - first + 1) as usize;
        if part.first_page != first
            || part.last_page != last
            || part.length == 0
            || part.info.page_count != count
            || part.info.pages.len() != count
            || part.info.encrypted
            || part.info.signature_fields != 0
            || part.info.object_count == 0
            || part.info.object_count > pdf_core::MAX_OBJECTS
            || part.info.version != source.version
        {
            return Err("Invalid split chunk range or information".into());
        }
        for (index, page) in part.info.pages.iter().enumerate() {
            let mut expected = source.pages[first as usize - 1 + index].clone();
            expected.number = index as u32 + 1;
            if *page != expected {
                return Err("Split chunk changed source page geometry".into());
            }
        }
        let end = offset
            .checked_add(part.length)
            .filter(|end| *end <= payload.len())
            .ok_or("Split chunk lengths exceed payload")?;
        let bytes = &payload[offset..end];
        if !bytes.starts_with(b"%PDF-") {
            return Err("Split chunk lacks a verified PDF header".into());
        }
        result.push(bytes);
        offset = end;
        first = last + 1;
    }
    if offset != payload.len() || first as usize != source.page_count + 1 {
        return Err("Split batch has unclaimed bytes or pages".into());
    }
    Ok(result)
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
    let read_fields = request
        .control
        .get("read")
        .and_then(serde_json::Value::as_object)
        .map(|value| value.len());
    let operation: Operation = serde_json::from_value(request.control)
        .map_err(|error| format!("Invalid document operation: {error}"))?;
    // Serde's internally tagged unit variants can ignore extra fields despite
    // deny_unknown_fields. These operations accept only their tag, never paths.
    if matches!(
        operation,
        Operation::Open
            | Operation::Info
            | Operation::Forms
            | Operation::Metadata
            | Operation::PrepareSave
            | Operation::Sanitize
    ) && field_count != 1
    {
        return Err("This document operation accepts no extra fields or paths".into());
    }
    if matches!(operation, Operation::Open) {
        return match PdfDocument::from_bytes(&request.payload, Some("")) {
            Ok(document)
                if document
                    .info()
                    .map_err(|error| error.to_string())?
                    .encrypted =>
            {
                reply(
                    DocumentResult::PasswordRequired { incorrect: false },
                    PayloadKind::None,
                    vec![],
                )
            }
            Ok(document) => read_reply(&document, ReadOperation::Info, Some(&request.payload)),
            Err(error) if error.is_invalid_password() => reply(
                DocumentResult::PasswordRequired { incorrect: false },
                PayloadKind::None,
                vec![],
            ),
            Err(error) => Err(error.to_string()),
        };
    }
    if let Operation::Unlock {
        password_bytes_len,
        read,
    } = &operation
    {
        if matches!(
            read,
            ReadOperation::Info | ReadOperation::Forms | ReadOperation::Metadata
        ) && read_fields != Some(1)
        {
            return Err("This read operation accepts no extra fields or paths".into());
        }
        let length = *password_bytes_len as usize;
        if length > MAX_PASSWORD_BYTES {
            return Err("Password exceeds 4096 UTF-8 bytes".into());
        }
        let split = request
            .payload
            .len()
            .checked_sub(length)
            .filter(|split| *split > 0)
            .ok_or("Password length leaves no PDF input")?;
        let (ciphertext, password) = request.payload.split_at(split);
        let password = std::str::from_utf8(password).map_err(|_| "Password must be UTF-8")?;
        let document = match PdfDocument::from_bytes(ciphertext, Some(password)) {
            Ok(document) => document,
            Err(error) if error.is_invalid_password() => {
                return reply(
                    DocumentResult::PasswordRequired { incorrect: true },
                    PayloadKind::None,
                    vec![],
                );
            }
            Err(error) => return Err(error.to_string()),
        };
        if !document
            .info()
            .map_err(|error| error.to_string())?
            .encrypted
        {
            return Err("Unlock only accepts an encrypted PDF".into());
        }
        return read_reply(&document, read.clone(), None);
    }
    if let Operation::Images { lengths } = operation {
        return images(&request.payload, &lengths);
    }
    if let Operation::Merge { lengths } = operation {
        return merge(&request.payload, &lengths);
    }
    if let Operation::Edit {
        mutation:
            Mutation::ImageOverlay {
                overlay,
                image_bytes_len,
            },
    } = &operation
    {
        let length = *image_bytes_len as usize;
        if length == 0 || length > pdf_core::MAX_STREAM_BYTES {
            return Err("Overlay image must contain 1 byte to 32 MiB".into());
        }
        let split = request
            .payload
            .len()
            .checked_sub(length)
            .filter(|split| *split > 0)
            .ok_or("Overlay image length leaves no PDF input")?;
        let (pdf, image) = request.payload.split_at(split);
        let mut document = PdfDocument::from_bytes(pdf, None).map_err(|error| error.to_string())?;
        pdf_convert::add_image_overlay(&mut document, overlay, image)
            .map_err(|error| error.to_string())?;
        return document_reply(document, Some(overlay.page as usize - 1));
    }
    if let Operation::Edit {
        mutation: Mutation::TextOverlay {
            overlay,
            font_bytes_len,
        },
    } = &operation
    {
        let font_len = *font_bytes_len as usize;
        if font_len == 0 || font_len > 16 * 1024 * 1024 {
            return Err("Overlay font must contain 1 byte to 16 MiB".into());
        }
        let split = request
            .payload
            .len()
            .checked_sub(font_len)
            .filter(|split| *split > 0)
            .ok_or("Overlay font length leaves no PDF input")?;
        let (pdf, font) = request.payload.split_at(split);
        let mut document = PdfDocument::from_bytes(pdf, None).map_err(|error| error.to_string())?;
        document
            .add_text_overlay(overlay, font)
            .map_err(|error| error.to_string())?;
        return document_reply(document, Some(overlay.page as usize - 1));
    }
    if let Operation::Edit {
        mutation:
            Mutation::FillText {
                changes,
                buttons,
                choices,
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
        if changes.is_empty() && choices.is_empty() {
            return Err("Text or choice changes are required".into());
        }
        if !changes.is_empty() {
            document
                .fill_text(changes, font)
                .map_err(|error| error.to_string())?;
        }
        if !choices.is_empty() {
            document
                .fill_choice(choices, font)
                .map_err(|error| error.to_string())?;
        }
        return document_reply(document, Some(selected));
    }
    let mut document =
        PdfDocument::from_bytes(&request.payload, None).map_err(|error| error.to_string())?;
    match operation {
        Operation::Split { every } => {
            let parts = document
                .split_every(every)
                .map_err(|error| error.to_string())?;
            let size = parts
                .iter()
                .try_fold(0usize, |total, part| total.checked_add(part.bytes.len()))
                .filter(|total| *total <= crate::MAX_PDF_BYTES)
                .ok_or("Split payload exceeds128MiB")?;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(size)
                .map_err(|_| "Cannot allocate bounded split payload")?;
            let mut frames = Vec::with_capacity(parts.len());
            for part in parts {
                frames.push(SplitFrame {
                    first_page: part.first_page,
                    last_page: part.last_page,
                    length: part.bytes.len(),
                    info: part.info,
                });
                payload.extend_from_slice(&part.bytes);
            }
            reply(
                DocumentResult::Split { parts: frames },
                PayloadKind::Pdf,
                payload,
            )
        }
        Operation::Notes { page, offset } => read_reply(
            &document,
            ReadOperation::Notes { page, offset },
            Some(&request.payload),
        ),
        Operation::Forms => read_reply(&document, ReadOperation::Forms, Some(&request.payload)),
        Operation::Info => read_reply(&document, ReadOperation::Info, Some(&request.payload)),
        Operation::Render {
            page,
            scale,
            logical_zoom,
        } => read_reply(
            &document,
            ReadOperation::Render {
                page,
                scale,
                logical_zoom,
            },
            Some(&request.payload),
        ),
        Operation::Text { page } => read_reply(
            &document,
            ReadOperation::Text { page },
            Some(&request.payload),
        ),
        Operation::Metadata => {
            read_reply(&document, ReadOperation::Metadata, Some(&request.payload))
        }
        Operation::Edit { mutation } => {
            let selected_page = apply(&mut document, mutation)?;
            document_reply(document, selected_page)
        }
        Operation::PrepareSave => document_reply(document, None),
        Operation::Sanitize => {
            let (candidate, report) = document
                .sanitized_copy()
                .map_err(|error| error.to_string())?;
            let payload = candidate
                .verified_bytes()
                .map_err(|error| error.to_string())?;
            let info = candidate.info().map_err(|error| error.to_string())?;
            reply(
                DocumentResult::Sanitized { info, report },
                PayloadKind::Pdf,
                payload,
            )
        }
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
        Operation::Merge { .. } => unreachable!("merge handled before PDF parsing"),
        Operation::Open | Operation::Unlock { .. } => {
            unreachable!("opening handled before PDF parsing")
        }
    }
}

fn read_reply(
    document: &PdfDocument,
    operation: ReadOperation,
    original: Option<&[u8]>,
) -> Result<Reply, String> {
    match operation {
        ReadOperation::Notes { page, offset } => {
            let notes = document
                .list_notes(&[page])
                .map_err(|error| error.to_string())?;
            let start = offset as usize;
            if start > notes.len() {
                return Err("Note offset is out of range".into());
            }
            let mut entries = vec![];
            let mut bytes = 0usize;
            for note in notes.iter().skip(start).take(128) {
                if note.contents.len() > pdf_core::MAX_NOTE_CONTENT_BYTES
                    || note.author.len() > pdf_core::MAX_NOTE_AUTHOR_BYTES
                {
                    return Err("Existing note exceeds this reader's 64 KiB contents or 1 KiB author tier; no text was truncated".into());
                }
                let size = note.contents.len() + note.author.len();
                if bytes + size > 512 * 1024 {
                    break;
                }
                bytes += size;
                let refusal_reason = match document.note_editability(page, note.annotation_index) {
                    Ok(()) => None,
                    Err(pdf_core::Error::Limit(message)) => return Err(message.into()),
                    Err(error) => Some(error.to_string().chars().take(512).collect()),
                };
                entries.push(NoteEntry {
                    note: note.clone(),
                    editable: refusal_reason.is_none(),
                    refusal_reason,
                });
            }
            let end = start + entries.len();
            let next_offset = (end < notes.len()).then_some(end as u32);
            reply(
                DocumentResult::Notes {
                    entries,
                    next_offset,
                },
                PayloadKind::None,
                vec![],
            )
        }
        ReadOperation::Forms => reply(
            DocumentResult::Forms {
                fields: document
                    .list_form_fields()
                    .map_err(|error| error.to_string())?,
                choices: document
                    .list_choice_fields()
                    .map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
        ReadOperation::Info => reply(
            DocumentResult::Info {
                info: document.info().map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
        ReadOperation::Render {
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
            let image = if let Some(original) = original {
                pdf_render::render_view_page(original, page, scale, logical_zoom)
                    .map_err(|error| error.to_string())?
            } else {
                document
                    .render_decrypted_page(page, scale, logical_zoom)
                    .map_err(|error| error.to_string())?
            };
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
        ReadOperation::Text { page } => {
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
        ReadOperation::Metadata => reply(
            DocumentResult::Metadata {
                metadata: document.metadata().map_err(|error| error.to_string())?,
            },
            PayloadKind::None,
            vec![],
        ),
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
        Mutation::Note { change } => {
            let page = change.page();
            match change {
                NoteChange::Add {
                    rectangle,
                    contents,
                    author,
                    ..
                } => {
                    document
                        .add_note(page, rectangle, &contents, &author)
                        .map_err(|error| error.to_string())?;
                }
                NoteChange::Edit {
                    annotation_index,
                    expected_id,
                    contents,
                    author,
                    ..
                } => {
                    expected_note(document, page, annotation_index, expected_id)?;
                    document
                        .edit_note(
                            page,
                            annotation_index,
                            contents.as_deref(),
                            author.as_deref(),
                        )
                        .map_err(|error| error.to_string())?;
                }
                NoteChange::Remove {
                    annotation_index,
                    expected_id,
                    ..
                } => {
                    expected_note(document, page, annotation_index, expected_id)?;
                    document
                        .remove_note(page, annotation_index)
                        .map_err(|error| error.to_string())?;
                }
            }
            Some(page as usize - 1)
        }
        Mutation::VectorOverlay { overlay } => {
            let page = overlay.page;
            document
                .add_vector_overlay(&overlay)
                .map_err(|error| error.to_string())?;
            Some(page as usize - 1)
        }
        Mutation::FillText { .. }
        | Mutation::TextOverlay { .. }
        | Mutation::ImageOverlay { .. } => {
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

fn expected_note(
    document: &PdfDocument,
    page: u32,
    index: u32,
    id: Option<(u32, u16)>,
) -> Result<(), String> {
    let notes = document
        .list_notes(&[page])
        .map_err(|error| error.to_string())?;
    if !notes
        .iter()
        .any(|note| note.annotation_index == index && note.object_id == id)
    {
        return Err("Note occurrence or object identity is stale".into());
    }
    Ok(())
}

fn merge(payload: &[u8], lengths: &[usize]) -> Result<Reply, String> {
    if lengths.len() < 2 || lengths.len() > pdf_core::MAX_PAGES {
        return Err("Merge requires 2–20,000 PDF inputs including the open document".into());
    }
    // Validate the complete frame before any PDF parsing or candidate mutation.
    let mut position = 0usize;
    let mut slices = Vec::with_capacity(lengths.len());
    for length in lengths {
        if *length == 0 {
            return Err("Merge input is empty".into());
        }
        let end = position
            .checked_add(*length)
            .filter(|end| *end <= payload.len())
            .ok_or("Merge lengths exceed their binary payload")?;
        slices.push(&payload[position..end]);
        position = end;
    }
    if position != payload.len() {
        return Err("Merge payload contains unclaimed bytes".into());
    }
    let mut merged = PdfDocument::from_bytes(slices[0], None).map_err(|error| error.to_string())?;
    for bytes in &slices[1..] {
        let incoming = PdfDocument::from_bytes(bytes, None).map_err(|error| error.to_string())?;
        merged
            .append(&incoming)
            .map_err(|error| error.to_string())?;
    }
    document_reply(merged, Some(0))
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
