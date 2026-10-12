use super::{Edit, HISTORY_BYTES, HISTORY_LIMIT};
use crate::state::DocumentStatus;
use pdf_core::DocumentInfo;
use pdf_worker::document::{DocumentResult, MetadataChange, Mutation, Operation};
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
            path: Some(std::path::absolute(path).map_err(|error| error.to_string())?),
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
