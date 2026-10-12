use crate::search::{
    MAX_QUERY_CHARS, PageText, SearchHit, SearchKey, limit_page_text, literal_excerpt,
};
use crate::state::{DocumentStatus, MetadataReadKey, RenderKey, TextKey};
use crate::thumbnails::ThumbnailKey;
use eframe::egui;
#[cfg(test)]
use pdf_core::PdfDocument;
use pdf_render::RenderedPage;
use pdf_worker::PayloadKind;
use pdf_worker::document::{DocumentResult, Operation};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};

const HISTORY_LIMIT: usize = 8;
const HISTORY_BYTES: usize = 256 * 1024 * 1024;
const SNAPSHOT_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_TABS: usize = 16;

// Tiny noninterlaced RGB fixtures: a solid red 2×3 image and a solid blue 3×2 image.
#[cfg(test)]
pub(crate) const TEST_RED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0,
    0, 0, 54, 136, 73, 214, 0, 0, 0, 16, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 68, 12,
    40, 20, 0, 68, 208, 5, 251, 164, 207, 222, 128, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
#[cfg(test)]
const TEST_BLUE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 2, 8, 2, 0,
    0, 0, 18, 22, 241, 77, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 96, 96, 248, 15, 67, 48, 22,
    0, 53, 222, 5, 251, 226, 150, 96, 158, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

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
    Images {
        id: u64,
        paths: Vec<PathBuf>,
    },
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
            | Self::Images { id, .. }
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

mod document;
use document::WorkingDocument;

fn process(
    documents: &mut HashMap<u64, WorkingDocument>,
    command: Command,
) -> Result<Event, String> {
    if let Command::Images { id, paths } = command {
        if documents.len() >= MAX_TABS {
            return Err(
                "Close a PDF before creating another document; at most 16 tabs can be open.".into(),
            );
        }
        if documents.contains_key(&id) {
            return Err("New document identifier is already open".into());
        }
        let document = WorkingDocument::from_images(&paths)?;
        check_snapshot_budget(documents, &document)?;
        let status = document.status()?;
        documents.insert(id, document);
        return Ok(Event::Loaded {
            id,
            path: PathBuf::from("Images — unsaved.pdf"),
            status,
        });
    }
    if let Command::Load { id, path } = command {
        if documents.contains_key(&id) {
            return Err("Document identifier is already open".into());
        }
        if documents.len() >= MAX_TABS {
            return Err(
                "Close a PDF before opening another document; at most 16 tabs can be open.".into(),
            );
        }
        let document = WorkingDocument::load(&path)?;
        check_snapshot_budget(documents, &document)?;
        let status = document.status()?;
        documents.insert(id, document);
        return Ok(Event::Loaded { id, path, status });
    }
    if let Command::Close(id) = command {
        documents.remove(&id);
        return Ok(Event::Closed(id));
    }
    let id = command.id();
    let remaining = SNAPSHOT_BYTES.saturating_sub(
        documents
            .iter()
            .filter(|(other, _)| **other != id)
            .map(|(_, document)| document.byte_len())
            .sum::<usize>(),
    );
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
            let page = document
                .current
                .info
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
                values: document.metadata()?,
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
            let selected_page = document.edit_with_budget(edit, remaining)?;
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
        Command::Load { .. } | Command::Images { .. } | Command::Close(_) => {
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
    let (result, reply) = document::execute(
        Operation::Render {
            page: key.page,
            scale: f32::from_bits(key.scale_bits),
            logical_zoom: view_zoom,
        },
        &document.current.bytes,
    )?;
    let DocumentResult::Render { raster } = result else {
        return Err("Unexpected raster response".into());
    };
    let length = u64::from(raster.width)
        .checked_mul(u64::from(raster.height))
        .and_then(|pixels| pixels.checked_mul(4));
    if reply.kind != PayloadKind::Rgba
        || raster.page != key.page
        || raster.scale.to_bits() != key.scale_bits
        || raster.logical_zoom.to_bits() != view_zoom.to_bits()
        || raster.width == 0
        || raster.height == 0
        || raster.width > pdf_render::MAX_DIMENSION
        || raster.height > pdf_render::MAX_DIMENSION
        || length != Some(reply.payload.len() as u64)
        || reply.payload.len() > pdf_worker::MAX_RGBA_BYTES
        || [raster.width, raster.height] != key.pixel_size
    {
        return Err(
            "This PDF page's preview dimensions differ from its inspected visible area.".into(),
        );
    }
    Ok(RenderedPage {
        width: raster.width,
        height: raster.height,
        rgba: reply.payload,
    })
}

fn page_text(document: &WorkingDocument, page: usize) -> Result<(String, bool), String> {
    if page >= document.current.info.page_count {
        return Err("Page is outside document".into());
    }
    let (result, reply) = document::execute(
        Operation::Text {
            page: page as u32 + 1,
        },
        &document.current.bytes,
    )?;
    let DocumentResult::Text { text, truncated } = result else {
        return Err("Unexpected page text response".into());
    };
    if reply.kind != PayloadKind::None
        || !reply.payload.is_empty()
        || text.len() > pdf_worker::document::MAX_TEXT_BYTES
    {
        return Err("Invalid page text payload".into());
    }
    let (text, parent_truncated) = limit_page_text(text);
    Ok((text, truncated || parent_truncated))
}

fn check_snapshot_budget(
    documents: &HashMap<u64, WorkingDocument>,
    candidate: &WorkingDocument,
) -> Result<(), String> {
    let total = documents
        .values()
        .map(WorkingDocument::byte_len)
        .sum::<usize>();
    if candidate.byte_len() > SNAPSHOT_BYTES.saturating_sub(total) {
        return Err(
            "Open document snapshots exceed 512 MiB; close a tab before continuing.".into(),
        );
    }
    Ok(())
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
    fn child_launch_failure_retains_edits_history_source_and_save_destination() {
        let (directory, source) = fixture();
        let mut documents = HashMap::new();
        process(
            &mut documents,
            Command::Load {
                id: 1,
                path: source.clone(),
            },
        )
        .expect("load child");
        process(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: Edit::Rotate(1, 90),
            },
        )
        .expect("edit child");
        let original = std::fs::read(&source).expect("source");
        let bytes = documents[&1].current.bytes.clone();
        let status = documents[&1].status().expect("status");
        let next_revision = documents[&1].next_revision;
        let undo = documents[&1].undo.len();
        let destination = directory.path().join("failed-save.pdf");
        let context =
            crate::state::Tab::new(1, source.clone(), documents[&1].status().expect("status"));
        document::with_test_executable(directory.path().join("missing-worker"), || {
            assert!(
                process(
                    &mut documents,
                    Command::Edit {
                        id: 1,
                        edit: Edit::Rotate(1, 90)
                    }
                )
                .is_err()
            );
            assert!(process(&mut documents, Command::Render(context.key().expect("key"))).is_err());
            assert!(
                process(
                    &mut documents,
                    Command::Text(TextKey {
                        tab: 1,
                        revision: status.revision,
                        page: 0
                    })
                )
                .is_err()
            );
            assert!(
                process(
                    &mut documents,
                    Command::Save {
                        id: 1,
                        path: destination.clone()
                    }
                )
                .is_err()
            );
            assert!(
                process(
                    &mut documents,
                    Command::Load {
                        id: 2,
                        path: source.clone()
                    }
                )
                .is_err()
            );
        });
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[&1].current.bytes, bytes);
        assert_eq!(documents[&1].current.revision, status.revision);
        assert_eq!(documents[&1].next_revision, next_revision);
        assert_eq!(documents[&1].undo.len(), undo);
        assert!(documents[&1].status().expect("status").dirty);
        assert_eq!(std::fs::read(source).expect("original"), original);
        assert!(!destination.exists());
        // Test override is thread-local and restored: a real child can retry the operation.
        process(
            &mut documents,
            Command::Save {
                id: 1,
                path: destination.clone(),
            },
        )
        .expect("retry save");
        assert!(destination.exists());
    }

    #[test]
    fn snapshot_budget_failure_after_successful_child_edit_leaves_parent_unchanged() {
        let (_directory, source) = fixture();
        let mut document = WorkingDocument::load(&source).expect("load");
        let bytes = document.current.bytes.clone();
        assert!(document.edit_with_budget(Edit::Rotate(1, 90), 1).is_err());
        assert_eq!(document.current.bytes, bytes);
        assert_eq!(document.current.revision, 0);
        assert_eq!(document.next_revision, 1);
        assert!(document.undo.is_empty() && document.redo.is_empty());
        assert!(!document.status().expect("status").dirty);
    }

    #[test]
    #[ignore = "build pdf-worker with --features test-helper before this fault-injection gate"]
    fn crashed_timed_out_and_corrupt_children_cannot_commit_or_publish() {
        let (directory, source) = fixture();
        let current = std::env::current_exe().expect("test executable");
        let helper = current
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target directory")
            .join(if cfg!(windows) {
                "pdf-worker-test-helper.exe"
            } else {
                "pdf-worker-test-helper"
            });
        assert!(
            helper.is_file(),
            "build pdf-worker --features test-helper first"
        );
        let mut working = WorkingDocument::load(&source).expect("load");
        working.edit(Edit::Rotate(1, 90)).expect("first edit");
        let committed = working.current.bytes.clone();
        let revision = working.current.revision;
        let next_revision = working.next_revision;
        let original = std::fs::read(&source).expect("source");
        let limits = pdf_worker::Limits::for_test(
            pdf_worker::MEMORY_BYTES,
            30,
            std::time::Duration::from_millis(500),
        )
        .expect("reduced deadline");
        for mode in [
            "abort",
            "timeout",
            "truncated",
            "checksum",
            "oversize",
            "malformed",
        ] {
            let destination = directory.path().join(format!("{mode}.pdf"));
            document::with_test_executor(
                helper.clone(),
                vec![std::ffi::OsString::from(mode)],
                limits,
                || {
                    assert!(working.edit(Edit::Rotate(1, 90)).is_err(), "{mode}");
                    assert!(working.save(&destination).is_err(), "{mode}");
                },
            );
            assert_eq!(working.current.bytes, committed, "{mode}");
            assert_eq!(working.current.revision, revision, "{mode}");
            assert_eq!(working.next_revision, next_revision, "{mode}");
            assert_eq!(working.undo.len(), 1, "{mode}");
            assert!(working.redo.is_empty() && working.status().expect("status").dirty);
            assert_eq!(std::fs::read(&source).expect("source retained"), original);
            assert!(!destination.exists(), "{mode}");
        }
        working
            .edit(Edit::Rotate(1, 90))
            .expect("successful retry after faults");
    }

    #[test]
    #[ignore = "build the normal native executable before exercising its early worker entry"]
    fn native_executable_self_hosts_document_worker_without_gui_startup() {
        let (directory, path) = fixture();
        let current = std::env::current_exe().expect("test executable");
        let helper = current
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target directory")
            .join(if cfg!(windows) {
                "pdf-editor-offline-native.exe"
            } else {
                "pdf-editor-offline-native"
            });
        assert!(helper.is_file(), "build normal native executable first");
        document::with_test_executor(
            helper,
            vec![std::ffi::OsString::from("--pdf-worker")],
            pdf_worker::Limits::default(),
            || {
                let mut working = WorkingDocument::load(&path).expect("self-hosted info");
                let text = page_text(&working, 0).expect("self-hosted text");
                assert!(text.0.contains("Hello PDF workstation"));
                working.edit(Edit::Rotate(1, 90)).expect("self-hosted edit");
                let tab = crate::state::Tab::new(1, path, working.status().expect("status"));
                let image =
                    render(&working, tab.key().expect("key"), 1.0).expect("self-hosted raster");
                assert_eq!([image.width, image.height], [80, 100]);
                working
                    .save(&directory.path().join("self-hosted.pdf"))
                    .expect("self-hosted verified save");
                assert!(!working.status().expect("saved").dirty);
            },
        );
    }

    #[test]
    fn image_conversion_creates_an_unsaved_tab_with_real_pixels_history_and_verified_save() {
        let (directory, path) = fixture();
        let red = directory.path().join("red.png");
        let blue = directory.path().join("blue.png");
        std::fs::write(&red, TEST_RED_PNG).expect("red fixture");
        std::fs::write(&blue, TEST_BLUE_PNG).expect("blue fixture");
        let mut documents = HashMap::new();
        process(&mut documents, Command::Load { id: 1, path }).expect("previous PDF");
        documents
            .get_mut(&1)
            .expect("previous")
            .edit(Edit::Rotate(1, 90))
            .expect("existing edit");
        let previous = documents[&1].current.bytes.clone();
        let Event::Loaded { id, status, .. } = process(
            &mut documents,
            Command::Images {
                id: 2,
                paths: vec![blue.clone(), red.clone()],
            },
        )
        .expect("convert") else {
            panic!("new tab event");
        };
        assert_eq!(id, 2);
        assert!(status.dirty && !status.can_undo);
        assert_eq!(status.info.page_count, 2);
        assert_eq!(status.info.pages[0].media_box, [0.0, 0.0, 3.0, 2.0]);
        assert_eq!(status.info.pages[1].media_box, [0.0, 0.0, 2.0, 3.0]);
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("directory")
                .count(),
            3
        ); // No PDF was written by conversion.
        let mut tab = crate::state::Tab::new(2, "Images — unsaved.pdf".into(), status);
        for (page, expected) in [(0, [0, 0, 255, 255]), (1, [255, 0, 0, 255])] {
            tab.set_page(page);
            let image = render(&documents[&2], tab.key().expect("render key"), tab.scale)
                .expect("real preview");
            assert!(image.rgba.chunks_exact(4).all(|pixel| pixel == expected));
        }
        assert!(
            process(
                &mut documents,
                Command::Images {
                    id: 3,
                    paths: vec![directory.path().join("missing.png")]
                }
            )
            .is_err()
        );
        assert_eq!(documents.len(), 2);
        assert_eq!(documents[&1].current.bytes, previous);
        assert!(documents[&1].status().expect("previous").dirty);
        let working = documents.get_mut(&2).expect("converted");
        working
            .edit(Edit::Rotate(1, 90))
            .expect("edit generated PDF");
        working.edit(Edit::Undo).expect("undo");
        assert!(working.status().expect("unsaved origin").dirty);
        assert!(working.save(&red).is_err());
        assert_eq!(std::fs::read(&red).expect("image preserved"), TEST_RED_PNG);
        let output = directory.path().join("images.pdf");
        working.save(&output).expect("verified save");
        assert!(!working.status().expect("saved").dirty);
        assert!(working.save(&output).is_err());
        assert_eq!(
            PdfDocument::open(output, None)
                .expect("reopen")
                .page_count(),
            2
        );
        assert_eq!(std::fs::read(blue).expect("image preserved"), TEST_BLUE_PNG);
    }

    #[test]
    fn image_input_preflight_bounds_regular_files_and_aggregate_before_decoding() {
        let directory = tempfile::tempdir().expect("directory");
        assert!(WorkingDocument::from_images(&[directory.path().to_path_buf()]).is_err());
        let oversized = directory.path().join("oversized.png");
        std::fs::File::create(&oversized)
            .expect("sparse file")
            .set_len(pdf_core::MAX_STREAM_BYTES as u64 + 1)
            .expect("size");
        assert!(
            WorkingDocument::from_images(&[oversized])
                .err()
                .expect("limit")
                .contains("32 MiB")
        );
        let mut paths = vec![];
        for index in 0..5 {
            let path = directory.path().join(format!("aggregate-{index}.png"));
            let length = if index == 4 {
                1
            } else {
                pdf_core::MAX_STREAM_BYTES as u64
            };
            std::fs::File::create(&path)
                .expect("sparse file")
                .set_len(length)
                .expect("size");
            paths.push(path);
        }
        let error = WorkingDocument::from_images(&paths)
            .err()
            .expect("aggregate refusal");
        assert!(error.contains("aggregate-4.png") && error.contains("128 MiB")); // Decode would reject the zero-filled first file instead.
    }

    #[test]
    fn image_conversion_refuses_tab_capacity_collisions_and_invalid_formats() {
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
        let original = documents[&1].current.bytes.clone();
        assert!(
            process(
                &mut documents,
                Command::Images {
                    id: 1,
                    paths: vec![path.clone()]
                }
            )
            .is_err()
        );
        let error = process(
            &mut documents,
            Command::Images {
                id: 2,
                paths: vec![path.clone()],
            },
        )
        .err()
        .expect("format refusal");
        assert!(error.contains("PNG/JPEG"));
        assert_eq!(documents[&1].current.bytes, original);
        for id in 2..=MAX_TABS as u64 {
            process(
                &mut documents,
                Command::Load {
                    id,
                    path: path.clone(),
                },
            )
            .expect("load tab");
        }
        assert!(
            process(
                &mut documents,
                Command::Images {
                    id: 17,
                    paths: vec![path]
                }
            )
            .err()
            .expect("capacity")
            .contains("16 tabs")
        );
        assert_eq!(documents.len(), MAX_TABS);
        assert_eq!(documents[&1].current.bytes, original);
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
        let edited = working.document_for_test().metadata().expect("metadata");
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
        process(&mut documents, Command::Close(1)).expect("close original before signed reopen");
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
                .document_for_test()
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
        assert_eq!(working.current.info.page_count, 4);
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
        assert_eq!(working.current.info.page_count, 3);
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
