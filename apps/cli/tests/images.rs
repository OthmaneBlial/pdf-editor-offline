#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const PIXELS: [u8; 24] = [
    255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 20, 40, 60, 255, 255, 255, 0, 255, 10, 30, 50, 64,
];

fn png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
    }
    bytes
}

// Synthetic baseline 2x2 gray JPEG: zero DC and EOB produce samples of 128.
fn jpeg() -> Vec<u8> {
    fn segment(bytes: &mut Vec<u8>, marker: u8, payload: &[u8]) {
        bytes.extend([0xff, marker]);
        bytes.extend(((payload.len() + 2) as u16).to_be_bytes());
        bytes.extend(payload);
    }
    let mut bytes = vec![0xff, 0xd8];
    let mut quantization = vec![0];
    quantization.extend([1; 64]);
    segment(&mut bytes, 0xdb, &quantization);
    segment(&mut bytes, 0xc0, &[8, 0, 2, 0, 2, 1, 1, 0x11, 0]);
    let mut huffman = Vec::new();
    for table in [0, 0x10] {
        huffman.extend([table, 1]);
        huffman.extend([0; 15]);
        huffman.push(0);
    }
    segment(&mut bytes, 0xc4, &huffman);
    segment(&mut bytes, 0xda, &[1, 1, 0, 0, 63, 0]);
    bytes.extend([0x3f, 0xff, 0xd9]);
    bytes
}

fn convert(inputs: &[&Path], output: &Path, dry_run: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
    command
        .arg("images-to-pdf")
        .args(inputs)
        .arg("-o")
        .arg(output)
        .arg("--json");
    if dry_run {
        command.arg("--dry-run");
    }
    command.output().unwrap()
}
fn data(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}
fn error(output: &Output, code: i32, kind: &str) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["kind"], kind);
}
fn image(document: &Document, page: lopdf::ObjectId) -> &Stream {
    let page = document.get_dictionary(page).unwrap();
    let resources = document
        .dereference(page.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
    document
        .dereference(xobjects.get(b"Im0").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
}

#[test]
fn caller_order_geometry_lossless_alpha_original_jpeg_and_source_preservation() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.jpeg");
    let output = dir.path().join("images.pdf");
    let png_bytes = png(3, 2, &PIXELS);
    let jpeg_bytes = jpeg();
    std::fs::write(&first, &png_bytes).unwrap();
    std::fs::write(&second, &jpeg_bytes).unwrap();
    let report = data(&convert(&[&first, &second], &output, false));
    assert_eq!(report["page_count"], 2);
    assert_eq!(report["pages"][0]["alpha_mask"], true);
    assert_eq!(report["pages"][1]["encoding"], "original_dct");
    assert_eq!(report["metadata_sanitized"], false);
    let document = Document::load(&output).unwrap();
    let pages: Vec<_> = document.get_pages().into_values().collect();
    for (page, size) in pages.iter().zip([[3, 2], [2, 2]]) {
        assert_eq!(
            document
                .get_dictionary(*page)
                .unwrap()
                .get(b"MediaBox")
                .unwrap(),
            &Object::Array(vec![0.into(), 0.into(), size[0].into(), size[1].into()])
        );
    }
    let first_image = image(&document, pages[0]);
    let expected_rgb: Vec<_> = PIXELS
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect();
    assert_eq!(first_image.decompressed_content().unwrap(), expected_rgb);
    let mask = document
        .dereference(first_image.dict.get(b"SMask").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap();
    assert_eq!(
        mask.decompressed_content().unwrap(),
        PIXELS.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>()
    );
    assert_eq!(image(&document, pages[1]).content, jpeg_bytes);
    assert_eq!(std::fs::read(&first).unwrap(), png_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), jpeg_bytes);
    let reversed = dir.path().join("reversed.pdf");
    data(&convert(&[&second, &first], &reversed, false));
    let reversed = Document::load(reversed).unwrap();
    assert_eq!(
        image(&reversed, reversed.get_pages()[&1]).content,
        jpeg_bytes
    );
}

#[test]
fn dry_run_and_no_clobber_validate_without_changing_inputs_or_destination() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("output.pdf");
    let bytes = png(3, 2, &PIXELS);
    std::fs::write(&input, &bytes).unwrap();
    assert_eq!(data(&convert(&[&input], &output, true))["dry_run"], true);
    assert!(!output.exists());
    std::fs::write(&output, b"existing destination").unwrap();
    error(&convert(&[&input], &output, false), 3, "filesystem");
    assert_eq!(std::fs::read(&output).unwrap(), b"existing destination");
    error(&convert(&[&input], &input, false), 3, "filesystem");
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    #[cfg(unix)]
    {
        let input_alias = dir.path().join("input-alias.png");
        std::os::unix::fs::symlink(&input, &input_alias).unwrap();
        assert_eq!(
            data(&convert(&[&input_alias], &output, true))["page_count"],
            1
        );
        let alias = dir.path().join("output-alias.pdf");
        std::os::unix::fs::symlink(&input, &alias).unwrap();
        error(&convert(&[&input], &alias, false), 3, "filesystem");
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}

#[test]
fn unsupported_decode_io_and_nonregular_inputs_have_typed_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.bin");
    let output = dir.path().join("output.pdf");
    std::fs::write(&input, b"GIF89a").unwrap();
    error(&convert(&[&input], &output, false), 4, "refused");
    std::fs::write(&input, b"\x89PNG\r\n\x1a\n").unwrap();
    error(&convert(&[&input], &output, true), 1, "document");
    let mut exif = jpeg();
    exif.splice(2..2, [0xff, 0xe1, 0, 8, b'E', b'x', b'i', b'f', 0, 0]);
    std::fs::write(&input, &exif).unwrap();
    error(&convert(&[&input], &output, false), 4, "refused");
    error(
        &convert(&[&dir.path().join("missing")], &output, false),
        3,
        "filesystem",
    );
    error(&convert(&[dir.path()], &output, false), 4, "refused");
    #[cfg(unix)]
    {
        error(
            &convert(&[Path::new("/dev/null")], &output, false),
            4,
            "refused",
        );
        let fifo = dir.path().join("fifo");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        error(&convert(&[&fifo], &output, false), 4, "refused");
    }
    assert!(!output.exists());
}

#[test]
fn limits_preflight_all_files_before_decoding_and_reject_large_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.pdf");
    let oversized = dir.path().join("oversized.png");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(pdf_core::MAX_STREAM_BYTES as u64 + 1)
        .unwrap();
    error(&convert(&[&oversized], &output, false), 5, "resource_limit");
    let files: Vec<_> = (0..5)
        .map(|index| dir.path().join(format!("{index}.png")))
        .collect();
    for file in &files {
        std::fs::File::create(file)
            .unwrap()
            .set_len(pdf_core::MAX_STREAM_BYTES as u64)
            .unwrap();
    }
    error(
        &convert(
            &files.iter().map(|p| p.as_path()).collect::<Vec<_>>(),
            &output,
            true,
        ),
        5,
        "resource_limit",
    );
    std::fs::write(&oversized, png(8193, 1, &vec![0; 8193 * 4])).unwrap();
    error(&convert(&[&oversized], &output, false), 5, "resource_limit");
    assert!(!output.exists());
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let binary = if name == "qpdf" {
        std::path::PathBuf::from(
            std::env::var_os("PDF_REFERENCE_QPDF").unwrap_or_else(|| name.into()),
        )
    } else {
        std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
            || name.into(),
            |dir| std::path::PathBuf::from(dir).join(name),
        )
    };
    let output = Command::new(binary).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if args == ["-v"] {
        [output.stdout, output.stderr].concat()
    } else {
        if name == "pdftoppm" {
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        output.stdout
    }
}

#[test]
#[ignore = "requires independent image-enabled Poppler26.10.0 and qpdf12.4.2"]
fn independent_readers_verify_cli_geometry_samples_alpha_and_original_jpeg() {
    assert!(
        String::from_utf8(reference("pdftoppm", &["-v"]))
            .unwrap()
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    assert!(
        String::from_utf8(reference("qpdf", &["--version"]))
            .unwrap()
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.jpg");
    let output = dir.path().join("images.pdf");
    std::fs::write(&first, png(3, 2, &PIXELS)).unwrap();
    std::fs::write(&second, jpeg()).unwrap();
    data(&convert(&[&first, &second], &output, false));
    let path = output.to_str().unwrap();
    reference("qpdf", &["--check", path]);
    let model: Value = serde_json::from_slice(&reference("qpdf", &["--json", path])).unwrap();
    assert_eq!(model["pages"].as_array().unwrap().len(), 2);
    let document = Document::load(&output).unwrap();
    for (id, object) in &document.objects {
        if let Ok(stream) = object.as_stream() {
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Image") {
                continue;
            }
            let object_arg = format!("--show-object={},{}", id.0, id.1);
            if stream.dict.get(b"Filter").and_then(Object::as_name).ok() == Some(b"DCTDecode") {
                assert_eq!(
                    reference("qpdf", &[&object_arg, "--raw-stream-data", path]),
                    jpeg()
                );
            } else {
                assert_eq!(
                    reference("qpdf", &[&object_arg, "--filtered-stream-data", path]),
                    stream.decompressed_content().unwrap()
                );
            }
        }
    }
    for (page, (width, height)) in [(1, (3, 2)), (2, (2, 2))] {
        let number = page.to_string();
        let info = String::from_utf8(reference(
            "pdfinfo",
            &["-f", &number, "-l", &number, "-box", path],
        ))
        .unwrap();
        let media = info
            .lines()
            .find(|line| line.contains("MediaBox:"))
            .unwrap();
        let values: Vec<f64> = media
            .split_once("MediaBox:")
            .unwrap()
            .1
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(values, vec![0.0, 0.0, f64::from(width), f64::from(height)]);
        let ppm = reference(
            "pdftoppm",
            &["-singlefile", "-f", &number, "-cropbox", "-r", "576", path],
        );
        let mut lines = ppm.splitn(4, |b| *b == b'\n');
        assert_eq!(lines.next(), Some(b"P6".as_slice()));
        assert_eq!(
            lines.next().unwrap(),
            format!("{} {}", width * 8, height * 8).as_bytes()
        );
        assert_eq!(lines.next(), Some(b"255".as_slice()));
        let rgb = lines.next().unwrap();
        for y in 0..height as usize {
            for x in 0..width as usize {
                let offset = ((y * 8 + 4) * width as usize * 8 + x * 8 + 4) * 3;
                for channel in 0..3 {
                    let expected = if page == 2 {
                        128
                    } else {
                        let sample = &PIXELS[(y * width as usize + x) * 4..];
                        (u32::from(sample[channel]) * u32::from(sample[3])
                            + 255 * (255 - u32::from(sample[3]))
                            + 127)
                            / 255
                    };
                    assert!(
                        (i64::from(rgb[offset + channel]) - i64::from(expected)).abs() <= 1,
                        "page {page}, pixel {x},{y}, channel {channel}: {} != {expected}",
                        rgb[offset + channel]
                    );
                }
            }
        }
    }
}
