use pdf_render::{RenderError, RenderedPage, render_page, render_view_page};
use std::path::Path;
use std::process::Command;

const NAMES: [&str; 7] = [
    "Comment",
    "Key",
    "Note",
    "Help",
    "NewParagraph",
    "Paragraph",
    "Insert",
];

#[derive(Clone)]
struct Fixture<'a> {
    rotation: i32,
    flags: u32,
    rect: &'a str,
    entries: &'a str,
    content: &'a str,
    note: bool,
}
impl Default for Fixture<'_> {
    fn default() -> Self {
        Self {
            rotation: 0,
            flags: 4,
            rect: "/Rect [50 50 74 74]",
            entries: "",
            content: "q 1 0 0 rg 20 30 15 100 re f Q",
            note: true,
        }
    }
}

// Handwritten public synthetic PDF: renderer and reference receive identical bytes.
// Page origin/crop are inherited; a later cyan appearance exercises state isolation.
fn pdf(f: &Fixture<'_>) -> Vec<u8> {
    let stream = |dict: &str, content: &str| {
        format!(
            "<< {dict} /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
    };
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [10 20 130 140] /CropBox [20 30 120 130] /Rotate {} /Resources << /ExtGState << /GS0 << /ca 0.2 /CA 0.2 /BM /Multiply >> >> >> >>",
            f.rotation
        ),
        format!(
            "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Annots [{} 7 0 R] >>",
            if f.note { "5 0 R" } else { "" }
        ),
        stream("", f.content),
        format!(
            "<< /Type /Annot /Subtype /Text {} /F {} {} >>",
            f.rect, f.flags, f.entries
        ),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 24 24]",
            "0 0 1 rg 0 0 24 24 re f",
        ),
        "<< /Type /Annot /Subtype /Stamp /Rect [90 90 100 100] /F 4 /AP << /N 8 0 R >> >>"
            .to_owned(),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 10 10]",
            "0 1 1 rg 0 0 10 10 re f",
        ),
        // Inert signature marker proves read-only rendering does not rewrite objects.
        "<< /FT /Sig /V << /Type /Sig /Contents <00414243> >> >>".to_owned(),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
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
fn image(f: &Fixture<'_>, scale: f32, zoom: f32) -> RenderedPage {
    render_view_page(&pdf(f), 0, scale, zoom).expect("native note")
}
fn baseline(f: &Fixture<'_>, scale: f32, zoom: f32) -> RenderedPage {
    image(
        &Fixture {
            note: false,
            ..f.clone()
        },
        scale,
        zoom,
    )
}
fn pixel(image: &RenderedPage, x: usize, y: usize) -> &[u8] {
    &image.rgba[(y * image.width as usize + x) * 4..][..4]
}
fn bounds(rotation: i32, scale: f32, zoom: f32) -> [f64; 4] {
    // PDF upper-left (50,74), independently projected from crop [20,30,120,130].
    let [x, y] = match rotation {
        0 => [30.0, 56.0],
        90 => [44.0, 30.0],
        180 => [70.0, 44.0],
        270 => [56.0, 70.0],
        _ => panic!("fixture rotation"),
    };
    let scale = f64::from(scale);
    let side = 24.0 * scale / f64::from(zoom);
    [x * scale, y * scale, x * scale + side, y * scale + side]
}
fn changes(before: &RenderedPage, after: &RenderedPage, bounds: [f64; 4]) -> usize {
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut count = 0;
    for (i, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let x = (i % before.width as usize) as f64;
            let y = (i / before.width as usize) as f64;
            assert!(
                x >= bounds[0] && x < bounds[2] && y >= bounds[1] && y < bounds[3],
                "outside source-defined note footprint {x},{y}: {bounds:?}"
            );
            count += 1;
        }
    }
    count
}

#[test]
fn seven_original_symbols_are_distinct_and_default_null_and_custom_names_use_note() {
    let f = Fixture::default();
    let base = baseline(&f, 1.0, 1.0);
    let mut painted = vec![];
    for name in NAMES {
        let entries = format!("/Name /{name}");
        let actual = image(
            &Fixture {
                entries: &entries,
                ..f.clone()
            },
            1.0,
            1.0,
        );
        assert!(changes(&base, &actual, bounds(0, 1.0, 1.0)) > 60, "{name}");
        assert!(!painted.contains(&actual), "distinct {name}");
        painted.push(actual);
    }
    for entries in ["", "/Name null /AP null", "/Name /Custom", "/Name /Note"] {
        assert_eq!(
            image(
                &Fixture {
                    entries,
                    ..f.clone()
                },
                1.0,
                1.0
            ),
            painted[2]
        );
    }
}

#[test]
fn every_note_is_intrinsically_upright_and_fixed_logical_size_at_rotations_dpr_and_capped_preview()
{
    for rotation in [0, 90, 180, 270] {
        for (scale, zoom) in [(1.0, 1.0), (2.0, 2.0), (4.0, 2.0), (0.5, 2.0)] {
            for name in NAMES {
                let entries = format!("/Name /{name}");
                let f = Fixture {
                    rotation,
                    flags: 0,
                    entries: &entries,
                    ..Default::default()
                };
                let actual = image(&f, scale, zoom);
                assert!(
                    changes(
                        &baseline(&f, scale, zoom),
                        &actual,
                        bounds(rotation, scale, zoom)
                    ) > 0
                );
                assert_eq!(
                    actual,
                    image(
                        &Fixture {
                            flags: 24,
                            ..f.clone()
                        },
                        scale,
                        zoom
                    )
                );
                // Identical local artwork across rotation, at a common raster origin.
                let upright = image(
                    &Fixture {
                        rotation: 0,
                        ..f.clone()
                    },
                    scale,
                    zoom,
                );
                let a = bounds(rotation, scale, zoom);
                let b = bounds(0, scale, zoom);
                let side = (24.0 * scale / zoom) as usize;
                // Solid sample pixels avoid claiming translated curved-edge AA identity.
                for (dx, dy) in [(side / 8, side / 8), (side / 2, side / 2)] {
                    assert_eq!(
                        pixel(&actual, a[0] as usize + dx, a[1] as usize + dy),
                        pixel(&upright, b[0] as usize + dx, b[1] as usize + dy)
                    );
                }
            }
        }
    }
}

#[test]
fn precedence_visibility_and_inert_contents_do_not_replace_or_activate_appearances() {
    let f = Fixture::default();
    let base = baseline(&f, 1.0, 1.0);
    for flags in [2, 32, 34] {
        assert_eq!(
            image(
                &Fixture {
                    flags,
                    rect: "/Rect /Invalid",
                    entries: "/Name 1 /CA 2",
                    ..f.clone()
                },
                1.0,
                1.0
            ),
            base
        );
    }
    assert_eq!(
        image(
            &Fixture {
                flags: 1,
                ..f.clone()
            },
            1.0,
            1.0
        ),
        image(&f, 1.0, 1.0)
    );
    let explicit = image(
        &Fixture {
            entries: "/AP << /N 6 0 R >> /Name 1 /C [2] /CA 2",
            ..f.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&explicit, 40, 66), [0, 0, 255, 255]);
    for name in NAMES {
        let entries = format!("/AP << /N 6 0 R >> /Name /{name}");
        assert_eq!(
            image(
                &Fixture {
                    entries: &entries,
                    ..f.clone()
                },
                1.0,
                1.0
            ),
            explicit
        );
    }
    for entries in [
        "/AP << >>",
        "/AP 123",
        "/AP /Bad",
        "/AP 99 0 R",
        "/AP << /N << /Other 6 0 R >> >> /AS /Missing",
    ] {
        assert_eq!(
            image(
                &Fixture {
                    entries,
                    ..f.clone()
                },
                1.0,
                1.0
            ),
            base
        );
    }
    for entries in [
        "/Contents (untrusted text) /A << /S /URI /URI (https://invalid.example) >>",
        "/Contents <FEFF0041> /AA << /E << /S /JavaScript /JS (throw) >> >>",
        "/Open true",
    ] {
        let bytes = pdf(&Fixture {
            entries,
            ..f.clone()
        });
        let original = bytes.clone();
        assert_eq!(
            render_page(&bytes, 0, 1.0).expect("read-only"),
            image(&f, 1.0, 1.0)
        );
        assert_eq!(bytes, original);
    }
}

#[test]
fn shared_colors_opacity_and_state_isolation_preserve_unrelated_artwork() {
    let f = Fixture {
        content: "0 0 1 rg 50 50 24 24 re f /GS0 gs 0 1 0 rg 1 0 0 1 3 0 cm 0 0 m 1 1 l",
        ..Default::default()
    };
    let base = baseline(&f, 1.0, 1.0);
    let opaque = image(
        &Fixture {
            entries: "/C [1 0 0]",
            ..f.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&opaque, 33, 59), [255, 0, 0, 255]);
    let half = image(
        &Fixture {
            entries: "/C [1 0 0] /CA 0.5",
            ..f.clone()
        },
        1.0,
        1.0,
    );
    assert!(changes(&base, &half, bounds(0, 1.0, 1.0)) > 0);
    assert_eq!(pixel(&half, 75, 35), pixel(&base, 75, 35));
    let transparent = image(
        &Fixture {
            entries: "/C []",
            ..f.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&transparent, 33, 59), pixel(&base, 33, 59));
    assert!(changes(&base, &transparent, bounds(0, 1.0, 1.0)) > 0);
    for (a, (b, c)) in half
        .rgba
        .chunks_exact(4)
        .zip(opaque.rgba.chunks_exact(4).zip(base.rgba.chunks_exact(4)))
    {
        for channel in 0..3 {
            let expected = (u16::from(b[channel]) + u16::from(c[channel])) / 2;
            assert!((i32::from(a[channel]) - i32::from(expected)).abs() <= 1);
        }
    }
    assert_eq!(
        image(
            &Fixture {
                entries: "/CA 0",
                ..f.clone()
            },
            1.0,
            1.0
        ),
        base
    );
}

#[test]
fn malformed_note_geometry_or_style_fails_controlled_without_input_mutation() {
    for rect in [
        "",
        "/Rect null",
        "/Rect /Bad",
        "/Rect [50 50 74]",
        "/Rect [50 50 74 74 9]",
        "/Rect [50 50 /Bad 74]",
        "/Rect [50 50 50 74]",
        "/Rect [0 0 10000000000000000000000000000000000000000000 24]",
    ] {
        let bytes = pdf(&Fixture {
            rect,
            ..Default::default()
        });
        let original = bytes.clone();
        assert_eq!(
            render_page(&bytes, 0, 1.0),
            Err(RenderError::RendererFailed)
        );
        assert_eq!(bytes, original);
    }
    for entries in [
        "/Name 123",
        "/C [0 1]",
        "/C [0 /Bad 1]",
        "/C [2]",
        "/CA /Bad",
        "/CA -1",
        "/CA 2",
    ] {
        assert_eq!(
            render_page(
                &pdf(&Fixture {
                    entries,
                    ..Default::default()
                }),
                0,
                1.0
            ),
            Err(RenderError::RendererFailed)
        );
    }
}

fn poppler(path: &Path) -> RenderedPage {
    let dir = std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("pinned Poppler directory");
    let output = Command::new(Path::new(&dir).join("pdftoppm"))
        .args(["-singlefile", "-cropbox", "-r", "72"])
        .arg(path)
        .output()
        .expect("reference render");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut lines = output.stdout.splitn(4, |byte| *byte == b'\n');
    assert_eq!(lines.next(), Some(b"P6".as_slice()));
    assert_eq!(lines.next(), Some(b"100 100".as_slice()));
    assert_eq!(lines.next(), Some(b"255".as_slice()));
    let rgb = lines.next().expect("pixels");
    assert_eq!(rgb.len(), 30_000);
    RenderedPage {
        width: 100,
        height: 100,
        rgba: rgb
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    }
}

fn reference_changes(before: &RenderedPage, after: &RenderedPage, bounds: [f64; 4]) -> usize {
    // Poppler default AA/edge inclusion may touch the adjacent pixel, as in the
    // existing AP/attachment gates. No tolerance is applied to any other pixel.
    let changed = changes(
        before,
        after,
        [
            bounds[0] - 1.0,
            bounds[1] - 1.0,
            bounds[2] + 1.0,
            bounds[3] + 1.0,
        ],
    );
    let mut boundary = 0;
    let mut first = None;
    for (i, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        let x = (i % before.width as usize) as f64;
        let y = (i / before.width as usize) as f64;
        if a != b && (x < bounds[0] || x >= bounds[2] || y < bounds[1] || y >= bounds[3]) {
            boundary += 1;
            first.get_or_insert((x, y, a, b));
        }
    }
    if boundary != 0 {
        eprintln!(
            "Poppler default-AA note boundary: {boundary} adjacent pixels, first {first:?}, source bounds {bounds:?}"
        );
    }
    changed
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0"]
fn pinned_poppler_confirms_all_standard_names_upright_footprints_visibility_and_precedence() {
    let dir = std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("pinned Poppler directory");
    let version = Command::new(Path::new(&dir).join("pdftoppm"))
        .arg("-v")
        .output()
        .expect("version");
    assert!(
        String::from_utf8_lossy(&version.stderr)
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    let temp = tempfile::tempdir().expect("temp");
    let source_path = temp.path().join("source.pdf");
    let base_path = temp.path().join("base.pdf");
    for rotation in [0, 90, 180, 270] {
        for name in NAMES.into_iter().chain([""]) {
            let entries = if name.is_empty() {
                String::new()
            } else {
                format!("/Name /{name}")
            };
            let f = Fixture {
                rotation,
                flags: 0,
                entries: &entries,
                ..Default::default()
            };
            let bytes = pdf(&f);
            std::fs::write(&source_path, &bytes).expect("source");
            std::fs::write(
                &base_path,
                pdf(&Fixture {
                    note: false,
                    ..f.clone()
                }),
            )
            .expect("base");
            assert!(
                changes(
                    &baseline(&f, 1.0, 1.0),
                    &image(&f, 1.0, 1.0),
                    bounds(rotation, 1.0, 1.0)
                ) > 0
            );
            assert!(
                reference_changes(
                    &poppler(&base_path),
                    &poppler(&source_path),
                    bounds(rotation, 1.0, 1.0)
                ) > 0,
                "Poppler {name}, rotation {rotation}"
            );
            assert_eq!(
                std::fs::read(&source_path).expect("source unchanged"),
                bytes
            );
        }
    }
    for (flags, entries) in [(2, ""), (32, ""), (4, "/CA 0"), (4, "/AP << >>")] {
        let f = Fixture {
            flags,
            entries,
            ..Default::default()
        };
        std::fs::write(&source_path, pdf(&f)).expect("source");
        std::fs::write(&base_path, pdf(&Fixture { note: false, ..f })).expect("base");
        assert_eq!(poppler(&source_path), poppler(&base_path), "{entries}");
    }
    let f = Fixture {
        entries: "/C [0 1 0] /CA 0.5",
        content: "q 0 0 1 rg 50 50 24 24 re f Q",
        ..Default::default()
    };
    std::fs::write(&source_path, pdf(&f)).expect("opacity source");
    std::fs::write(&base_path, pdf(&Fixture { note: false, ..f })).expect("opacity base");
    let actual = poppler(&source_path);
    assert!(reference_changes(&poppler(&base_path), &actual, bounds(0, 1.0, 1.0)) > 0);
    assert!(
        actual
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] == 0 && (126..=129).contains(&p[1]) && (126..=129).contains(&p[2]))
    );
    let mut explicit = None;
    for name in NAMES {
        let entries = format!("/AP << /N 6 0 R >> /Name /{name} /C [1 0 0] /CA 0.5");
        std::fs::write(
            &source_path,
            pdf(&Fixture {
                entries: &entries,
                ..Default::default()
            }),
        )
        .expect("explicit");
        let actual = poppler(&source_path);
        assert_eq!(pixel(&actual, 40, 66), [0, 0, 255, 255]);
        if let Some(before) = &explicit {
            assert_eq!(before, &actual);
        }
        explicit = Some(actual);
    }
}
