use pdf_core::{ContentTextChange, Error, PdfDocument, Result};
use std::path::Path;

pub(super) struct Face<'a> {
    pub page: u32,
    pub operation: u32,
    pub source_sha256: &'a str,
    pub expected: &'a str,
    pub font: pdf_core::ContentTextFont,
    pub replacement: Option<&'a str>,
    pub allow_width_change: bool,
    pub output: &'a Path,
    pub dry_run: bool,
}
pub(super) fn face(
    input: &Path,
    password: Option<&str>,
    request: Face<'_>,
) -> Result<serde_json::Value> {
    let bytes = pdf_core::read_pdf_bytes(input)?;
    let fingerprint = hexadecimal(&pdf_worker::payload_fingerprint(&bytes));
    if request.source_sha256.len() != 64
        || !request
            .source_sha256
            .as_bytes()
            .iter()
            .all(u8::is_ascii_hexdigit)
    {
        return Err(Error::Invalid(
            "source-sha256 must contain exactly 64 hexadecimal digits",
        ));
    }
    if !request.source_sha256.eq_ignore_ascii_case(&fingerprint) {
        return Err(Error::Refused("existing text source fingerprint is stale"));
    }
    let mut document = PdfDocument::from_bytes(&bytes, password)?;
    document.ensure_editable()?;
    let run = document
        .content_text_runs(request.page)?
        .into_iter()
        .find(|run| run.identity.operation == request.operation)
        .ok_or(Error::Refused(
            "selected operation is not an inventoried text run",
        ))?;
    let before = run.replacement_position(request.expected)?;
    let change = pdf_core::ContentTextFaceChange {
        identity: run.identity.clone(),
        expected: request.expected.into(),
        font: request.font,
        replacement: request.replacement.map(str::to_owned),
        layout_policy: if request.allow_width_change {
            pdf_core::ContentTextLayoutPolicy::AllowWidthChange
        } else {
            Default::default()
        },
    };
    let after = run.face_position(&change)?;
    document.change_content_text_face(request.page, &[change])?;
    if !request.dry_run {
        document.save_new(request.output)?;
    }
    Ok(
        serde_json::json!({"operation":"set-content-text-font","page":request.page,"text_operation":request.operation,
        "output":super::json_path(request.output),"dry_run":request.dry_run,"font":request.font,"encoding":"win_ansi",
        "original_text_preserved":request.replacement.is_none(),"original_font_changed":run.standard_font!=Some(request.font),"font_resource_isolated":true,"original_matrix_preserved":true,
        "before":before,"after":after,"secure_erasure":false,"reflow":false}),
    )
}

pub(super) fn hexadecimal(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn inspect(
    input: &Path,
    password: Option<&str>,
    page: u32,
    offset: u32,
) -> Result<serde_json::Value> {
    let bytes = pdf_core::read_pdf_bytes(input)?;
    let document = PdfDocument::from_bytes(&bytes, password)?;
    let inventory = pdf_worker::document::content_text_inventory(&document, &bytes, page, offset)?;
    Ok(
        serde_json::json!({"source_sha256":hexadecimal(&inventory.source_fingerprint),"inventory":inventory,"scope":"direct standard Courier/Helvetica/Times ASCII Tj runs"}),
    )
}

pub(super) struct Replacement<'a> {
    pub page: u32,
    pub operation: u32,
    pub source_sha256: &'a str,
    pub expected: &'a str,
    pub replacement: &'a str,
    pub allow_width_change: bool,
    pub output: &'a Path,
    pub dry_run: bool,
}

pub(super) fn replace(
    input: &Path,
    password: Option<&str>,
    request: Replacement<'_>,
) -> Result<serde_json::Value> {
    let bytes = pdf_core::read_pdf_bytes(input)?;
    let fingerprint = hexadecimal(&pdf_worker::payload_fingerprint(&bytes));
    if request.source_sha256.len() != 64
        || !request
            .source_sha256
            .as_bytes()
            .iter()
            .all(u8::is_ascii_hexdigit)
    {
        return Err(Error::Invalid(
            "source-sha256 must contain exactly 64 hexadecimal digits",
        ));
    }
    if !request.source_sha256.eq_ignore_ascii_case(&fingerprint) {
        return Err(Error::Refused("existing text source fingerprint is stale"));
    }
    let mut document = PdfDocument::from_bytes(&bytes, password)?;
    document.ensure_editable()?;
    let run = document
        .content_text_runs(request.page)?
        .into_iter()
        .find(|run| run.identity.operation == request.operation)
        .ok_or(Error::Refused(
            "selected operation is not an inventoried text run",
        ))?;
    let before = run
        .text
        .as_deref()
        .and_then(|text| run.replacement_position(text).ok());
    let after = run.replacement_position(request.replacement).ok();
    let layout_policy = if request.allow_width_change {
        pdf_core::ContentTextLayoutPolicy::AllowWidthChange
    } else {
        Default::default()
    };
    document.replace_content_text(
        request.page,
        &[ContentTextChange {
            layout_policy,
            identity: run.identity,
            expected: request.expected.into(),
            replacement: request.replacement.into(),
        }],
    )?;
    if !request.dry_run {
        document.save_new(request.output)?;
    }
    Ok(
        serde_json::json!({"operation":"replace-content-text","page":request.page,"text_operation":request.operation,"output":super::json_path(request.output),"dry_run":request.dry_run,"original_operand_replaced":true,"layout_policy":layout_policy,"before":before,"after":after,"secure_erasure":false,"reflow":false}),
    )
}

pub(super) struct Properties<'a> {
    pub page: u32,
    pub operation: u32,
    pub source_sha256: &'a str,
    pub expected: &'a str,
    pub font_size: Option<f32>,
    pub matrix_translation: Option<[f32; 2]>,
    pub fill_rgb: Option<[f32; 3]>,
    pub output: &'a Path,
    pub dry_run: bool,
}
pub(super) fn properties(
    input: &Path,
    password: Option<&str>,
    request: Properties<'_>,
) -> Result<serde_json::Value> {
    let bytes = pdf_core::read_pdf_bytes(input)?;
    let fingerprint = hexadecimal(&pdf_worker::payload_fingerprint(&bytes));
    if request.source_sha256.len() != 64
        || !request
            .source_sha256
            .as_bytes()
            .iter()
            .all(u8::is_ascii_hexdigit)
    {
        return Err(Error::Invalid(
            "source-sha256 must contain exactly 64 hexadecimal digits",
        ));
    }
    if !request.source_sha256.eq_ignore_ascii_case(&fingerprint) {
        return Err(Error::Refused("existing text source fingerprint is stale"));
    }
    let mut document = PdfDocument::from_bytes(&bytes, password)?;
    document.ensure_editable()?;
    let run = document
        .content_text_runs(request.page)?
        .into_iter()
        .find(|run| run.identity.operation == request.operation)
        .ok_or(Error::Refused(
            "selected operation is not an inventoried text run",
        ))?;
    let before = run.replacement_position(request.expected)?;
    let change = pdf_core::ContentTextPropertyChange {
        identity: run.identity.clone(),
        expected: request.expected.into(),
        font_size: request.font_size,
        matrix_translation: request.matrix_translation,
        fill_rgb: request.fill_rgb,
    };
    let after = run.property_position(&change)?;
    document.update_content_text_properties(request.page, &[change])?;
    if !request.dry_run {
        document.save_new(request.output)?;
    }
    Ok(
        serde_json::json!({"operation":"set-content-text-properties","page":request.page,"text_operation":request.operation,"output":super::json_path(request.output),"dry_run":request.dry_run,"original_text_preserved":true,"original_font_preserved":true,"font_size":request.font_size,"matrix_position":request.matrix_translation,"rgb":request.fill_rgb,"before":before,"after":after,"secure_erasure":false,"reflow":false}),
    )
}
