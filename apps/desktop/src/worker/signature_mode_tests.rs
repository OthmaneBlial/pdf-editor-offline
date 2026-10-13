#![allow(clippy::unwrap_used)]
use super::*;
use std::path::Path;
const PDF: &[u8] = include_bytes!("../../../../tests/fixtures/pdfs/forms.pdf");
const FONT: &[u8] = include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const VALUE: &str = "Élodie ΩЖ";

fn exercise(reference: bool) {
    for typed in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        let asset = temp.path().join(if typed {
            "signature.ttf"
        } else {
            "signature.png"
        });
        std::fs::write(&source, PDF).unwrap();
        let asset_bytes = if typed { FONT } else { TEST_RED_PNG };
        std::fs::write(&asset, asset_bytes).unwrap();
        let mut journal = Journal::open(temp.path().join("journal")).unwrap();
        let mut documents = HashMap::new();
        process_with_journal(
            &mut documents,
            Command::Load {
                id: 1,
                path: source.clone(),
            },
            Some(&mut journal),
        )
        .unwrap();
        let original = documents[&1].current.bytes.clone();
        let key = FormsReadKey {
            tab: 1,
            revision: 0,
            generation: 1,
        };
        let edit = || {
            if typed {
                Edit::TextOverlay {
                    key,
                    font_file: asset.clone(),
                    overlay: pdf_core::TextOverlay {
                        page: 1,
                        rectangle: [350.0, 400.0, 550.0, 500.0],
                        value: VALUE.into(),
                        size: 24.0,
                        color: [0.0; 3],
                        alignment: pdf_core::TextAlignment::Center,
                    },
                }
            } else {
                Edit::ImageOverlay {
                    key,
                    image_file: asset.clone(),
                    overlay: pdf_core::ImageOverlay {
                        page: 1,
                        rectangle: [350.0, 400.0, 550.0, 500.0],
                        rotation: 90,
                    },
                }
            }
        };
        process_with_journal(
            &mut documents,
            Command::Edit {
                id: 1,
                edit: edit(),
            },
            Some(&mut journal),
        )
        .unwrap();
        let changed = documents[&1].current.bytes.clone();
        assert_ne!(changed, original);
        assert_eq!(documents[&1].undo.len(), 1);
        assert_eq!(journal.recover().unwrap()[0].current.bytes, changed);
        assert!(
            process_with_journal(
                &mut documents,
                Command::Edit {
                    id: 1,
                    edit: edit()
                },
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(documents[&1].current.bytes, changed);
        for (edit, expected) in [(Edit::Undo, &original), (Edit::Redo, &changed)] {
            process_with_journal(
                &mut documents,
                Command::Edit { id: 1, edit },
                Some(&mut journal),
            )
            .unwrap();
            assert_eq!(&documents[&1].current.bytes, expected);
        }
        let output = temp.path().join("visual-signature.pdf");
        process_with_journal(
            &mut documents,
            Command::Save {
                id: 1,
                path: output.clone(),
            },
            Some(&mut journal),
        )
        .unwrap();
        let saved = std::fs::read(&output).unwrap();
        assert!(
            process_with_journal(
                &mut documents,
                Command::Save {
                    id: 1,
                    path: output.clone()
                },
                Some(&mut journal)
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&output).unwrap(), saved);
        assert!(!documents[&1].status().unwrap().dirty);
        let before = pdf_core::PdfDocument::from_bytes(PDF, None).unwrap();
        let after = pdf_core::PdfDocument::from_bytes(&saved, None).unwrap();
        assert_eq!(after.info().unwrap().pages, before.info().unwrap().pages);
        assert_eq!(
            after.info().unwrap().signature_fields,
            before.info().unwrap().signature_fields
        );
        assert_eq!(
            after.list_form_fields().unwrap(),
            before.list_form_fields().unwrap()
        );
        if typed {
            assert!(after.extract_text(&[1]).unwrap().contains(VALUE));
        }
        assert_eq!(std::fs::read(&source).unwrap(), PDF);
        assert_eq!(std::fs::read(&asset).unwrap(), asset_bytes);
        let raster_before = pdf_render::render_page(PDF, 0, 1.0).unwrap();
        let raster_after = pdf_render::render_page(&saved, 0, 1.0).unwrap();
        let mut changed_pixels = 0;
        for (i, (a, b)) in raster_before
            .rgba
            .chunks_exact(4)
            .zip(raster_after.rgba.chunks_exact(4))
            .enumerate()
        {
            if a != b {
                assert!((350..550).contains(&(i % 595)) && (342..442).contains(&(i / 595)));
                changed_pixels += 1;
            }
        }
        assert!(changed_pixels > 20);
        if reference {
            let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
            assert!(
                std::process::Command::new(std::env::var_os("PDF_QPDF_BIN").unwrap())
                    .arg("--check")
                    .arg(&output)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
            let raster = |path: &Path| {
                let out = std::process::Command::new(bin.join("pdftoppm"))
                    .args(["-singlefile", "-r", "72"])
                    .arg(path)
                    .output()
                    .unwrap();
                assert!(out.status.success());
                out.stdout
                    .strip_prefix(b"P6\n595 842\n255\n")
                    .unwrap()
                    .to_vec()
            };
            let a = raster(&source);
            let b = raster(&output);
            let mut count = 0;
            for (i, (a, b)) in a.chunks_exact(3).zip(b.chunks_exact(3)).enumerate() {
                if a != b {
                    // Poppler samples the image's closed right/bottom edge at
                    // integer pixels; text remains strictly inside the band.
                    let x = i % 595;
                    let y = i / 595;
                    assert!(
                        if typed {
                            (350..550).contains(&x) && (342..442).contains(&y)
                        } else {
                            (350..=550).contains(&x) && (342..=442).contains(&y)
                        },
                        "outside signature typed={typed} x={x} y={y} before={a:?} after={b:?}"
                    );
                    count += 1;
                }
            }
            assert!(count > 20);
            if typed {
                let out = std::process::Command::new(bin.join("pdftotext"))
                    .args(["-raw"])
                    .arg(&output)
                    .arg("-")
                    .output()
                    .unwrap();
                assert!(out.status.success());
                assert!(String::from_utf8(out.stdout).unwrap().contains(VALUE));
            }
        }
    }
}
#[test]
fn typed_and_image_visual_artwork_use_real_child_history_and_verified_save() {
    exercise(false);
}
#[test]
#[ignore = "requires fresh normal native executable and pinned qpdf/Poppler"]
fn production_typed_and_image_visual_signatures_preserve_saved_artwork() {
    let qpdf = std::process::Command::new(std::env::var_os("PDF_QPDF_BIN").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&qpdf.stdout).contains("12.4.2"));
    let bin = PathBuf::from(std::env::var_os("PDF_REFERENCE_BIN_DIR").unwrap());
    let poppler = std::process::Command::new(bin.join("pdftotext"))
        .arg("-v")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&poppler.stderr).contains("26.10.0"));
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "pdf-editor-offline-native.exe"
        } else {
            "pdf-editor-offline-native"
        });
    assert!(binary.is_file());
    document::with_test_executor(
        binary,
        vec!["--pdf-worker".into()],
        pdf_worker::Limits::default(),
        || exercise(true),
    );
}
