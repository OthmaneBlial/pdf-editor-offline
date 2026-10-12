//! Offline PDF processing. Page numbers in this public API are one-based.
//! Saving creates a verified new copy and never replaces an existing file.

mod error;
mod structure;

pub use error::{Error, Result};
use lopdf::{Document, LoadOptions, Object, ObjectId};
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_STREAM_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PAGES: usize = 20_000;
pub const MAX_OBJECTS: usize = 1_000_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PageInfo {
    pub number: u32,
    pub media_box: [f32; 4],
    pub rotation: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentInfo {
    pub version: String,
    pub page_count: usize,
    pub object_count: usize,
    pub encrypted: bool,
    pub signature_fields: usize,
    pub pages: Vec<PageInfo>,
}

#[derive(Clone)]
pub struct PdfDocument {
    document: Document,
    pages: Vec<ObjectId>,
    encrypted_input: bool,
}

impl PdfDocument {
    pub fn open(path: impl AsRef<Path>, password: Option<&str>) -> Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(MAX_INPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes, password)
    }

    pub fn from_bytes(bytes: &[u8], password: Option<&str>) -> Result<Self> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(Error::Limit("input exceeds 128 MiB"));
        }
        if !bytes.starts_with(b"%PDF-") {
            return Err(Error::Invalid("missing PDF header"));
        }
        let document = Document::load_mem_with_options(
            bytes,
            LoadOptions {
                password: password.map(str::to_owned),
                strict: true,
                max_decompressed_size: Some(MAX_STREAM_BYTES),
                ..Default::default()
            },
        )?;
        let encrypted_input =
            document.trailer.has(b"Encrypt") || document.encryption_state.is_some();
        Self::from_document(document, encrypted_input)
    }

    fn from_document(document: Document, encrypted_input: bool) -> Result<Self> {
        if document.objects.len() > MAX_OBJECTS {
            return Err(Error::Limit("too many PDF objects"));
        }
        let pages = structure::validate_pages(&document)?;
        let result = Self {
            document,
            pages,
            encrypted_input,
        };
        result.info()?;
        Ok(result)
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn info(&self) -> Result<DocumentInfo> {
        let pages = self
            .pages
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let media_box = structure::page_box(&self.document, *id, b"MediaBox")?;
                let rotation = structure::inherited(&self.document, *id, b"Rotate")?
                    .map(Object::as_i64)
                    .transpose()?
                    .unwrap_or(0);
                if rotation % 90 != 0 {
                    return Err(Error::Invalid("page rotation is not a multiple of 90"));
                }
                Ok(PageInfo {
                    number: index as u32 + 1,
                    media_box,
                    rotation: rotation.rem_euclid(360),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(DocumentInfo {
            version: self.document.version.clone(),
            page_count: self.pages.len(),
            object_count: self.document.objects.len(),
            encrypted: self.encrypted_input,
            signature_fields: self
                .document
                .objects
                .values()
                .filter(|object| {
                    object.as_dict().is_ok_and(|dict| {
                        [b"FT".as_slice(), b"Type".as_slice()].iter().any(|key| {
                            dict.get(key)
                                .and_then(Object::as_name)
                                .is_ok_and(|name| name == b"Sig")
                        })
                    })
                })
                .count(),
            pages,
        })
    }

    pub fn extract_text(&self, pages: &[u32]) -> Result<String> {
        self.validate_selection(pages)?;
        let mut text = String::new();
        for page in pages {
            let extracted = self
                .document
                .extract_text_with_limit(&[*page], MAX_STREAM_BYTES)?;
            if text.len().saturating_add(extracted.len()) > MAX_INPUT_BYTES {
                return Err(Error::Limit("extracted text exceeds 128 MiB"));
            }
            text.push_str(&extracted);
        }
        Ok(text)
    }

    pub fn all_pages(&self) -> Vec<u32> {
        (1..=self.pages.len() as u32).collect()
    }

    pub fn validate_selection(&self, pages: &[u32]) -> Result<()> {
        if pages.is_empty() {
            return Err(Error::Pages("select at least one page"));
        }
        if pages
            .iter()
            .any(|page| *page == 0 || *page as usize > self.pages.len())
        {
            return Err(Error::Pages("page number outside document"));
        }
        Ok(())
    }

    pub fn ensure_editable(&self) -> Result<()> {
        if self.encrypted_input {
            return Err(Error::Refused(
                "encrypted inputs are read-only until encrypted export is implemented",
            ));
        }
        if self.info()?.signature_fields > 0 {
            return Err(Error::Refused(
                "rewriting could invalidate digital signatures; signed inputs are read-only",
            ));
        }
        Ok(())
    }

    /// Add a quarter-turn rotation to the selected pages, preserving inherited rotation.
    pub fn rotate(&mut self, pages: &[u32], degrees: i64) -> Result<()> {
        self.ensure_editable()?;
        self.validate_selection(pages)?;
        if degrees % 90 != 0 {
            return Err(Error::Pages("rotation must be a multiple of 90 degrees"));
        }
        let info = self.info()?;
        let unique: std::collections::BTreeSet<_> = pages.iter().copied().collect();
        for page in unique {
            let index = page as usize - 1;
            self.document
                .get_object_mut(self.pages[index])?
                .as_dict_mut()?
                .set(
                    "Rotate",
                    (info.pages[index].rotation + degrees.rem_euclid(360)).rem_euclid(360),
                );
        }
        Ok(())
    }

    /// Lossless stream compression; output may be larger for already optimized PDFs.
    pub fn compress(&mut self) -> Result<()> {
        self.ensure_editable()?;
        self.document.compress();
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.ensure_editable()?;
        let mut bytes = Vec::new();
        self.document.clone().save_to(&mut bytes)?;
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(Error::Limit("output exceeds 128 MiB"));
        }
        Ok(bytes)
    }

    /// Serialize, fsync and reopen a same-directory temporary file before publishing.
    /// Existing paths (including symlinks) are never overwritten.
    pub fn save_new(&self, output: impl AsRef<Path>) -> Result<()> {
        self.ensure_editable()?;
        let output = output.as_ref();
        if output.symlink_metadata().is_ok() {
            return Err(Error::OutputExists(output.to_owned()));
        }
        let parent = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        self.document.clone().save_to(temporary.as_file_mut())?;
        temporary.as_file().sync_all()?;
        let reopened = Self::open(temporary.path(), None)?;
        if reopened.info()?.pages != self.info()?.pages || reopened.pages != self.pages {
            return Err(Error::Verification(
                "page structure changed during serialization",
            ));
        }
        if reopened.document.objects.len() != self.document.objects.len() {
            return Err(Error::Verification(
                "object count changed during serialization",
            ));
        }
        temporary.persist_noclobber(output).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                Error::OutputExists(output.to_owned())
            } else {
                Error::Io(error.error)
            }
        })?;
        Ok(())
    }
}
