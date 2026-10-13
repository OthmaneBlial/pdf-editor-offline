#![allow(clippy::expect_used)]
use lopdf::dictionary;
use pdf_core::{FormValue, PdfDocument};
use std::process::{Command, Output};
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] = include_bytes!("../../desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn fill(
    input: &std::path::Path,
    font: &std::path::Path,
    output: &std::path::Path,
    extra: &[&str],
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args([
            "fill-text",
            input.to_str().expect("test path"),
            "--field",
            "full_name",
            "--value",
            "Ada é",
            "--font-file",
            font.to_str().expect("font path"),
            "-o",
            output.to_str().expect("output path"),
            "--json",
        ])
        .args(extra)
        .output()
        .expect("actual CLI child")
}
#[test]
fn cli_text_dry_run_save_noclobber_and_invalid_font_are_real() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let input = temp.path().join("input.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("output.pdf");
    std::fs::write(&input, PDF).expect("fixture");
    std::fs::write(&font, FONT).expect("font");
    let dry = fill(&input, &font, &output, &["--dry-run"]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert!(!output.exists());
    let saved = fill(&input, &font, &output, &[]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stdout)
    );
    let doc = PdfDocument::open(&output, None).expect("actual saved PDF");
    assert!(
        doc.extract_text(&[1])
            .expect("appearance")
            .contains("Ada é")
    );
    assert_eq!(
        doc.list_form_fields()
            .expect("fields")
            .into_iter()
            .find(|f| f.name == "full_name")
            .expect("field")
            .value,
        FormValue::Text("Ada é".into())
    );
    let bytes = std::fs::read(&output).expect("saved bytes");
    assert_eq!(fill(&input, &font, &output, &[]).status.code(), Some(3));
    assert_eq!(std::fs::read(&output).expect("unchanged output"), bytes);
    std::fs::write(&font, b"not a font").expect("corrupt font");
    let refused = temp.path().join("refused.pdf");
    assert!(!fill(&input, &font, &refused, &[]).status.success());
    assert!(!refused.exists());
    assert_eq!(std::fs::read(&input).expect("source unchanged"), PDF);
}

#[test]
fn signed_and_readonly_text_never_publish_and_font_reads_are_bounded() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let input = temp.path().join("signed.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("refused.pdf");
    std::fs::write(&font, FONT).expect("font");
    let mut signed = lopdf::Document::load_mem(PDF).expect("fixture");
    signed.add_object(dictionary! {"FT"=>"Sig","Type"=>"Annot","Subtype"=>"Widget"});
    signed.save(&input).expect("signed marker fixture");
    let source = std::fs::read(&input).expect("source bytes");
    assert_eq!(fill(&input, &font, &output, &[]).status.code(), Some(4));
    assert!(!output.exists());
    assert_eq!(std::fs::read(&input).expect("source unchanged"), source);
    let mut readonly = lopdf::Document::load_mem(PDF).expect("fixture");
    readonly
        .get_object_mut((8, 0))
        .expect("text widget")
        .as_dict_mut()
        .expect("widget dictionary")
        .set("Ff", 1);
    readonly.save(&input).expect("read-only fixture");
    assert_eq!(fill(&input, &font, &output, &[]).status.code(), Some(4));
    assert!(!output.exists());
    std::fs::write(&input, PDF).expect("fixture");
    std::fs::File::create(&font)
        .expect("large font")
        .set_len(16 * 1024 * 1024 + 1)
        .expect("sparse oversized font");
    assert_eq!(fill(&input, &font, &output, &[]).status.code(), Some(5));
    assert!(!output.exists());
}

#[test]
fn encrypted_text_with_correct_password_is_still_read_only_without_password_echo() {
    use std::io::Write;
    use std::process::Stdio;
    let temp = tempfile::tempdir().expect("temporary directory");
    let input = temp.path().join("encrypted.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("refused.pdf");
    std::fs::write(&font, FONT).expect("font");
    let mut document = lopdf::Document::load_mem(PDF).expect("fixture");
    document.trailer.set(
        "ID",
        vec![
            lopdf::Object::string_literal("text-id"),
            lopdf::Object::string_literal("text-id"),
        ],
    );
    let encryption = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &document,
        owner_password: "owner",
        user_password: "private-reader",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .expect("test encryption");
    document.encrypt(&encryption).expect("encrypt");
    document.save(&input).expect("encrypted fixture");
    let original = std::fs::read(&input).expect("source bytes");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .args([
            "fill-text",
            input.to_str().expect("path"),
            "--field",
            "full_name",
            "--value",
            "Ada",
            "--font-file",
            font.to_str().expect("font path"),
            "-o",
            output.to_str().expect("output path"),
            "--password-stdin",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("actual child");
    child
        .stdin
        .take()
        .expect("password pipe")
        .write_all(b"private-reader\n")
        .expect("password stdin");
    let rejected = child.wait_with_output().expect("bounded CLI result");
    assert_eq!(rejected.status.code(), Some(4));
    assert!(!output.exists());
    assert!(!String::from_utf8_lossy(&rejected.stdout).contains("private-reader"));
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private-reader"));
    assert_eq!(std::fs::read(input).expect("source unchanged"), original);
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn actual_cli_text_copy_matches_independent_text_geometry_and_visible_widget() {
    let poppler = std::path::PathBuf::from(
        std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("pinned Poppler directory"),
    );
    let qpdf = std::env::var_os("PDF_QPDF_BIN").expect("pinned qpdf");
    let reference = |binary: &std::path::Path, args: &[&str]| {
        let output = Command::new(binary)
            .args(args)
            .output()
            .expect("independent reader");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    let version = reference(&poppler.join("pdftotext"), &["-v"]);
    assert!(
        String::from_utf8_lossy(&version.stderr)
            .lines()
            .any(|line| line == "pdftotext version 26.10.0")
    );
    let version = reference(std::path::Path::new(&qpdf), &["--version"]);
    assert!(
        String::from_utf8_lossy(&version.stdout)
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let temp = tempfile::tempdir().expect("temporary directory");
    let input = temp.path().join("input.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("output.pdf");
    std::fs::write(&input, PDF).expect("source");
    std::fs::write(&font, FONT).expect("font");
    let filled = fill(&input, &font, &output, &[]);
    assert!(
        filled.status.success(),
        "{}",
        String::from_utf8_lossy(&filled.stdout)
    );
    reference(
        std::path::Path::new(&qpdf),
        &["--check", output.to_str().expect("output path")],
    );
    let model: serde_json::Value = serde_json::from_slice(
        &reference(
            std::path::Path::new(&qpdf),
            &["--json", output.to_str().expect("output path")],
        )
        .stdout,
    )
    .expect("independent JSON");
    assert_eq!(model["qpdf"][1]["obj:8 0 R"]["value"]["/V"], "u:Ada é");
    let text = |path: &std::path::Path| {
        String::from_utf8(
            reference(
                &poppler.join("pdftotext"),
                &["-raw", path.to_str().expect("path"), "-"],
            )
            .stdout,
        )
        .expect("UTF-8 reference text")
    };
    let before = text(&input);
    assert_eq!(before.matches("Synthetic value").count(), 1);
    assert_eq!(
        text(&output),
        before.replacen("Synthetic value", "Ada é", 1)
    );
    let pixels = |path: &std::path::Path| {
        let bytes = reference(
            &poppler.join("pdftoppm"),
            &[
                "-f",
                "1",
                "-singlefile",
                "-r",
                "72",
                "-png",
                path.to_str().expect("path"),
            ],
        )
        .stdout;
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .expect("reference PNG");
        let mut pixels = vec![
            0;
            reader
                .output_buffer_size()
                .expect("bounded reference image")
        ];
        let frame = reader.next_frame(&mut pixels).expect("reference raster");
        assert_eq!(
            (frame.width, frame.height, frame.color_type),
            (595, 842, png::ColorType::Rgb)
        );
        pixels.truncate(frame.buffer_size());
        pixels
    };
    let before = pixels(&input);
    let after = pixels(&output);
    let mut changed = 0;
    for (index, (a, b)) in before
        .chunks_exact(3)
        .zip(after.chunks_exact(3))
        .enumerate()
    {
        if a != b {
            let (x, y) = (index % 595, index / 595);
            assert!(
                (72..300).contains(&x) && (90..120).contains(&y),
                "outside text widget {x},{y}"
            );
            changed += 1;
        }
    }
    assert!(changed > 0, "real visible text appearance");
    assert_eq!(std::fs::read(input).expect("source unchanged"), PDF);
}
