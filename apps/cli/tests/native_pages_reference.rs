#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn source(crop_first: bool) -> Vec<u8> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [4 0 R 6 0 R 8 0 R] /Count 3 >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for index in 0..3 {
        let crop = if index == 0 && crop_first {
            "/CropBox [20 30 280 370]"
        } else {
            ""
        };
        let rotation = if index == 1 { 90 } else { 0 };
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] {crop} /Rotate {rotation} /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            300 + index * 20, 400 + index * 20, 5 + index * 2,
        ));
        let content = format!(
            "BT /F1 12 Tf 50 300 Td (Independent native page {}) Tj ET",
            index + 1
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    bytes
}

fn reference(name: &str, arguments: &[&str]) -> Vec<u8> {
    let executable = std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
        || PathBuf::from(name),
        |directory| PathBuf::from(directory).join(name),
    );
    let output = Command::new(executable).args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[derive(Debug, PartialEq)]
struct Page {
    text: String,
    media: [f64; 4],
    crop: [f64; 4],
    rotation: i64,
    raster: Vec<u8>,
}

fn page(path: &Path, number: usize) -> Page {
    let number = number.to_string();
    let path = path.to_str().unwrap();
    let info = String::from_utf8(reference(
        "pdfinfo",
        &["-f", &number, "-l", &number, "-box", path],
    ))
    .unwrap();
    let rectangle = |name: &str| -> [f64; 4] {
        let line = info.lines().find(|line| line.contains(name)).unwrap();
        let fields = line
            .split_whitespace()
            .rev()
            .take(4)
            .map(|field| field.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        [fields[3], fields[2], fields[1], fields[0]]
    };
    let rotation = info
        .lines()
        .find(|line| line.contains(" rot:"))
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    let text = String::from_utf8(reference(
        "pdftotext",
        &["-f", &number, "-l", &number, path, "-"],
    ))
    .unwrap();
    Page {
        text: text.split_whitespace().collect::<Vec<_>>().join(" "),
        media: rectangle("MediaBox:"),
        crop: rectangle("CropBox:"),
        rotation,
        raster: reference(
            "pdftoppm",
            &["-f", &number, "-singlefile", "-cropbox", "-r", "36", path],
        ),
    }
}

fn cli(input: &Path, output: &Path, operation: &str, options: &[&str], expected: i32) {
    let result = Command::new(env!("CARGO_BIN_EXE_pdf-editor-offline"))
        .arg("--json")
        .arg(operation)
        .arg(input)
        .args(options)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(expected),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(envelope["ok"], expected == 0);
}

#[test]
#[ignore = "independent gate requires Poppler pdfinfo/pdftotext/pdftoppm"]
fn duplicated_pages_match_independent_text_geometry_and_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    let output = directory.path().join("duplicates.pdf");
    let original = source(true);
    std::fs::write(&input, &original).unwrap();
    let pages = (1..=3)
        .map(|number| page(&input, number))
        .collect::<Vec<_>>();
    cli(&input, &output, "duplicate-pages", &["--pages", "3,1"], 0);
    for (index, source) in [0, 0, 1, 2, 2].into_iter().enumerate() {
        assert_eq!(page(&output, index + 1), pages[source]);
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[test]
#[ignore = "independent gate requires Poppler pdfinfo/pdftotext/pdftoppm"]
fn native_page_outputs_match_independent_text_geometry_and_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pdf");
    let original = source(false);
    std::fs::write(&input, &original).unwrap();
    let pages = (1..=3)
        .map(|number| page(&input, number))
        .collect::<Vec<_>>();
    for (operation, options, order) in [
        ("reverse", vec![], vec![2, 1, 0]),
        ("reorder", vec!["--order", "3,1,2"], vec![2, 0, 1]),
        ("extract-pages", vec!["--pages", "3,1"], vec![2, 0]),
        ("delete-pages", vec!["--pages", "2"], vec![0, 2]),
    ] {
        let output = directory.path().join(format!("{operation}.pdf"));
        cli(&input, &output, operation, &options, 0);
        let info = String::from_utf8(reference("pdfinfo", &[output.to_str().unwrap()])).unwrap();
        let count: usize = info
            .lines()
            .find_map(|line| line.strip_prefix("Pages:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(count, order.len());
        for (position, expected) in order.iter().enumerate() {
            assert_eq!(
                page(&output, position + 1),
                pages[*expected],
                "{operation} page {}",
                position + 1
            );
        }
    }
    let cropped = directory.path().join("crop.pdf");
    let expected_crop = directory.path().join("expected-crop.pdf");
    std::fs::write(&expected_crop, source(true)).unwrap();
    cli(
        &input,
        &cropped,
        "crop",
        &["--pages", "1", "--rectangle", "20", "30", "280", "370"],
        0,
    );
    for number in 1..=3 {
        assert_eq!(page(&cropped, number), page(&expected_crop, number));
    }
    let inserted = directory.path().join("inserted.pdf");
    cli(
        &input,
        &inserted,
        "insert-blank",
        &["--position", "2", "--width", "200", "--height", "250"],
        0,
    );
    for (output_page, original_page) in [(1, 0), (3, 1), (4, 2)] {
        assert_eq!(page(&inserted, output_page), pages[original_page]);
    }
    let blank = page(&inserted, 2);
    assert!(blank.text.is_empty());
    assert_eq!(blank.media, [0.0, 0.0, 200.0, 250.0]);
    assert_eq!(blank.crop, blank.media);
    assert_eq!(blank.rotation, 0);
    let header = b"P6\n100 125\n255\n";
    assert!(blank.raster.starts_with(header));
    assert_eq!(blank.raster.len() - header.len(), 100 * 125 * 3);
    assert!(
        blank.raster[header.len()..]
            .iter()
            .all(|channel| *channel == 255)
    );
    let retained = std::fs::read(&inserted).unwrap();
    let merged = directory.path().join("merged.pdf");
    cli(&input, &merged, "merge", &[input.to_str().unwrap()], 0);
    for number in 1..=6 {
        assert_eq!(page(&merged, number), pages[(number - 1) % 3]);
    }
    cli(&input, &inserted, "reverse", &[], 3);
    assert_eq!(std::fs::read(&inserted).unwrap(), retained);
    let dry = directory.path().join("dry.pdf");
    cli(&input, &dry, "reverse", &["--dry-run"], 0);
    assert!(!dry.exists());
    assert_eq!(std::fs::read(input).unwrap(), original);
}
