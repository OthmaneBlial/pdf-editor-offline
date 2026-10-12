use crate::state::{DocumentStatus, RenderKey};
use eframe::egui;
use pdf_core::PdfDocument;
use pdf_render::RenderedPage;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};

const HISTORY_LIMIT: usize = 8;
const HISTORY_BYTES: usize = 256 * 1024 * 1024;

pub enum Edit {
    Rotate(u32, i64),
    Undo,
    Redo,
}

pub enum Command {
    Load { id: u64, path: PathBuf },
    Render(RenderKey),
    Edit { id: u64, edit: Edit },
    Save { id: u64, path: PathBuf },
    Close(u64),
}

impl Command {
    fn id(&self) -> u64 {
        match self {
            Self::Load { id, .. }
            | Self::Edit { id, .. }
            | Self::Save { id, .. }
            | Self::Close(id) => *id,
            Self::Render(key) => key.tab,
        }
    }
}

pub enum Event {
    Loaded {
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
    },
    Updated {
        id: u64,
        status: DocumentStatus,
    },
    Saved {
        id: u64,
        path: PathBuf,
        status: DocumentStatus,
    },
    Rendered {
        key: RenderKey,
        image: RenderedPage,
    },
    Closed(u64),
    Error {
        id: u64,
        render_key: Option<RenderKey>,
        message: String,
    },
}

pub struct Worker {
    sender: SyncSender<Command>,
    pub events: Receiver<Event>,
}

impl Worker {
    pub fn new(context: egui::Context) -> Self {
        let (sender, commands) = mpsc::sync_channel::<Command>(1);
        let (responses, events) = mpsc::channel();
        std::thread::spawn(move || {
            let mut documents = HashMap::new();
            while let Ok(command) = commands.recv() {
                let id = command.id();
                let render_key = match &command {
                    Command::Render(key) => Some(*key),
                    _ => None,
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    process(&mut documents, command)
                }));
                let event = match result {
                    Ok(Ok(event)) => event,
                    Ok(Err(message)) => Event::Error {
                        id,
                        render_key,
                        message,
                    },
                    Err(_) => {
                        Event::Error {
                            id,
                            render_key: None,
                            message: "Document operation failed. The open document has been retained; save a new copy before retrying.".into(),
                        }
                    }
                };
                if responses.send(event).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self { sender, events }
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.sender.try_send(command).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => "Document worker is busy".into(),
            mpsc::TrySendError::Disconnected(_) => {
                "Document worker stopped; restart the application".into()
            }
        })
    }
}

struct Snapshot {
    document: PdfDocument,
    bytes: Vec<u8>,
    revision: u64,
}

struct WorkingDocument {
    current: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    saved_revision: u64,
    next_revision: u64,
}

impl WorkingDocument {
    fn load(path: &PathBuf) -> Result<Self, String> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(pdf_core::MAX_INPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let document = PdfDocument::from_bytes(&bytes, None).map_err(|e| e.to_string())?;
        Ok(Self {
            current: Snapshot {
                document,
                bytes,
                revision: 0,
            },
            undo: vec![],
            redo: vec![],
            saved_revision: 0,
            next_revision: 1,
        })
    }

    fn status(&self) -> Result<DocumentStatus, String> {
        Ok(DocumentStatus {
            info: self.current.document.info().map_err(|e| e.to_string())?,
            revision: self.current.revision,
            dirty: self.current.revision != self.saved_revision,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        })
    }

    fn edit(&mut self, edit: Edit) -> Result<(), String> {
        match edit {
            Edit::Rotate(page, degrees) => {
                let mut document = self.current.document.clone();
                document
                    .rotate(&[page], degrees)
                    .map_err(|e| e.to_string())?;
                let bytes = document.to_bytes().map_err(|e| e.to_string())?;
                let snapshot = Snapshot {
                    document,
                    bytes,
                    revision: self.next_revision,
                };
                self.next_revision += 1;
                self.undo
                    .push(std::mem::replace(&mut self.current, snapshot));
                self.redo.clear();
                self.trim_history();
            }
            Edit::Undo => {
                let previous = self.undo.pop().ok_or("Nothing to undo")?;
                self.redo
                    .push(std::mem::replace(&mut self.current, previous));
            }
            Edit::Redo => {
                let next = self.redo.pop().ok_or("Nothing to redo")?;
                self.undo.push(std::mem::replace(&mut self.current, next));
            }
        }
        Ok(())
    }

    fn trim_history(&mut self) {
        // shortcut: serialized-byte budget estimates clone size; isolate the worker for strict process-memory limits.
        while self.undo.len() > HISTORY_LIMIT
            || self.undo.iter().map(|item| item.bytes.len()).sum::<usize>() > HISTORY_BYTES
        {
            self.undo.remove(0);
        }
    }

    fn save(&mut self, path: &PathBuf) -> Result<(), String> {
        self.current
            .document
            .save_new(path)
            .map_err(|e| e.to_string())?;
        self.saved_revision = self.current.revision;
        Ok(())
    }
}

fn process(
    documents: &mut HashMap<u64, WorkingDocument>,
    command: Command,
) -> Result<Event, String> {
    if let Command::Load { id, path } = command {
        let document = WorkingDocument::load(&path)?;
        let status = document.status()?;
        documents.insert(id, document);
        return Ok(Event::Loaded { id, path, status });
    }
    if let Command::Close(id) = command {
        documents.remove(&id);
        return Ok(Event::Closed(id));
    }
    let id = command.id();
    let document = documents
        .get_mut(&id)
        .ok_or("Document is unavailable; close and reopen it")?;
    match command {
        Command::Render(key) => {
            if key.revision != document.current.revision {
                return Err("Preview request is stale".into());
            }
            let image = pdf_render::render_page(
                &document.current.bytes,
                key.page,
                f32::from_bits(key.scale_bits),
            )
            .map_err(|e| e.to_string())?;
            Ok(Event::Rendered { key, image })
        }
        Command::Edit { edit, .. } => {
            document.edit(edit)?;
            Ok(Event::Updated {
                id,
                status: document.status()?,
            })
        }
        Command::Save { path, .. } => {
            document.save(&path)?;
            Ok(Event::Saved {
                id,
                path,
                status: document.status()?,
            })
        }
        Command::Load { .. } | Command::Close(_) => {
            unreachable!("load and close handled before lookup")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object, Stream, dictionary};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("source.pdf");
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let content = pdf.add_object(Stream::new(
            dictionary! {},
            b"1 0 0 rg 0 0 100 80 re f".to_vec(),
        ));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 80.into()], "Resources" => dictionary!{}, "Contents" => content });
        pdf.objects.insert(
            pages,
            Object::Dictionary(
                dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
            ),
        );
        let catalog = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        pdf.trailer.set("Root", catalog);
        pdf.save(&path).expect("save fixture");
        (directory, path)
    }

    #[test]
    fn rotation_history_save_and_original_are_preserved() {
        let (directory, path) = fixture();
        let original = std::fs::read(&path).expect("original");
        let mut working = WorkingDocument::load(&path).expect("load fixture");
        working.edit(Edit::Rotate(1, 90)).expect("rotate");
        assert!(working.status().expect("status").dirty);
        assert_eq!(working.status().expect("status").info.pages[0].rotation, 90);
        working.edit(Edit::Undo).expect("undo");
        assert!(!working.status().expect("status").dirty);
        assert_eq!(working.status().expect("status").info.pages[0].rotation, 0);
        working.edit(Edit::Redo).expect("redo");
        let output = directory.path().join("rotated.pdf");
        working.save(&output).expect("save new output");
        assert!(!working.status().expect("status").dirty);
        assert!(working.save(&output).is_err());
        working.edit(Edit::Undo).expect("undo after saving");
        assert!(working.status().expect("status").dirty);
        assert_eq!(std::fs::read(&path).expect("read original"), original);
        assert_eq!(
            PdfDocument::open(&output, None)
                .expect("reopen output")
                .info()
                .expect("info")
                .pages[0]
                .rotation,
            90
        );
    }

    #[test]
    fn history_is_bounded_and_failed_edits_leave_document_unchanged() {
        let (_directory, path) = fixture();
        let mut working = WorkingDocument::load(&path).expect("load fixture");
        assert!(working.edit(Edit::Rotate(2, 90)).is_err());
        assert_eq!(working.current.revision, 0);
        assert!(working.undo.is_empty());
        for _ in 0..20 {
            working.edit(Edit::Rotate(1, 90)).expect("rotate");
        }
        assert_eq!(working.undo.len(), HISTORY_LIMIT);
        working.edit(Edit::Undo).expect("undo");
        working.edit(Edit::Rotate(1, 90)).expect("new edit");
        assert!(working.redo.is_empty());
    }

    #[test]
    fn actor_rejects_stale_revision_and_closed_document() {
        let (_directory, path) = fixture();
        let mut documents = HashMap::new();
        assert!(process(&mut documents, Command::Load { id: 1, path }).is_ok());
        assert!(
            process(
                &mut documents,
                Command::Render(RenderKey {
                    tab: 1,
                    revision: 99,
                    page: 0,
                    scale_bits: 1.0_f32.to_bits()
                })
            )
            .is_err()
        );
        assert!(process(&mut documents, Command::Close(1)).is_ok());
        assert!(
            process(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Undo
                }
            )
            .is_err()
        );
    }
}
