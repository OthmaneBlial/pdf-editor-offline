#![allow(clippy::unwrap_used)]

use pdf_core::{Error, MAX_INPUT_BYTES, read_input_bytes, read_pdf_bytes};
use std::io::Write;

#[test]
fn caller_bound_accepts_exact_bytes_and_rejects_larger_inputs_and_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.bin");
    std::fs::write(&path, b"1234").unwrap();
    assert_eq!(read_input_bytes(&path, 4).unwrap(), b"1234");
    assert!(matches!(read_input_bytes(&path, 3), Err(Error::Limit(_))));
    assert!(matches!(read_input_bytes(&path, 0), Err(Error::Limit(_))));
    assert!(matches!(
        read_input_bytes(&path, usize::MAX),
        Err(Error::Limit(_))
    ));
    std::fs::write(&path, b"").unwrap();
    assert!(read_input_bytes(&path, 0).unwrap().is_empty());
}

#[test]
fn regular_input_bytes_round_trip_without_modifying_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("document.pdf");
    let original = b"%PDF-1.7\nsynthetic regular input\n";
    std::fs::write(&path, original).unwrap();
    assert_eq!(read_pdf_bytes(&path).unwrap(), original);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn exact_size_boundary_is_read_and_oversized_input_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("boundary.pdf");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"%PDF-").unwrap();
    file.set_len(MAX_INPUT_BYTES as u64).unwrap();
    let bytes = read_pdf_bytes(&path).unwrap();
    assert_eq!(bytes.len(), MAX_INPUT_BYTES);
    assert_eq!(&bytes[..5], b"%PDF-");
    drop(bytes);
    file.set_len(MAX_INPUT_BYTES as u64 + 1).unwrap();
    assert!(matches!(
        read_pdf_bytes(&path),
        Err(Error::Limit("input exceeds 128 MiB"))
    ));
    assert_eq!(file.metadata().unwrap().len(), MAX_INPUT_BYTES as u64 + 1);
}

#[test]
fn directories_and_missing_inputs_return_controlled_errors() {
    let directory = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_pdf_bytes(directory.path()),
        Err(Error::Refused("input must be a regular file"))
    ));
    assert!(matches!(
        read_pdf_bytes(directory.path().join("missing.pdf")),
        Err(Error::Io(_))
    ));
}

#[cfg(unix)]
#[test]
fn regular_symlinks_are_allowed_and_dangling_symlinks_fail() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.pdf");
    let link = directory.path().join("link.pdf");
    std::fs::write(&target, b"%PDF-regular").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_eq!(read_pdf_bytes(&link).unwrap(), b"%PDF-regular");
    std::fs::remove_file(&target).unwrap();
    assert!(matches!(read_pdf_bytes(&link), Err(Error::Io(_))));
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn character_devices_and_named_pipes_are_refused_before_reading() {
    assert!(matches!(
        read_pdf_bytes("/dev/null"),
        Err(Error::Refused("input must be a regular file"))
    ));
    let directory = tempfile::tempdir().unwrap();
    let pipe = directory.path().join("document.pdf");
    let result = std::process::Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap();
    assert!(result.success());
    // No writer is opened: a blocking File::open regression would hang here.
    assert!(matches!(
        read_pdf_bytes(&pipe),
        Err(Error::Refused("input must be a regular file"))
    ));
    let link = directory.path().join("pipe-link.pdf");
    std::os::unix::fs::symlink(&pipe, &link).unwrap();
    assert!(matches!(
        read_pdf_bytes(&link),
        Err(Error::Refused("input must be a regular file"))
    ));
}
