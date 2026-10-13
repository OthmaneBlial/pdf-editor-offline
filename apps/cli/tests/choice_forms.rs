#![allow(clippy::expect_used)]
use pdf_core::PdfDocument;
use std::process::{Command, Output};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] = include_bytes!("../../desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
fn fill(
    input: &std::path::Path,
    font: &std::path::Path,
    output: &std::path::Path,
    args: &[&str],
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("fill-choice")
        .arg(input)
        .args(["--field", "priority", "--font-file"])
        .arg(font)
        .arg("-o")
        .arg(output)
        .arg("--json")
        .args(args)
        .output()
        .expect("actual isolated CLI")
}
#[test]
fn indexed_discovery_dry_run_save_clear_and_no_clobber_are_actual() {
    let temp = tempfile::tempdir().expect("temp");
    let input = temp.path().join("input.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("saved.pdf");
    std::fs::write(&input, SOURCE).expect("input");
    std::fs::write(&font, FONT).expect("font");
    let discovered = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("list-choices")
        .arg(&input)
        .arg("--json")
        .output()
        .expect("discovery");
    assert!(discovered.status.success());
    let report: serde_json::Value = serde_json::from_slice(&discovered.stdout).expect("JSON");
    assert_eq!(
        report["data"]["choices"][0]["selected_indices"],
        serde_json::json!([1])
    );
    assert!(
        fill(&input, &font, &output, &["--index", "2", "--dry-run"])
            .status
            .success()
    );
    assert!(!output.exists());
    let saved = fill(&input, &font, &output, &["--index", "2"]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stdout)
    );
    let pdf = PdfDocument::open(&output, None).expect("saved");
    assert!(pdf.extract_text(&[1]).expect("text").contains("High"));
    assert_eq!(
        pdf.list_choice_fields().expect("choices")[0].selected_indices,
        Some(vec![2])
    );
    let before = std::fs::read(&output).expect("savedbytes");
    assert_eq!(
        fill(&input, &font, &output, &["--index", "0"])
            .status
            .code(),
        Some(3)
    );
    assert_eq!(std::fs::read(&output).expect("unchanged"), before);
    assert_eq!(
        fill(&input, &font, &input, &["--index", "2"]).status.code(),
        Some(3)
    );
    let clear = temp.path().join("clear.pdf");
    assert!(fill(&input, &font, &clear, &["--clear"]).status.success());
    assert_eq!(
        PdfDocument::open(&clear, None)
            .expect("cleared")
            .list_choice_fields()
            .expect("choices")[0]
            .selected_indices,
        Some(vec![])
    );
    assert_eq!(std::fs::read(&input).expect("original"), SOURCE);
}
#[test]
fn malformed_indices_fonts_readonly_and_signed_fields_never_publish() {
    let temp = tempfile::tempdir().expect("temp");
    let input = temp.path().join("input.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("refused.pdf");
    std::fs::write(&input, SOURCE).expect("input");
    std::fs::write(&font, FONT).expect("font");
    for args in [
        vec!["--index", "3"],
        vec!["--index", "1", "--index", "1"],
        vec!["--index", "2", "--index", "1"],
        vec!["--index", "0", "--top-index", "0"],
        vec!["--clear", "--index", "1"],
        vec![],
    ] {
        let result = fill(&input, &font, &output, &args);
        assert!(!result.status.success());
        let json: serde_json::Value =
            serde_json::from_slice(&result.stdout).expect("bounded JSONerror");
        assert_eq!(json["ok"], false);
        assert!(!output.exists());
    }
    std::fs::write(&font, b"bad font").expect("badfont");
    assert!(
        !fill(&input, &font, &output, &["--index", "2"])
            .status
            .success()
    );
    assert!(!output.exists());
    std::fs::write(&font, FONT).expect("font");
    for signed in [false, true] {
        let mut doc = lopdf::Document::load_mem(SOURCE).expect("fixture");
        if signed {
            doc.add_object(lopdf::dictionary! {"FT"=>"Sig","Type"=>"Annot","Subtype"=>"Widget"});
        } else {
            doc.get_dictionary_mut((14, 0))
                .expect("field")
                .set("Ff", 1 | (1 << 17));
        }
        doc.save(&input).expect("fixture");
        let original = std::fs::read(&input).expect("bytes");
        assert_eq!(
            fill(&input, &font, &output, &["--index", "2"])
                .status
                .code(),
            Some(4)
        );
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).expect("original"), original);
    }
}

#[test]
fn repeatable_list_indices_and_explicit_top_index_preserve_all_selections() {
    let temp = tempfile::tempdir().expect("temp");
    let input = temp.path().join("list.pdf");
    let font = temp.path().join("font.ttf");
    let output = temp.path().join("saved.pdf");
    let mut source = lopdf::Document::load_mem(SOURCE).expect("fixture");
    let field = source.get_dictionary_mut((14, 0)).expect("field");
    field.set("Ff", 1 << 21);
    field.set("Rect", vec![72.into(), 550.into(), 220.into(), 650.into()]);
    source
        .get_object_mut((15, 0))
        .expect("AP")
        .as_stream_mut()
        .expect("stream")
        .dict
        .set("BBox", vec![0.into(), 0.into(), 148.into(), 100.into()]);
    source.save(&input).expect("input");
    std::fs::write(&font, FONT).expect("font");
    let result = fill(
        &input,
        &font,
        &output,
        &["--index", "0", "--index", "2", "--top-index", "1"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let report = PdfDocument::open(&output, None)
        .expect("saved")
        .list_choice_fields()
        .expect("choices");
    assert_eq!(report[0].selected_indices, Some(vec![0, 2]));
    assert_eq!(report[0].top_index, 1);
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0 and qpdf 12.4.2"]
fn actual_cli_choice_has_exact_poppler_text_values_and_outside_pixels() {
    let bin = std::path::PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("Poppler"));
    let qpdf = std::path::PathBuf::from(std::env::var_os("PDF_QPDF_BIN").expect("qpdf"));
    let reference = |binary: &std::path::Path, args: &[&str]| {
        let output = Command::new(binary).args(args).output().expect("reference");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    assert!(
        String::from_utf8_lossy(&reference(&bin.join("pdftotext"), &["-v"]).stderr)
            .lines()
            .any(|line| line == "pdftotext version 26.10.0")
    );
    assert!(
        String::from_utf8_lossy(&reference(&qpdf, &["--version"]).stdout)
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let temp = tempfile::tempdir().expect("temp");
    let input = temp.path().join("source.pdf");
    let output = temp.path().join("saved.pdf");
    let font = temp.path().join("font.ttf");
    std::fs::write(&input, SOURCE).expect("input");
    std::fs::write(&font, FONT).expect("font");
    assert!(
        fill(&input, &font, &output, &["--index", "2"])
            .status
            .success()
    );
    reference(&qpdf, &["--check", output.to_str().expect("path")]);
    let model: serde_json::Value = serde_json::from_slice(
        &reference(&qpdf, &["--json", output.to_str().expect("path")]).stdout,
    )
    .expect("qpdfJSON");
    assert_eq!(model["qpdf"][1]["obj:14 0 R"]["value"]["/V"], "u:High");
    assert_eq!(
        model["qpdf"][1]["obj:14 0 R"]["value"]["/I"],
        serde_json::json!([2])
    );
    let text = |path: &std::path::Path| {
        String::from_utf8(
            reference(
                &bin.join("pdftotext"),
                &["-raw", path.to_str().expect("path"), "-"],
            )
            .stdout,
        )
        .expect("UTF8")
    };
    let before = text(&input);
    assert_eq!(before.matches("Normal").count(), 1);
    assert_eq!(text(&output), before.replacen("Normal", "High", 1));
    let pixels = |path: &std::path::Path| {
        let bytes = reference(
            &bin.join("pdftoppm"),
            &[
                "-r",
                "72",
                "-png",
                "-singlefile",
                path.to_str().expect("path"),
            ],
        )
        .stdout;
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .expect("PNG");
        assert_eq!((reader.info().width, reader.info().height), (595, 842));
        let mut pixels = vec![0; reader.output_buffer_size().expect("boundedimage")];
        let frame = reader.next_frame(&mut pixels).expect("pixels");
        assert_eq!(frame.color_type, png::ColorType::Rgb);
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
                (72..220).contains(&x) && (190..220).contains(&y),
                "outside{x},{y}"
            );
            changed += 1;
        }
    }
    assert!(changed > 0);
    assert_eq!(std::fs::read(&input).expect("source"), SOURCE);
}
