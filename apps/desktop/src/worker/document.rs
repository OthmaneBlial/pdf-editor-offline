use super::{Edit, HISTORY_BYTES, HISTORY_LIMIT};
use crate::state::DocumentStatus;
use pdf_core::DocumentInfo;
use pdf_worker::document::{DocumentResult, MetadataChange, Mutation, Operation, ReadOperation};
use pdf_worker::journal::{DocumentStateRef, Journal, SnapshotRef};
use pdf_worker::{PayloadKind, Reply, Request};
use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(super) struct Snapshot {
    pub bytes: Vec<u8>,
    pub info: DocumentInfo,
    pub revision: u64,
}

pub(super) struct WorkingDocument {
    pub current: Snapshot,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub saved_revision: Option<u64>,
    pub next_revision: u64,
    pub path: Option<PathBuf>,
    // Session-only credentials; never included in DocumentStateRef or any diagnostics.
    pub(super) password: Option<String>,
}

pub(super) enum OpenResult {
    Open(WorkingDocument),
    PasswordRequired([u8; 32]),
}

fn validate_forms(fields: &[pdf_core::FormField], pages: usize) -> Result<(), String> {
    use pdf_core::{FormFieldKind, FormValue};
    if fields.len() > 10_000
        || serde_json::to_vec(fields)
            .map_err(|error| error.to_string())?
            .len()
            > pdf_worker::MAX_CONTROL_BYTES
    {
        return Err("Forms response exceeds its field/report bound".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut widgets = std::collections::BTreeSet::new();
    let mut text_bytes = 0usize;
    for field in fields {
        if field.object_id.0 == 0
            || field.object_id.0 as usize > pdf_core::MAX_OBJECTS
            || !ids.insert(field.object_id)
            || field.name.len() > 4096
        {
            return Err("Invalid form field identity or name".into());
        }
        text_bytes = text_bytes.saturating_add(field.name.len());
        let values: Vec<&[u8]> = match &field.value {
            FormValue::Text(text) => vec![text.as_bytes()],
            FormValue::Name(bytes) => vec![bytes.as_slice()],
            FormValue::TextArray(values) => values.iter().map(|text| text.as_bytes()).collect(),
            _ => vec![],
        };
        for value in values {
            if value.len() > 65_536 {
                return Err("Form value exceeds its text bound".into());
            }
            text_bytes = text_bytes.saturating_add(value.len());
        }
        if text_bytes > 4 * 1024 * 1024 {
            return Err("Form report text exceeds 4 MiB".into());
        }
        if field
            .refusal_reason
            .as_ref()
            .is_some_and(|reason| reason.len() > 8192)
        {
            return Err("Form refusal reason exceeds its bound".into());
        }
        if field
            .text_refusal_reason
            .as_ref()
            .is_some_and(|reason| reason.len() > 8192)
            || field.writable_text
                && (field.kind != FormFieldKind::Text
                    || field.name.is_empty()
                    || field.flags & (1 | 1 << 13 | 1 << 20 | 1 << 24 | 1 << 25) != 0
                    || field.widgets.is_empty()
                    || field.text_refusal_reason.is_some()
                    || !matches!(field.value, FormValue::Text(_) | FormValue::Empty))
        {
            return Err("Invalid writable text capability or refusal bound".into());
        }
        if field.writable_button
            && (!matches!(
                field.kind,
                FormFieldKind::CheckBox | FormFieldKind::RadioButton
            ) || field.name.is_empty()
                || field.flags & 1 != 0
                || field.widgets.is_empty()
                || field.refusal_reason.is_some())
        {
            return Err("Invalid writable button capability".into());
        }
        for widget in &field.widgets {
            if field.writable_button
                && (widget
                    .on_state
                    .as_ref()
                    .is_none_or(|state| state.is_empty() || state == b"Off")
                    || widget.appearance_state.is_none())
            {
                return Err("Writable button lacks a valid existing appearance state".into());
            }
            if widget.object_id.0 == 0
                || widget.object_id.0 as usize > pdf_core::MAX_OBJECTS
                || !widgets.insert(widget.object_id)
                || widgets.len() > 100_000
                || widget.page == 0
                || widget.page as usize > pages
                || widget.rectangle.iter().any(|number| !number.is_finite())
                || widget.rectangle[2] <= widget.rectangle[0]
                || widget.rectangle[3] <= widget.rectangle[1]
                || [&widget.appearance_state, &widget.on_state]
                    .iter()
                    .any(|state| state.as_ref().is_some_and(|bytes| bytes.len() > 1024))
            {
                return Err("Invalid form widget geometry, state or identity".into());
            }
        }
    }
    Ok(())
}

fn validate_choices(
    choices: &[pdf_core::ChoiceField],
    fields: &[pdf_core::FormField],
    pages: usize,
) -> Result<(), String> {
    if choices.len() > 10_000
        || serde_json::to_vec(choices)
            .map_err(|error| error.to_string())?
            .len()
            > pdf_worker::MAX_CONTROL_BYTES
    {
        return Err("Choice report exceeds its bound".into());
    }
    validate_forms(
        &choices
            .iter()
            .map(|choice| choice.field.clone())
            .collect::<Vec<_>>(),
        pages,
    )?;
    let mut text_bytes = 0usize;
    let by_id: std::collections::BTreeMap<_, _> = fields
        .iter()
        .map(|field| (field.object_id, field))
        .collect();
    for choice in choices {
        if by_id
            .get(&choice.field.object_id)
            .is_none_or(|field| *field != &choice.field)
            || choice.field.kind != pdf_core::FormFieldKind::Choice
            || choice.options.len() > 10_000
            || choice.combo != (choice.field.flags & (1 << 17) != 0)
            || choice.multi_select != (choice.field.flags & (1 << 21) != 0)
            || choice.editable_combo != (choice.field.flags & (1 << 18) != 0)
            || choice
                .refusal_reason
                .as_ref()
                .is_some_and(|reason| reason.len() > 8192)
            || choice.top_index as usize >= choice.options.len().max(1)
            || choice.writable
                && (choice.refusal_reason.is_some()
                    || choice.selected_indices.is_none()
                    || choice.field.name.is_empty()
                    || choice.field.flags & 1 != 0
                    || choice.editable_combo
                    || choice.options.is_empty()
                    || choice.combo && choice.multi_select)
        {
            return Err("Invalid choice identity, flags, scroll or capability".into());
        }
        for (index, option) in choice.options.iter().enumerate() {
            if option.index as usize != index
                || option.export.len() > 65_536
                || option.display.len() > 65_536
            {
                return Err("Invalid indexed choice option".into());
            }
            text_bytes = text_bytes
                .saturating_add(option.export.len())
                .saturating_add(option.display.len());
        }
        if let Some(indices) = &choice.selected_indices
            && (indices.len() > choice.options.len()
                || !choice.multi_select && indices.len() > 1
                || indices.windows(2).any(|pair| pair[0] >= pair[1])
                || indices
                    .iter()
                    .any(|index| *index as usize >= choice.options.len()))
        {
            return Err("Invalid choice selection indices".into());
        }
        let default = match &choice.default_value {
            pdf_core::FormValue::Text(value) => vec![value.as_bytes()],
            pdf_core::FormValue::TextArray(values) => {
                values.iter().map(|value| value.as_bytes()).collect()
            }
            pdf_core::FormValue::Name(bytes) => vec![bytes.as_slice()],
            _ => vec![],
        };
        for value in default {
            if value.len() > 65_536 {
                return Err("Choice default exceeds its bound".into());
            }
            text_bytes = text_bytes.saturating_add(value.len());
        }
        if text_bytes > 4 * 1024 * 1024 {
            return Err("Choice text exceeds 4 MiB".into());
        }
    }
    Ok(())
}

fn validate_selection(pages: &[u32], count: usize) -> Result<(), String> {
    if pages.is_empty() || pages.len() > count || pages.len() > pdf_core::MAX_PAGES {
        return Err("Select between one and the document's page count.".into());
    }
    let mut seen = vec![false; count];
    for &page in pages {
        let index = (page as usize)
            .checked_sub(1)
            .filter(|index| *index < count)
            .ok_or("Selected page is outside the document")?;
        if std::mem::replace(&mut seen[index], true) {
            return Err("Page selection contains a duplicate".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod choice_reply_tests {
    use super::*;
    #[test]
    fn choice_reply_validation_rejects_forged_indices_identity_flags_and_bounds() {
        let pdf = pdf_core::PdfDocument::from_bytes(
            include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf"),
            None,
        )
        .expect("fixture");
        let fields = pdf.list_form_fields().expect("fields");
        let choices = pdf.list_choice_fields().expect("choices");
        validate_choices(&choices, &fields, 1).expect("actual boundedreport");
        for index in 0..7 {
            let mut invalid = choices.clone();
            match index {
                0 => invalid[0].selected_indices = Some(vec![99]),
                1 => invalid[0].options[0].index = 1,
                2 => invalid[0].field.name = "forged".into(),
                3 => invalid[0].combo = false,
                4 => invalid[0].top_index = 3,
                5 => invalid[0].options[0].display = "A".repeat(65_537),
                _ => invalid[0].default_value = pdf_core::FormValue::Text("A".repeat(65_537)),
            }
            assert!(validate_choices(&invalid, &fields, 1).is_err());
        }
        let mut readonly = choices;
        readonly[0].field.flags |= 1;
        let mut fields = fields;
        let field = fields
            .iter_mut()
            .find(|field| field.object_id == readonly[0].field.object_id)
            .expect("field");
        field.flags |= 1;
        assert!(validate_choices(&readonly, &fields, 1).is_err());
    }
}

impl Snapshot {
    fn as_ref(&self) -> SnapshotRef<'_> {
        SnapshotRef {
            revision: self.revision,
            bytes: &self.bytes,
            info: &self.info,
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
struct TestExecutor {
    executable: PathBuf,
    arguments: Vec<OsString>,
    limits: pdf_worker::Limits,
}

#[cfg(test)]
std::thread_local! {
    static TEST_EXECUTABLE: std::cell::RefCell<Option<TestExecutor>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn with_test_executable<T>(path: PathBuf, action: impl FnOnce() -> T) -> T {
    with_test_executor(path, Vec::new(), pdf_worker::Limits::default(), action)
}

#[cfg(test)]
pub(super) fn with_test_executor<T>(
    path: PathBuf,
    arguments: Vec<OsString>,
    limits: pdf_worker::Limits,
    action: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<TestExecutor>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_EXECUTABLE.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = TEST_EXECUTABLE.with(|slot| {
        slot.replace(Some(TestExecutor {
            executable: path,
            arguments,
            limits,
        }))
    });
    let _restore = Restore(previous);
    action()
}

pub(super) fn execute(
    operation: Operation,
    bytes: &[u8],
) -> Result<(DocumentResult, Reply), String> {
    execute_limited(operation, bytes, pdf_worker::Limits::default())
}

pub(super) fn execute_limited(
    operation: Operation,
    bytes: &[u8],
    limits: pdf_worker::Limits,
) -> Result<(DocumentResult, Reply), String> {
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    #[cfg(not(test))]
    let (helper, arguments) = (current, vec![OsString::from("--pdf-worker")]);
    #[cfg(test)]
    let (helper, arguments) = {
        let directory = current
            .parent()
            .and_then(Path::parent)
            .ok_or("Test executable directory is unavailable")?;
        let name = if cfg!(windows) {
            "pdf-editor-offline-worker.exe"
        } else {
            "pdf-editor-offline-worker"
        };
        (directory.join(name), Vec::<OsString>::new())
    };
    #[cfg(test)]
    let (helper, arguments, limits) = TEST_EXECUTABLE.with(|slot| {
        slot.borrow()
            .clone()
            .map_or((helper, arguments, limits), |executor| {
                (executor.executable, executor.arguments, executor.limits)
            })
    });
    let reply = pdf_worker::execute(
        &helper,
        &arguments,
        Request {
            control: serde_json::to_value(operation).map_err(|error| error.to_string())?,
            payload: bytes.to_vec(),
        },
        limits,
    )
    .map_err(|error| error.to_string())?;
    if reply.control.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err(reply
            .control
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Document operation failed")
            .to_owned());
    }
    let result = serde_json::from_value(reply.control.clone())
        .map_err(|error| format!("Invalid document-worker response: {error}"))?;
    Ok((result, reply))
}

fn unlock_read(
    read: ReadOperation,
    bytes: &[u8],
    password: &str,
) -> Result<(DocumentResult, Reply), String> {
    if password.len() > pdf_worker::document::MAX_PASSWORD_BYTES {
        return Err("Password exceeds 4096 UTF-8 bytes".into());
    }
    let length = bytes
        .len()
        .checked_add(password.len())
        .filter(|size| *size <= pdf_worker::MAX_PDF_BYTES)
        .ok_or("Encrypted PDF and password exceed the 128 MiB transport bound")?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(length)
        .map_err(|_| "Cannot allocate bounded password request")?;
    payload.extend_from_slice(bytes);
    payload.extend_from_slice(password.as_bytes());
    execute(
        Operation::Unlock {
            password_bytes_len: password.len() as u32,
            read,
        },
        &payload,
    )
}

pub(super) fn validate_info(info: &DocumentInfo) -> Result<(), String> {
    if info.version.len() > 128
        || info.page_count == 0
        || info.page_count > pdf_core::MAX_PAGES
        || info.object_count > pdf_core::MAX_OBJECTS
        || info.pages.len() != info.page_count
        || info.signature_fields > pdf_core::MAX_OBJECTS
    {
        return Err("Document worker returned invalid document limits".into());
    }
    for (index, page) in info.pages.iter().enumerate() {
        if page.number as usize != index + 1
            || page.rotation % 90 != 0
            || page
                .media_box
                .iter()
                .chain(&page.crop_box)
                .any(|value| !value.is_finite())
            || page.media_box[0] >= page.media_box[2]
            || page.media_box[1] >= page.media_box[3]
            || page.crop_box[0] >= page.crop_box[2]
            || page.crop_box[1] >= page.crop_box[3]
        {
            return Err("Document worker returned invalid page geometry".into());
        }
    }
    Ok(())
}

impl WorkingDocument {
    #[cfg(test)]
    pub fn load(path: &Path) -> Result<Self, String> {
        match Self::probe(path)? {
            OpenResult::Open(document) => Ok(document),
            OpenResult::PasswordRequired(_) => {
                Err("PDF requires an explicit read-only password opening".into())
            }
        }
    }

    pub fn probe(path: &Path) -> Result<OpenResult, String> {
        let bytes = pdf_core::read_pdf_bytes(path).map_err(|error| error.to_string())?;
        let (result, reply) = execute(Operation::Open, &bytes)?;
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected document information payload".into());
        }
        if let DocumentResult::PasswordRequired { incorrect: false } = result {
            return Ok(OpenResult::PasswordRequired(
                pdf_worker::payload_fingerprint(&bytes),
            ));
        }
        let DocumentResult::Info { info } = result else {
            return Err("Unexpected document opening response".into());
        };
        validate_info(&info)?;
        if info.encrypted {
            return Err("Encrypted opening must require explicit session-only consent".into());
        }
        Ok(OpenResult::Open(Self {
            current: Snapshot {
                bytes,
                info,
                revision: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: Some(0),
            next_revision: 1,
            path: Some(std::path::absolute(path).map_err(|error| error.to_string())?),
            password: None,
        }))
    }

    pub fn unlock(
        path: &Path,
        fingerprint: [u8; 32],
        password: String,
    ) -> Result<OpenResult, String> {
        if password.len() > pdf_worker::document::MAX_PASSWORD_BYTES {
            return Err("Password exceeds 4096 UTF-8 bytes".into());
        }
        let bytes = pdf_core::read_pdf_bytes(path).map_err(|error| error.to_string())?;
        if pdf_worker::payload_fingerprint(&bytes) != fingerprint {
            return Err("The encrypted source changed. Cancel and open the PDF again.".into());
        }
        let (result, reply) = unlock_read(ReadOperation::Info, &bytes, &password)?;
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected password-opening payload".into());
        }
        if let DocumentResult::PasswordRequired { incorrect: true } = result {
            return Ok(OpenResult::PasswordRequired(fingerprint));
        }
        let DocumentResult::Info { info } = result else {
            return Err("Unexpected password-opening response".into());
        };
        validate_info(&info)?;
        if !info.encrypted {
            return Err("Password opening must preserve encrypted read-only status".into());
        }
        Ok(OpenResult::Open(Self {
            current: Snapshot {
                bytes,
                info,
                revision: 0,
            },
            undo: vec![],
            redo: vec![],
            saved_revision: Some(0),
            next_revision: 1,
            path: Some(std::path::absolute(path).map_err(|error| error.to_string())?),
            password: Some(password),
        }))
    }

    pub fn session_only(&self) -> bool {
        self.password.is_some() || self.current.info.encrypted
    }

    pub fn read(&self, read: ReadOperation) -> Result<(DocumentResult, Reply), String> {
        if let Some(password) = &self.password {
            unlock_read(read, &self.current.bytes, password)
        } else {
            let operation = match read {
                ReadOperation::Notes { page, offset } => Operation::Notes { page, offset },
                ReadOperation::Info => Operation::Info,
                ReadOperation::Forms => Operation::Forms,
                ReadOperation::Metadata => Operation::Metadata,
                ReadOperation::Text { page } => Operation::Text { page },
                ReadOperation::Render {
                    page,
                    scale,
                    logical_zoom,
                } => Operation::Render {
                    page,
                    scale,
                    logical_zoom,
                },
            };
            execute(operation, &self.current.bytes)
        }
    }

    pub fn merge(&self, revision: u64, paths: &[PathBuf]) -> Result<Self, String> {
        if revision != self.current.revision {
            return Err("Merge request is stale. Review the current document again.".into());
        }
        if (self.current.info.encrypted || self.current.info.signature_fields != 0)
            || paths.is_empty()
            || paths.len() >= pdf_core::MAX_PAGES
        {
            return Err("Merge needs an editable source and 1–19,999 additional PDFs".into());
        }
        let mut total = self.current.bytes.len() as u64;
        for path in paths {
            let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
            if !metadata.is_file() {
                return Err("Merge inputs must be regular files".into());
            }
            total = total
                .checked_add(metadata.len())
                .ok_or("Merge input size overflow")?;
            if total > pdf_core::MAX_INPUT_BYTES as u64 {
                return Err("Merge inputs including the current PDF exceed 128 MiB".into());
            }
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total as usize)
            .map_err(|_| "Cannot allocate bounded merge input")?;
        bytes.extend_from_slice(&self.current.bytes);
        let mut lengths = Vec::with_capacity(paths.len() + 1);
        lengths.push(self.current.bytes.len());
        for path in paths {
            let input = pdf_core::read_input_bytes(path, pdf_core::MAX_INPUT_BYTES - bytes.len())
                .map_err(|error| format!("{}: {error}", path.display()))?;
            bytes
                .try_reserve(input.len())
                .map_err(|_| "Cannot allocate bounded merge input")?;
            lengths.push(input.len());
            bytes.extend_from_slice(&input);
        }
        let (result, reply) = execute(Operation::Merge { lengths }, &bytes)?;
        let DocumentResult::Document {
            info,
            selected_page: Some(0),
        } = result
        else {
            return Err("Unexpected merged-document response".into());
        };
        validate_info(&info)?;
        if reply.kind != PayloadKind::Pdf
            || reply.payload.is_empty()
            || reply.payload.len() > pdf_core::MAX_INPUT_BYTES
            || info.page_count <= self.current.info.page_count
            || !info.pages.starts_with(&self.current.info.pages)
            || (info.encrypted || info.signature_fields != 0)
        {
            return Err("Invalid merged PDF payload or source geometry".into());
        }
        Ok(Self {
            current: Snapshot {
                bytes: reply.payload,
                info,
                revision: 0,
            },
            undo: vec![],
            redo: vec![],
            saved_revision: None,
            next_revision: 1,
            path: None,
            password: None,
        })
    }

    pub fn from_images(paths: &[PathBuf]) -> Result<Self, String> {
        if paths.is_empty() || paths.len() > pdf_core::MAX_PAGES {
            return Err("Select 1–20,000 PNG/JPEG images.".into());
        }
        let mut preflight = 0u64;
        for path in paths {
            let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
            if !metadata.is_file() {
                return Err("Image input must be a regular file".into());
            }
            preflight = preflight
                .checked_add(metadata.len())
                .ok_or("Image input size overflow")?;
            if metadata.len() > pdf_core::MAX_STREAM_BYTES as u64
                || preflight > pdf_core::MAX_INPUT_BYTES as u64
            {
                return Err(format!(
                    "{}: Image inputs allow 32 MiB per file and 128 MiB total.",
                    path.display()
                ));
            }
        }
        let mut bytes = Vec::new();
        let mut lengths = Vec::with_capacity(paths.len());
        for path in paths {
            let bound = pdf_core::MAX_STREAM_BYTES.min(pdf_core::MAX_INPUT_BYTES - bytes.len());
            let image = pdf_core::read_input_bytes(path, bound).map_err(|error| {
                format!(
                    "{}: {error}. Image inputs allow 32 MiB per file and 128 MiB total.",
                    path.display()
                )
            })?;
            lengths.push(image.len());
            bytes.extend_from_slice(&image);
        }
        let (result, reply) = execute(Operation::Images { lengths }, &bytes).map_err(|error| format!("{error}. Images to PDF supports PNG/JPEG; unsupported orientation or color metadata must be resolved before importing."))?;
        let DocumentResult::Document {
            info,
            selected_page: _,
        } = result
        else {
            return Err("Unexpected converted-document response".into());
        };
        validate_info(&info)?;
        if reply.kind != PayloadKind::Pdf || reply.payload.is_empty() {
            return Err("Missing converted PDF payload".into());
        }
        Ok(Self {
            current: Snapshot {
                bytes: reply.payload,
                info,
                revision: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: None,
            next_revision: 1,
            path: None,
            password: None,
        })
    }

    pub fn status(&self) -> Result<DocumentStatus, String> {
        Ok(DocumentStatus {
            info: self.current.info.clone(),
            revision: self.current.revision,
            dirty: Some(self.current.revision) != self.saved_revision,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        })
    }

    pub fn byte_len(&self) -> usize {
        self.current.bytes.len()
            + self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|snapshot| snapshot.bytes.len())
                .sum::<usize>()
    }

    pub fn extract_pages(&self, revision: u64, pages: Vec<u32>) -> Result<Self, String> {
        if revision != self.current.revision {
            return Err("Page selection is stale. Select pages again.".into());
        }
        validate_selection(&pages, self.current.info.page_count)?;
        let expected_pages = pages.len();
        let (result, reply) = execute(Operation::ExtractPages { pages }, &self.current.bytes)?;
        let DocumentResult::Document {
            info,
            selected_page,
        } = result
        else {
            return Err("Unexpected extracted document response".into());
        };
        validate_info(&info)?;
        if reply.kind != PayloadKind::Pdf
            || reply.payload.is_empty()
            || reply.payload.len() > HISTORY_BYTES
            || info.page_count != expected_pages
            || selected_page != Some(0)
        {
            return Err("Invalid extracted document payload or selection".into());
        }
        Ok(Self {
            current: Snapshot {
                bytes: reply.payload,
                info,
                revision: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: None,
            next_revision: 1,
            path: None,
            password: None,
        })
    }

    pub fn sanitize(&self, revision: u64) -> Result<(Self, pdf_core::SanitizationReport), String> {
        if revision != self.current.revision {
            return Err("Sanitization request is stale. Review the current document again.".into());
        }
        let (result, reply) = execute(Operation::Sanitize, &self.current.bytes)?;
        let DocumentResult::Sanitized { info, report } = result else {
            return Err("Unexpected sanitization response".into());
        };
        validate_info(&info)?;
        if reply.kind != PayloadKind::Pdf
            || reply.payload.is_empty()
            || reply.payload.len() > HISTORY_BYTES
            || info.pages != self.current.info.pages
            || !report.full_rewrite
            || report.attachment_annotations_removed.len() > 20_000
            || report.limitations.len() > 16
            || report.limitations.iter().any(|text| text.len() > 512)
            || report.limitations.is_empty()
            || [
                report.info_links_removed,
                report.xmp_links_removed,
                report.associated_file_links_removed,
                report.embedded_file_name_trees_removed,
                report.javascript_name_trees_removed,
                report.collections_removed,
                report.action_links_removed,
                report.automatic_triggers_removed,
                report.unreachable_objects_removed,
                report.transport_containers_removed,
            ]
            .iter()
            .any(|count| *count > 2_000_000)
            || report
                .attachment_annotations_removed
                .iter()
                .any(|annotation| {
                    annotation.page == 0
                        || annotation.page as usize > info.page_count
                        || annotation.rectangle.iter().any(|value| !value.is_finite())
                        || annotation.rectangle[0] >= annotation.rectangle[2]
                        || annotation.rectangle[1] >= annotation.rectangle[3]
                })
        {
            return Err("Invalid sanitized payload or removal report".into());
        }
        Ok((
            Self {
                current: Snapshot {
                    bytes: reply.payload,
                    info,
                    revision: 0,
                },
                undo: vec![],
                redo: vec![],
                saved_revision: None,
                next_revision: 1,
                path: None,
                password: None,
            },
            report,
        ))
    }

    pub fn notes(
        &self,
        page: u32,
        offset: u32,
    ) -> Result<(Vec<pdf_worker::document::NoteEntry>, Option<u32>), String> {
        let (result, reply) = self.read(ReadOperation::Notes { page, offset })?;
        let DocumentResult::Notes {
            entries,
            next_offset,
        } = result
        else {
            return Err("Unexpected notes reply".into());
        };
        let mut bytes = 0usize;
        let mut indices = std::collections::BTreeSet::new();
        if reply.kind != PayloadKind::None
            || !reply.payload.is_empty()
            || entries.len() > 128
            || next_offset.is_some_and(|next| {
                Some(next) != offset.checked_add(entries.len() as u32) || entries.is_empty()
            })
        {
            return Err("Invalid bounded note report".into());
        }
        for entry in &entries {
            let note = &entry.note;
            bytes = bytes.saturating_add(note.contents.len() + note.author.len());
            if page == 0
                || page as usize > self.current.info.page_count
                || note.page != page
                || note.annotation_index == 0
                || note.annotation_index > 10_000
                || !indices.insert(note.annotation_index)
                || note.contents.len() > pdf_core::MAX_NOTE_CONTENT_BYTES
                || note.author.len() > pdf_core::MAX_NOTE_AUTHOR_BYTES
                || note.rectangle.iter().any(|value| !value.is_finite())
                || note.rectangle[0] >= note.rectangle[2]
                || note.rectangle[1] >= note.rectangle[3]
                || note
                    .object_id
                    .is_some_and(|id| id.0 == 0 || id.0 as usize > pdf_core::MAX_OBJECTS)
                || entry.editable != entry.refusal_reason.is_none()
                || entry
                    .refusal_reason
                    .as_ref()
                    .is_some_and(|reason| reason.is_empty() || reason.len() > 2048)
                || entry.editable
                    && (self.current.info.encrypted || self.current.info.signature_fields != 0)
            {
                return Err("Invalid note identity, geometry or eligibility".into());
            }
        }
        if bytes > 512 * 1024 {
            return Err("Notes report exceeds 512 KiB".into());
        }
        Ok((entries, next_offset))
    }

    pub fn split(&self, revision: u64, every: u32, budget: usize) -> Result<Vec<Self>, String> {
        if revision != self.current.revision
            || self.current.info.encrypted
            || self.current.info.signature_fields != 0
        {
            return Err("Split request is stale or the source is readonly".into());
        }
        let (result, reply) = execute(Operation::Split { every }, &self.current.bytes)?;
        let DocumentResult::Split { parts } = result else {
            return Err("Unexpected split response".into());
        };
        if reply.kind != PayloadKind::Pdf || reply.payload.len() > budget {
            return Err("Split payload exceeds the available snapshot budget".into());
        }
        let slices =
            pdf_worker::document::split_slices(&parts, &reply.payload, &self.current.info, every)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(parts.len())
            .map_err(|_| "Cannot allocate bounded split documents")?;
        for (part, bytes) in parts.into_iter().zip(slices) {
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| "Cannot allocate split snapshot")?;
            owned.extend_from_slice(bytes);
            result.push(Self {
                current: Snapshot {
                    bytes: owned,
                    info: part.info,
                    revision: 0,
                },
                undo: vec![],
                redo: vec![],
                saved_revision: None,
                next_revision: 1,
                path: None,
                password: None,
            });
        }
        Ok(result)
    }
    pub fn metadata(&self) -> Result<pdf_core::DocumentMetadata, String> {
        let (result, reply) = self.read(ReadOperation::Metadata)?;
        let DocumentResult::Metadata { metadata } = result else {
            return Err("Unexpected metadata response".into());
        };
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected metadata payload".into());
        }
        Ok(metadata)
    }

    pub fn forms(&self) -> Result<(Vec<pdf_core::FormField>, Vec<pdf_core::ChoiceField>), String> {
        let (result, reply) = self.read(ReadOperation::Forms)?;
        let DocumentResult::Forms { fields, choices } = result else {
            return Err("Unexpected forms response".into());
        };
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected forms payload".into());
        }
        validate_forms(&fields, self.current.info.page_count)?;
        validate_choices(&choices, &fields, self.current.info.page_count)?;
        Ok((fields, choices))
    }

    pub fn as_ref<'a>(&'a self, id: &'a str) -> DocumentStateRef<'a> {
        DocumentStateRef {
            id,
            current: self.current.as_ref(),
            undo: self.undo.iter().map(Snapshot::as_ref).collect(),
            redo: self.redo.iter().map(Snapshot::as_ref).collect(),
            saved_revision: self.saved_revision,
            next_revision: self.next_revision,
            path: self.path.as_deref(),
        }
    }

    #[cfg(test)]
    pub fn edit(&mut self, edit: Edit) -> Result<Option<usize>, String> {
        self.edit_with_budget(edit, HISTORY_BYTES)
    }

    #[cfg(test)]
    pub fn edit_with_budget(&mut self, edit: Edit, budget: usize) -> Result<Option<usize>, String> {
        self.edit_durable(edit, budget, "test", |_| Ok(()))
    }

    pub fn edit_durable(
        &mut self,
        edit: Edit,
        budget: usize,
        id: &str,
        commit: impl FnOnce(&DocumentStateRef<'_>) -> Result<(), String>,
    ) -> Result<Option<usize>, String> {
        if self.session_only() {
            return Err("Password-opened PDFs are read only and session only".into());
        }
        let budget = budget.min(HISTORY_BYTES);
        if matches!(edit, Edit::Undo | Edit::Redo) {
            let undo = matches!(edit, Edit::Undo);
            let target = if undo {
                self.undo.last()
            } else {
                self.redo.last()
            }
            .ok_or(if undo {
                "Nothing to undo"
            } else {
                "Nothing to redo"
            })?;
            let mut candidate = self.as_ref(id);
            candidate.current = target.as_ref();
            if undo {
                candidate.undo.pop();
                candidate.redo.push(self.current.as_ref());
            } else {
                candidate.redo.pop();
                candidate.undo.push(self.current.as_ref());
            }
            commit(&candidate)?;
            drop(candidate);
            if undo {
                let previous = self.undo.pop().ok_or("Nothing to undo")?;
                self.redo
                    .push(std::mem::replace(&mut self.current, previous));
            } else {
                let next = self.redo.pop().ok_or("Nothing to redo")?;
                self.undo.push(std::mem::replace(&mut self.current, next));
            }
            return Ok(None);
        }
        let mut expected_duplicate = None;
        let mut expected_fill = None;
        let mut expected_overlay = false;
        let mut expected_note = None;
        let mut text_payload = None;
        let mutation = match edit {
            Edit::VectorOverlay { key, overlay } => {
                if self.current.info.encrypted || self.current.info.signature_fields > 0 {
                    return Err("Signed or encrypted documents are read only".into());
                }
                if key.revision != self.current.revision {
                    return Err("Drawing draft is stale. Close and reopen drawing controls.".into());
                }
                if overlay.page == 0 || overlay.page as usize > self.current.info.page_count {
                    return Err("Drawing page is outside this document".into());
                }
                expected_fill = Some((self.current.info.page_count, overlay.page as usize - 1));
                expected_overlay = true;
                Mutation::VectorOverlay { overlay }
            }
            Edit::ImageOverlay {
                key,
                overlay,
                image_file,
            } => {
                if self.current.info.encrypted || self.current.info.signature_fields > 0 {
                    return Err("Signed or encrypted documents are read only".into());
                }
                if key.revision != self.current.revision
                    || overlay.page == 0
                    || overlay.page as usize > self.current.info.page_count
                {
                    return Err("Image draft revision or page is stale".into());
                }
                let remaining = pdf_worker::MAX_PDF_BYTES
                    .saturating_sub(self.current.bytes.len())
                    .min(pdf_core::MAX_STREAM_BYTES);
                if remaining == 0 {
                    return Err("No input budget remains for the image".into());
                }
                let image = pdf_core::read_input_bytes(&image_file, remaining)
                    .map_err(|error| error.to_string())?;
                if image.is_empty() {
                    return Err("Choose a nonempty PNG/JPEG".into());
                }
                let mut payload = Vec::new();
                payload
                    .try_reserve_exact(self.current.bytes.len() + image.len())
                    .map_err(|_| "Cannot allocate bounded image overlay payload")?;
                payload.extend_from_slice(&self.current.bytes);
                payload.extend_from_slice(&image);
                expected_fill = Some((self.current.info.page_count, overlay.page as usize - 1));
                expected_overlay = true;
                text_payload = Some(payload);
                Mutation::ImageOverlay {
                    overlay,
                    image_bytes_len: image.len() as u32,
                }
            }
            Edit::TextOverlay {
                key,
                overlay,
                font_file,
            } => {
                if self.current.info.encrypted || self.current.info.signature_fields > 0 {
                    return Err("Signed or encrypted documents are read only".into());
                }
                if key.revision != self.current.revision {
                    return Err("Overlay draft is stale. Reopen the overlay dialog.".into());
                }
                if overlay.page == 0
                    || overlay.page as usize > self.current.info.page_count
                    || overlay.value.is_empty()
                    || overlay.value.len() > 64 * 1024
                {
                    return Err("Overlay page or text value is invalid".into());
                }
                let font = pdf_core::read_input_bytes(&font_file, 16 * 1024 * 1024)
                    .map_err(|e| e.to_string())?;
                let combined = self
                    .current
                    .bytes
                    .len()
                    .checked_add(font.len())
                    .filter(|len| *len <= pdf_worker::MAX_PDF_BYTES)
                    .ok_or("Combined document and overlay font exceed the input limit")?;
                if font.is_empty() {
                    return Err("Choose a nonempty covering font".into());
                }
                let mut payload = Vec::with_capacity(combined);
                payload.extend_from_slice(&self.current.bytes);
                payload.extend_from_slice(&font);
                expected_fill = Some((self.current.info.page_count, overlay.page as usize - 1));
                expected_overlay = true;
                text_payload = Some(payload);
                Mutation::TextOverlay {
                    overlay,
                    font_bytes_len: font.len() as u32,
                }
            }
            Edit::Note { key, change } => {
                if key.revision != self.current.revision || key.page != change.page() {
                    return Err("Note request revision or page is stale".into());
                }
                expected_note = Some(
                    (key.page as usize)
                        .checked_sub(1)
                        .filter(|page| *page < self.current.info.page_count)
                        .ok_or("Note page is outside the document")?,
                );
                Mutation::Note { change }
            }
            Edit::FillText {
                key,
                changes,
                buttons,
                choices,
                page,
                font_file,
            } => {
                if self.current.info.encrypted || self.current.info.signature_fields > 0 {
                    return Err("Signed or encrypted form documents are read only".into());
                }
                if key.revision != self.current.revision {
                    return Err("Form draft is stale. Read the form again.".into());
                }
                if page == 0 || page as usize > self.current.info.page_count {
                    return Err("Current form page is outside the document".into());
                }
                if changes.is_empty() && choices.is_empty()
                    || changes.len() > 10_000
                    || buttons.len() > 10_000
                    || choices.len() > 10_000
                {
                    return Err("Text changes exceed the bounded form request".into());
                }
                let mut names = std::collections::BTreeSet::new();
                if changes.iter().any(|change| {
                    change.name.is_empty()
                        || change.name.len() > 4096
                        || change.value.len() > 65_536
                        || !names.insert(&change.name)
                }) {
                    return Err("Text changes must have unique bounded names and values".into());
                }
                if buttons.iter().any(|change| {
                    change.name.is_empty()
                        || change.name.len() > 4096
                        || !names.insert(&change.name)
                }) {
                    return Err("Mixed form names must be unique and bounded".into());
                }
                if choices.iter().any(|change| {
                    change.name.is_empty()
                        || change.name.len() > 4096
                        || !names.insert(&change.name)
                        || change.indices.len() > 10_000
                        || change.indices.windows(2).any(|pair| pair[0] >= pair[1])
                }) {
                    return Err(
                        "Choice changes must use unique bounded names and sorted indices".into(),
                    );
                }
                let raw_bytes = changes
                    .iter()
                    .map(|change| change.name.len() + change.value.len())
                    .sum::<usize>()
                    + buttons
                        .iter()
                        .map(|change| change.name.len())
                        .sum::<usize>()
                    + choices
                        .iter()
                        .map(|change| change.name.len() + change.indices.len() * 4)
                        .sum::<usize>();
                if raw_bytes > 4 * 1024 * 1024
                    || serde_json::to_vec(&(&changes, &buttons, &choices))
                        .map_err(|error| error.to_string())?
                        .len()
                        > 4 * 1024 * 1024
                {
                    return Err("Text and button changes exceed the bounded form request".into());
                }
                let font = pdf_core::read_input_bytes(&font_file, 16 * 1024 * 1024)
                    .map_err(|error| error.to_string())?;
                if font.is_empty()
                    || self
                        .current
                        .bytes
                        .len()
                        .checked_add(font.len())
                        .is_none_or(|len| len > pdf_worker::MAX_PDF_BYTES)
                {
                    return Err("Combined document and text font exceed the input limit".into());
                }
                let font_bytes_len = font.len() as u32;
                let mut payload = Vec::with_capacity(self.current.bytes.len() + font.len());
                payload.extend_from_slice(&self.current.bytes);
                payload.extend_from_slice(&font);
                text_payload = Some(payload);
                expected_fill = Some((self.current.info.page_count, page as usize - 1));
                Mutation::FillText {
                    changes,
                    buttons,
                    choices,
                    page,
                    font_bytes_len,
                }
            }
            Edit::FillButtons { key, changes, page } => {
                if self.current.info.encrypted || self.current.info.signature_fields > 0 {
                    return Err("Signed or encrypted form documents are read only".into());
                }
                if key.revision != self.current.revision {
                    return Err("Form draft is stale. Read the form again.".into());
                }
                if page == 0 || page as usize > self.current.info.page_count {
                    return Err("Current form page is outside the document".into());
                }
                if changes.is_empty()
                    || changes.len() > 10_000
                    || serde_json::to_vec(&changes)
                        .map_err(|error| error.to_string())?
                        .len()
                        > 4 * 1024 * 1024
                {
                    return Err("Button changes exceed the bounded form request".into());
                }
                let mut names = std::collections::BTreeSet::new();
                if changes.iter().any(|change| {
                    change.name.is_empty()
                        || change.name.len() > 4096
                        || !names.insert(&change.name)
                }) {
                    return Err("Button change names must be unique and bounded".into());
                }
                expected_fill = Some((self.current.info.page_count, page as usize - 1));
                Mutation::FillButtons { changes, page }
            }
            Edit::DuplicatePages {
                revision,
                pages,
                page,
            } => {
                if revision != self.current.revision {
                    return Err("Page selection is stale. Select pages again.".into());
                }
                validate_selection(&pages, self.current.info.page_count)?;
                if page == 0 || page as usize > self.current.info.page_count {
                    return Err("Current page is outside the document".into());
                }
                let count = self
                    .current
                    .info
                    .page_count
                    .checked_add(pages.len())
                    .filter(|count| *count <= pdf_core::MAX_PAGES)
                    .ok_or("Duplicated pages exceed the page limit")?;
                expected_duplicate = Some((
                    count,
                    page as usize - 1 + pages.iter().filter(|number| **number < page).count(),
                ));
                Mutation::DuplicatePages { pages, page }
            }
            Edit::DeletePages {
                revision,
                pages,
                page,
            } => {
                if revision != self.current.revision {
                    return Err("Page selection is stale. Select pages again.".into());
                }
                validate_selection(&pages, self.current.info.page_count)?;
                if page == 0 || page as usize > self.current.info.page_count {
                    return Err("Current page is outside the document".into());
                }
                Mutation::DeletePages { pages, page }
            }
            Edit::Rotate(page, degrees) => Mutation::Rotate { page, degrees },
            Edit::Move { page, earlier } => Mutation::Move { page, earlier },
            Edit::Reverse { page } => Mutation::Reverse { page },
            Edit::InsertBlank {
                position,
                width,
                height,
            } => Mutation::InsertBlank {
                position,
                width,
                height,
            },
            Edit::Crop { page, rectangle } => Mutation::Crop { page, rectangle },
            Edit::Metadata { key, changes } => {
                if key.revision != self.current.revision {
                    return Err(
                        "Metadata draft is stale. Read the document properties again.".into(),
                    );
                }
                Mutation::Metadata {
                    changes: changes
                        .into_iter()
                        .map(|(key, value)| MetadataChange { key, value })
                        .collect(),
                }
            }
            Edit::Undo | Edit::Redo => unreachable!("history handled above"),
        };
        let (result, reply) = execute(
            Operation::Edit { mutation },
            text_payload.as_deref().unwrap_or(&self.current.bytes),
        )?;
        let DocumentResult::Document {
            info,
            selected_page,
        } = result
        else {
            return Err("Unexpected edit response".into());
        };
        validate_info(&info)?;
        if expected_overlay && info.pages != self.current.info.pages {
            return Err("Overlay changed the original page geometry".into());
        }
        if expected_note.is_some_and(|page| {
            info.pages != self.current.info.pages || selected_page != Some(page)
        }) {
            return Err("Note edit changed page geometry or selection".into());
        }
        if expected_fill
            .is_some_and(|(count, page)| info.page_count != count || selected_page != Some(page))
        {
            return Err("Invalid filled document page count or selection".into());
        }
        if expected_duplicate
            .is_some_and(|(count, page)| info.page_count != count || selected_page != Some(page))
        {
            return Err("Invalid duplicated document page count or selection".into());
        }
        if reply.kind != PayloadKind::Pdf
            || reply.payload.is_empty()
            || selected_page.is_some_and(|page| page >= info.page_count)
        {
            return Err("Invalid edited document payload or selection".into());
        }
        if reply.payload.len() > budget {
            return Err("Edited document exceeds the available snapshot budget; close a tab before retrying.".into());
        }
        let next_revision = self
            .next_revision
            .checked_add(1)
            .ok_or("Document revision counter exhausted")?;
        let snapshot = Snapshot {
            bytes: reply.payload,
            info,
            revision: self.next_revision,
        };
        let mut undo: Vec<_> = self
            .undo
            .iter()
            .map(Snapshot::as_ref)
            .chain(std::iter::once(self.current.as_ref()))
            .collect();
        let mut total =
            snapshot.bytes.len() + undo.iter().map(|entry| entry.bytes.len()).sum::<usize>();
        let mut dropped = 0;
        while undo.len() > HISTORY_LIMIT || total > budget {
            total -= undo.remove(0).bytes.len();
            dropped += 1;
        }
        let candidate = DocumentStateRef {
            id,
            current: snapshot.as_ref(),
            undo,
            redo: Vec::new(),
            saved_revision: self.saved_revision,
            next_revision,
            path: self.path.as_deref(),
        };
        commit(&candidate)?;
        drop(candidate);
        self.undo
            .push(std::mem::replace(&mut self.current, snapshot));
        self.redo.clear();
        self.undo.drain(..dropped);
        self.next_revision = next_revision;
        Ok(selected_page)
    }

    pub fn prepare_save(&self) -> Result<Vec<u8>, String> {
        if self.session_only() {
            return Err("Password-opened PDFs cannot be saved or exported".into());
        }
        let (result, reply) = execute(Operation::PrepareSave, &self.current.bytes)?;
        let DocumentResult::Document { info, .. } = result else {
            return Err("Unexpected save preparation response".into());
        };
        validate_info(&info)?;
        if reply.kind != PayloadKind::Pdf
            || reply.payload.is_empty()
            || info.pages != self.current.info.pages
        {
            return Err("Invalid verified save payload".into());
        }
        Ok(reply.payload)
    }

    #[cfg(test)]
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.prepare_save()?;
        publish(&bytes, path)?;
        self.saved_revision = Some(self.current.revision);
        Ok(())
    }

    pub fn save_durable(
        &mut self,
        path: &Path,
        id: &str,
        journal: &mut Journal,
    ) -> Result<(), String> {
        if journal
            .save_intents()
            .iter()
            .any(|intent| intent.document_id == id)
        {
            return Err("A previous save acknowledgement remains unresolved. Resolve or explicitly cancel it before saving another copy.".into());
        }
        match path.symlink_metadata() {
            Ok(_) => return Err("Output already exists; choose a new file".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.to_string()),
        }
        let bytes = self.prepare_save()?;
        let intent = journal
            .begin_save(id, self.current.revision, path, &bytes)
            .map_err(|error| {
                format!("Save journal failed; the document remains unsaved: {error}")
            })?;
        publish(&bytes, path)?;
        journal.finish_save(&intent.token).map_err(|error| format!("The new copy was published, but its save acknowledgement failed. The document remains unsaved and the save intent is recoverable: {error}"))?;
        self.saved_revision = Some(self.current.revision);
        self.path = Some(intent.destination);
        Ok(())
    }

    pub fn resolve_save(&mut self, id: &str, journal: &mut Journal) -> Result<PathBuf, String> {
        let intent = journal
            .save_intents()
            .iter()
            .find(|intent| intent.document_id == id)
            .ok_or("No pending save intent exists")?
            .clone();
        if intent.revision != self.current.revision {
            return Err(
                "The pending save belongs to another revision; its evidence remains retained"
                    .into(),
            );
        }
        let artifact = journal
            .save_artifact(&intent.token)
            .map_err(|error| error.to_string())?;
        match intent.destination.symlink_metadata() {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                publish(&artifact, &intent.destination)?
            }
            Err(error) => return Err(error.to_string()),
        }
        journal.finish_save(&intent.token).map_err(|error| format!("The pending copy could not be acknowledged; the document and save evidence remain retained: {error}"))?;
        self.saved_revision = Some(self.current.revision);
        self.path = Some(intent.destination.clone());
        Ok(intent.destination)
    }

    #[cfg(test)]
    pub fn document_for_test(&self) -> pdf_core::PdfDocument {
        pdf_core::PdfDocument::from_bytes(&self.current.bytes, None)
            .expect("trusted fixture inspection")
    }
}

pub(super) fn publish(bytes: &[u8], path: &Path) -> Result<(), String> {
    if path.symlink_metadata().is_ok() {
        return Err("Output already exists; choose a new file".into());
    }
    if bytes.len() > pdf_core::MAX_INPUT_BYTES {
        return Err("Output exceeds 128 MiB".into());
    }
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    for expected in bytes.chunks(buffer.len()) {
        temporary
            .read_exact(&mut buffer[..expected.len()])
            .map_err(|error| error.to_string())?;
        if &buffer[..expected.len()] != expected {
            return Err("Written output differs from verified bytes".into());
        }
    }
    if temporary
        .read(&mut buffer[..1])
        .map_err(|error| error.to_string())?
        != 0
    {
        return Err("Written output exceeds verified bytes".into());
    }
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}
