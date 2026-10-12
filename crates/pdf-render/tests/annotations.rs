use pdf_render::{RenderError, RenderedPage, render_page, render_view_page};
use std::path::{Path, PathBuf};
use std::process::Command;

const PAINT: &str = "q 1 1 0 rg 0 0 24 24 re f 0 0 1 rg 0 18 6 6 re f Q";

#[derive(Clone)]
struct Fixture<'a> {
    rotation: i32,
    flags: u32,
    subtype: &'a str,
    rect: &'a str,
    bbox: &'a str,
    matrix: &'a str,
    paint: &'a str,
    annotations: bool,
    overlap: bool,
    group: bool,
    filter: &'a str,
}

impl Default for Fixture<'_> {
    fn default() -> Self {
        Self {
            rotation: 0,
            flags: 4,
            subtype: "Text",
            rect: "50 50 74 74",
            bbox: "0 0 24 24",
            matrix: "1 0 0 1 0 0",
            paint: PAINT,
            annotations: true,
            overlap: false,
            group: false,
            filter: "",
        }
    }
}

// Independent handwritten PDF fixture: no application parser/writer builds the reference input.
fn pdf(fixture: &Fixture<'_>) -> Vec<u8> {
    let annots = if !fixture.annotations {
        ""
    } else if fixture.overlap {
        "/Annots [5 0 R 7 0 R]"
    } else {
        "/Annots [5 0 R]"
    };
    let group = if fixture.group {
        "/Group << /S /Transparency /I false /K false >>"
    } else {
        ""
    };
    let rect = if fixture.rect.is_empty() {
        String::new()
    } else {
        format!("/Rect [{}]", fixture.rect)
    };
    let geometry_entry = |key: &str, value: &str| {
        if value.is_empty() {
            String::new()
        } else if value == "null" || value == "/Invalid" {
            format!("/{key} {value}")
        } else {
            format!("/{key} [{value}]")
        }
    };
    let bbox = geometry_entry("BBox", fixture.bbox);
    let matrix = geometry_entry("Matrix", fixture.matrix);
    let content = "q 1 0 0 rg 20 30 15 100 re f Q";
    let overlay = "q /G1 gs 1 0 1 rg 0 0 24 24 re f Q";
    let stream = |dictionary: &str, content: &str| {
        format!(
            "<< {dictionary} /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
    };
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [10 20 130 140] /CropBox [20 30 120 130] /Rotate {} /Resources << /ExtGState << /G1 9 0 R >> >> >>",
            fixture.rotation
        ),
        format!("<< /Type /Page /Parent 2 0 R /Contents 4 0 R {annots} >>"),
        stream("", content),
        format!(
            "<< /Type /Annot /Subtype /{} {rect} /F {} /AP << /N 6 0 R >> >>",
            fixture.subtype, fixture.flags
        ),
        stream(
            &format!(
                "/Type /XObject /Subtype /Form {bbox} {matrix} {group} {}",
                fixture.filter
            ),
            fixture.paint,
        ),
        format!("<< /Type /Annot /Subtype /Stamp {rect} /F 4 /AP << /N 8 0 R >> >>"),
        stream(
            &format!("/Type /XObject /Subtype /Form /BBox [0 0 24 24] {group}"),
            overlay,
        ),
        "<< /Type /ExtGState /ca 0.5 /CA 0.5 /BM /Normal >>".to_owned(),
    ];
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

fn image(fixture: &Fixture<'_>) -> RenderedPage {
    render_page(&pdf(fixture), 0, 1.0).expect("render fixture")
}
fn pixel(image: &RenderedPage, x: usize, y: usize) -> &[u8] {
    &image.rgba[(y * image.width as usize + x) * 4..][..4]
}
fn anchor(rotation: i32) -> [usize; 2] {
    // Page transform for CropBox [20 30 120 130], at the fixed PDF anchor (50,74).
    match rotation {
        0 => [30, 56],
        90 => [44, 30],
        180 => [70, 44],
        270 => [56, 70],
        _ => unreachable!(),
    }
}

fn changed_bounds(before: &RenderedPage, after: &RenderedPage) -> [usize; 4] {
    assert_eq!((before.width, before.height), (after.width, after.height));
    let mut bounds = [usize::MAX, usize::MAX, 0, 0];
    let mut count = 0;
    for (index, (old, new)) in before
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .enumerate()
    {
        if old != new {
            let (x, y) = (index % before.width as usize, index / before.width as usize);
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x);
            bounds[3] = bounds[3].max(y);
            count += 1;
        }
    }
    assert!(count > 0, "annotation missing");
    bounds
}

#[test]
fn visibility_flags_follow_screen_rules_and_invisible_is_not_blanket_hiding() {
    let baseline = image(&Fixture {
        annotations: false,
        ..Default::default()
    });
    for (subtype, flags) in [
        ("Text", 2),
        ("Text", 32),
        ("Widget", 2),
        ("Widget", 36),
        ("UnknownSynthetic", 1),
    ] {
        assert_eq!(
            image(&Fixture {
                subtype,
                flags,
                ..Default::default()
            }),
            baseline,
            "{subtype} flags{flags}"
        );
    }
    for subtype in ["Text", "Widget", "Stamp", "UnknownSynthetic"] {
        let flags = if subtype == "UnknownSynthetic" { 0 } else { 1 };
        let rendered = image(&Fixture {
            subtype,
            flags,
            ..Default::default()
        });
        assert_eq!(pixel(&rendered, 40, 65), [255, 255, 0, 255]);
        assert_ne!(rendered, baseline);
    }
}

#[test]
fn text_implicitly_stays_upright_and_no_rotate_preserves_the_upper_left_anchor() {
    for rotation in [0, 90, 180, 270] {
        let baseline = image(&Fixture {
            rotation,
            annotations: false,
            ..Default::default()
        });
        let note = image(&Fixture {
            rotation,
            ..Default::default()
        });
        let stamp = image(&Fixture {
            rotation,
            subtype: "Stamp",
            flags: 16,
            ..Default::default()
        });
        assert_eq!(note, stamp);
        let [x, y] = anchor(rotation);
        assert_eq!(changed_bounds(&baseline, &note), [x, y, x + 23, y + 23]);
        assert_eq!(pixel(&note, x + 3, y + 3), [0, 0, 255, 255]);
        assert_eq!(pixel(&note, x + 12, y + 12), [255, 255, 0, 255]);
    }
}

#[test]
fn no_zoom_cancels_logical_zoom_but_retains_dpr_and_capped_raster_resolution() {
    let note = Fixture {
        rect: "60 66 84 90",
        ..Default::default()
    };
    let plain = Fixture {
        rect: note.rect,
        annotations: false,
        ..Default::default()
    };
    for zoom in [0.5, 1.0, 2.0] {
        for dpr in [1.0, 2.0] {
            let scale = zoom * dpr;
            let before = render_view_page(&pdf(&plain), 0, scale, zoom).expect("baseline");
            for (subtype, flags) in [("Text", 4), ("Stamp", 8)] {
                let after = render_view_page(
                    &pdf(&Fixture {
                        rect: note.rect,
                        subtype,
                        flags,
                        ..Default::default()
                    }),
                    0,
                    scale,
                    zoom,
                )
                .expect("view");
                let [x0, y0, x1, y1] = changed_bounds(&before, &after);
                assert_eq!([x0, y0], [(40.0 * scale) as usize; (2)]);
                assert_eq!([x1 - x0 + 1, y1 - y0 + 1], [(24.0 * dpr) as usize; 2]);
            }
            let stamp = render_view_page(
                &pdf(&Fixture {
                    rect: note.rect,
                    subtype: "Stamp",
                    flags: 0,
                    ..Default::default()
                }),
                0,
                scale,
                zoom,
            )
            .expect("scaling stamp");
            let [x0, y0, x1, y1] = changed_bounds(&before, &stamp);
            assert_eq!([x1 - x0 + 1, y1 - y0 + 1], [(24.0 * scale) as usize; 2]);
        }
    }
    // Simulate resolution cap: zoom4/DPR2 would request scale8, but raster plan chose scale2.
    let before = render_view_page(&pdf(&plain), 0, 2.0, 4.0).expect("capped baseline");
    let after = render_view_page(&pdf(&note), 0, 2.0, 4.0).expect("capped note");
    let [x0, y0, x1, y1] = changed_bounds(&before, &after);
    assert_eq!([x1 - x0 + 1, y1 - y0 + 1], [12, 12]);
    assert_eq!((x1 - x0 + 1) as f32 * 4.0 / 2.0, 24.0);
    // Density export at scale2 uses view zoom1, retaining a physical 24-point icon.
    let before = render_page(&pdf(&plain), 0, 2.0).expect("density baseline");
    let after = render_page(&pdf(&note), 0, 2.0).expect("density note");
    let [x0, y0, x1, y1] = changed_bounds(&before, &after);
    assert_eq!([x1 - x0 + 1, y1 - y0 + 1], [48, 48]);
}

#[test]
fn translated_nonzero_appearance_bounds_and_clipping_map_exactly() {
    let expected = image(&Fixture::default());
    let mapped = image(&Fixture {
        bbox: "10 20 22 32",
        matrix: "2 0 0 2 5 7",
        paint: "q 1 1 0 rg 10 20 12 12 re f 0 0 1 rg 10 29 3 3 re f Q",
        ..Default::default()
    });
    assert_eq!(mapped, expected);
    let clipped = image(&Fixture {
        paint: "q 1 1 0 rg -100 -100 1000 1000 re f 0 0 1 rg 0 18 6 6 re f Q",
        ..Default::default()
    });
    assert_eq!(clipped, expected);
    for (bbox, matrix) in [("0 0 0 24", "1 0 0 1 0 0"), ("0 0 24 24", "0 0 0 0 0 0")] {
        let bytes = pdf(&Fixture {
            bbox,
            matrix,
            ..Default::default()
        });
        let original = bytes.clone();
        assert_eq!(
            render_page(&bytes, 0, 1.0),
            Err(RenderError::RendererFailed)
        );
        assert_eq!(bytes, original);
    }
    let bytes = pdf(&Fixture {
        bbox: "0 0 99999999999999999999999999999999999999999999999999.0 24",
        ..Default::default()
    });
    assert_eq!(
        render_page(&bytes, 0, 1.0),
        Err(RenderError::RendererFailed)
    );
    let matrix = format!("{}.0 0 0 1 0 0", "9".repeat(400));
    let bytes = pdf(&Fixture {
        matrix: &matrix,
        ..Default::default()
    });
    assert_eq!(
        render_page(&bytes, 0, 1.0),
        Err(RenderError::RendererFailed)
    );
}

#[test]
fn overlapping_appearances_keep_page_resources_transparency_and_draw_order() {
    for group in [false, true] {
        let rendered = image(&Fixture {
            overlap: true,
            group,
            ..Default::default()
        });
        // /G1 comes from inherited page Resources; the second AP blends over the first.
        let yellow = pixel(&rendered, 42, 68);
        let blue = pixel(&rendered, 33, 59);
        assert_eq!(yellow[0], 255);
        assert!((127..=128).contains(&yellow[1]));
        assert!((127..=128).contains(&yellow[2]));
        assert!((127..=128).contains(&blue[0]));
        assert_eq!(&blue[1..], [0, 255, 255]);
        let baseline = image(&Fixture {
            annotations: false,
            ..Default::default()
        });
        assert_eq!(changed_bounds(&baseline, &rendered), [30, 56, 53, 79]);
    }
}

#[test]
fn view_zoom_validation_and_signed_read_only_rendering_remain_controlled() {
    for zoom in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            render_view_page(b"invalid", 0, 1.0, zoom),
            Err(RenderError::InvalidViewZoom)
        );
    }
    let signed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../trust_lab/corpus/v1/signed.pdf");
    let original = std::fs::read(&signed).expect("signed corpus");
    render_view_page(&original, 0, 0.25, 2.0).expect("read-only signed rendering");
    assert_eq!(
        std::fs::read(&signed).expect("unchanged signed corpus"),
        original
    );
}

#[test]
fn resolved_appearance_with_missing_or_malformed_rectangle_is_a_controlled_error() {
    let baseline = image(&Fixture {
        annotations: false,
        ..Default::default()
    });
    for rect in [
        "",
        "50 50 74",
        "/Invalid 50 74 74",
        "50 50 50 74",
        "50 50 74 74 100",
    ] {
        let bytes = pdf(&Fixture {
            rect,
            ..Default::default()
        });
        let original = bytes.clone();
        assert!(
            matches!(
                render_page(&bytes, 0, 1.0),
                Err(RenderError::RendererFailed)
            ),
            "malformed Rect {rect:?} did not fail"
        );
        assert_eq!(bytes, original);
        for flags in [2, 32] {
            assert_eq!(
                image(&Fixture {
                    rect,
                    flags,
                    ..Default::default()
                }),
                baseline,
                "hidden malformed geometry must not be interpreted"
            );
        }
    }
    // PDF rectangles describe opposite corners; the syntax reader normalizes their order.
    assert_eq!(
        image(&Fixture {
            rect: "74 50 50 74",
            ..Default::default()
        }),
        image(&Fixture::default())
    );
}

#[test]
fn appearance_arrays_require_exact_numeric_arity_and_optional_matrix_identity_is_preserved() {
    let baseline = image(&Fixture {
        annotations: false,
        ..Default::default()
    });
    let normal = image(&Fixture::default());
    for matrix in ["", "null"] {
        assert_eq!(
            image(&Fixture {
                matrix,
                ..Default::default()
            }),
            normal,
            "absent/null Matrix must be identity"
        );
    }
    let cases = [
        ("", "1 0 0 1 0 0"),
        ("null", "1 0 0 1 0 0"),
        ("/Invalid", "1 0 0 1 0 0"),
        ("0 0 24", "1 0 0 1 0 0"),
        ("0 0 /Invalid 24", "1 0 0 1 0 0"),
        ("0 0 24 24 99", "1 0 0 1 0 0"),
        ("0 0 24 24", "/Invalid"),
        ("0 0 24 24", "1 0 0 1 0"),
        ("0 0 24 24", "1 0 /Invalid 1 0 0"),
        ("0 0 24 24", "1 0 0 1 0 0 99"),
        ("0 0 24 24", "1 0 0 1 99 0 R 0"),
    ];
    for (bbox, matrix) in cases {
        let bytes = pdf(&Fixture {
            bbox,
            matrix,
            ..Default::default()
        });
        let original = bytes.clone();
        assert!(
            matches!(
                render_page(&bytes, 0, 1.0),
                Err(RenderError::RendererFailed)
            ),
            "BBox {bbox:?}, Matrix {matrix:?} silently accepted"
        );
        assert_eq!(bytes, original);
        for flags in [2, 32] {
            assert_eq!(
                image(&Fixture {
                    bbox,
                    matrix,
                    flags,
                    ..Default::default()
                }),
                baseline
            );
        }
    }
}

#[test]
fn appearance_geometry_is_checked_before_decode_without_changing_other_warning_policy() {
    // Plain operators are deliberately mislabeled as JPEG data. The old constructor
    // aborts decoding before it reaches geometry; unresolved-appearance behavior is retained.
    let filter = "/Filter /DCTDecode";
    assert!(
        image(&Fixture {
            filter,
            ..Default::default()
        }) == image(&Fixture {
            annotations: false,
            ..Default::default()
        }),
        "undecodable appearance did not retain its previous warning policy"
    );
    for (bbox, matrix) in [
        ("", "1 0 0 1 0 0"),
        ("0 0 0 24", "1 0 0 1 0 0"),
        ("0 0 24 24", "0 0 0 0 0 0"),
        ("0 0 24 24", "1 0 0 1 0 0 99"),
    ] {
        assert!(
            matches!(
                render_page(
                    &pdf(&Fixture {
                        bbox,
                        matrix,
                        filter,
                        ..Default::default()
                    }),
                    0,
                    1.0
                ),
                Err(RenderError::RendererFailed)
            ),
            "geometry was not checked before decode"
        );
    }
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let executable = std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
        || PathBuf::from(name),
        |directory| PathBuf::from(directory).join(name),
    );
    let output = Command::new(executable)
        .args(args)
        .output()
        .expect("reference executable");
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

fn poppler_pixels(path: &Path, aa: bool) -> Vec<u8> {
    let ppm = reference(
        "pdftoppm",
        &[
            "-singlefile",
            "-cropbox",
            "-r",
            "72",
            "-aaVector",
            if aa { "yes" } else { "no" },
            path.to_str().expect("fixture path"),
        ],
    );
    let mut lines = ppm.splitn(4, |byte| *byte == b'\n');
    assert_eq!(lines.next(), Some(b"P6".as_slice()));
    assert_eq!(lines.next(), Some(b"100 100".as_slice()));
    assert_eq!(lines.next(), Some(b"255".as_slice()));
    let rgb = lines.next().expect("PPM pixels").to_vec();
    assert_eq!(rgb.len(), 100 * 100 * 3);
    rgb
}

fn source_point([x, y]: [f64; 2], rotation: i32) -> [f64; 2] {
    match rotation {
        0 => [x - 20.0, 130.0 - y],
        90 => [y - 30.0, x - 20.0],
        180 => [120.0 - x, y - 30.0],
        270 => [130.0 - y, 120.0 - x],
        _ => unreachable!(),
    }
}
fn source_rectangle(rect: [f64; 4], rotation: i32) -> [f64; 4] {
    let a = source_point([rect[0], rect[1]], rotation);
    let b = source_point([rect[2], rect[3]], rotation);
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}
fn source_geometry(fixture: &Fixture<'_>) -> Option<([f64; 4], [f64; 4])> {
    if fixture.flags & (2 | 32) != 0 {
        return None;
    }
    if fixture.subtype == "Text" || fixture.flags & 16 != 0 {
        let [x, y] = anchor(fixture.rotation);
        let (x, y) = (x as f64, y as f64);
        Some(([x, y, x + 24.0, y + 24.0], [x, y, x + 6.0, y + 6.0]))
    } else {
        Some((
            source_rectangle([50.0, 50.0, 74.0, 74.0], fixture.rotation),
            source_rectangle([50.0, 68.0, 56.0, 74.0], fixture.rotation),
        ))
    }
}
fn in_support([x, y]: [f64; 2], rect: [f64; 4]) -> bool {
    x >= rect[0] - 1.0 && x <= rect[2] + 1.0 && y >= rect[1] - 1.0 && y <= rect[3] + 1.0
}
fn near_edge([x, y]: [f64; 2], rect: [f64; 4]) -> bool {
    in_support([x, y], rect)
        && ((x - rect[0]).abs() <= 1.0
            || (x - rect[2]).abs() <= 1.0
            || (y - rect[1]).abs() <= 1.0
            || (y - rect[3]).abs() <= 1.0)
}

fn reference_case(directory: &Path, fixture: &Fixture<'_>, aa: bool) {
    let original = Fixture {
        annotations: false,
        ..fixture.clone()
    };
    let path = directory.join("original.pdf");
    let output = directory.join("annotated.pdf");
    std::fs::write(&path, pdf(&original)).expect("original fixture");
    std::fs::write(&output, pdf(fixture)).expect("annotated fixture");
    let before = poppler_pixels(&path, aa);
    let after = poppler_pixels(&output, aa);
    let native_before = image(&original);
    let native_after = image(fixture);
    let geometry = source_geometry(fixture);
    let mut differences = 0;
    let mut first = None;
    for index in 0..100 * 100 {
        let (x, y) = (index % 100, index / 100);
        let point = [x as f64 + 0.5, y as f64 + 0.5];
        let expected = &after[index * 3..][..3];
        let actual = &native_after.rgba[index * 4..][..3];
        if !geometry.is_some_and(|(outer, _)| in_support(point, outer)) {
            // Exact original-page preservation outside the source AP plus its bounded
            // one-pixel raster edge support. This is not a whole-image error threshold.
            assert_eq!(
                &before[index * 3..][..3],
                expected,
                "reference changed outside AP at{x},{y}"
            );
            assert_eq!(
                &native_before.rgba[index * 4..][..3],
                actual,
                "native changed outside AP at{x},{y}"
            );
            assert_eq!(
                expected, actual,
                "baseline fixture difference outside AP at{x},{y}"
            );
        } else {
            let (outer, marker) = geometry.expect("visible geometry");
            let boundary = near_edge(point, outer) || near_edge(point, marker);
            if !boundary {
                assert_eq!(
                    expected, actual,
                    "interior at{x},{y} rotation{} AA{aa}",
                    fixture.rotation
                );
            }
            if expected != actual {
                differences += 1;
                first.get_or_insert((x, y, expected.to_vec(), actual.to_vec()));
            }
        }
    }
    if let Some((outer, marker)) = geometry {
        // Positive source-derived landmarks establish orientation and position independently.
        for (box_, color) in [(marker, [0, 0, 255]), (outer, [255, 255, 0])] {
            let x = ((box_[0] + box_[2]) / 2.0) as usize;
            let y = ((box_[1] + box_[3]) / 2.0) as usize;
            let index = y * 100 + x;
            assert_eq!(&after[index * 3..][..3], color);
            assert_eq!(&native_after.rgba[index * 4..][..3], color);
        }
        assert_ne!(before, after, "reference appearance missing");
    } else {
        assert_eq!(before, after, "hidden reference appearance drawn");
    }
    println!(
        "Poppler vectorAA{aa} rotation{} {} flags{}: boundary differences{differences}, first{first:?}; full raster equivalence is not claimed",
        fixture.rotation, fixture.subtype, fixture.flags
    );
}

fn pinned_reference() {
    let version = String::from_utf8(reference("pdftoppm", &["-v"])).expect("version UTF8");
    assert!(
        version
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
}

#[test]
#[ignore = "requires independent Poppler26.10.0 pdftoppm"]
fn poppler_differential_verifies_upright_flags_bbox_clipping_and_original_page_pixels() {
    pinned_reference();
    let directory = tempfile::tempdir().expect("temporary fixtures");
    for rotation in [0, 90, 180, 270] {
        for (subtype, flags) in [("Text", 4), ("Stamp", 16), ("Text", 32), ("Widget", 1)] {
            reference_case(
                directory.path(),
                &Fixture {
                    rotation,
                    subtype,
                    flags,
                    ..Default::default()
                },
                false,
            );
        }
    }
    reference_case(
        directory.path(),
        &Fixture {
            bbox: "10 20 22 32",
            matrix: "2 0 0 2 5 7",
            paint: "q 1 1 0 rg 10 20 12 12 re f 0 0 1 rg 10 29 3 3 re f Q",
            ..Default::default()
        },
        false,
    );
    reference_case(
        directory.path(),
        &Fixture {
            paint: "q 1 1 0 rg -100 -100 1000 1000 re f 0 0 1 rg 0 18 6 6 re f Q",
            ..Default::default()
        },
        false,
    );
}

#[test]
#[ignore = "requires independent Poppler26.10.0 pdftoppm"]
fn default_aa_preserves_page_pixels_and_matches_annotation_interiors() {
    pinned_reference();
    let directory = tempfile::tempdir().expect("temporary fixtures");
    for rotation in [0, 90, 180, 270] {
        reference_case(
            directory.path(),
            &Fixture {
                rotation,
                ..Default::default()
            },
            true,
        );
    }
}
