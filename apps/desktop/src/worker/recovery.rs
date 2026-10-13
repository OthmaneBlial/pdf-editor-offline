use super::document::{self, Snapshot, WorkingDocument};
use super::{Event, MAX_TABS, SNAPSHOT_BYTES};
use crate::state::DocumentStatus;
use pdf_worker::PayloadKind;
use pdf_worker::document::{DocumentResult, Operation};
use pdf_worker::journal::{Journal, SaveIntentRef, SessionState, SessionStore};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct RecoverySession {
    pub id: String,
    pub documents: usize,
    pub bytes: usize,
    pub busy: bool,
    pub error: Option<String>,
}

pub struct RecoveredDocument {
    pub id: u64,
    pub path: PathBuf,
    pub status: DocumentStatus,
    pub pending_save: Option<PathBuf>,
}

pub(super) struct Recovery {
    store: SessionStore,
    root: PathBuf,
    journal: Option<Journal>,
    pub initial_error: Option<String>,
    #[cfg(test)]
    _temporary: Option<tempfile::TempDir>,
}

impl Recovery {
    pub fn new() -> Result<Self, String> {
        #[cfg(test)]
        {
            let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
            let mut recovery = Self::at(&temporary.path().join("sessions"))?;
            recovery._temporary = Some(temporary);
            Ok(recovery)
        }
        #[cfg(not(test))]
        {
            Self::at(&default_root()?)
        }
    }

    pub fn at(root: &Path) -> Result<Self, String> {
        let store = SessionStore::open(root).map_err(|error| error.to_string())?;
        let (journal, initial_error) = match store.create() {
            Ok(journal) => (Some(journal), None),
            Err(error) => (
                None,
                Some(format!(
                    "Recovery journal is unavailable. Document changes cannot be committed: {error}"
                )),
            ),
        };
        Ok(Self {
            store,
            root: std::path::absolute(root).map_err(|error| error.to_string())?,
            journal,
            initial_error,
            #[cfg(test)]
            _temporary: None,
        })
    }

    pub fn journal(&mut self) -> Result<&mut Journal, String> {
        if self.journal.is_none() {
            self.journal = Some(self.store.create().map_err(|error| {
                format!(
                    "Recovery journal is unavailable; no document changes were committed: {error}"
                )
            })?);
            self.initial_error = None;
        }
        self.journal
            .as_mut()
            .ok_or_else(|| "Recovery journal is unavailable".to_owned())
    }

    pub fn list(&self) -> Result<Event, String> {
        let mut sessions = Vec::new();
        for candidate in self.store.discover().map_err(|error| error.to_string())? {
            if self
                .journal
                .as_ref()
                .is_some_and(|journal| journal.directory() == candidate.path)
            {
                continue;
            }
            let id = candidate
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid recovery session name")?
                .to_owned();
            let expired = candidate.modified.is_some_and(|modified| {
                SystemTime::now()
                    .duration_since(modified)
                    .is_ok_and(|age| age > Duration::from_secs(7 * 24 * 60 * 60))
            });
            let (documents, busy, mut error) = match candidate.state {
                SessionState::Available(summary) => (
                    summary.document_count,
                    false,
                    if summary.save_intents > 0 {
                        Some(format!(
                            "{} save acknowledgement(s) remain unresolved. Recovery preserves their artifacts and destinations.",
                            summary.save_intents
                        ))
                    } else {
                        None
                    },
                ),
                SessionState::Active => (
                    0,
                    true,
                    Some("Another application instance holds this session lock.".into()),
                ),
                SessionState::Invalid(reason) => (0, false, Some(reason)),
            };
            if expired && !busy {
                error = Some(format!(
                    "Older than seven days; retained until explicit discard. {}",
                    error.unwrap_or_default()
                ));
            }
            if candidate.disk_bytes.is_none() {
                error = Some(format!(
                    "Disk usage could not be inspected. {}",
                    error.unwrap_or_default()
                ));
            }
            let bytes = usize::try_from(candidate.disk_bytes.unwrap_or(0))
                .map_err(|_| "Recovery disk usage exceeds the platform size range")?;
            sessions.push(RecoverySession {
                id,
                documents,
                bytes,
                busy,
                error,
            });
        }
        Ok(Event::RecoveryListed { sessions })
    }

    fn session_path(&self, session: &str) -> Result<PathBuf, String> {
        if session.len() != 40
            || !session.starts_with("session-")
            || !session.as_bytes()[8..]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err("Invalid recovery session identifier".into());
        }
        let path = self.root.join(session);
        if self
            .journal
            .as_ref()
            .is_some_and(|journal| journal.directory() == path)
        {
            return Err("The active session cannot be restored or discarded".into());
        }
        Ok(path)
    }

    pub fn discard(&self, session: &str) -> Result<Event, String> {
        let path = self.session_path(session)?;
        self.store
            .discard_session(path)
            .map_err(|error| error.to_string())?;
        self.list()
    }

    pub fn restore(
        &mut self,
        session: &str,
        documents: &mut HashMap<u64, WorkingDocument>,
    ) -> Result<Event, String> {
        let path = self.session_path(session)?;
        let source = self
            .store
            .open_session(path)
            .map_err(|error| error.to_string())?;
        let summary = source.summary();
        if summary.document_count == 0 {
            return Err("This session has no active document snapshots; its retained backups can be explicitly discarded".into());
        }
        if summary.document_count > MAX_TABS.saturating_sub(documents.len()) {
            return Err("Recovery would exceed sixteen open tabs. Close a tab first.".into());
        }
        let existing_bytes = documents
            .values()
            .map(WorkingDocument::byte_len)
            .sum::<usize>();
        if summary.bytes > SNAPSHOT_BYTES.saturating_sub(existing_bytes) {
            return Err(
                "Recovery would exceed the 512 MiB snapshot budget. Close a tab first.".into(),
            );
        }
        let started = Instant::now();
        let mut states = source.recover().map_err(|error| error.to_string())?;
        for state in &mut states {
            for snapshot in std::iter::once(&mut state.current)
                .chain(&mut state.undo)
                .chain(&mut state.redo)
            {
                snapshot.info = inspect_snapshot(&snapshot.bytes, started)?;
            }
        }
        let mut next_id = documents.keys().copied().max().unwrap_or(0);
        let mut incoming = Vec::with_capacity(states.len());
        let mut remap = HashMap::new();
        for state in states {
            next_id = next_id
                .checked_add(1)
                .ok_or("Document identifier counter exhausted")?;
            remap.insert(state.id, next_id.to_string());
            let snapshot = |entry: pdf_worker::journal::Snapshot| Snapshot {
                revision: entry.revision,
                bytes: entry.bytes,
                info: entry.info,
            };
            let working = WorkingDocument {
                current: snapshot(state.current),
                undo: state.undo.into_iter().map(snapshot).collect(),
                redo: state.redo.into_iter().map(snapshot).collect(),
                saved_revision: state.saved_revision,
                next_revision: state.next_revision,
                path: state.path,
                password: None,
            };
            incoming.push((next_id, working));
        }
        let mut intents = source.save_intents().to_vec();
        let mut artifacts = BTreeMap::new();
        for intent in &mut intents {
            if !artifacts.contains_key(&intent.artifact_hash) {
                let artifact = source
                    .save_artifact(&intent.token)
                    .map_err(|error| error.to_string())?;
                let info = inspect_snapshot(&artifact, started)?;
                if info.encrypted || info.signature_fields > 0 {
                    return Err("A recovery save artifact is not a writable unsigned PDF; the source session remains retained".into());
                }
                artifacts.insert(intent.artifact_hash.clone(), artifact);
            }
            intent.document_id = remap
                .get(&intent.document_id)
                .ok_or("Recovery intent refers to an unavailable document")?
                .clone();
            intent.token = format!("recovered-{}-{}", intent.document_id, intent.artifact_hash);
        }
        let ids: Vec<_> = incoming.iter().map(|(id, _)| id.to_string()).collect();
        let references: Vec<_> = incoming
            .iter()
            .zip(&ids)
            .map(|((_, document), id)| document.as_ref(id))
            .collect();
        let intent_references: Vec<_> = intents
            .iter()
            .map(|intent| SaveIntentRef {
                intent,
                artifact: artifacts[&intent.artifact_hash].as_slice(),
            })
            .collect();
        let mut restored = Vec::with_capacity(incoming.len());
        for (id, working) in &incoming {
            restored.push(RecoveredDocument {
                id: *id,
                path: working
                    .path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("Images — recovered.pdf")),
                status: working.status()?,
                pending_save: intents
                    .iter()
                    .find(|intent| intent.document_id == id.to_string())
                    .map(|intent| intent.destination.clone()),
            });
        }
        self.journal()?.commit_documents_with_intents_ref(&references, &intent_references).map_err(|error| format!("Recovery journal commit failed; existing tabs and source session were retained: {error}"))?;
        for (id, working) in incoming {
            documents.insert(id, working);
        }
        // The source remains recoverable if retirement is refused; destination data is already durable.
        if let Err(error) = source.discard() {
            self.initial_error = Some(format!(
                "Documents were recovered, but the previous session remains retained: {error}"
            ));
        }
        Ok(Event::RecoveryRestored {
            documents: restored,
        })
    }

    pub fn finish(mut self, documents: &HashMap<u64, WorkingDocument>) {
        if documents.values().all(WorkingDocument::session_only)
            && self
                .journal
                .as_ref()
                .is_some_and(|journal| journal.summary().document_count == 0)
            && self
                .journal
                .as_ref()
                .is_some_and(|journal| journal.save_intents().is_empty())
            && let Some(journal) = self.journal.take()
        {
            let _ = journal.discard();
        }
    }
}

fn inspect_snapshot(bytes: &[u8], started: Instant) -> Result<pdf_core::DocumentInfo, String> {
    let remaining = Duration::from_secs(180).checked_sub(started.elapsed()).filter(|duration| !duration.is_zero())
        .ok_or("Recovery validation exceeded its deadline; existing tabs and source session remain retained")?;
    let limits = pdf_worker::Limits::for_test(
        pdf_worker::MEMORY_BYTES,
        pdf_worker::CPU_SECONDS,
        remaining.min(Duration::from_secs(pdf_worker::CPU_SECONDS)),
    )
    .map_err(|error| error.to_string())?;
    let (result, reply) = document::execute_limited(Operation::Open, bytes, limits)?;
    if matches!(result, DocumentResult::PasswordRequired { .. }) {
        return Err("Encrypted recovery snapshots require explicit reopening of the original with its password; the source session remains retained".into());
    }
    let DocumentResult::Info { info } = result else {
        return Err("Recovery child returned an unexpected response".into());
    };
    if reply.kind != PayloadKind::None || !reply.payload.is_empty() {
        return Err("Recovery child returned an unexpected payload".into());
    }
    document::validate_info(&info)?;
    Ok(info)
}

#[test]
#[allow(clippy::unwrap_used)]
fn legacy_encrypted_snapshots_including_empty_passwords_never_restore_without_awareness() {
    for password in ["", "reader"] {
        let (cipher, _) = super::password_tests::encryption::fixture(3, password, false);
        let error = inspect_snapshot(&cipher, Instant::now()).unwrap_err();
        assert!(error.contains("explicit reopening"));
    }
}

#[cfg(not(test))]
fn default_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    return Ok(
        PathBuf::from(std::env::var_os("HOME").ok_or("Home directory unavailable")?)
            .join("Library/Application Support/PDF Editor Offline/sessions"),
    );
    #[cfg(windows)]
    return Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or("Local application data directory unavailable")?,
    )
    .join("PDF Editor Offline/sessions"));
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or(
                PathBuf::from(std::env::var_os("HOME").ok_or("Home directory unavailable")?)
                    .join(".local/state"),
            );
        Ok(base.join("pdf-editor-offline/sessions"))
    }
}
