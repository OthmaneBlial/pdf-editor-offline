#![allow(clippy::unwrap_used)]
#[path = "../../../tests/support/split_fixture.rs"]
mod support;
use serde_json::Value;
fn cli(args: &[&std::ffi::OsStr]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn actual_cli_dry_run_publishes_deterministic_verified_chunks_and_never_clobbers() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    let directory = temp.path().join("new");
    let bytes = support::fixture(5);
    std::fs::write(&source, &bytes).unwrap();
    let args = [
        std::ffi::OsStr::new("split"),
        source.as_os_str(),
        std::ffi::OsStr::new("--every"),
        std::ffi::OsStr::new("2"),
        std::ffi::OsStr::new("--output-dir"),
        directory.as_os_str(),
        std::ffi::OsStr::new("--json"),
    ];
    let mut dry = args.to_vec();
    dry.push(std::ffi::OsStr::new("--dry-run"));
    let result = cli(&dry);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!directory.exists());
    let json: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(json["data"]["outputs"].as_array().unwrap().len(), 3);
    assert_eq!(json["data"]["privacy_cleanup"], false);
    let result = cli(&args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let first = std::fs::read(directory.join("part-0001.pdf")).unwrap();
    for (index, count) in [2, 2, 1].into_iter().enumerate() {
        assert_eq!(
            pdf_core::PdfDocument::open(directory.join(format!("part-{:04}.pdf", index + 1)), None)
                .unwrap()
                .page_count(),
            count
        );
    }
    let retry = cli(&args);
    assert_eq!(retry.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&retry.stdout).unwrap()["error"]["kind"],
        "filesystem"
    );
    assert_eq!(
        std::fs::read(directory.join("part-0001.pdf")).unwrap(),
        first
    );
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}
#[test]
fn usage_signed_and_structural_refusals_create_no_directory() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("new");
    for (bytes, every, exit, kind) in [
        (support::fixture(2), "0", 2, "arguments"),
        (
            include_bytes!("../../../tests/fixtures/pdfs/signed.pdf").to_vec(),
            "1",
            4,
            "refused",
        ),
        (
            include_bytes!("../../../tests/fixtures/pdfs/bookmarks.pdf").to_vec(),
            "1",
            4,
            "refused",
        ),
    ] {
        let source = temp.path().join("source.pdf");
        std::fs::write(&source, &bytes).unwrap();
        let result = cli(&[
            "split".as_ref(),
            source.as_os_str(),
            "--every".as_ref(),
            every.as_ref(),
            "--output-dir".as_ref(),
            directory.as_os_str(),
            "--json".as_ref(),
        ]);
        assert_eq!(result.status.code(), Some(exit));
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["kind"],
            kind
        );
        assert!(!directory.exists());
        assert_eq!(std::fs::read(source).unwrap(), bytes);
    }
}

#[test]
#[ignore = "requires pinned qpdf12.4.2 and Poppler26.10"]
fn independent_readers_verify_every_published_chunk_text_images_rotation_geometry_and_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    let directory = temp.path().join("new");
    let bytes = support::fixture(5);
    std::fs::write(&source, &bytes).unwrap();
    let result = cli(&[
        "split".as_ref(),
        source.as_os_str(),
        "--every".as_ref(),
        "2".as_ref(),
        "--output-dir".as_ref(),
        directory.as_os_str(),
        "--json".as_ref(),
    ]);
    assert!(result.status.success());
    let qpdf = std::env::var_os("PDF_QPDF_BIN").unwrap();
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let original = pdf_core::PdfDocument::from_bytes(&bytes, None)
        .unwrap()
        .info()
        .unwrap();
    let run = |tool: &str, path: &std::path::Path, page: u32| {
        let mut command = std::process::Command::new(bin.join(tool));
        command.args(["-f", &page.to_string(), "-l", &page.to_string()]);
        if tool == "pdftoppm" {
            command.args(["-r", "72", "-singlefile"]);
        }
        command.arg(path);
        if tool == "pdftotext" {
            command.arg("-");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let mut source_page = 1;
    for part in 1..=3 {
        let path = directory.join(format!("part-{part:04}.pdf"));
        assert!(
            std::process::Command::new(&qpdf)
                .arg("--check")
                .arg(&path)
                .output()
                .unwrap()
                .status
                .success()
        );
        let doc = pdf_core::PdfDocument::open(&path, None).unwrap();
        for page in 1..=doc.page_count() as u32 {
            assert_eq!(
                run("pdftotext", &source, source_page),
                run("pdftotext", &path, page)
            );
            assert_eq!(
                run("pdftoppm", &source, source_page),
                run("pdftoppm", &path, page)
            );
            let mut expected = original.pages[source_page as usize - 1].clone();
            expected.number = page;
            assert_eq!(doc.info().unwrap().pages[page as usize - 1], expected);
            let before = pdf_render::render_page(&bytes, source_page as usize - 1, 1.0).unwrap();
            let after =
                pdf_render::render_page(&std::fs::read(&path).unwrap(), page as usize - 1, 1.0)
                    .unwrap();
            assert_eq!(before.rgba, after.rgba);
            source_page += 1;
        }
    }
    assert_eq!(source_page, 6);
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}
