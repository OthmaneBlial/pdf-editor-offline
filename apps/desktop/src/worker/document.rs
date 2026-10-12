use super::{Edit, HISTORY_BYTES, HISTORY_LIMIT};
use crate::state::DocumentStatus;
use pdf_core::DocumentInfo;
use pdf_worker::document::{DocumentResult, MetadataChange, Mutation, Operation};
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
        slot.borrow().clone().map_or(
            (helper, arguments, pdf_worker::Limits::default()),
            |executor| (executor.executable, executor.arguments, executor.limits),
        )
    });
    #[cfg(not(test))]
    let limits = pdf_worker::Limits::default();
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
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = pdf_core::read_pdf_bytes(path).map_err(|error| error.to_string())?;
        let (result, reply) = execute(Operation::Info, &bytes)?;
        let DocumentResult::Info { info } = result else {
            return Err("Unexpected document information response".into());
        };
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected document information payload".into());
        }
        validate_info(&info)?;
        Ok(Self {
            current: Snapshot {
                bytes,
                info,
                revision: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: Some(0),
            next_revision: 1,
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

    pub fn metadata(&self) -> Result<pdf_core::DocumentMetadata, String> {
        let (result, reply) = execute(Operation::Metadata, &self.current.bytes)?;
        let DocumentResult::Metadata { metadata } = result else {
            return Err("Unexpected metadata response".into());
        };
        if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
            return Err("Unexpected metadata payload".into());
        }
        Ok(metadata)
    }

    #[cfg(test)]
    pub fn edit(&mut self, edit: Edit) -> Result<Option<usize>, String> {
        self.edit_with_budget(edit, HISTORY_BYTES)
    }

    pub fn edit_with_budget(&mut self, edit: Edit, budget: usize) -> Result<Option<usize>, String> {
        let budget = budget.min(HISTORY_BYTES);
        if let Edit::Undo = edit {
            let previous = self.undo.pop().ok_or("Nothing to undo")?;
            self.redo
                .push(std::mem::replace(&mut self.current, previous));
            return Ok(None);
        }
        if let Edit::Redo = edit {
            let next = self.redo.pop().ok_or("Nothing to redo")?;
            self.undo.push(std::mem::replace(&mut self.current, next));
            return Ok(None);
        }
        let mutation = match edit {
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
        let (result, reply) = execute(Operation::Edit { mutation }, &self.current.bytes)?;
        let DocumentResult::Document {
            info,
            selected_page,
        } = result
        else {
            return Err("Unexpected edit response".into());
        };
        validate_info(&info)?;
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
        self.next_revision = next_revision;
        self.undo
            .push(std::mem::replace(&mut self.current, snapshot));
        self.redo.clear();
        self.trim_history(budget);
        Ok(selected_page)
    }

    fn trim_history(&mut self, budget: usize) {
        while self.undo.len() + self.redo.len() > HISTORY_LIMIT || self.byte_len() > budget {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else {
                self.redo.remove(0);
            }
        }
    }

    pub fn prepare_save(&self) -> Result<Vec<u8>, String> {
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

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.prepare_save()?;
        publish(&bytes, path)?;
        self.saved_revision = Some(self.current.revision);
        Ok(())
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
