#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::PdfDocument;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args(args)
        .output()
        .unwrap()
}

fn fixture(rotation: i64) -> Document {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let content = document.add_object(Stream::new(
        dictionary! {},
        b"q 1 0 0 rg 20 30 15 60 re f Q".to_vec(),
    ));
    let page = document
        .add_object(dictionary! {"Type" => "Page", "Parent" => pages, "Contents" => content});
    document.objects.insert(
        pages,
        dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()],
        "MediaBox" => vec![10.into(),20.into(),130.into(),140.into()],
        "CropBox" => vec![20.into(),30.into(),120.into(),130.into()],
            "Rotate" => rotation, "Resources" => dictionary! {},
        }
        .into(),
    );
    let root = document.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    document.trailer.set("Root", root);
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("synthetic-first!"),
            Object::string_literal("synthetic-next!!"),
        ],
    );
    document
}

fn save(mut document: Document, path: &Path) {
    document.save(path).unwrap();
}

fn add(input: &Path, output: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "add-note",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--rectangle",
        "50",
        "50",
        "74",
        "74",
        "--contents",
        "café 中文 😀\nReal comment",
        "--author",
        "Élodie",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ];
    args.extend(extra);
    cli(&args)
}

#[test]
fn add_and_list_use_actual_saved_unicode_notes_without_changing_original() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("original.pdf");
    let output = directory.path().join("note.pdf");
    save(fixture(0), &input);
    let original = std::fs::read(&input).unwrap();
    let result = add(&input, &output, &[]);
    assert!(result.status.success(), "{result:?}");
    let added: Value = serde_json::from_slice(&result.stdout).unwrap();
    let result = cli(&[
        "list-notes",
        output.to_str().unwrap(),
        "--pages",
        "1",
        "--json",
    ]);
    assert!(result.status.success());
    let listed: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(listed["data"]["notes"][0], added["data"]["note"]);
    assert_eq!(
        listed["data"]["notes"][0]["contents"],
        "café 中文 😀\nReal comment"
    );
    assert_eq!(listed["data"]["notes"][0]["flags"], 4);
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[test]
fn note_dry_run_invalid_arguments_geometry_limits_and_no_clobber_are_atomic() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("original.pdf");
    let output = directory.path().join("note.pdf");
    save(fixture(0), &input);
    let original = std::fs::read(&input).unwrap();
    assert!(add(&input, &output, &["--dry-run"]).status.success());
    assert!(!output.exists());
    for (rectangle, page, contents, expected) in [
        (["50", "50", "74", "74"], "0", "x", 1),
        (["0", "0", "24", "24"], "1", "x", 1),
        (["NaN", "50", "74", "74"], "1", "x", 1),
        (["50", "50", "74", "74"], "1", "x", 0),
    ] {
        let mut args = vec![
            "add-note",
            input.to_str().unwrap(),
            "--page",
            page,
            "--rectangle",
        ];
        args.extend(rectangle);
        args.extend([
            "--contents",
            contents,
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(cli(&args).status.code(), Some(expected));
        if expected == 0 {
            std::fs::remove_file(&output).unwrap();
        }
        assert!(!output.exists());
    }
    let oversized = "x".repeat(pdf_core::MAX_NOTE_CONTENT_BYTES + 1);
    let result = cli(&[
        "add-note",
        input.to_str().unwrap(),
        "--page",
        "1",
        "--rectangle",
        "50",
        "50",
        "74",
        "74",
        "--contents",
        &oversized,
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(5));
    assert!(!output.exists());
    assert_eq!(
        cli(&[
            "add-note",
            input.to_str().unwrap(),
            "--page",
            "1",
            "--rectangle",
            "50",
            "50",
            "74",
            "--contents",
            "x",
            "-o",
            output.to_str().unwrap(),
            "--json"
        ])
        .status
        .code(),
        Some(2)
    );
    std::fs::write(&output, b"sentinel").unwrap();
    assert_eq!(add(&input, &output, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).unwrap(), b"sentinel");
    assert_eq!(add(&input, &input, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn note_output_symlink_is_never_followed() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("original.pdf");
    let target = directory.path().join("target.pdf");
    let output = directory.path().join("linked.pdf");
    save(fixture(0), &input);
    std::fs::write(&target, b"sentinel").unwrap();
    std::os::unix::fs::symlink(&target, &output).unwrap();
    assert_eq!(add(&input, &output, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(target).unwrap(), b"sentinel");
    assert!(output.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn signed_and_decrypted_v2_inputs_allow_listing_but_refuse_note_export() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("note.pdf");
    let signed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1/signed.pdf");
    let original = std::fs::read(&signed).unwrap();
    assert!(
        cli(&["list-notes", signed.to_str().unwrap(), "--json"])
            .status
            .success()
    );
    assert_eq!(add(&signed, &output, &[]).status.code(), Some(4));
    assert_eq!(std::fs::read(&signed).unwrap(), original);
    assert!(!output.exists());
    let input = directory.path().join("encrypted.pdf");
    let mut source = fixture(0);
    let page = source.get_pages()[&1];
    source.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Dictionary(dictionary! {"Subtype"=>"Text", "Rect"=>vec![50.into(),50.into(),74.into(),74.into()], "Contents"=>lopdf::text_string("Encrypted café 中文")})]);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &source,
        owner_password: "owner",
        user_password: "synthetic-password",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    source.encrypt(&state).unwrap();
    save(source, &input);
    let original = std::fs::read(&input).unwrap();
    assert!(
        PdfDocument::open(&input, Some("synthetic-password"))
            .unwrap()
            .info()
            .unwrap()
            .encrypted
    );
    for (operation, password, expected) in [
        ("list-notes", "synthetic-password", 0),
        ("add-note", "synthetic-password", 4),
        ("add-note", "wrong-secret", 1),
    ] {
        let mut args = vec![
            operation,
            input.to_str().unwrap(),
            "--json",
            "--password-stdin",
        ];
        if operation == "add-note" {
            args.extend([
                "--page",
                "1",
                "--rectangle",
                "50",
                "50",
                "74",
                "74",
                "--contents",
                "x",
                "-o",
                output.to_str().unwrap(),
            ]);
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
            .args(args)
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
        assert_eq!(result.status.code(), Some(expected), "{result:?}");
        if operation == "list-notes" {
            assert_eq!(
                serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"]["notes"][0]["contents"],
                "Encrypted café 中文"
            );
        }
        assert!(!String::from_utf8_lossy(&result.stdout).contains(password));
        assert!(!String::from_utf8_lossy(&result.stderr).contains(password));
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
}

fn device(point: [usize; 2], rotation: i64) -> [usize; 2] {
    let [x, y] = point;
    match rotation {
        0 => [x - 20, 130 - y],
        90 => [y - 30, x - 20],
        180 => [120 - x, y - 30],
        270 => [130 - y, 120 - x],
        _ => unreachable!(),
    }
}

fn assert_icon(
    before: &[u8],
    after: &[u8],
    width: usize,
    channels: usize,
    rotation: i64,
    upright: bool,
) {
    assert_eq!(before.len(), after.len());
    // Poppler 26.10 AnnotText forces NoRotate; Gfx pivots around the upper-left.
    // Source: poppler Annot.cc/Gfx.cc at 2703be11769c1f86f4d0dc3b276ec94bba5ac6d3.
    // Hayro instead applies page rotation to the entire appearance. Test both exact footprints.
    let a = if upright {
        device([50, 74], rotation)
    } else {
        device([50, 50], rotation)
    };
    let b = if upright {
        [a[0] + 24, a[1] + 24]
    } else {
        device([74, 74], rotation)
    };
    let x0 = a[0].min(b[0]).saturating_sub(1);
    let x1 = a[0].max(b[0]) + 1;
    let y0 = a[1].min(b[1]).saturating_sub(1);
    let y1 = a[1].max(b[1]) + 1;
    let mut changed = 0;
    for (index, (old, new)) in before
        .chunks_exact(channels)
        .zip(after.chunks_exact(channels))
        .enumerate()
    {
        if old != new {
            changed += 1;
            let (x, y) = (index % width, index / width);
            assert!(
                x >= x0 && x <= x1 && y >= y0 && y <= y1,
                "pixel changed outside note: {x},{y}, rotation {rotation}"
            );
        }
    }
    assert!(
        changed > 200,
        "note icon missing: only {changed} pixels changed"
    );
    let [x, y] = if upright {
        [a[0] + 8, a[1] + 7]
    } else {
        device([58, 67], rotation)
    };
    let rgb = &after[(y * width + x) * channels..][..3];
    assert!(
        rgb[0] > 240 && rgb[1] > 200 && rgb[1] < 245 && rgb[2] < 100,
        "note interior not yellow: {rgb:?}"
    );
}

#[test]
fn hayro_displays_note_in_pdf_coordinates_and_preserves_pixels_outside_it() {
    for rotation in [0, 90, 180, 270] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("original.pdf");
        let output = directory.path().join("note.pdf");
        save(fixture(rotation), &input);
        assert!(add(&input, &output, &[]).status.success());
        let before = pdf_render::render_page(&std::fs::read(&input).unwrap(), 0, 1.0).unwrap();
        let after = pdf_render::render_page(&std::fs::read(&output).unwrap(), 0, 1.0).unwrap();
        assert_eq!((before.width, before.height), (after.width, after.height));
        assert_icon(
            &before.rgba,
            &after.rgba,
            before.width as usize,
            4,
            rotation,
            false,
        );
    }
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let path = std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
        || PathBuf::from(name),
        |directory| PathBuf::from(directory).join(name),
    );
    let output = Command::new(path).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if args == ["-v"] {
        [output.stdout, output.stderr].concat()
    } else {
        output.stdout
    }
}

fn ppm(bytes: &[u8]) -> (usize, Vec<u8>) {
    let mut lines = bytes.splitn(4, |byte| *byte == b'\n');
    assert_eq!(lines.next().unwrap(), b"P6");
    let dimensions = std::str::from_utf8(lines.next().unwrap())
        .unwrap()
        .split_whitespace()
        .map(|number| number.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.next().unwrap(), b"255");
    let pixels = lines.next().unwrap().to_vec();
    assert_eq!(pixels.len(), dimensions[0] * dimensions[1] * 3);
    (dimensions[0], pixels)
}

#[test]
#[ignore = "requires independent Poppler 26.10.0 reference executables"]
fn poppler_displays_the_vector_icon_and_preserves_original_page_content() {
    let version = String::from_utf8(reference("pdftoppm", &["-v"])).unwrap();
    assert!(
        version
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    for rotation in [0, 90, 180, 270] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("original.pdf");
        let output = directory.path().join("note.pdf");
        save(fixture(rotation), &input);
        assert!(add(&input, &output, &[]).status.success());
        let raster = |path: &Path| {
            ppm(&reference(
                "pdftoppm",
                &[
                    "-singlefile",
                    "-cropbox",
                    "-r",
                    "72",
                    path.to_str().unwrap(),
                ],
            ))
        };
        let (width, before) = raster(&input);
        let (after_width, after) = raster(&output);
        assert_eq!(width, after_width);
        // Exact upright footprint, not Hayro parity. The central fixture avoids crop clipping.
        assert_icon(&before, &after, width, 3, rotation, true);
        assert_eq!(
            reference("pdftotext", &[input.to_str().unwrap(), "-"]),
            reference("pdftotext", &[output.to_str().unwrap(), "-"])
        );
    }
}
