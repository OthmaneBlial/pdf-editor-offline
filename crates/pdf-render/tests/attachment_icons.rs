use pdf_render::{RenderError, RenderedPage, render_page, render_view_page};
use std::path::Path;
use std::process::Command;

#[derive(Clone)]
struct Fixture<'a> {
    rotation: i32,
    flags: u32,
    rect: &'a str,
    name: &'a str,
    ap: &'a str,
    extra: &'a str,
    content: &'a str,
    attachment: bool,
    second: bool,
    signed_marker: bool,
}
impl Default for Fixture<'_> {
    fn default() -> Self {
        Self {
            rotation: 0,
            flags: 4,
            rect: "[50 50 74 74]",
            name: "",
            ap: "",
            extra: "",
            content: "q 1 0 0 rg 20 30 15 100 re f Q",
            attachment: true,
            second: false,
            signed_marker: false,
        }
    }
}

// Independently handwritten public synthetic PDF, including an inert embedded
// payload. No application parser/writer creates these differential fixtures.
fn pdf(fixture: &Fixture<'_>) -> Vec<u8> {
    let annots = match (fixture.attachment, fixture.second) {
        (true, true) => "5 0 R 8 0 R",
        (true, false) => "5 0 R",
        (false, true) => "8 0 R",
        (false, false) => "",
    };
    let stream = |dict: &str, value: &str| {
        format!(
            "<< {dict} /Length {} >>\nstream\n{value}\nendstream",
            value.len()
        )
    };
    let entry = |name: &str, value: &str| {
        if value.is_empty() {
            String::new()
        } else {
            format!("/{name} {value}")
        }
    };
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [10 20 130 140] /CropBox [20 30 120 130] /Rotate {} /Resources << /ExtGState << /GS0 << /ca 0.2 /CA 0.2 /BM /Multiply >> >> >> >>",
            fixture.rotation
        ),
        format!("<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Annots [{annots}] >>"),
        stream("", fixture.content),
        format!(
            "<< /Type /Annot /Subtype /FileAttachment {} {} /F {} /FS 7 0 R {} {} >>",
            entry("Rect", fixture.rect),
            entry("Name", fixture.name),
            fixture.flags,
            fixture.ap,
            fixture.extra
        ),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 24 24]",
            "0 0 1 rg 0 0 24 24 re f",
        ),
        "<< /Type /Filespec /F (attachment.txt) /EF << /F 10 0 R >> >>".to_owned(),
        "<< /Type /Annot /Subtype /Stamp /Rect [90 90 100 100] /F 4 /AP << /N 9 0 R >> >>"
            .to_owned(),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 10 10]",
            "0 1 1 rg 0 0 10 10 re f",
        ),
        stream(
            "/Type /EmbeddedFile /Subtype /text#2Fplain",
            "public synthetic attachment payload",
        ),
    ];
    if fixture.signed_marker {
        objects.push("<< /FT /Sig /V << /Type /Sig /Contents <00414243> >> >>".into());
    }
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![];
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
fn image(fixture: &Fixture<'_>, scale: f32, zoom: f32) -> RenderedPage {
    render_view_page(&pdf(fixture), 0, scale, zoom).expect("native paint")
}
fn baseline(fixture: &Fixture<'_>, scale: f32, zoom: f32) -> RenderedPage {
    image(
        &Fixture {
            attachment: false,
            ..fixture.clone()
        },
        scale,
        zoom,
    )
}
fn pixel(image: &RenderedPage, x: usize, y: usize) -> &[u8] {
    &image.rgba[(y * image.width as usize + x) * 4..][..4]
}
fn source_point(rotation: i32, x: f64, y: f64) -> [f64; 2] {
    match rotation {
        0 => [x - 20.0, 130.0 - y],
        90 => [y - 30.0, x - 20.0],
        180 => [120.0 - x, y - 30.0],
        270 => [130.0 - y, 120.0 - x],
        _ => panic!("fixture rotation"),
    }
}
fn footprint(rotation: i32, flags: u32, scale: f32, zoom: f32) -> [f64; 4] {
    let anchor = source_point(rotation, 50.0, 74.0);
    let factor = if flags & 8 != 0 {
        1.0 / f64::from(zoom)
    } else {
        1.0
    };
    let corners = if flags & 16 != 0 {
        [
            [anchor[0], anchor[1]],
            [anchor[0] + 24.0 * factor, anchor[1] + 24.0 * factor],
        ]
    } else {
        let a = source_point(rotation, 50.0, 50.0);
        let b = source_point(rotation, 74.0, 74.0);
        [
            [
                anchor[0] + (a[0] - anchor[0]) * factor,
                anchor[1] + (a[1] - anchor[1]) * factor,
            ],
            [
                anchor[0] + (b[0] - anchor[0]) * factor,
                anchor[1] + (b[1] - anchor[1]) * factor,
            ],
        ]
    };
    let scale = f64::from(scale);
    [
        corners[0][0].min(corners[1][0]) * scale,
        corners[0][1].min(corners[1][1]) * scale,
        corners[0][0].max(corners[1][0]) * scale,
        corners[0][1].max(corners[1][1]) * scale,
    ]
}
fn changes(before: &RenderedPage, after: &RenderedPage, bounds: [f64; 4]) -> usize {
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut changed = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (
                (index % before.width as usize) as f64,
                (index / before.width as usize) as f64,
            );
            assert!(
                x >= bounds[0].floor() - 1.0
                    && x < bounds[2].ceil() + 1.0
                    && y >= bounds[1].floor() - 1.0
                    && y < bounds[3].ceil() + 1.0,
                "outside fixed source footprint {x},{y} {bounds:?}"
            );
            changed += 1;
        }
    }
    changed
}

#[test]
fn four_original_icons_are_distinct_visible_and_default_pushpin_is_explicit() {
    let default = image(&Fixture::default(), 1.0, 1.0);
    let base = baseline(&Fixture::default(), 1.0, 1.0);
    let mut icons = vec![];
    for name in ["/Graph", "/Paperclip", "/PushPin", "/Tag"] {
        let fixture = Fixture {
            name,
            ..Default::default()
        };
        let painted = image(&fixture, 1.0, 1.0);
        assert!(changes(&base, &painted, [30.0, 56.0, 54.0, 80.0]) > 30);
        if name == "/PushPin" {
            assert_eq!(painted, default);
        }
        assert!(icons.iter().all(|previous| previous != &painted));
        icons.push(painted);
    }
    assert_eq!(
        image(
            &Fixture {
                name: "/UnrecognizedCustomIcon",
                ..Default::default()
            },
            1.0,
            1.0
        ),
        default
    );
    assert_eq!(
        image(
            &Fixture {
                name: "null",
                ap: "/AP null",
                extra: "/C null /CA null",
                ..Default::default()
            },
            1.0,
            1.0
        ),
        default
    );
}

#[test]
fn rotation_crop_origin_density_zoom_and_capped_scale_keep_source_defined_footprints() {
    for rotation in [0, 90, 180, 270] {
        for flags in [0, 8, 16, 24] {
            for (scale, zoom) in [(1.0, 1.0), (2.0, 2.0), (4.0, 2.0), (0.5, 4.0)] {
                let fixture = Fixture {
                    rotation,
                    flags,
                    ..Default::default()
                };
                let base = baseline(&fixture, scale, zoom);
                let painted = image(&fixture, scale, zoom);
                assert!(changes(&base, &painted, footprint(rotation, flags, scale, zoom)) > 0);
            }
        }
    }
    // Fixed physical density 2: NoZoom at logical zoom 2 has the same local
    // icon pixels as density 1/zoom 1, anchored at twice the screen coordinates.
    let a = image(
        &Fixture {
            flags: 24,
            ..Default::default()
        },
        1.0,
        1.0,
    );
    let b = image(
        &Fixture {
            flags: 24,
            ..Default::default()
        },
        2.0,
        2.0,
    );
    let painted_bounds = |base: &RenderedPage, painted: &RenderedPage| {
        let mut bounds = [usize::MAX, usize::MAX, 0, 0];
        for (index, (a, b)) in base
            .rgba
            .chunks_exact(4)
            .zip(painted.rgba.chunks_exact(4))
            .enumerate()
        {
            if a != b {
                let (x, y) = (
                    index % painted.width as usize,
                    index / painted.width as usize,
                );
                bounds = [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ];
            }
        }
        bounds
    };
    let x = painted_bounds(
        &baseline(
            &Fixture {
                flags: 24,
                ..Default::default()
            },
            1.0,
            1.0,
        ),
        &a,
    );
    let y = painted_bounds(
        &baseline(
            &Fixture {
                flags: 24,
                ..Default::default()
            },
            2.0,
            2.0,
        ),
        &b,
    );
    assert_eq!(x, [y[0] - 30, y[1] - 56, y[2] - 30, y[3] - 56]);
    // Exact solid interior probes; AA on curved edges can differ by one channel
    // unit at the translated raster position, so full-raster identity is not claimed.
    for (x, y) in [(12, 6), (12, 10), (3, 3)] {
        assert_eq!(pixel(&a, 30 + x, 56 + y), pixel(&b, 60 + x, 112 + y));
    }
    let c = image(
        &Fixture {
            flags: 24,
            ..Default::default()
        },
        4.0,
        2.0,
    );
    assert!(
        changes(
            &baseline(
                &Fixture {
                    flags: 24,
                    ..Default::default()
                },
                4.0,
                2.0
            ),
            &c,
            [120.0, 224.0, 168.0, 272.0]
        ) > 0
    );
}

#[test]
fn screen_visibility_and_explicit_or_unresolved_appearance_precedence_are_preserved() {
    let base = baseline(&Fixture::default(), 1.0, 1.0);
    for flags in [2, 32, 34] {
        assert_eq!(
            image(
                &Fixture {
                    flags,
                    rect: "",
                    name: "123",
                    extra: "/CA /Invalid",
                    ..Default::default()
                },
                1.0,
                1.0
            ),
            base
        );
    }
    assert_ne!(
        image(
            &Fixture {
                flags: 1,
                ..Default::default()
            },
            1.0,
            1.0
        ),
        base
    );
    let explicit = image(
        &Fixture {
            ap: "/AP << /N 6 0 R >>",
            name: "/Graph",
            extra: "/C [2] /CA 2",
            ..Default::default()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&explicit, 40, 66), [0, 0, 255, 255]);
    for name in ["/Paperclip", "/PushPin", "/Tag", "123"] {
        assert_eq!(
            image(
                &Fixture {
                    ap: "/AP << /N 6 0 R >>",
                    name,
                    extra: "/C [2] /CA /Invalid",
                    ..Default::default()
                },
                1.0,
                1.0
            ),
            explicit
        );
    }
    for ap in [
        "/AP << >>",
        "/AP 123",
        "/AP /Invalid",
        "/AP 99 0 R",
        "/AP << /N << /Other 6 0 R >> >> /AS /Missing",
    ] {
        assert_eq!(
            image(
                &Fixture {
                    ap,
                    ..Default::default()
                },
                1.0,
                1.0
            ),
            base
        );
    }
}

#[test]
fn optional_device_colors_transparent_background_and_whole_icon_opacity_are_real() {
    let source = "q 0 0 1 rg 50 50 24 24 re f Q";
    let fixture = Fixture {
        name: "/Graph",
        content: source,
        ..Default::default()
    };
    let red = image(
        &Fixture {
            extra: "/C [1 0 0]",
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&red, 33, 59), [255, 0, 0, 255]);
    let gray = image(
        &Fixture {
            extra: "/C [0.5]",
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    let p = pixel(&gray, 33, 59);
    assert_eq!(p[0], p[1]);
    assert_eq!(p[1], p[2]);
    assert!((120..=136).contains(&p[0]));
    let cmyk = image(
        &Fixture {
            extra: "/C [0 1 1 0]",
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    let p = pixel(&cmyk, 33, 59);
    assert!(p[0] > 180 && p[1] < 120 && p[2] < 120);
    let transparent = image(
        &Fixture {
            extra: "/C []",
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(pixel(&transparent, 33, 59), [0, 0, 255, 255]);
    assert!(
        changes(
            &baseline(&fixture, 1.0, 1.0),
            &transparent,
            [30.0, 56.0, 54.0, 80.0]
        ) > 0
    );
    let half = image(
        &Fixture {
            extra: "/C [1 0 0] /CA 0.5",
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    let p = pixel(&half, 33, 59);
    assert!((127..=128).contains(&p[0]) && p[1] == 0 && (127..=128).contains(&p[2]));
    assert_eq!(
        image(
            &Fixture {
                extra: "/CA 0",
                ..fixture.clone()
            },
            1.0,
            1.0
        ),
        baseline(&fixture, 1.0, 1.0)
    );
    // Opacity applies once to the whole icon; overlapping border/glyph elements
    // must equal the alpha composite of the corresponding opaque pixels.
    let blue = baseline(&fixture, 1.0, 1.0);
    for (a, (b, c)) in half
        .rgba
        .chunks_exact(4)
        .zip(red.rgba.chunks_exact(4).zip(blue.rgba.chunks_exact(4)))
    {
        for channel in 0..3 {
            let expected = (u16::from(b[channel]) + u16::from(c[channel])) / 2;
            assert!((i32::from(a[channel]) - i32::from(expected)).abs() <= 1);
        }
    }
}

#[test]
fn malformed_default_icon_geometry_name_color_and_opacity_fail_controlled() {
    for rect in [
        "",
        "null",
        "/Invalid",
        "[50 50 74]",
        "[50 50 74 74 99]",
        "[50 50 /NaN 74]",
        "[50 50 50 74]",
        "[0 0 10000000000000000000000000000000000000000000 24]",
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
    for extra in [
        "/C /Invalid",
        "/C [0 1]",
        "/C [0 0 0 0 0]",
        "/C [2]",
        "/C [-1]",
        "/C [0 /Invalid 1]",
        "/CA /Invalid",
        "/CA -1",
        "/CA 2",
    ] {
        assert_eq!(
            render_page(
                &pdf(&Fixture {
                    extra,
                    ..Default::default()
                }),
                0,
                1.0
            ),
            Err(RenderError::RendererFailed)
        );
    }
    assert_eq!(
        render_page(
            &pdf(&Fixture {
                name: "123",
                ..Default::default()
            }),
            0,
            1.0
        ),
        Err(RenderError::RendererFailed)
    );
}

#[test]
fn transformed_float32_corner_overflow_is_warned_before_any_raster_flush() {
    use hayro::hayro_interpret::{InterpreterSettings, InterpreterWarning, util::TransformExt};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let bytes = pdf(&Fixture {
        rotation: 180,
        flags: 16,
        rect: "[-300000000000000000000000000000000000000 50 300000000000000000000000000000000000000 74]",
        ..Default::default()
    });
    let parsed = hayro::hayro_syntax::Pdf::new(bytes.clone()).expect("synthetic finite rectangle");
    let warning = Arc::new(AtomicBool::new(false));
    let capture = warning.clone();
    let mut context = hayro::vello_cpu::RenderContext::new(100, 100);
    hayro::render_into(
        &parsed.pages()[0],
        &hayro::RenderCache::new(),
        &InterpreterSettings {
            warning_sink: Arc::new(move |value| {
                if matches!(value, InterpreterWarning::InvalidAnnotationGeometry) {
                    capture.store(true, Ordering::Relaxed);
                }
            }),
            ..Default::default()
        },
        &Default::default(),
        &mut context,
        parsed.pages()[0].initial_transform(true).to_kurbo(),
    );
    assert!(
        warning.load(Ordering::Relaxed),
        "finite transform coefficients must not hide overflowing transformed icon corners"
    );
    assert_eq!(
        render_page(&bytes, 0, 1.0),
        Err(RenderError::RendererFailed)
    );
}

#[test]
fn drawing_order_pending_paths_and_page_graphics_state_cannot_leak_into_or_out_of_icon() {
    let fixture = Fixture {
        second: true,
        content: "/GS0 gs 0 1 0 rg 20 30 15 100 re f 90 90 10 10 re",
        extra: "/C [1 0 0] /CA 0.5",
        ..Default::default()
    };
    let base = baseline(&fixture, 1.0, 1.0);
    let icon = image(&fixture, 1.0, 1.0);
    assert!(changes(&base, &icon, [30.0, 56.0, 54.0, 80.0]) > 0);
    assert_eq!(pixel(&icon, 75, 35), [0, 255, 255, 255]);
    let signed = image(
        &Fixture {
            signed_marker: true,
            ..fixture.clone()
        },
        1.0,
        1.0,
    );
    assert_eq!(signed, icon);
    let bytes = pdf(&fixture);
    let original = bytes.clone();
    render_page(&bytes, 0, 1.0).expect("read only");
    assert_eq!(bytes, original);
}

#[test]
fn actual_exported_sanitizer_default_icon_source_paints_and_removed_pair_preserves_all_other_pixels()
 {
    // Frozen public synthetic export from core make_fixture + sanitized_copy at
    // 377859d. Current sanitizer behavior is independently exercised by core's
    // live tests; this pair pins the renderer input/projection, not future erasure.
    let source = include_bytes!("fixtures/attachment-default-source.pdf");
    let removed = include_bytes!("fixtures/attachment-default-sanitized.pdf");
    let before = render_page(source, 0, 1.0).expect("actual sanitizer source");
    let after = render_page(removed, 0, 1.0).expect("actual removed attachment");
    assert_eq!((before.width, before.height), (200, 200));
    let mut changed = 0;
    for (index, (a, b)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (index % 200, index / 200);
            assert!(
                (140..160).contains(&x) && (40..60).contains(&y),
                "outside actual attachment{x},{y}"
            );
            changed += 1;
        }
    }
    assert!(
        changed > 20,
        "actual absent-AP/default-Name PushPin must be painted"
    );
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let dir = std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("pinned Poppler directory");
    let output = Command::new(Path::new(&dir).join(name))
        .args(args)
        .output()
        .expect("reference process");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn pinned_reference() {
    let dir = std::env::var_os("PDF_REFERENCE_BIN_DIR").expect("Poppler");
    let output = Command::new(Path::new(&dir).join("pdftoppm"))
        .arg("-v")
        .output()
        .expect("version");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
}
fn poppler(path: &Path) -> RenderedPage {
    let ppm = reference(
        "pdftoppm",
        &[
            "-singlefile",
            "-cropbox",
            "-r",
            "72",
            path.to_str().expect("fixturepath"),
        ],
    );
    let mut lines = ppm.splitn(4, |byte| *byte == b'\n');
    assert_eq!(lines.next(), Some(b"P6".as_slice()));
    assert_eq!(lines.next(), Some(b"100 100".as_slice()));
    assert_eq!(lines.next(), Some(b"255".as_slice()));
    let rgb = lines.next().expect("pixels");
    assert_eq!(rgb.len(), 100 * 100 * 3);
    let rgba = rgb
        .chunks_exact(3)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
        .collect();
    RenderedPage {
        width: 100,
        height: 100,
        rgba,
    }
}

#[test]
#[ignore = "requires pinned Poppler 26.10.0"]
fn pinned_poppler_confirms_standard_names_flags_precedence_opacity_and_preserved_page() {
    pinned_reference();
    let directory = tempfile::tempdir().expect("temp");
    let mut index = 0;
    for rotation in [0, 90, 180, 270] {
        for flags in [0, 16] {
            for name in ["", "/Graph", "/PushPin", "/Paperclip", "/Tag"] {
                let fixture = Fixture {
                    rotation,
                    flags,
                    name,
                    second: true,
                    ..Default::default()
                };
                let source = pdf(&fixture);
                let path = directory.path().join(format!("source-{index}.pdf"));
                let basepath = directory.path().join(format!("base-{index}.pdf"));
                std::fs::write(&path, &source).expect("fixture");
                std::fs::write(
                    &basepath,
                    pdf(&Fixture {
                        attachment: false,
                        ..fixture.clone()
                    }),
                )
                .expect("baseline");
                let bounds = footprint(rotation, flags, 1.0, 1.0);
                assert!(
                    changes(
                        &baseline(&fixture, 1.0, 1.0),
                        &image(&fixture, 1.0, 1.0),
                        bounds
                    ) > 0
                );
                assert!(changes(&poppler(&basepath), &poppler(&path), bounds) > 0);
                assert_eq!(std::fs::read(&path).expect("immutable source"), source);
                index += 1;
            }
        }
    }
    for extra in ["/CA 0", "/C [0 1 0] /CA 0.5"] {
        let fixture = Fixture {
            name: "/Graph",
            content: "q 0 0 1 rg 50 50 24 24 re f Q",
            extra,
            ..Default::default()
        };
        let path = directory.path().join("opacity.pdf");
        let basepath = directory.path().join("opacity-base.pdf");
        std::fs::write(&path, pdf(&fixture)).expect("fixture");
        std::fs::write(
            &basepath,
            pdf(&Fixture {
                attachment: false,
                ..fixture.clone()
            }),
        )
        .expect("base");
        let actual = poppler(&path);
        let base = poppler(&basepath);
        if extra == "/CA 0" {
            assert_eq!(actual, base);
        } else {
            assert!(changes(&base, &actual, [30.0, 56.0, 54.0, 80.0]) > 0);
            assert!(
                actual.rgba.chunks_exact(4).any(|p| p[0] == 0
                    && (126..=129).contains(&p[1])
                    && (126..=129).contains(&p[2]))
            );
        }
    }
    for flags in [2, 32] {
        let fixture = Fixture {
            flags,
            ..Default::default()
        };
        let path = directory.path().join("hidden.pdf");
        let basepath = directory.path().join("hidden-base.pdf");
        std::fs::write(&path, pdf(&fixture)).expect("fixture");
        std::fs::write(
            &basepath,
            pdf(&Fixture {
                attachment: false,
                ..fixture.clone()
            }),
        )
        .expect("base");
        assert_eq!(poppler(&path), poppler(&basepath));
    }
    let a = Fixture {
        ap: "/AP << /N 6 0 R >>",
        name: "/Graph",
        extra: "/C [1 0 0] /CA 0",
        ..Default::default()
    };
    let b = Fixture {
        name: "/Tag",
        ..a.clone()
    };
    let pa = directory.path().join("ap-a.pdf");
    let pb = directory.path().join("ap-b.pdf");
    std::fs::write(&pa, pdf(&a)).expect("fixture");
    std::fs::write(&pb, pdf(&b)).expect("fixture");
    assert_eq!(poppler(&pa), poppler(&pb));
    assert_eq!(pixel(&poppler(&pa), 40, 66), [0, 0, 255, 255]);
}
