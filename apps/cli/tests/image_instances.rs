#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::process::Command;
#[path = "../../../tests/support/image_fixture.rs"]
mod image;
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
fn run(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn actual_cli_source_bound_png_jpeg_replacement_dry_run_preservation_and_refusals() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    let source = support::fixture(2);
    std::fs::write(&input, &source).unwrap();
    let listed = run(&[
        "list-image-instances".as_ref(),
        input.as_os_str(),
        "--page".as_ref(),
        "1".as_ref(),
    ]);
    assert!(listed.status.success());
    let json: Value = serde_json::from_slice::<Value>(&listed.stdout).unwrap()["data"].clone();
    let selection = temp.path().join("selection.json");
    std::fs::write(
        &selection,
        serde_json::to_vec(&json["instances"][0]["selection"]).unwrap(),
    )
    .unwrap();
    for (name, data) in [
        ("alpha.png", {
            let mut bytes = vec![];
            {
                let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&[0, 0, 255, 255, 255, 0, 0, 0])
                    .unwrap();
            }
            bytes
        }),
        ("gray.jpg", image::jpeg()),
    ] {
        let asset = temp.path().join(name);
        std::fs::write(&asset, &data).unwrap();
        let output = temp.path().join(format!("{name}.pdf"));
        let args = [
            "replace-image".as_ref(),
            input.as_os_str(),
            "--image".as_ref(),
            asset.as_os_str(),
            "--selection-file".as_ref(),
            selection.as_os_str(),
            "-o".as_ref(),
            output.as_os_str(),
        ];
        let mut dry = args.to_vec();
        dry.push("--dry-run".as_ref());
        let result = run(&dry);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert!(!output.exists());
        let result = run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let saved = std::fs::read(&output).unwrap();
        let pdf = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
        let original = pdf_core::PdfDocument::from_bytes(&source, None).unwrap();
        assert_eq!(original.info().unwrap().pages, pdf.info().unwrap().pages);
        assert_eq!(
            original.extract_text(&[1, 2]).unwrap(),
            pdf.extract_text(&[1, 2]).unwrap()
        );
        assert_eq!(
            pdf_render::render_page(&source, 1, 1.0).unwrap().rgba,
            pdf_render::render_page(&saved, 1, 1.0).unwrap().rgba
        );
        assert_eq!(run(&args).status.code(), Some(3));
        assert_eq!(std::fs::read(&output).unwrap(), saved);
        assert_eq!(std::fs::read(&asset).unwrap(), data);
    }
    let bad = temp.path().join("bad.gif");
    std::fs::write(&bad, b"GIF89a").unwrap();
    let output = temp.path().join("bad.pdf");
    let args = [
        "replace-image".as_ref(),
        input.as_os_str(),
        "--image".as_ref(),
        bad.as_os_str(),
        "--selection-file".as_ref(),
        selection.as_os_str(),
        "-o".as_ref(),
        output.as_os_str(),
    ];
    assert_eq!(run(&args).status.code(), Some(4));
    assert!(!output.exists());
    let mut stale = json["instances"][0]["selection"].clone();
    stale["source_hash"] = Value::String("0".repeat(64));
    std::fs::write(&selection, serde_json::to_vec(&stale).unwrap()).unwrap();
    std::fs::write(&bad, image::jpeg()).unwrap();
    assert_eq!(run(&args).status.code(), Some(4));
    assert!(!output.exists());
    assert_eq!(std::fs::read(&input).unwrap(), source);
}
