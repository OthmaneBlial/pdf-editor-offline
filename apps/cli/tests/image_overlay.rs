#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};
#[path = "../../../tests/support/image_fixture.rs"]
mod image;
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn png() -> Vec<u8> {
    let mut bytes = vec![];
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255, 0, 0, 255, 0])
            .unwrap();
    }
    bytes
}
fn insert(input: &Path, image: &Path, output: &Path, dry: bool, rotation: u16) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"));
    command
        .arg("add-image-overlay")
        .arg(input)
        .arg("--image")
        .arg(image)
        .args([
            "--page",
            "1",
            "--rect",
            "60",
            "30",
            "100",
            "70",
            "--rotation",
            &rotation.to_string(),
            "-o",
        ])
        .arg(output)
        .arg("--json");
    if dry {
        command.arg("--dry-run");
    }
    command.output().unwrap()
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}
fn error(output: Output, code: i32, kind: &str) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["error"]["kind"],
        kind
    );
}
#[test]
fn real_child_png_jpeg_geometry_alpha_dry_run_no_clobber_and_source_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let source = support::fixture(2);
    std::fs::write(&input, &source).unwrap();
    for (name, image, alpha) in [
        ("image.png", png(), true),
        ("image.jpg", image::jpeg(), false),
    ] {
        let path = temp.path().join(name);
        std::fs::write(&path, &image).unwrap();
        let output = temp.path().join(format!("{name}.pdf"));
        let dry = success(insert(&input, &path, &output, true, 90));
        assert_eq!(dry["dry_run"], true);
        assert_eq!(dry["image"]["alpha_mask"], alpha);
        assert!(!output.exists());
        let actual = success(insert(&input, &path, &output, false, 90));
        assert_eq!(actual["existing_images_replaced"], false);
        assert_eq!(actual["info"]["page_count"], 2);
        let published = std::fs::read(&output).unwrap();
        let before = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
        let after = pdf_core::PdfDocument::open(&output, None).unwrap();
        assert_eq!(before.info().unwrap().pages, after.info().unwrap().pages);
        assert_eq!(
            before.extract_text(&[1, 2]).unwrap(),
            after.extract_text(&[1, 2]).unwrap()
        );
        error(insert(&input, &path, &output, false, 0), 3, "filesystem");
        assert_eq!(std::fs::read(&output).unwrap(), published);
        assert_eq!(std::fs::read(&path).unwrap(), image);
    }
    assert_eq!(std::fs::read(input).unwrap(), source);
}
#[test]
fn readonly_unsupported_decode_input_limits_and_rotation_refuse_without_publication() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    std::fs::write(&input, support::fixture(2)).unwrap();
    let path = temp.path().join("image");
    let output = temp.path().join("output.pdf");
    std::fs::write(&path, b"GIF89a").unwrap();
    error(insert(&input, &path, &output, false, 0), 4, "refused");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\n").unwrap();
    error(insert(&input, &path, &output, false, 0), 1, "document");
    std::fs::write(&path, image::jpeg()).unwrap();
    error(insert(&input, &path, &output, false, 45), 1, "document");
    let signed = temp.path().join("signed.pdf");
    std::fs::write(
        &signed,
        include_bytes!("../../../tests/fixtures/pdfs/signed.pdf"),
    )
    .unwrap();
    error(insert(&signed, &path, &output, false, 0), 4, "refused");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(pdf_core::MAX_STREAM_BYTES as u64 + 1)
        .unwrap();
    error(
        insert(&input, &path, &output, false, 0),
        5,
        "resource_limit",
    );
    assert!(!output.exists());
}
#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn production_cli_inserted_png_jpeg_copies_pass_independent_readers() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let source = support::fixture(2);
    std::fs::write(&input, &source).unwrap();
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let reference = |tool: &str, path: &Path| {
        let mut command = Command::new(bin.join(tool));
        command.args(["-f", "1", "-l", "1"]);
        if tool == "pdftoppm" {
            command.args(["-r", "72", "-singlefile"]);
        }
        command.arg(path);
        if tool == "pdftotext" {
            command.arg("-");
        }
        let result = command.output().unwrap();
        assert!(result.status.success());
        result.stdout
    };
    let ppm = |bytes: Vec<u8>| {
        let marker = b"\n255\n";
        let start = bytes
            .windows(marker.len())
            .position(|part| part == marker)
            .unwrap()
            + marker.len();
        (bytes[..start].to_vec(), bytes[start..].to_vec())
    };
    let (header, before) = ppm(reference("pdftoppm", &input));
    for (index, image) in [png(), image::jpeg()].into_iter().enumerate() {
        let path = temp.path().join(format!("image-{index}"));
        std::fs::write(&path, &image).unwrap();
        for rotation in [0, 90, 180, 270] {
            let output = temp.path().join(format!("{index}-{rotation}.pdf"));
            success(insert(&input, &path, &output, false, rotation));
            assert!(
                Command::new(&qpdf)
                    .arg("--check")
                    .arg(&output)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            assert_eq!(
                reference("pdftotext", &input),
                reference("pdftotext", &output)
            );
            let (after_header, after) = ppm(reference("pdftoppm", &output));
            assert_eq!(after_header, header);
            let mut changed = 0;
            for (index, (a, b)) in before
                .chunks_exact(3)
                .zip(after.chunks_exact(3))
                .enumerate()
            {
                let x = index % 200;
                let y = index / 200;
                if (59..=101).contains(&x) && (229..=271).contains(&y) {
                    changed += usize::from(a != b);
                } else {
                    assert_eq!(a, b);
                }
            }
            assert!(changed > 500);
        }
        assert_eq!(std::fs::read(path).unwrap(), image);
    }
    assert_eq!(std::fs::read(input).unwrap(), source);
}
