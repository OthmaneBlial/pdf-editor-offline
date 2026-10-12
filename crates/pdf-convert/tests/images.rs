#![allow(clippy::unwrap_used)]

use lopdf::{Document, Object};
use pdf_convert::{Error, images_to_pdf};
use std::path::PathBuf;
use std::process::Command;

fn png(
    color: png::ColorType,
    pixels: &[u8],
    palette: Option<&[u8]>,
    trns: Option<&[u8]>,
    srgb: bool,
) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, 4, 2);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    if let Some(palette) = palette {
        encoder.set_palette(palette);
    }
    if let Some(alpha) = trns {
        encoder.set_trns(alpha);
    }
    if srgb {
        encoder.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
    }
    {
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
    }
    output
}

fn jpeg() -> Vec<u8> {
    let mut output = Vec::new();
    let mut pixels = Vec::new();
    for y in 0..16 {
        for _ in 0..16 {
            pixels.extend_from_slice(if y < 8 { &[255, 0, 0] } else { &[0, 0, 255] });
        }
    }
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 100)
        .encode(&pixels, 16, 16, image::ExtendedColorType::Rgb8)
        .unwrap();
    output
}

fn image_streams(document: &Document) -> Vec<((u32, u16), &lopdf::Stream)> {
    document
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            (stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
                .then_some((*id, stream))
        })
        .collect()
}

fn decoded(stream: &lopdf::Stream) -> Vec<u8> {
    stream.decompressed_content().unwrap()
}

#[test]
fn rgba_samples_and_straight_alpha_are_exact_and_pages_keep_caller_order() {
    let samples = [
        255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 64, 0, 0, 255, 255, 255, 255, 0,
        255, 20, 30, 40, 128, 40, 30, 20, 255,
    ];
    let rgba = png(png::ColorType::Rgba, &samples, None, None, false);
    let gray = png(
        png::ColorType::Grayscale,
        &[0, 64, 128, 255, 255, 128, 64, 0],
        None,
        None,
        false,
    );
    let originals = [rgba.clone(), gray.clone()];
    let conversion = images_to_pdf(&[&rgba, &gray, &rgba]).unwrap();
    assert_eq!(conversion.document.page_count(), 3);
    assert_eq!(
        conversion
            .pages
            .iter()
            .map(|p| (p.page, p.width, p.height, p.alpha_mask, p.color_space))
            .collect::<Vec<_>>(),
        [
            (1, 4, 2, true, "DeviceRGB"),
            (2, 4, 2, false, "DeviceGray"),
            (3, 4, 2, true, "DeviceRGB")
        ]
    );
    let bytes = conversion.document.to_bytes().unwrap();
    let raw = Document::load_mem(&bytes).unwrap();
    let images = image_streams(&raw);
    let stream = images
        .iter()
        .find(|(_, stream)| stream.dict.has(b"SMask"))
        .unwrap()
        .1;
    assert_eq!(
        decoded(stream),
        samples
            .chunks_exact(4)
            .flat_map(|pixel| pixel[..3].iter().copied())
            .collect::<Vec<_>>()
    );
    let mask = stream.dict.get(b"SMask").unwrap().as_reference().unwrap();
    assert_eq!(
        decoded(raw.get_object(mask).unwrap().as_stream().unwrap()),
        samples
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>()
    );
    for page in conversion.document.info().unwrap().pages {
        assert_eq!(page.media_box, [0.0, 0.0, 4.0, 2.0]);
        assert_eq!(page.rotation, 0);
    }
    let rendered = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    assert_eq!(&rendered.rgba[..4], [255, 0, 0, 255]);
    assert_eq!(&rendered.rgba[16..20], [0, 0, 255, 255]);
    assert_eq!(rgba, originals[0]);
    assert_eq!(gray, originals[1]);
}

#[test]
fn palette_transparency_and_color_key_expand_losslessly() {
    let indices = [0, 1, 2, 3, 3, 2, 1, 0];
    let palette = [255, 0, 0, 0, 255, 0, 0, 0, 255, 20, 30, 40];
    let alpha = [255, 128, 0, 64];
    let indexed = png(
        png::ColorType::Indexed,
        &indices,
        Some(&palette),
        Some(&alpha),
        false,
    );
    let conversion = images_to_pdf(&[&indexed]).unwrap();
    let raw = Document::load_mem(&conversion.document.to_bytes().unwrap()).unwrap();
    let image = image_streams(&raw)
        .into_iter()
        .find(|(_, stream)| stream.dict.has(b"SMask"))
        .unwrap()
        .1;
    assert_eq!(
        decoded(image),
        indices
            .iter()
            .flat_map(|index| palette[*index as usize * 3..][..3].iter().copied())
            .collect::<Vec<_>>()
    );
    let mask = raw
        .get_object(image.dict.get(b"SMask").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(decoded(mask), indices.map(|index| alpha[index as usize]));
    let rgb = [
        255, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0, 255, 255, 0, 0,
    ];
    let keyed = png(
        png::ColorType::Rgb,
        &rgb,
        None,
        Some(&[0, 255, 0, 0, 0, 0]),
        false,
    );
    let conversion = images_to_pdf(&[&keyed]).unwrap();
    let raw = Document::load_mem(&conversion.document.to_bytes().unwrap()).unwrap();
    let image = image_streams(&raw)
        .into_iter()
        .find(|(_, stream)| stream.dict.has(b"SMask"))
        .unwrap()
        .1;
    assert_eq!(decoded(image), rgb);
    let mask = raw
        .get_object(image.dict.get(b"SMask").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(decoded(mask), [0, 255, 0, 255, 255, 0, 255, 0]);
}

#[test]
fn srgb_profile_and_intent_are_real_and_grayscale_values_remain_exact() {
    let samples = [0, 64, 128, 255, 255, 128, 64, 0];
    let gray = png(png::ColorType::Grayscale, &samples, None, None, true);
    let conversion = images_to_pdf(&[&gray, &gray]).unwrap();
    assert!(conversion.pages.iter().all(|page| page.srgb_profile));
    let bytes = conversion.document.to_bytes().unwrap();
    let raw = Document::load_mem(&bytes).unwrap();
    let profiles = image_streams(&raw)
        .into_iter()
        .map(|(_, stream)| {
            assert_eq!(
                stream.dict.get(b"Intent").unwrap().as_name().unwrap(),
                b"RelativeColorimetric"
            );
            let colorspace = stream.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
            assert_eq!(colorspace[0].as_name().unwrap(), b"ICCBased");
            assert_eq!(
                decoded(stream),
                samples
                    .into_iter()
                    .flat_map(|gray| [gray, gray, gray])
                    .collect::<Vec<_>>()
            );
            colorspace[1].as_reference().unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(profiles[0], profiles[1]);
    let profile = raw.get_object(profiles[0]).unwrap().as_stream().unwrap();
    assert_eq!(profile.dict.get(b"N").unwrap().as_i64().unwrap(), 3);
    let profile = moxcms::ColorProfile::new_from_slice(&profile.content).unwrap();
    assert!(profile.description.is_some());
    let rendered = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    for (pixel, gray) in rendered.rgba.chunks_exact(4).zip(samples) {
        for value in &pixel[..3] {
            assert!((*value as i16 - gray as i16).abs() <= 1);
        }
    }
}

#[test]
fn jpeg_codestream_is_preserved_after_strict_decode_and_verified_save() {
    let jpeg = jpeg();
    let original = jpeg.clone();
    let conversion = images_to_pdf(&[&jpeg]).unwrap();
    assert_eq!(conversion.pages[0].encoding, "original_dct");
    assert_eq!(
        (conversion.pages[0].width, conversion.pages[0].height),
        (16, 16)
    );
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("jpeg.pdf");
    conversion.document.save_new(&output).unwrap();
    let raw = Document::load(&output).unwrap();
    let stream = image_streams(&raw)[0].1;
    assert_eq!(
        stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
        b"DCTDecode"
    );
    assert_eq!(stream.content, jpeg);
    let rendered = pdf_render::render_page(&std::fs::read(&output).unwrap(), 0, 1.0).unwrap();
    assert!(rendered.rgba[0] > 245 && rendered.rgba[2] < 10);
    let bottom = (15 * 16) * 4;
    assert!(rendered.rgba[bottom + 2] > 245 && rendered.rgba[bottom] < 10);
    assert_eq!(jpeg, original);
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut output = (data.len() as u32).to_be_bytes().to_vec();
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let crc = crc32fast::hash(&output[4..]);
    output.extend_from_slice(&crc.to_be_bytes());
    output
}

#[test]
fn packed_grayscale_gray_alpha_and_gray_jpeg_preserve_actual_samples() {
    let mut packed = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut packed, 4, 2);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::One);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[0x50, 0xA0])
            .unwrap();
    }
    let gray_alpha = png(
        png::ColorType::GrayscaleAlpha,
        &[
            0, 255, 64, 128, 128, 0, 255, 64, 255, 255, 128, 128, 64, 0, 0, 255,
        ],
        None,
        None,
        false,
    );
    let mut gray_jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut gray_jpeg, 100)
        .encode(&[64; 64], 8, 8, image::ExtendedColorType::L8)
        .unwrap();
    let conversion = images_to_pdf(&[&packed, &gray_alpha, &gray_jpeg]).unwrap();
    let raw = Document::load_mem(&conversion.document.to_bytes().unwrap()).unwrap();
    let images = image_streams(&raw);
    assert_eq!(decoded(images[0].1), [0, 255, 0, 255, 255, 0, 255, 0]);
    let alpha = images
        .iter()
        .find(|(_, stream)| stream.dict.has(b"SMask"))
        .unwrap()
        .1;
    assert_eq!(decoded(alpha), [0, 64, 128, 255, 255, 128, 64, 0]);
    assert_eq!(
        decoded(
            raw.get_object(alpha.dict.get(b"SMask").unwrap().as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap()
        ),
        [255, 128, 0, 64, 255, 128, 0, 255]
    );
    let jpeg = images
        .iter()
        .find(|(_, stream)| {
            stream.dict.get(b"Filter").and_then(Object::as_name).ok() == Some(b"DCTDecode")
        })
        .unwrap()
        .1;
    assert_eq!(jpeg.content, gray_jpeg);
    assert_eq!(
        jpeg.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
        b"DeviceGray"
    );
}

#[test]
fn metadata_and_dimension_limits_precede_decoder_allocation() {
    let valid = png(png::ColorType::Rgb, &[255; 24], None, None, false);
    let oversized_metadata = insert_png(
        &valid,
        b"iCCP",
        &vec![0; pdf_convert::MAX_IMAGE_METADATA_BYTES + 1],
    );
    assert!(matches!(
        images_to_pdf(&[&oversized_metadata]),
        Err(Error::Limit(_))
    ));
    let mut oversized_jpeg = jpeg();
    let sof = oversized_jpeg
        .windows(2)
        .position(|marker| marker == [0xFF, 0xC0])
        .unwrap();
    // Width in SOF0 precedes all decoded sample allocations.
    oversized_jpeg[sof + 7..sof + 9].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(matches!(
        images_to_pdf(&[&oversized_jpeg]),
        Err(Error::Limit(_))
    ));
    let valid_jpeg = jpeg();
    let metadata = vec![0; 65530];
    let mut oversized = valid_jpeg;
    for _ in 0..17 {
        oversized = app1(&oversized, 0xFE, &metadata);
    }
    assert!(matches!(images_to_pdf(&[&oversized]), Err(Error::Limit(_))));
    let mut header = valid[16..29].to_vec();
    header[12] = 1;
    let interlaced = [
        &valid[..8],
        chunk(b"IHDR", &header).as_slice(),
        &valid[33..],
    ]
    .concat();
    assert!(matches!(
        images_to_pdf(&[&interlaced]),
        Err(Error::Unsupported(_))
    ));
}
fn insert_png(bytes: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    [&bytes[..33], chunk(kind, data).as_slice(), &bytes[33..]].concat()
}
fn app1(bytes: &[u8], marker: u8, data: &[u8]) -> Vec<u8> {
    let mut output = bytes[..2].to_vec();
    output.extend_from_slice(&[0xFF, marker]);
    output.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
    output.extend_from_slice(data);
    output.extend_from_slice(&bytes[2..]);
    output
}

#[test]
fn corrupt_bombs_unsupported_color_orientation_and_late_failures_never_return_partial_documents() {
    let valid = png(png::ColorType::Rgb, &[255; 24], None, None, false);
    let original = valid.clone();
    assert!(images_to_pdf(&[]).is_err());
    assert!(images_to_pdf(&[b"not an image"]).is_err());
    let mut corrupt = valid.clone();
    corrupt[30] ^= 1;
    assert!(matches!(images_to_pdf(&[&corrupt]), Err(Error::Invalid(_))));
    let idat = valid.windows(4).position(|kind| kind == b"IDAT").unwrap();
    let length = u32::from_be_bytes(valid[idat - 4..idat].try_into().unwrap()) as usize;
    let mut payload = valid[idat + 4..idat + 4 + length].to_vec();
    payload[0] = 0; // Fresh CRC, invalid zlib header: exercise the bounded decoder.
    let bad_pixels = [
        &valid[..idat - 4],
        chunk(b"IDAT", &payload).as_slice(),
        &valid[idat + 8 + length..],
    ]
    .concat();
    assert!(matches!(
        images_to_pdf(&[&bad_pixels]),
        Err(Error::Decode(_))
    ));
    let mut header = valid[16..29].to_vec();
    header[..4].copy_from_slice(&9000u32.to_be_bytes());
    let bomb = [
        &valid[..8],
        chunk(b"IHDR", &header).as_slice(),
        &valid[33..],
    ]
    .concat();
    assert!(matches!(images_to_pdf(&[&bomb]), Err(Error::Limit(_))));
    let mut header = valid[16..29].to_vec();
    header[8] = 16;
    let depth = [
        &valid[..8],
        chunk(b"IHDR", &header).as_slice(),
        &valid[33..],
    ]
    .concat();
    assert!(matches!(
        images_to_pdf(&[&depth]),
        Err(Error::Unsupported(_))
    ));
    for (kind, data) in [
        (b"iCCP", b"profile\0\0fake-compressed-bomb".as_slice()),
        (b"gAMA", b"\0\0\xB1\x8F"),
        (b"eXIf", b"orientation6"),
        (b"acTL", b"\0\0\0\x02\0\0\0\0"),
    ] {
        let unsupported = insert_png(&valid, kind, data);
        assert!(matches!(
            images_to_pdf(&[&unsupported]),
            Err(Error::Unsupported(_))
        ));
    }
    let jpeg = jpeg();
    let rotated = app1(
        &jpeg,
        0xE1,
        b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0",
    );
    assert!(matches!(
        images_to_pdf(&[&rotated]),
        Err(Error::Unsupported(_))
    ));
    let icc = app1(&jpeg, 0xE2, b"ICC_PROFILE\0\x01\x01profile");
    assert!(matches!(images_to_pdf(&[&icc]), Err(Error::Unsupported(_))));
    assert!(images_to_pdf(&[&jpeg[..jpeg.len() - 2]]).is_err());
    assert!(images_to_pdf(&[&valid, &rotated]).is_err());
    assert_eq!(valid, original);
    let conversion = images_to_pdf(&[&valid]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.pdf");
    std::fs::write(&path, b"sentinel").unwrap();
    assert!(conversion.document.save_new(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"sentinel");
}

fn reference(name: &str, args: &[&str]) -> Vec<u8> {
    let path = if name == "qpdf" {
        std::env::var_os("QPDF")
            .map(PathBuf::from)
            .unwrap_or_else(|| name.into())
    } else {
        std::env::var_os("PDF_REFERENCE_BIN_DIR").map_or_else(
            || name.into(),
            |directory| PathBuf::from(directory).join(name),
        )
    };
    let output = Command::new(path).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if name == "pdftoppm" && args != ["-v"] {
        assert!(
            output.stderr.is_empty(),
            "reference rendering diagnostics: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    if args == ["-v"] {
        [output.stdout, output.stderr].concat()
    } else {
        output.stdout
    }
}

#[test]
#[ignore = "requires independent Poppler26.10.0 and qpdf12.4.2"]
fn independent_readers_verify_geometry_pixels_soft_masks_and_original_jpeg_data() {
    assert!(
        String::from_utf8(reference("pdftoppm", &["-v"]))
            .unwrap()
            .lines()
            .any(|line| line == "pdftoppm version 26.10.0")
    );
    assert!(
        String::from_utf8(reference("qpdf", &["--version"]))
            .unwrap()
            .lines()
            .any(|line| line == "qpdf version 12.4.2")
    );
    let samples = [
        255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 64, 0, 0, 255, 255, 255, 255, 0,
        255, 20, 30, 40, 128, 40, 30, 20, 255,
    ];
    let alpha = png(png::ColorType::Rgba, &samples, None, None, false);
    let tagged = png(
        png::ColorType::Rgb,
        &[
            255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0,
            0,
        ],
        None,
        None,
        true,
    );
    let jpeg = jpeg();
    let palette = png(
        png::ColorType::Indexed,
        &[0, 1, 2, 0, 2, 1, 0, 2],
        Some(&[255, 0, 0, 0, 255, 0, 0, 0, 255]),
        Some(&[255, 128, 0]),
        false,
    );
    let conversion = images_to_pdf(&[&alpha, &tagged, &jpeg, &palette]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("images.pdf");
    conversion.document.save_new(&output).unwrap();
    let path = output.to_str().unwrap();
    reference("qpdf", &["--check", path]);
    let model: serde_json::Value =
        serde_json::from_slice(&reference("qpdf", &["--json", path])).unwrap();
    assert_eq!(model["pages"].as_array().unwrap().len(), 4);
    let raw = Document::load(&output).unwrap();
    for (id, stream) in image_streams(&raw) {
        let object = format!("--show-object={},{}", id.0, id.1);
        if stream.dict.get(b"Filter").and_then(Object::as_name).ok() == Some(b"DCTDecode") {
            assert_eq!(
                reference("qpdf", &[&object, "--raw-stream-data", path]),
                jpeg
            );
        } else {
            assert_eq!(
                reference("qpdf", &[&object, "--filtered-stream-data", path]),
                decoded(stream)
            );
        }
    }
    let bytes = std::fs::read(&output).unwrap();
    // Preserve the observed density-1 transparency disagreement as explicit evidence,
    // instead of claiming that exact stored samples imply identical viewer sampling.
    let ppm = reference(
        "pdftoppm",
        &["-singlefile", "-f", "1", "-cropbox", "-r", "72", path],
    );
    let rgb = ppm.splitn(4, |byte| *byte == b'\n').nth(3).unwrap();
    let native = pdf_render::render_page(&bytes, 0, 1.0).unwrap();
    assert_eq!(&native.rgba[4..7], &[127, 255, 127]);
    assert_eq!(&rgb[3..6], &[132, 224, 102]);
    for page in 1..=4 {
        let number = page.to_string();
        let ppm = reference(
            "pdftoppm",
            &["-singlefile", "-f", &number, "-cropbox", "-r", "576", path],
        );
        let mut lines = ppm.splitn(4, |byte| *byte == b'\n');
        assert_eq!(lines.next(), Some(b"P6".as_slice()));
        let dimensions = lines.next().unwrap();
        assert_eq!(
            dimensions,
            if page == 3 {
                b"128 128".as_slice()
            } else {
                b"32 16".as_slice()
            }
        );
        assert_eq!(lines.next(), Some(b"255".as_slice()));
        let rgb_reference = lines.next().unwrap();
        let native = pdf_render::render_page(&bytes, page - 1, 8.0).unwrap();
        assert_eq!(
            rgb_reference.len(),
            native.width as usize * native.height as usize * 3
        );
        // Source pixels are eight device pixels wide. Compare the interior center;
        // boundary interpolation differs between readers (tracked separately below).
        for y in (4..native.height as usize).step_by(8) {
            for x in (4..native.width as usize).step_by(8) {
                let pixel = y * native.width as usize + x;
                let actual = &native.rgba[pixel * 4..pixel * 4 + 4];
                let expected = &rgb_reference[pixel * 3..pixel * 3 + 3];
                for (a, b) in actual[..3].iter().zip(expected) {
                    assert!(
                        (*a as i16 - *b as i16).abs() <= 1,
                        "page{page} RGB {actual:?} != {expected:?}"
                    );
                }
            }
        }
        let info = String::from_utf8(reference(
            "pdfinfo",
            &["-f", &number, "-l", &number, "-box", path],
        ))
        .unwrap();
        let media = info
            .lines()
            .find(|line| line.contains("MediaBox:"))
            .unwrap();
        let values = media
            .split_once("MediaBox:")
            .unwrap()
            .1
            .split_whitespace()
            .map(|number| number.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            if page == 3 {
                vec![0.0, 0.0, 16.0, 16.0]
            } else {
                vec![0.0, 0.0, 4.0, 2.0]
            }
        );
    }
}
