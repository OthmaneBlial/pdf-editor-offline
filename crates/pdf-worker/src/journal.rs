//! Durable parent-side snapshots. PDF payloads are opaque; recovery requires child validation.
//! Use one private session directory per app launch. Locks are advisory, not a sandbox.
//! Eight combined undo/redo entries and 256 MiB of snapshot occurrences per document,
//! sixteen documents and 512 MiB recovered occurrences/retained unique blobs are capped.
//! Previous-manifest blobs count toward the disk budget; ordinary close/edit is not erasure.
//! Paths retain native Unix bytes/Windows wide units. Save intents bind source revision and a separately verified
//! artifact; this module never publishes or overwrites user output files.

use pdf_core::DocumentInfo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

mod native_path;
mod session;
pub use session::{SessionCandidate, SessionState, SessionStore};

#[cfg(windows)]
#[path = "journal_windows.rs"]
mod windows;

pub const MAX_DOCUMENTS: usize = 16;
pub const MAX_HISTORY: usize = 8;
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_JOURNAL_BYTES: usize = 512 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("journal I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("journal is already locked by another process")]
    Locked,
    #[error("journal rejected: {0}")]
    Invalid(&'static str),
    #[error("journal resource limit: {0}")]
    Limit(&'static str),
    #[error("journal manifest rejected: {0}")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, JournalError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub document_count: usize,
    /// Combined current/undo/redo snapshot occurrences, not resident memory.
    pub bytes: usize,
    pub save_intents: usize,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub revision: u64,
    pub bytes: Vec<u8>,
    pub info: DocumentInfo,
}

#[derive(Debug, Clone)]
pub struct DocumentState {
    pub id: String,
    pub current: Snapshot,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub saved_revision: Option<u64>,
    pub next_revision: u64,
    pub path: Option<PathBuf>,
}

/// Borrowed commit inputs avoid cloning retained PDF bytes in the parent.
#[derive(Clone, Copy)]
pub struct SnapshotRef<'a> {
    pub revision: u64,
    pub bytes: &'a [u8],
    pub info: &'a DocumentInfo,
}
pub struct DocumentStateRef<'a> {
    pub id: &'a str,
    pub current: SnapshotRef<'a>,
    pub undo: Vec<SnapshotRef<'a>>,
    pub redo: Vec<SnapshotRef<'a>>,
    pub saved_revision: Option<u64>,
    pub next_revision: u64,
    pub path: Option<&'a Path>,
}
impl Snapshot {
    pub fn as_ref(&self) -> SnapshotRef<'_> {
        SnapshotRef {
            revision: self.revision,
            bytes: &self.bytes,
            info: &self.info,
        }
    }
}
impl DocumentState {
    pub fn as_ref(&self) -> DocumentStateRef<'_> {
        DocumentStateRef {
            id: &self.id,
            current: self.current.as_ref(),
            undo: self.undo.iter().map(Snapshot::as_ref).collect(),
            redo: self.redo.iter().map(Snapshot::as_ref).collect(),
            saved_revision: self.saved_revision,
            next_revision: self.next_revision,
            path: self.path.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveIntent {
    pub token: String,
    pub document_id: String,
    pub revision: u64,
    #[serde(with = "native_path")]
    pub destination: PathBuf,
    pub source_hash: String,
    pub artifact_hash: String,
    pub artifact_bytes: usize,
}

pub struct SaveIntentRef<'a> {
    pub intent: &'a SaveIntent,
    pub artifact: &'a [u8],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSnapshot {
    revision: u64,
    hash: String,
    length: usize,
    info: DocumentInfo,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDocument {
    id: String,
    current: StoredSnapshot,
    undo: Vec<StoredSnapshot>,
    redo: Vec<StoredSnapshot>,
    saved_revision: Option<u64>,
    next_revision: u64,
    #[serde(with = "native_path::optional")]
    path: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    generation: u64,
    documents: Vec<StoredDocument>,
    save_intents: Vec<SaveIntent>,
}

pub struct Journal {
    directory: PathBuf,
    directory_handle: File,
    _lock: File,
    manifest: Manifest,
    backup: Option<Manifest>,
    poisoned: bool,
}

impl Journal {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Windows lacks the directory fsync step used on Unix; power-loss guarantees differ.
    pub fn durability(&self) -> &'static str {
        if cfg!(unix) {
            "file_and_directory_fsync_atomic_replace"
        } else {
            "file_fsync_atomic_replace_without_directory_fsync"
        }
    }
    /// Opens an existing recovery directory or creates a new private session directory.
    /// A missing current manifest may use its valid backup; a corrupt current is an error.
    pub fn open(directory: impl AsRef<Path>) -> Result<Self> {
        Self::open_internal(directory.as_ref(), true, true)
    }

    fn open_internal(directory: &Path, verify_blobs: bool, read_manifests: bool) -> Result<Self> {
        match directory.symlink_metadata() {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(JournalError::Invalid(
                        "session path must be a real directory",
                    ));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o077 != 0 {
                        return Err(JournalError::Invalid(
                            "session directory must be private (0700)",
                        ));
                    }
                }
                #[cfg(windows)]
                windows::validate_directory(directory)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                #[cfg(not(windows))]
                {
                    let mut builder = std::fs::DirBuilder::new();
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        builder.mode(0o700);
                    }
                    builder.create(directory)?;
                }
                #[cfg(windows)]
                {
                    windows::create_directory(directory)?;
                    windows::validate_directory(directory)?;
                }
                sync_directory(directory.parent().unwrap_or(Path::new(".")))?;
            }
            Err(error) => return Err(error.into()),
        }
        let lock_path = directory.join("session.lock");
        if lock_path
            .symlink_metadata()
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err(JournalError::Invalid("lock path must be a regular file"));
        }
        let mut options = private_options();
        options.read(true).write(true).create(true);
        let lock = options.open(lock_path)?;
        validate_handle(&lock, 1024)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(JournalError::Locked),
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
        let directory_handle = session::open_directory(directory, true)?;
        // Bind the advisory lock to the pinned directory, even if its pathname changed during open.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let witness =
                open_private_relative(&directory_handle, directory, "session.lock", false)?;
            let original = lock.metadata()?;
            let actual = witness.metadata()?;
            if original.dev() != actual.dev() || original.ino() != actual.ino() {
                return Err(JournalError::Invalid("session lock directory was replaced"));
            }
        }
        #[cfg(windows)]
        windows::verify_path_identity(&lock, &directory.join("session.lock"))?;
        let current = if read_manifests {
            read_private_manifest(&directory_handle, directory, "manifest.json")?
        } else {
            None
        };
        let backup = if read_manifests {
            read_private_manifest(&directory_handle, directory, "manifest.previous.json")?
        } else {
            None
        };
        let manifest = current.or_else(|| backup.clone()).unwrap_or(Manifest {
            schema: 1,
            generation: 0,
            documents: Vec::new(),
            save_intents: Vec::new(),
        });
        validate_manifest(&manifest)?;
        if let Some(backup) = &backup {
            validate_manifest(backup)?;
        }
        let result = Self {
            directory: directory.to_owned(),
            directory_handle,
            _lock: lock,
            manifest,
            backup,
            poisoned: false,
        };
        result.validate_retained(&result.manifest, result.backup.as_ref())?;
        if verify_blobs {
            for (hash, length) in retained(&result.manifest, result.backup.as_ref())? {
                result.read_blob(&hash, length)?;
            }
            result.cleanup()?;
        }
        Ok(result)
    }

    pub fn recover(&self) -> Result<Vec<DocumentState>> {
        self.manifest
            .documents
            .iter()
            .map(|document| {
                let snapshot = |stored: &StoredSnapshot| -> Result<Snapshot> {
                    Ok(Snapshot {
                        revision: stored.revision,
                        bytes: self.read_blob(&stored.hash, stored.length)?,
                        info: stored.info.clone(),
                    })
                };
                Ok(DocumentState {
                    id: document.id.clone(),
                    current: snapshot(&document.current)?,
                    undo: document.undo.iter().map(snapshot).collect::<Result<_>>()?,
                    redo: document.redo.iter().map(snapshot).collect::<Result<_>>()?,
                    saved_revision: document.saved_revision,
                    next_revision: document.next_revision,
                    path: document.path.clone(),
                })
            })
            .collect()
    }

    pub fn save_intents(&self) -> &[SaveIntent] {
        &self.manifest.save_intents
    }

    pub fn save_artifact(&self, token: &str) -> Result<Vec<u8>> {
        let intent = self
            .manifest
            .save_intents
            .iter()
            .find(|intent| intent.token == token)
            .ok_or(JournalError::Invalid("unknown save intent"))?;
        self.read_blob(&intent.artifact_hash, intent.artifact_bytes)
    }

    /// Metadata only: opaque PDF blobs are neither loaded nor parsed.
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            document_count: self.manifest.documents.len(),
            bytes: self
                .manifest
                .documents
                .iter()
                .flat_map(snapshots)
                .map(|s| s.length)
                .sum(),
            save_intents: self.manifest.save_intents.len(),
        }
    }

    /// Actual known journal files, including backup/orphan blobs. Closing a tab is not erasure.
    pub fn owned_disk_bytes(&self) -> Result<u64> {
        session::disk_usage(self)
    }

    /// Cancel an unpublished/uncertain save; this never changes the saved revision or user files.
    pub fn cancel_save(&mut self, token: &str) -> Result<()> {
        if !self
            .manifest
            .save_intents
            .iter()
            .any(|intent| intent.token == token)
        {
            return Err(JournalError::Invalid("unknown save intent"));
        }
        let mut candidate = self.manifest.clone();
        candidate
            .save_intents
            .retain(|intent| intent.token != token);
        self.prepare(&mut candidate)?;
        self.publish(candidate)
    }

    /// Explicit discard only. Unknown/private-file violations refuse before removing anything.
    pub fn discard(self) -> Result<()> {
        session::discard(self)
    }

    /// Writes blobs and the new manifest durably before returning success.
    pub fn commit_document(&mut self, document: &DocumentState) -> Result<()> {
        self.commit_document_ref(&document.as_ref())
    }

    pub fn commit_document_ref(&mut self, document: &DocumentStateRef<'_>) -> Result<()> {
        self.commit_documents_ref(std::slice::from_ref(document))
    }

    /// One transaction for several incoming documents, preserving other existing documents.
    pub fn commit_documents_ref(&mut self, documents: &[DocumentStateRef<'_>]) -> Result<()> {
        self.commit_documents_with_intents_ref(documents, &[])
    }

    /// Atomic recovery import, including still-uncertain save intents. No destination is inspected.
    pub fn commit_documents_with_intents_ref(
        &mut self,
        documents: &[DocumentStateRef<'_>],
        intents: &[SaveIntentRef<'_>],
    ) -> Result<()> {
        if documents.len() > MAX_DOCUMENTS {
            return Err(JournalError::Limit("more than sixteen incoming documents"));
        }
        if intents.len() > MAX_DOCUMENTS {
            return Err(JournalError::Limit(
                "more than sixteen incoming save intents",
            ));
        }
        let mut candidate = self.manifest.clone();
        let mut ids = BTreeSet::new();
        for document in documents {
            if !ids.insert(document.id) {
                return Err(JournalError::Invalid(
                    "duplicate incoming document identifier",
                ));
            }
            let stored = stored_document(document)?;
            if let Some(old) = candidate
                .documents
                .iter_mut()
                .find(|old| old.id == document.id)
            {
                *old = stored;
            } else {
                candidate.documents.push(stored);
            }
        }
        candidate
            .documents
            .sort_by(|left, right| left.id.cmp(&right.id));
        candidate
            .save_intents
            .retain(|intent| !ids.contains(intent.document_id.as_str()));
        for incoming in intents {
            let intent = incoming.intent;
            if !ids.contains(intent.document_id.as_str()) {
                return Err(JournalError::Invalid(
                    "incoming intent has no incoming document",
                ));
            }
            if incoming.artifact.len() != intent.artifact_bytes
                || incoming.artifact.len() > crate::MAX_PDF_BYTES
                || hash_bytes(incoming.artifact) != intent.artifact_hash
            {
                return Err(JournalError::Invalid(
                    "incoming save artifact differs from intent",
                ));
            }
            candidate.save_intents.push(intent.clone());
        }
        self.prepare(&mut candidate)?;
        for document in documents {
            for snapshot in std::iter::once(&document.current)
                .chain(&document.undo)
                .chain(&document.redo)
            {
                if let Err(error) = self.store_blob(snapshot.bytes) {
                    self.poisoned = true;
                    return Err(error);
                }
            }
        }
        for incoming in intents {
            if let Err(error) = self.store_blob(incoming.artifact) {
                self.poisoned = true;
                return Err(error);
            }
        }
        self.publish(candidate)
    }

    /// Records a save intent before the caller publishes a fresh output path.
    pub fn begin_save(
        &mut self,
        id: &str,
        revision: u64,
        destination: &Path,
        artifact: &[u8],
    ) -> Result<SaveIntent> {
        let destination = std::path::absolute(destination)?;
        validate_path(&destination)?;
        let current = &self
            .manifest
            .documents
            .iter()
            .find(|document| document.id == id)
            .ok_or(JournalError::Invalid("unknown document"))?
            .current;
        if revision != current.revision {
            return Err(JournalError::Invalid(
                "save revision does not match committed current revision",
            ));
        }
        if artifact.is_empty() || artifact.len() > crate::MAX_PDF_BYTES {
            return Err(JournalError::Limit("save artifact outside 1..128 MiB"));
        }
        let token = format!(
            "{}-{}-{}",
            id,
            self.manifest
                .generation
                .checked_add(1)
                .ok_or(JournalError::Limit("manifest generation overflow"))?,
            current.hash
        );
        let intent = SaveIntent {
            token,
            document_id: id.to_owned(),
            revision,
            destination,
            source_hash: current.hash.clone(),
            artifact_hash: hash_bytes(artifact),
            artifact_bytes: artifact.len(),
        };
        let mut candidate = self.manifest.clone();
        candidate.save_intents.retain(|old| old.document_id != id);
        candidate.save_intents.push(intent.clone());
        self.prepare(&mut candidate)?;
        if let Err(error) = self.store_blob(artifact) {
            self.poisoned = true;
            return Err(error);
        }
        self.publish(candidate)?;
        Ok(intent)
    }

    /// Acknowledge publication only after checking the actual destination bytes.
    pub fn finish_save(&mut self, token: &str) -> Result<()> {
        let intent = self
            .manifest
            .save_intents
            .iter()
            .find(|intent| intent.token == token)
            .ok_or(JournalError::Invalid("unknown save intent"))?
            .clone();
        let actual = read_regular(&intent.destination, crate::MAX_PDF_BYTES)?;
        if actual.len() != intent.artifact_bytes || hash_bytes(&actual) != intent.artifact_hash {
            return Err(JournalError::Invalid(
                "published artifact differs from durable save intent",
            ));
        }
        let mut candidate = self.manifest.clone();
        let document = candidate
            .documents
            .iter_mut()
            .find(|document| document.id == intent.document_id)
            .ok_or(JournalError::Invalid("save document missing"))?;
        document.saved_revision = Some(intent.revision);
        document.path = Some(intent.destination);
        candidate.save_intents.retain(|old| old.token != token);
        self.prepare(&mut candidate)?;
        self.publish(candidate)
    }

    pub fn close_document(&mut self, id: &str) -> Result<()> {
        if !self
            .manifest
            .documents
            .iter()
            .any(|document| document.id == id)
        {
            return Err(JournalError::Invalid("unknown document"));
        }
        let mut candidate = self.manifest.clone();
        candidate.documents.retain(|document| document.id != id);
        candidate
            .save_intents
            .retain(|intent| intent.document_id != id);
        self.prepare(&mut candidate)?;
        self.publish(candidate)
    }

    fn prepare(&self, candidate: &mut Manifest) -> Result<()> {
        if self.poisoned {
            return Err(JournalError::Invalid(
                "journal publication failed; reopen before further changes",
            ));
        }
        candidate.generation = self
            .manifest
            .generation
            .checked_add(1)
            .ok_or(JournalError::Limit("manifest generation overflow"))?;
        validate_manifest(candidate)?;
        self.validate_retained(candidate, Some(&self.manifest))?;
        let mut all = retained(candidate, Some(&self.manifest))?;
        if let Some(backup) = &self.backup {
            all.extend(retained(backup, None)?);
        }
        if all.values().sum::<usize>() > MAX_JOURNAL_BYTES {
            return Err(JournalError::Limit(
                "transaction and backup blobs exceed 512 MiB",
            ));
        }
        encode_manifest(candidate)?;
        Ok(())
    }
    fn validate_retained(&self, current: &Manifest, backup: Option<&Manifest>) -> Result<()> {
        let size = retained(current, backup)?
            .values()
            .try_fold(0usize, |sum, length| sum.checked_add(*length))
            .ok_or(JournalError::Limit("retained blob byte overflow"))?;
        if size > MAX_JOURNAL_BYTES {
            return Err(JournalError::Limit("retained blobs exceed 512 MiB"));
        }
        Ok(())
    }
    fn publish(&mut self, candidate: Manifest) -> Result<()> {
        let result = (|| -> Result<()> {
            atomic_write(
                &self.directory_handle,
                &self.directory,
                "manifest.previous.json",
                &encode_manifest(&self.manifest)?,
            )?;
            atomic_write(
                &self.directory_handle,
                &self.directory,
                "manifest.json",
                &encode_manifest(&candidate)?,
            )
        })();
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        self.backup = Some(std::mem::replace(&mut self.manifest, candidate));
        // Cleanup is optional after the committed manifest. Never turn a durable commit into failure.
        let _ = self.cleanup();
        Ok(())
    }
    fn read_blob(&self, hash: &str, length: usize) -> Result<Vec<u8>> {
        validate_hash(hash)?;
        let file = open_private_relative(
            &self.directory_handle,
            &self.directory,
            &format!("{hash}.blob"),
            false,
        )?;
        validate_handle(&file, crate::MAX_PDF_BYTES)?;
        let bytes = read_bounded(file, crate::MAX_PDF_BYTES)?;
        if bytes.len() != length || hash_bytes(&bytes) != hash {
            return Err(JournalError::Invalid("snapshot blob length/hash mismatch"));
        }
        Ok(bytes)
    }
    fn store_blob(&self, bytes: &[u8]) -> Result<()> {
        let hash = hash_bytes(bytes);
        let name = format!("{hash}.blob");
        match open_private_relative(&self.directory_handle, &self.directory, &name, false) {
            Ok(_) => {
                self.read_blob(&hash, bytes.len())?;
                return Ok(());
            }
            Err(JournalError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut file = open_private_relative(&self.directory_handle, &self.directory, &name, true)?;
        validate_handle(&file, crate::MAX_PDF_BYTES)?;
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        if result.is_err() {
            let _ = remove_relative(&self.directory_handle, &self.directory, &name);
        }
        result?;
        sync_handle(&self.directory_handle)
    }
    fn cleanup(&self) -> Result<()> {
        let keep = retained(&self.manifest, self.backup.as_ref())?;
        for name in session::entries(&self.directory_handle, &self.directory)? {
            let Some(name) = name.to_str() else {
                continue;
            };
            let orphan = name
                .strip_suffix(".blob")
                .is_some_and(|hash| validate_hash(hash).is_ok() && !keep.contains_key(hash));
            if orphan || name == "manifest.json.tmp" || name == "manifest.previous.json.tmp" {
                let file =
                    open_private_relative(&self.directory_handle, &self.directory, name, false)?;
                validate_handle(&file, crate::MAX_PDF_BYTES)?;
                remove_relative(&self.directory_handle, &self.directory, name)?;
            }
        }
        sync_handle(&self.directory_handle)
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output
}

fn stored_document(document: &DocumentStateRef<'_>) -> Result<StoredDocument> {
    validate_id(document.id)?;
    if document.undo.len().saturating_add(document.redo.len()) > MAX_HISTORY {
        return Err(JournalError::Limit(
            "combined undo/redo exceeds eight snapshots",
        ));
    }
    let stored = |snapshot: &SnapshotRef<'_>| -> Result<StoredSnapshot> {
        if snapshot.bytes.is_empty() || snapshot.bytes.len() > crate::MAX_PDF_BYTES {
            return Err(JournalError::Limit(
                "snapshot must contain 1..128 MiB opaque bytes",
            ));
        }
        validate_info(snapshot.info)?;
        Ok(StoredSnapshot {
            revision: snapshot.revision,
            hash: hash_bytes(snapshot.bytes),
            length: snapshot.bytes.len(),
            info: snapshot.info.clone(),
        })
    };
    Ok(StoredDocument {
        id: document.id.to_owned(),
        current: stored(&document.current)?,
        undo: document.undo.iter().map(stored).collect::<Result<_>>()?,
        redo: document.redo.iter().map(stored).collect::<Result<_>>()?,
        saved_revision: document.saved_revision,
        next_revision: document.next_revision,
        path: document.path.map(Path::to_owned),
    })
}
fn validate_hash(hash: &str) -> Result<()> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(JournalError::Invalid("invalid SHA256 blob name"));
    }
    Ok(())
}
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(JournalError::Invalid("invalid document identifier"));
    }
    Ok(())
}
fn validate_path(path: &Path) -> Result<()> {
    native_path::validate(path).map_err(|_| JournalError::Invalid("invalid saved path"))
}
fn validate_info(info: &DocumentInfo) -> Result<()> {
    if info.version.len() > 16
        || info.page_count == 0
        || info.page_count > pdf_core::MAX_PAGES
        || info.pages.len() != info.page_count
        || info.object_count > pdf_core::MAX_OBJECTS
    {
        return Err(JournalError::Invalid("document information exceeds bounds"));
    }
    for (index, page) in info.pages.iter().enumerate() {
        if page.number as usize != index + 1
            || !page
                .media_box
                .iter()
                .chain(&page.crop_box)
                .all(|value| value.is_finite())
            || ![0, 90, 180, 270].contains(&page.rotation)
        {
            return Err(JournalError::Invalid("invalid page information"));
        }
    }
    Ok(())
}
fn snapshots(document: &StoredDocument) -> impl Iterator<Item = &StoredSnapshot> {
    std::iter::once(&document.current)
        .chain(&document.undo)
        .chain(&document.redo)
}
fn validate_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.schema != 1 {
        return Err(JournalError::Invalid("unsupported journal schema"));
    }
    if manifest.documents.len() > MAX_DOCUMENTS || manifest.save_intents.len() > MAX_DOCUMENTS {
        return Err(JournalError::Limit("more than sixteen documents/intents"));
    }
    let mut ids = BTreeSet::new();
    let mut recovered_bytes = 0usize;
    for document in &manifest.documents {
        validate_id(&document.id)?;
        if !ids.insert(&document.id) {
            return Err(JournalError::Invalid("duplicate document identifier"));
        }
        if document.undo.len().saturating_add(document.redo.len()) > MAX_HISTORY {
            return Err(JournalError::Limit(
                "combined history exceeds eight snapshots",
            ));
        }
        if let Some(path) = &document.path {
            validate_path(path)?;
        }
        let mut lengths = BTreeMap::new();
        let mut revisions = BTreeSet::new();
        let mut document_bytes = 0usize;
        for snapshot in snapshots(document) {
            validate_hash(&snapshot.hash)?;
            validate_info(&snapshot.info)?;
            if snapshot.length == 0 || snapshot.length > crate::MAX_PDF_BYTES {
                return Err(JournalError::Limit("snapshot bytes outside 1..128 MiB"));
            }
            if snapshot.revision >= document.next_revision || !revisions.insert(snapshot.revision) {
                return Err(JournalError::Invalid("invalid/duplicate snapshot revision"));
            }
            if lengths
                .insert(&snapshot.hash, snapshot.length)
                .is_some_and(|length| length != snapshot.length)
            {
                return Err(JournalError::Invalid("hash has inconsistent lengths"));
            }
            document_bytes = document_bytes
                .checked_add(snapshot.length)
                .ok_or(JournalError::Limit("document byte overflow"))?;
        }
        if document
            .saved_revision
            .is_some_and(|revision| revision >= document.next_revision)
        {
            return Err(JournalError::Invalid("invalid saved revision"));
        }
        if document_bytes > MAX_DOCUMENT_BYTES {
            return Err(JournalError::Limit(
                "document snapshot occurrences exceed 256 MiB",
            ));
        }
        recovered_bytes = recovered_bytes
            .checked_add(document_bytes)
            .ok_or(JournalError::Limit("recovery byte overflow"))?;
    }
    if recovered_bytes > MAX_JOURNAL_BYTES {
        return Err(JournalError::Limit(
            "recovered snapshot occurrences exceed 512 MiB",
        ));
    }
    let mut intent_ids = BTreeSet::new();
    let mut intent_tokens = BTreeSet::new();
    for intent in &manifest.save_intents {
        if !intent_ids.insert(&intent.document_id) || !intent_tokens.insert(&intent.token) {
            return Err(JournalError::Invalid("duplicate save intent"));
        }
        validate_path(&intent.destination)?;
        validate_hash(&intent.artifact_hash)?;
        validate_hash(&intent.source_hash)?;
        let document = manifest
            .documents
            .iter()
            .find(|document| document.id == intent.document_id)
            .ok_or(JournalError::Invalid(
                "save intent refers to missing document",
            ))?;
        if intent.token.len() > 160
            || intent.token.is_empty()
            || intent.revision != document.current.revision
            || intent.source_hash != document.current.hash
        {
            return Err(JournalError::Invalid(
                "save intent does not match current document",
            ));
        }
        if intent.artifact_bytes == 0 || intent.artifact_bytes > crate::MAX_PDF_BYTES {
            return Err(JournalError::Limit("save artifact outside 1..128 MiB"));
        }
        let mut lengths: BTreeMap<_, _> = snapshots(document)
            .map(|snapshot| (snapshot.hash.as_str(), snapshot.length))
            .collect();
        if lengths
            .insert(&intent.artifact_hash, intent.artifact_bytes)
            .is_some_and(|length| length != intent.artifact_bytes)
        {
            return Err(JournalError::Invalid(
                "artifact hash has inconsistent lengths",
            ));
        }
        if lengths.values().sum::<usize>() > MAX_DOCUMENT_BYTES {
            return Err(JournalError::Limit(
                "document plus save artifact exceeds 256 MiB",
            ));
        }
    }
    Ok(())
}
fn retained(current: &Manifest, backup: Option<&Manifest>) -> Result<BTreeMap<String, usize>> {
    let mut blobs = BTreeMap::new();
    for manifest in std::iter::once(current).chain(backup) {
        for snapshot in manifest.documents.iter().flat_map(snapshots) {
            if blobs
                .insert(snapshot.hash.clone(), snapshot.length)
                .is_some_and(|length| length != snapshot.length)
            {
                return Err(JournalError::Invalid(
                    "hash has inconsistent retained lengths",
                ));
            }
        }
        for intent in &manifest.save_intents {
            if blobs
                .insert(intent.artifact_hash.clone(), intent.artifact_bytes)
                .is_some_and(|length| length != intent.artifact_bytes)
            {
                return Err(JournalError::Invalid(
                    "artifact hash has inconsistent retained lengths",
                ));
            }
        }
    }
    Ok(blobs)
}
fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(windows)]
    windows::configure_options(&mut options);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(
            (rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC)
                .bits() as i32,
        );
    }
    options
}
fn validate_handle(file: &File, limit: usize) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(JournalError::Invalid("journal file must be regular"));
    }
    if metadata.len() > limit as u64 {
        return Err(JournalError::Limit("journal file exceeds byte limit"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.nlink() != 1
        {
            return Err(JournalError::Invalid(
                "journal files must be privately owned (0600) with one link",
            ));
        }
    }
    #[cfg(windows)]
    windows::validate_private(file, false)?;
    Ok(())
}
fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = path.symlink_metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(JournalError::Invalid(
            "journal input must be a regular file",
        ));
    }
    let mut options = private_options();
    let file = options.read(true).open(path)?;
    #[cfg(windows)]
    windows::reject_reparse(&file)?;
    // Published user files need not have private journal permissions.
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit as u64 {
        return Err(JournalError::Limit("regular file exceeds byte limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(JournalError::Limit("file grew beyond byte limit"));
    }
    Ok(bytes)
}
fn read_private_manifest(handle: &File, directory: &Path, name: &str) -> Result<Option<Manifest>> {
    let file = match open_private_relative(handle, directory, name, false) {
        Ok(file) => file,
        Err(JournalError::Io(error)) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    validate_handle(&file, MAX_MANIFEST_BYTES)?;
    let bytes = read_bounded(file, MAX_MANIFEST_BYTES)?;
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn open_private_relative(
    handle: &File,
    directory: &Path,
    name: &str,
    create: bool,
) -> Result<File> {
    #[cfg(unix)]
    {
        let _ = directory;
        let mut flags = rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC;
        flags |= if create {
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL
        } else {
            rustix::fs::OFlags::RDONLY
        };
        Ok(File::from(
            rustix::fs::openat(handle, name, flags, rustix::fs::Mode::from_raw_mode(0o600))
                .map_err(io::Error::from)?,
        ))
    }
    #[cfg(windows)]
    {
        let _ = handle;
        let mut options = private_options();
        if create {
            options.write(true).create_new(true);
        } else {
            options.read(true);
        }
        Ok(options.open(directory.join(name))?)
    }
}

fn read_bounded(file: File, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(JournalError::Limit("journal file grew beyond byte limit"));
    }
    Ok(bytes)
}

fn remove_relative(handle: &File, directory: &Path, name: &str) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = directory;
        rustix::fs::unlinkat(handle, name, rustix::fs::AtFlags::empty())
            .map_err(io::Error::from)?;
    }
    #[cfg(windows)]
    {
        let _ = handle;
        let file = windows::open_deletable_file(&directory.join(name))?;
        windows::delete_handle(&file)?;
    }
    Ok(())
}

fn sync_handle(handle: &File) -> Result<()> {
    #[cfg(unix)]
    handle.sync_all()?;
    #[cfg(windows)]
    {
        let _ = handle;
    }
    Ok(())
}
fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>> {
    struct Writer(Vec<u8>);
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_MANIFEST_BYTES.saturating_sub(self.0.len()) {
                return Err(io::Error::other("manifest exceeds 8 MiB"));
            }
            let length = self.0.len() + bytes.len();
            if length > self.0.capacity() {
                let capacity = length
                    .max(self.0.capacity().saturating_mul(2))
                    .min(MAX_MANIFEST_BYTES);
                self.0
                    .try_reserve_exact(capacity - self.0.len())
                    .map_err(io::Error::other)?;
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer(Vec::new());
    serde_json::to_writer(&mut writer, manifest)?;
    Ok(writer.0)
}
fn atomic_write(handle: &File, directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let destination = directory.join(name);
    if destination
        .symlink_metadata()
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(JournalError::Invalid(
            "manifest destination must be regular",
        ));
    }
    let temporary_name = format!("{name}.tmp");
    let mut file = open_private_relative(handle, directory, &temporary_name, true)?;
    validate_handle(&file, MAX_MANIFEST_BYTES)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(unix)]
        rustix::fs::renameat(handle, &temporary_name, handle, name).map_err(io::Error::from)?;
        #[cfg(windows)]
        std::fs::rename(directory.join(&temporary_name), destination)?;
        sync_handle(handle)
    })();
    if result.is_err() {
        let _ = remove_relative(handle, directory, &temporary_name);
    }
    result
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
