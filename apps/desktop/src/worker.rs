use crate::search::{
    MAX_QUERY_CHARS, PageText, SearchHit, SearchKey, limit_page_text, literal_excerpt,
};
use crate::state::{DocumentStatus, MetadataReadKey, RenderKey, TextKey};
use crate::thumbnails::ThumbnailKey;
use eframe::egui;
use pdf_core::PdfDocument;
use pdf_render::RenderedPage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};

const HISTORY_LIMIT: usize = 8;
const HISTORY_BYTES: usize = 256 * 1024 * 1024;

pub enum Edit {
    Rotate(u32, i64),
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
        key: MetadataReadKey,
        changes: Vec<(pdf_core::MetadataKey, Option<String>)>,
    },
    Undo,
    Redo,
}

pub enum Command {
    Load {
        id: u64,
        path: PathBuf,
    },
    Render(RenderKey),
    Thumbnail(ThumbnailKey),
    Text(TextKey),
    Metadata(MetadataReadKey),
    Search {
        key: SearchKey,
        page: usize,
        query: String,
        match_case: bool,
    },
    Edit {
        id: u64,
        edit: Edit,
    },
    Save {
        id: u64,
        path: PathBuf,
    },
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
            Self::Thumbnail(key) => key.tab,
            Self::Text(key) => key.tab,
            Self::Metadata(key) => key.tab,
            Self::Search { key, .. } => key.tab,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskKey {
    Render(RenderKey),
    Thumbnail(ThumbnailKey),
    Text(TextKey),
    Metadata(MetadataReadKey),
    Search(SearchKey),
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
        selected_page: Option<usize>,
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
    Thumbnail {
        key: ThumbnailKey,
        image: RenderedPage,
    },
    Text(PageText),
    Metadata {
        key: MetadataReadKey,
        values: pdf_core::DocumentMetadata,
    },
    Searched {
        key: SearchKey,
        page: usize,
        hit: Option<SearchHit>,
        text_truncated: bool,
    },
    Closed(u64),
    Error {
        id: u64,
        task: Option<TaskKey>,
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
                let task = match &command {
                    Command::Render(key) => Some(TaskKey::Render(*key)),
                    Command::Thumbnail(key) => Some(TaskKey::Thumbnail(*key)),
                    Command::Text(key) => Some(TaskKey::Text(*key)),
                    Command::Metadata(key) => Some(TaskKey::Metadata(*key)),
                    Command::Edit {
                        edit: Edit::Metadata { key, .. },
                        ..
                    } => Some(TaskKey::Metadata(*key)),
                    Command::Search { key, .. } => Some(TaskKey::Search(*key)),
                    _ => None,
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    process(&mut documents, command)
                }));
                let event = match result {
                    Ok(Ok(event)) => event,
                    Ok(Err(message)) => Event::Error {
                        id,
                        task,
                        message,
                    },
                    Err(_) => {
                        Event::Error {
                            id,
                            task,
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
        let bytes = pdf_core::read_pdf_bytes(path).map_err(|e| e.to_string())?;
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

    fn edit(&mut self, edit: Edit) -> Result<Option<usize>, String> {
        let mut selected_page = None;
        match edit {
            Edit::Undo => {
                let previous = self.undo.pop().ok_or("Nothing to undo")?;
                self.redo
                    .push(std::mem::replace(&mut self.current, previous));
            }
            Edit::Redo => {
                let next = self.redo.pop().ok_or("Nothing to redo")?;
                self.undo.push(std::mem::replace(&mut self.current, next));
            }
            edit => {
                let mut document = self.current.document.clone();
                match edit {
                    Edit::Rotate(page, degrees) => document
                        .rotate(&[page], degrees)
                        .map_err(|e| e.to_string())?,
                    Edit::Move { page, earlier } => {
                        let count = document.page_count();
                        let index = (page as usize)
                            .checked_sub(1)
                            .filter(|index| *index < count)
                            .ok_or("Move page is outside the document")?;
                        let target = if earlier {
                            index.checked_sub(1)
                        } else {
                            index.checked_add(1).filter(|next| *next < count)
                        }
                        .ok_or("Page cannot move farther in that direction")?;
                        let mut order: Vec<_> = (1..=count as u32).collect();
                        order.swap(index, target);
                        document.reorder(&order).map_err(|e| e.to_string())?;
                        selected_page = Some(target);
                    }
                    Edit::Reverse { page } => {
                        let index = (page as usize)
                            .checked_sub(1)
                            .filter(|index| *index < document.page_count())
                            .ok_or("Current page is outside the document")?;
                        selected_page = Some(document.page_count() - 1 - index);
                        document.reverse().map_err(|e| e.to_string())?;
                    }
                    Edit::InsertBlank {
                        position,
                        width,
                        height,
                    } => {
                        document
                            .insert_blank(position, width, height)
                            .map_err(|e| e.to_string())?;
                        selected_page = Some(position as usize - 1);
                    }
                    Edit::Crop { page, rectangle } => {
                        document
                            .crop(&[page], rectangle)
                            .map_err(|e| e.to_string())?;
                    }
                    Edit::Metadata { key, changes } => {
                        if key.revision != self.current.revision {
                            return Err(
                                "Metadata draft is stale. Read the document properties again."
                                    .into(),
                            );
                        }
                        let changes: Vec<_> = changes
                            .iter()
                            .map(|(key, value)| (*key, value.as_deref()))
                            .collect();
                        document
                            .update_metadata(&changes)
                            .map_err(|e| e.to_string())?;
                    }
                    Edit::Undo | Edit::Redo => unreachable!("history handled above"),
                }
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
        }
        Ok(selected_page)
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
            let image = render(document, key, f32::from_bits(key.zoom_bits))?;
            Ok(Event::Rendered { key, image })
        }
        Command::Thumbnail(key) => {
            if key
                .pixel_size
                .iter()
                .any(|side| *side == 0 || *side > crate::thumbnails::MAX_SIDE)
            {
                return Err("Thumbnail exceeds its pixel limit.".into());
            }
            // Recompute from inspected geometry before rasterization; no oversized scale can slip through a forged key.
            let info = document
                .current
                .document
                .info()
                .map_err(|e| e.to_string())?;
            let page = info
                .pages
                .get(key.page)
                .ok_or("Thumbnail page is unavailable")?;
            let expected = ThumbnailKey::new(
                key.tab,
                key.revision,
                key.page,
                page,
                f32::from_bits(key.device_ratio_bits),
            )?;
            if key != expected {
                return Err("Thumbnail geometry is stale".into());
            }
            let image = render(document, key.render_key(), 1.0)?;
            Ok(Event::Thumbnail { key, image })
        }
        Command::Text(key) => {
            if key.revision != document.current.revision {
                return Err("Text request is stale".into());
            }
            let (text, truncated) = page_text(document, key.page)?;
            Ok(Event::Text(PageText {
                key,
                text,
                truncated,
            }))
        }
        Command::Metadata(key) => {
            if key.revision != document.current.revision {
                return Err("Metadata request is stale".into());
            }
            Ok(Event::Metadata {
                key,
                values: document
                    .current
                    .document
                    .metadata()
                    .map_err(|e| e.to_string())?,
            })
        }
        Command::Search {
            key,
            page,
            query,
            match_case,
        } => {
            if key.revision != document.current.revision {
                return Err("Search request is stale".into());
            }
            if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
                return Err("Search accepts 1–256 literal characters".into());
            }
            let (text, text_truncated) = page_text(document, page)?;
            let hit = literal_excerpt(&text, &query, match_case)
                .map(|excerpt| SearchHit { page, excerpt });
            Ok(Event::Searched {
                key,
                page,
                hit,
                text_truncated,
            })
        }
        Command::Edit { edit, .. } => {
            if let Edit::Metadata { key, .. } = &edit
                && key.tab != id
            {
                return Err("Metadata draft belongs to another document".into());
            }
            let selected_page = document.edit(edit)?;
            Ok(Event::Updated {
                id,
                status: document.status()?,
                selected_page,
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

fn render(
    document: &WorkingDocument,
    key: RenderKey,
    view_zoom: f32,
) -> Result<RenderedPage, String> {
    if key.revision != document.current.revision {
        return Err("Preview request is stale".into());
    }
    let image = pdf_render::render_view_page(
        &document.current.bytes,
        key.page,
        f32::from_bits(key.scale_bits),
        view_zoom,
    )
    .map_err(|e| e.to_string())?;
    if [image.width, image.height] != key.pixel_size {
        return Err(
            "This PDF page's preview dimensions differ from its inspected visible area.".into(),
        );
    }
    Ok(image)
}

fn page_text(document: &WorkingDocument, page: usize) -> Result<(String, bool), String> {
    if page >= document.current.document.page_count() {
        return Err("Page is outside document".into());
    }
    let text = document
        .current
        .document
        .extract_text(&[page as u32 + 1])
        .map_err(|error| error.to_string())?;
    Ok(limit_page_text(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object, Stream, dictionary};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        fixture_pages(1)
    }

    fn fixture_pages(count: u32) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("source.pdf");
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let font = pdf.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        let mut kids = vec![];
        for number in 1..=count {
            let content = pdf.add_object(Stream::new(dictionary! {},format!("1 0 0 rg 0 0 100 80 re f BT /F1 12 Tf 10 20 Td (Hello PDF workstation Page{number}) Tj ET").into_bytes()));
            let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 80.into()], "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } }, "Contents" => content });
            kids.push(page.into());
        }
        pdf.objects.insert(
            pages,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        pdf.trailer.set("Root", catalog);
        pdf.save(&path).expect("save fixture");
        (directory, path)
    }

    #[test]
    fn metadata_worker_preserves_unicode_aliases_dates_xmp_and_history() {
        let (directory, path) = fixture();
        let mut pdf = Document::load(&path).expect("load fixture");
        let info=pdf.add_object(dictionary! { "Title"=>Object::string_literal("Original"),"Author"=>Object::string_literal("Ada"),"CreationDate"=>Object::string_literal("D:20261012090000Z"),"Custom"=>Object::string_literal("keep-custom") });
        let xmp = pdf.add_object(Stream::new(
            dictionary! { "Type"=>"Metadata","Subtype"=>"XML" },
            b"<x:xmpmeta>original packet</x:xmpmeta>".to_vec(),
        ));
        pdf.trailer.set("Info", info);
        let catalog = pdf
            .trailer
            .get(b"Root")
            .expect("root")
            .as_reference()
            .expect("root ref");
        pdf.get_dictionary_mut(catalog)
            .expect("catalog")
            .set("Metadata", xmp);
        pdf.get_dictionary_mut(catalog)
            .expect("catalog")
            .set("Info", info);
        pdf.save(&path).expect("metadata fixture");
        let original = std::fs::read(&path).expect("original");
        let mut documents = HashMap::new();
        process(
            &mut documents,
            Command::Load {
                id: 1,
                path: path.clone(),
            },
        )
        .expect("load");
        let key = MetadataReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        };
        let Event::Metadata {
            key: returned,
            values,
        } = process(&mut documents, Command::Metadata(key)).expect("read")
        else {
            panic!("metadata event");
        };
        assert_eq!(returned, key);
        assert_eq!(values.fields[&pdf_core::MetadataKey::Title], "Original");
        assert!(
            values.xmp_present
                && values.catalog_info_present
                && !values.xmp_synchronized
                && !values.secure_erasure
        );
        process(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::Metadata {
                    key,
                    changes: vec![
                        (pdf_core::MetadataKey::Title, Some("Résumé 東京 ✓".into())),
                        (pdf_core::MetadataKey::Author, None),
                    ],
                },
            },
        )
        .expect("edit");
        let working = documents.get_mut(&1).expect("working");
        assert_eq!(working.undo.len(), 1);
        let edited = working.current.document.metadata().expect("metadata");
        assert_eq!(
            edited.fields[&pdf_core::MetadataKey::Title],
            "Résumé 東京 ✓"
        );
        assert!(!edited.fields.contains_key(&pdf_core::MetadataKey::Author));
        working.edit(Edit::Undo).expect("undo");
        assert_eq!(working.current.bytes, original);
        working.edit(Edit::Redo).expect("redo");
        let output = directory.path().join("metadata.pdf");
        working.save(&output).expect("save");
        assert_eq!(
            PdfDocument::open(&output, None)
                .expect("reopen")
                .metadata()
                .expect("metadata")
                .fields[&pdf_core::MetadataKey::Title],
            "Résumé 東京 ✓"
        );
        let saved = Document::load(&output).expect("saved raw");
        let active = saved
            .trailer
            .get(b"Info")
            .expect("info")
            .as_reference()
            .expect("info ref");
        assert_ne!(active, info);
        let new_info = saved.get_dictionary(active).expect("new info");
        assert_eq!(
            new_info
                .get(b"CreationDate")
                .expect("date")
                .as_str()
                .expect("date bytes"),
            b"D:20261012090000Z"
        );
        assert_eq!(
            new_info
                .get(b"Custom")
                .expect("custom")
                .as_str()
                .expect("custom bytes"),
            b"keep-custom"
        );
        assert_eq!(
            saved
                .catalog()
                .expect("catalog")
                .get(b"Info")
                .expect("catalog info")
                .as_reference()
                .expect("alias"),
            info
        );
        assert_eq!(
            saved
                .get_dictionary(info)
                .expect("old alias")
                .get(b"Title")
                .expect("old title")
                .as_str()
                .expect("old title bytes"),
            b"Original"
        );
        assert_eq!(
            saved
                .get_object(xmp)
                .expect("xmp")
                .as_stream()
                .expect("stream")
                .content,
            b"<x:xmpmeta>original packet</x:xmpmeta>"
        );
        assert_eq!(std::fs::read(path).expect("original remains"), original);
    }

    #[test]
    fn metadata_worker_rejects_stale_invalid_and_read_only_changes_atomically() {
        let (_directory, path) = fixture();
        let mut documents = HashMap::new();
        process(
            &mut documents,
            Command::Load {
                id: 1,
                path: path.clone(),
            },
        )
        .expect("load");
        let key = MetadataReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        };
        let original = documents[&1].current.bytes.clone();
        for changes in [
            vec![],
            vec![(pdf_core::MetadataKey::Title, Some("NUL\0".into()))],
            vec![(
                pdf_core::MetadataKey::Title,
                Some("é".repeat(pdf_core::MAX_METADATA_VALUE_BYTES)),
            )],
            vec![
                (pdf_core::MetadataKey::Title, None),
                (pdf_core::MetadataKey::Title, None),
            ],
        ] {
            assert!(
                process(
                    &mut documents,
                    Command::Edit {
                        id: 1,
                        edit: Edit::Metadata { key, changes }
                    }
                )
                .is_err()
            );
            assert_eq!(documents[&1].current.bytes, original);
            assert!(documents[&1].undo.is_empty());
        }
        let stale = MetadataReadKey { revision: 9, ..key };
        assert!(process(&mut documents, Command::Metadata(stale)).is_err());
        assert!(
            process(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Metadata {
                        key: stale,
                        changes: vec![(pdf_core::MetadataKey::Title, Some("Stale".into()))]
                    }
                }
            )
            .is_err()
        );
        let wrong_tab = MetadataReadKey { tab: 2, ..key };
        assert!(
            process(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Metadata {
                        key: wrong_tab,
                        changes: vec![(pdf_core::MetadataKey::Title, Some("Other tab".into()))]
                    }
                }
            )
            .is_err()
        );
        let mut signed = Document::load(&path).expect("load raw");
        signed.add_object(dictionary! { "FT"=>"Sig" });
        signed.save(&path).expect("signed fixture");
        process(&mut documents, Command::Load { id: 1, path }).expect("signed load");
        assert!(process(&mut documents, Command::Metadata(key)).is_ok());
        let original = documents[&1].current.bytes.clone();
        assert!(
            process(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: Edit::Metadata {
                        key,
                        changes: vec![(pdf_core::MetadataKey::Title, Some("Blocked".into()))]
                    }
                }
            )
            .is_err()
        );
        assert_eq!(documents[&1].current.bytes, original);
        assert!(documents[&1].undo.is_empty());
    }

    #[test]
    fn organizer_edits_are_transactional_preserve_content_and_roundtrip_history() {
        let (directory, path) = fixture_pages(3);
        let original = std::fs::read(&path).expect("original");
        let mut working = WorkingDocument::load(&path).expect("load");
        assert_eq!(
            working
                .edit(Edit::Move {
                    page: 2,
                    earlier: true
                })
                .expect("move"),
            Some(0)
        );
        let text = |working: &WorkingDocument, page: u32| {
            working
                .current
                .document
                .extract_text(&[page])
                .expect("text")
        };
        assert!(text(&working, 1).contains("Page2"));
        assert!(text(&working, 2).contains("Page1"));
        working.edit(Edit::Undo).expect("undo");
        assert!(text(&working, 1).contains("Page1"));
        working.edit(Edit::Redo).expect("redo");
        assert!(text(&working, 1).contains("Page2"));
        assert_eq!(
            working.edit(Edit::Reverse { page: 1 }).expect("reverse"),
            Some(2)
        );
        assert!(text(&working, 1).contains("Page3"));
        assert!(text(&working, 3).contains("Page2"));
        assert_eq!(
            working
                .edit(Edit::InsertBlank {
                    position: 2,
                    width: 200.0,
                    height: 300.0
                })
                .expect("blank"),
            Some(1)
        );
        assert_eq!(working.current.document.page_count(), 4);
        assert!(text(&working, 2).trim().is_empty());
        assert!(text(&working, 3).contains("Page1"));
        working
            .edit(Edit::Crop {
                page: 1,
                rectangle: [10.0, 10.0, 90.0, 70.0],
            })
            .expect("crop");
        assert_eq!(
            working.status().expect("status").info.pages[0].crop_box,
            [10.0, 10.0, 90.0, 70.0]
        );
        assert!(text(&working, 1).contains("Page3"));
        let output = directory.path().join("organized.pdf");
        working.save(&output).expect("save");
        let reopened = PdfDocument::open(&output, None).expect("reopen");
        assert_eq!(
            reopened.info().expect("info").pages[0].crop_box,
            [10.0, 10.0, 90.0, 70.0]
        );
        assert!(reopened.extract_text(&[4]).expect("text").contains("Page2"));
        assert_eq!(std::fs::read(path).expect("original retained"), original);
        working.edit(Edit::Undo).expect("undo crop");
        assert_eq!(
            working.status().expect("status").info.pages[0].crop_box,
            [0.0, 0.0, 100.0, 80.0]
        );
        working.edit(Edit::Undo).expect("undo blank");
        assert_eq!(working.current.document.page_count(), 3);
        assert!(text(&working, 2).contains("Page1"));
        working.edit(Edit::Undo).expect("undo reverse");
        assert!(text(&working, 1).contains("Page2"));
        working.edit(Edit::Undo).expect("undo move");
        assert_eq!(working.current.bytes, original);
        assert!(working.status().expect("status").dirty); // The saved copy contains the organized revision.
    }

    #[test]
    fn signature_markers_block_every_organizer_mutation_in_the_worker() {
        let (_directory, path) = fixture_pages(3);
        let mut pdf = Document::load(&path).expect("load fixture");
        pdf.add_object(dictionary! { "FT"=>"Sig","Type"=>"Annot","Subtype"=>"Widget" });
        pdf.save(&path).expect("signed fixture");
        let mut working = WorkingDocument::load(&path).expect("load signed input");
        assert!(working.status().expect("status").info.signature_fields > 0);
        let original = working.current.bytes.clone();
        for edit in [
            Edit::Move {
                page: 2,
                earlier: true,
            },
            Edit::Reverse { page: 1 },
            Edit::InsertBlank {
                position: 2,
                width: 100.0,
                height: 80.0,
            },
            Edit::Crop {
                page: 1,
                rectangle: [10.0, 10.0, 90.0, 70.0],
            },
        ] {
            assert!(working.edit(edit).is_err());
            assert_eq!(working.current.bytes, original);
            assert!(working.undo.is_empty());
        }
    }

    #[test]
    fn invalid_organizer_edits_leave_bytes_revision_and_history_unchanged() {
        let (_directory, path) = fixture_pages(3);
        let mut working = WorkingDocument::load(&path).expect("load");
        let original = working.current.bytes.clone();
        for edit in [
            Edit::Move {
                page: 1,
                earlier: true,
            },
            Edit::Move {
                page: 3,
                earlier: false,
            },
            Edit::Move {
                page: 0,
                earlier: false,
            },
            Edit::Reverse { page: 4 },
            Edit::InsertBlank {
                position: 5,
                width: 100.0,
                height: 80.0,
            },
            Edit::InsertBlank {
                position: 1,
                width: f32::NAN,
                height: 80.0,
            },
            Edit::Crop {
                page: 1,
                rectangle: [0.0, 0.0, 101.0, 80.0],
            },
        ] {
            assert!(working.edit(edit).is_err());
            assert_eq!(working.current.bytes, original);
            assert_eq!(working.current.revision, 0);
            assert!(working.undo.is_empty() && working.redo.is_empty());
        }
    }

    #[test]
    fn thumbnail_actor_renders_real_pixels_and_rejects_forged_or_stale_keys() {
        let (_directory, path) = fixture();
        let mut documents = HashMap::new();
        process(&mut documents, Command::Load { id: 1, path }).expect("load");
        let info = documents[&1].status().expect("status").info;
        let key = ThumbnailKey::new(1, 0, 0, &info.pages[0], 2.0).expect("thumb key");
        let Event::Thumbnail {
            key: returned,
            image,
        } = process(&mut documents, Command::Thumbnail(key)).expect("thumbnail")
        else {
            panic!("thumbnail event");
        };
        assert_eq!(returned, key);
        assert_eq!([image.width, image.height], key.pixel_size);
        assert_eq!(
            image.rgba.len(),
            image.width as usize * image.height as usize * 4
        );
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .any(|pixel| pixel == [255, 0, 0, 255])
        );
        let forged = ThumbnailKey {
            scale_bits: 1000.0_f32.to_bits(),
            ..key
        };
        assert!(process(&mut documents, Command::Thumbnail(forged)).is_err());
        process(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::Rotate(1, 90),
            },
        )
        .expect("edit");
        assert!(process(&mut documents, Command::Thumbnail(key)).is_err());
        process(&mut documents, Command::Close(1)).expect("close");
        assert!(process(&mut documents, Command::Thumbnail(key)).is_err());
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
                    scale_bits: 1.0_f32.to_bits(),
                    zoom_bits: 1.0_f32.to_bits(),
                    device_ratio_bits: 1.0_f32.to_bits(),
                    pixel_size: [100, 80],
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

    #[test]
    fn actor_extracts_actual_pdf_text_and_searches_literal_case_sensitive_content() {
        let (_directory, path) = fixture();
        let mut documents = HashMap::new();
        process(&mut documents, Command::Load { id: 1, path }).expect("load");
        let event = process(
            &mut documents,
            Command::Text(TextKey {
                tab: 1,
                revision: 0,
                page: 0,
            }),
        )
        .expect("extract");
        assert!(
            matches!(event, Event::Text(PageText { text, truncated: false, .. }) if text.contains("Hello PDF workstation"))
        );
        let key = SearchKey {
            tab: 1,
            revision: 0,
            generation: 1,
        };
        let event = process(
            &mut documents,
            Command::Search {
                key,
                page: 0,
                query: "hello pdf".into(),
                match_case: false,
            },
        )
        .expect("search");
        assert!(
            matches!(event, Event::Searched { hit: Some(SearchHit { page: 0, excerpt }), .. } if excerpt.contains("Hello PDF"))
        );
        let event = process(
            &mut documents,
            Command::Search {
                key,
                page: 0,
                query: "hello pdf".into(),
                match_case: true,
            },
        )
        .expect("case search");
        assert!(matches!(event, Event::Searched { hit: None, .. }));
    }

    #[test]
    fn actor_renders_retina_pixels_and_preserves_logical_canvas_dimensions() {
        let (_directory, path) = fixture();
        let mut documents = HashMap::new();
        process(
            &mut documents,
            Command::Load {
                id: 1,
                path: path.clone(),
            },
        )
        .expect("load");
        let working = documents.get(&1).expect("working document");
        let mut tab = crate::state::Tab::new(1, path, working.status().expect("status"));
        tab.set_device_ratio(2.0);
        let key = tab.key().expect("retina render key");
        let plan = tab.raster_plan().expect("plan");
        let event = process(&mut documents, Command::Render(key)).expect("render real PDF");
        let Event::Rendered { image, .. } = event else {
            panic!("expected rendered PDF");
        };
        assert_eq!([image.width, image.height], [200, 160]);
        assert_eq!([image.width, image.height], plan.pixel_size);
        assert_eq!(image.rgba.len(), 200 * 160 * 4);
        assert_eq!(
            plan.display_size([image.width, image.height]),
            [100.0, 80.0]
        );
    }

    #[test]
    fn actor_passes_logical_zoom_separately_from_raster_density() {
        let (_directory, path) = fixture();
        let mut source = PdfDocument::open(&path, None).expect("source");
        source
            .add_note(1, [10.0, 20.0, 34.0, 44.0], "note", "")
            .expect("note");
        std::fs::write(&path, source.to_bytes().expect("bytes")).expect("synthetic fixture");
        let mut documents = HashMap::new();
        process(
            &mut documents,
            Command::Load {
                id: 1,
                path: path.clone(),
            },
        )
        .expect("load");
        let status = documents[&1].status().expect("status");
        let mut tab = crate::state::Tab::new(1, path, status);
        tab.set_scale(2.0);
        for ratio in [1.0, 2.0] {
            tab.set_device_ratio(ratio);
            let key = tab.key().expect("key");
            let Event::Rendered { image, .. } =
                process(&mut documents, Command::Render(key)).expect("render")
            else {
                panic!("expected image");
            };
            let anchor = [20.0 * ratio, 72.0 * ratio];
            let side = 24.0 * ratio;
            let mut yellow = 0;
            for (index, pixel) in image.rgba.chunks_exact(4).enumerate() {
                if pixel[0] > 240 && pixel[1] > 190 && pixel[2] < 120 {
                    yellow += 1;
                    let x = (index % image.width as usize) as f32;
                    let y = (index / image.width as usize) as f32;
                    assert!(x >= anchor[0] - 1.0 && x <= anchor[0] + side + 1.0);
                    assert!(y >= anchor[1] - 1.0 && y <= anchor[1] + side + 1.0);
                }
            }
            assert!(yellow > 100, "sticky icon must remain visible");
        }
        let mut invalid = tab.key().expect("key");
        invalid.zoom_bits = 0;
        assert!(process(&mut documents, Command::Render(invalid)).is_err());
    }
}
