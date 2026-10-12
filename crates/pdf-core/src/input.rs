use crate::{Error, MAX_INPUT_BYTES, Result};
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

/// Read a bounded regular file. Symlinks to regular files are allowed; special files are refused.
pub fn read_pdf_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    validate_input_metadata(&std::fs::metadata(path)?)?;
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags};
        // A path swapped to a FIFO after preflight must not block inside open.
        let descriptor = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        File::from(descriptor)
    };
    #[cfg(not(unix))]
    let file = File::open(path)?;
    validate_input_metadata(&file.metadata()?)?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::Limit("input exceeds 128 MiB"));
    }
    Ok(bytes)
}

fn validate_input_metadata(metadata: &Metadata) -> Result<()> {
    if !metadata.is_file() {
        return Err(Error::Refused("input must be a regular file"));
    }
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(Error::Limit("input exceeds 128 MiB"));
    }
    Ok(())
}
