use crate::{Error, MAX_INPUT_BYTES, Result};
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

/// Read a bounded regular file. Symlinks to regular files are allowed; special files are refused.
pub fn read_pdf_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    read_input_bytes(path, MAX_INPUT_BYTES)
}

/// Read a regular input with a caller-specific bound no greater than the core input limit.
pub fn read_input_bytes(path: impl AsRef<Path>, max_bytes: usize) -> Result<Vec<u8>> {
    if max_bytes > MAX_INPUT_BYTES {
        return Err(Error::Limit("input reader limit exceeds 128 MiB"));
    }
    let path = path.as_ref();
    validate_input_metadata(&std::fs::metadata(path)?, max_bytes)?;
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
    validate_input_metadata(&file.metadata()?, max_bytes)?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(size_error(max_bytes));
    }
    Ok(bytes)
}

fn validate_input_metadata(metadata: &Metadata, max_bytes: usize) -> Result<()> {
    if !metadata.is_file() {
        return Err(Error::Refused("input must be a regular file"));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(size_error(max_bytes));
    }
    Ok(())
}

fn size_error(max_bytes: usize) -> Error {
    Error::Limit(if max_bytes == MAX_INPUT_BYTES {
        "input exceeds 128 MiB"
    } else {
        "input exceeds caller byte limit"
    })
}
