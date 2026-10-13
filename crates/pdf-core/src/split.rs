use crate::{DocumentInfo, Error, MAX_INPUT_BYTES, PdfDocument, Result};
use lopdf::Object;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub const MAX_SPLIT_PARTS: usize = 1024;
pub const MAX_SPLIT_BYTES: usize = MAX_INPUT_BYTES;
const MAX_COPY_VALUES: usize = 2_000_000;
/// A verified full rewrite. Unreachable data from other pages may remain.
pub struct SplitPart {
    pub first_page: u32,
    pub last_page: u32,
    pub info: DocumentInfo,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitOutput {
    pub first_page: u32,
    pub last_page: u32,
    pub file: PathBuf,
    pub info: DocumentInfo,
}
impl PdfDocument {
    /// Guarded consecutive chunks. This is page organization, not privacy cleanup.
    pub fn split_every(&self, every: u32) -> Result<Vec<SplitPart>> {
        self.ensure_editable()?;
        if every == 0 {
            return Err(Error::Pages("split size must be positive"));
        }
        let size = every as usize;
        let count = self.page_count().div_ceil(size);
        if count > MAX_SPLIT_PARTS {
            return Err(Error::Limit("split exceeds 1024 output parts"));
        }
        copy_budget(self, count)?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(count)
            .map_err(|_| Error::Limit("split result allocation failed"))?;
        let mut total = 0usize;
        for start in (0..self.page_count()).step_by(size) {
            let end = start.saturating_add(size).min(self.page_count());
            let pages = (start + 1..=end)
                .map(|page| page as u32)
                .collect::<Vec<_>>();
            let part = self.extract_pages(&pages)?;
            let bytes = part.verified_bytes()?;
            total = total
                .checked_add(bytes.len())
                .filter(|total| *total <= MAX_SPLIT_BYTES)
                .ok_or(Error::Limit(
                    "split serialized outputs exceed 128 MiB total",
                ))?;
            parts.push(SplitPart {
                first_page: start as u32 + 1,
                last_page: end as u32,
                info: PdfDocument::from_bytes(&bytes, None)?.info()?,
                bytes,
            });
        }
        Ok(parts)
    }
    /// Verify all parts before creating an exclusive new directory. Publication
    /// is sequential: PartialSplit reports completed paths and never removes files.
    pub fn split_new_directory(
        &self,
        every: u32,
        directory: impl AsRef<Path>,
    ) -> Result<Vec<SplitOutput>> {
        let directory = directory.as_ref();
        if directory.symlink_metadata().is_ok() {
            return Err(Error::OutputExists(directory.to_path_buf()));
        }
        let parts = self.split_every(every)?;
        std::fs::create_dir(directory)?;
        publish_parts(parts, directory, |part, path| {
            PdfDocument::from_bytes(&part.bytes, None)?.save_new(path)
        })
    }
}
fn publish_parts(
    parts: Vec<SplitPart>,
    directory: &Path,
    mut publish: impl FnMut(&SplitPart, &Path) -> Result<()>,
) -> Result<Vec<SplitOutput>> {
    let mut outputs = Vec::with_capacity(parts.len());
    for (index, part) in parts.into_iter().enumerate() {
        let path = directory.join(format!("part-{:04}.pdf", index + 1));
        if let Err(source) = publish(&part, &path) {
            let completed = outputs
                .iter()
                .map(|output: &SplitOutput| output.file.clone())
                .collect::<Vec<_>>();
            return Err(Error::PartialSplit {
                directory: directory.to_path_buf(),
                completed_count: completed.len(),
                completed,
                failed_output: Some(path),
                source: Box::new(source),
            });
        }
        outputs.push(SplitOutput {
            first_page: part.first_page,
            last_page: part.last_page,
            file: path,
            info: part.info,
        });
    }
    Ok(outputs)
}
fn copy_budget(pdf: &PdfDocument, parts: usize) -> Result<()> {
    let max_values = MAX_COPY_VALUES / parts;
    let max_bytes = MAX_SPLIT_BYTES / parts;
    if pdf
        .document
        .objects
        .len()
        .saturating_add(pdf.document.trailer.len())
        .saturating_add(1)
        > max_values
    {
        return Err(Error::Limit(
            "split aggregate copy work exceeds two million values",
        ));
    }
    let mut nodes = 1usize;
    let mut payload = pdf
        .document
        .trailer
        .iter()
        .fold(0usize, |total, (key, _)| total.saturating_add(key.len()));
    let mut pending = pdf
        .document
        .objects
        .values()
        .map(|object| (object, 0usize))
        .chain(pdf.document.trailer.iter().map(|(_, value)| (value, 1)))
        .collect::<Vec<_>>();
    while let Some((object, depth)) = pending.pop() {
        nodes += 1;
        if nodes > max_values || depth > 128 {
            return Err(Error::Limit(
                "split aggregate copy work exceeds two million values or 128 levels",
            ));
        }
        let mut dictionary = None;
        match object {
            Object::Array(values) => {
                if values.len() > max_values.saturating_sub(nodes + pending.len()) {
                    return Err(Error::Limit(
                        "split aggregate copy work exceeds two million values",
                    ));
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Object::Dictionary(value) => dictionary = Some(value),
            Object::Stream(stream) => {
                payload = payload.saturating_add(stream.content.len());
                dictionary = Some(&stream.dict);
            }
            Object::String(value, _) | Object::Name(value) => {
                payload = payload.saturating_add(value.len())
            }
            _ => {}
        }
        if let Some(dictionary) = dictionary {
            if dictionary.len() > max_values.saturating_sub(nodes + pending.len()) {
                return Err(Error::Limit(
                    "split aggregate copy work exceeds two million values",
                ));
            }
            for (key, value) in dictionary {
                payload = payload.saturating_add(key.len());
                pending.push((value, depth + 1));
            }
        }
        if payload > max_bytes {
            return Err(Error::Limit(
                "split aggregate logical payload exceeds 128 MiB",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn part() -> SplitPart {
        let doc = PdfDocument::from_bytes(
            include_bytes!("../../../tests/fixtures/pdfs/rotated-page.pdf"),
            None,
        )
        .unwrap()
        .extract_pages(&[1])
        .unwrap();
        SplitPart {
            first_page: 1,
            last_page: 1,
            info: doc.info().unwrap(),
            bytes: doc.verified_bytes().unwrap(),
        }
    }
    #[test]
    fn publication_failure_reports_zero_or_exact_completed_paths_and_preserves_blockers() {
        for fail_at in [0, 1] {
            let temp = tempfile::tempdir().unwrap();
            let directory = temp.path().join("new");
            std::fs::create_dir(&directory).unwrap();
            let mut index = 0;
            let result = publish_parts(vec![part(), part()], &directory, |part, path| {
                let current = index;
                index += 1;
                if current == fail_at {
                    std::fs::write(path, b"preexisting blocker")?;
                }
                PdfDocument::from_bytes(&part.bytes, None)?.save_new(path)
            });
            let Err(Error::PartialSplit {
                completed,
                completed_count,
                failed_output,
                ..
            }) = result
            else {
                panic!("partial error");
            };
            assert_eq!(completed_count, fail_at);
            assert_eq!(completed.len(), fail_at);
            assert_eq!(
                std::fs::read(failed_output.unwrap()).unwrap(),
                b"preexisting blocker"
            );
            for path in completed {
                assert_eq!(PdfDocument::open(path, None).unwrap().page_count(), 1);
            }
        }
    }
}
