#![allow(clippy::unwrap_used)]

use serde_json::Value;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}

fn vector_pdf(extra: &str) -> Vec<u8> {
    let paint = "1 0 0 rg 0 0 50 80 re f";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 80] {extra} /Resources << >> /Contents 4 0 R >>"
        ),
        format!("<< /Length {} >>\nstream\n{paint}\nendstream", paint.len()),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}

fn decoded_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(frame.buffer_size());
    reader.finish().unwrap();
    (frame.width, frame.height, pixels)
}

#[test]
fn png_export_preserves_vectors_dimensions_opaque_pixels_and_input() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("vectors.pdf");
    let output = directory.path().join("page.png");
    let bytes = vector_pdf("");
    std::fs::write(&input, &bytes).unwrap();
    let result = cli(&[
        "render-page",
        input.to_str().unwrap(),
        "--page",
        "1",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["data"]["width"], 100);
    assert_eq!(report["data"]["height"], 80);
    assert_eq!(report["data"]["output_verified"], true);
    assert_eq!(&std::fs::read(&output).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    let (width, height, pixels) = decoded_png(&output);
    assert_eq!((width, height), (100, 80));
    assert_eq!(&pixels[(40 * 100 + 25) * 4..][..4], &[255, 0, 0, 255]);
    assert_eq!(&pixels[(40 * 100 + 75) * 4..][..4], &[255, 255, 255, 255]);
    assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}

#[test]
fn crop_rotation_fractional_scale_and_dry_run_use_actual_render_dimensions() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("cropped.pdf");
    let output = directory.path().join("cropped.png");
    std::fs::write(&input, vector_pdf("/CropBox [0 0 60 40] /Rotate 90")).unwrap();
    let args = [
        "render-page",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--scale",
        "1.5",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    let mut preview = args.to_vec();
    preview.push("--dry-run");
    let result = cli(&preview);
    assert!(result.status.success(), "{result:?}");
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        (
            report["data"]["width"].as_u64(),
            report["data"]["height"].as_u64()
        ),
        (Some(60), Some(90))
    );
    assert_eq!(report["data"]["output_verified"], false);
    assert!(!output.exists());
    let result = cli(&args);
    assert!(result.status.success(), "{result:?}");
    let (width, height, pixels) = decoded_png(&output);
    assert_eq!((width, height), (60, 90));
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 0, 0, 255])
    );
}

#[test]
fn render_errors_and_no_clobber_leave_sources_and_existing_outputs_intact() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    let output = directory.path().join("output.png");
    let bytes = vector_pdf("");
    std::fs::write(&input, &bytes).unwrap();
    for (page, scale, code) in [
        ("0", "1", 1),
        ("2", "1", 1),
        ("1", "0", 1),
        ("1", "-1", 1),
        ("1", "NaN", 1),
        ("1", "inf", 1),
        ("1", "10000", 5),
    ] {
        let result = cli(&[
            "render-page",
            input.to_str().unwrap(),
            "--page",
            page,
            "--scale",
            scale,
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(result.status.code(), Some(code), "{result:?}");
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    std::fs::write(&output, b"existing file").unwrap();
    let result = cli(&[
        "render-page",
        input.to_str().unwrap(),
        "--page",
        "1",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing file");
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn png_export_never_follows_an_existing_output_symlink() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    let target = directory.path().join("target");
    let output = directory.path().join("linked.png");
    std::fs::write(&input, vector_pdf("")).unwrap();
    std::fs::write(&target, b"untouched").unwrap();
    std::os::unix::fs::symlink(&target, &output).unwrap();
    let result = cli(&[
        "render-page",
        input.to_str().unwrap(),
        "--page",
        "1",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
    assert!(output.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn signed_pdf_can_be_rendered_read_only_and_malformed_pdf_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdfs");
    let signed = corpus.join("signed.pdf");
    let before = std::fs::read(&signed).unwrap();
    let output = directory.path().join("signed.png");
    let result = cli(&[
        "render-page",
        signed.to_str().unwrap(),
        "--page",
        "1",
        "--scale",
        "0.25",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    let (width, height, _) = decoded_png(&output);
    assert_eq!((width, height), (149, 211));
    assert_eq!(std::fs::read(&signed).unwrap(), before);
    let output = directory.path().join("bad.png");
    let result = cli(&[
        "render-page",
        corpus.join("malformed.pdf").to_str().unwrap(),
        "--page",
        "1",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn decrypted_v2_fixture_still_refuses_rendering_and_never_exposes_passwords() {
    use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
    use std::io::Write;
    use std::process::Stdio;

    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("encrypted.pdf");
    let output = directory.path().join("encrypted.png");
    let mut source = Document::load_mem(&vector_pdf("")).unwrap();
    source.trailer.set(
        "ID",
        vec![
            Object::string_literal("synthetic-first!"),
            Object::string_literal("synthetic-next!!"),
        ],
    );
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &source,
        owner_password: "synthetic-render-owner",
        user_password: "synthetic-render-user",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    source.save(&input).unwrap();
    let before = std::fs::read(&input).unwrap();
    let decrypted = pdf_core::PdfDocument::open(&input, Some("synthetic-render-user")).unwrap();
    assert!(decrypted.info().unwrap().encrypted);
    assert_eq!(decrypted.page_count(), 1);
    for (password, code) in [("synthetic-render-user", 4), ("wrong-render-password", 1)] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args([
                "render-page",
                input.to_str().unwrap(),
                "--page",
                "1",
                "-o",
                output.to_str().unwrap(),
                "--password-stdin",
                "--json",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{password}\n").as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.code(), Some(code), "{result:?}");
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["ok"], false);
        if code == 4 {
            assert_eq!(report["error"]["kind"], "refused");
            assert_eq!(
                report["error"]["message"],
                "operation refused: encrypted rendering is not supported"
            );
        }
        assert!(!String::from_utf8_lossy(&result.stdout).contains(password));
        assert!(!String::from_utf8_lossy(&result.stderr).contains(password));
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
