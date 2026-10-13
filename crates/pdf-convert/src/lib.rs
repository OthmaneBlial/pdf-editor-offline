//! Native, restricted image-to-PDF conversion. No file writes or metadata/orientation guessing.

mod jpeg;
mod png_image;

use lopdf::{Document, Object, Stream, dictionary};
use pdf_core::{MAX_INPUT_BYTES, MAX_OBJECTS, MAX_PAGES, MAX_STREAM_BYTES, PdfDocument};
use serde::Serialize;
use std::io::Write;

pub const MAX_IMAGE_DIMENSION: u32 = 8192;
pub const MAX_IMAGE_PIXELS: usize = 8 * 1024 * 1024;
pub const MAX_IMAGE_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid image: {0}")]
    Invalid(&'static str),
    #[error("image conversion unsupported: {0}")]
    Unsupported(&'static str),
    #[error("image resource limit exceeded: {0}")]
    Limit(&'static str),
    #[error("image decode failed: {0}")]
    Decode(&'static str),
    #[error(transparent)]
    Pdf(#[from] lopdf::Error),
    #[error(transparent)]
    Core(#[from] pdf_core::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImagePageInfo {
    pub page: u32,
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub color_space: &'static str,
    pub alpha_mask: bool,
    pub encoding: &'static str,
    pub srgb_profile: bool,
}

pub struct ImageConversion {
    pub document: PdfDocument,
    pub pages: Vec<ImagePageInfo>,
}

struct ImageData {
    width: u32,
    height: u32,
    color_space: &'static str,
    pixels: Vec<u8>,
    alpha: Option<Vec<u8>>,
    format: &'static str,
    jpeg_transform: Option<i64>,
    srgb_intent: Option<&'static str>,
}

fn dimensions(width: u32, height: u32, channels: usize) -> Result<usize> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or(Error::Limit("image dimensions overflow"))?;
    if width == 0 || height == 0 {
        return Err(Error::Invalid("empty image dimensions"));
    }
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION || pixels > MAX_IMAGE_PIXELS {
        return Err(Error::Limit(
            "image exceeds 8192 per side or 8388608 pixels",
        ));
    }
    let bytes = pixels
        .checked_mul(channels)
        .ok_or(Error::Limit("decoded image size overflow"))?;
    if bytes > MAX_STREAM_BYTES {
        return Err(Error::Limit("decoded image exceeds 32 MiB"));
    }
    Ok(bytes)
}

/// Convert PNG/JPEG byte slices in caller order. One source pixel equals one PDF point;
/// image density metadata does not select page dimensions. Unsupported color/orientation
/// metadata is refused. JPEG codestreams are retained; PNG samples/alpha are lossless.
/// Publication uses the returned document's existing verified `save_new` API.
pub fn images_to_pdf(images: &[&[u8]]) -> Result<ImageConversion> {
    if images.is_empty() {
        return Err(Error::Invalid("select at least one image"));
    }
    if images.len() > MAX_PAGES {
        return Err(Error::Limit("more than 20000 image pages"));
    }
    // At most four objects per page (image, mask, content, page), plus tree/catalog/ICC.
    if images.len().saturating_mul(4).saturating_add(3) > MAX_OBJECTS {
        return Err(Error::Limit("generated object count exceeds core limit"));
    }
    let mut input_bytes = 0usize;
    for bytes in images {
        if bytes.len() > MAX_STREAM_BYTES {
            return Err(Error::Limit("one image exceeds 32 MiB"));
        }
        input_bytes = input_bytes
            .checked_add(bytes.len())
            .ok_or(Error::Limit("image input size overflow"))?;
        if input_bytes > MAX_INPUT_BYTES {
            return Err(Error::Limit("image inputs exceed 128 MiB"));
        }
    }
    let mut document = Document::with_version("1.7");
    let parent = document.new_object_id();
    let mut kids = Vec::with_capacity(images.len());
    let mut pages = Vec::with_capacity(images.len());
    let mut stored_bytes = 0usize;
    let mut srgb_profile = None;
    for (index, bytes) in images.iter().enumerate() {
        let image = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            png_image::decode(bytes)?
        } else if bytes.starts_with(b"\xFF\xD8") {
            jpeg::decode(bytes)?
        } else {
            return Err(Error::Unsupported("only PNG and JPEG inputs are supported"));
        };
        let alpha_mask = image.alpha.is_some();
        let mask = if let Some(alpha) = image.alpha {
            let mut mask = Stream::new(
                dictionary! {"Type"=>"XObject", "Subtype"=>"Image", "Width"=>image.width, "Height"=>image.height,
                "ColorSpace"=>"DeviceGray", "BitsPerComponent"=>8, "Interpolate"=>false},
                alpha,
            );
            mask.compress()?;
            stored_bytes = stored_bytes.saturating_add(mask.content.len());
            Some(document.add_object(mask))
        } else {
            None
        };
        let mut dictionary = dictionary! {"Type"=>"XObject", "Subtype"=>"Image", "Width"=>image.width, "Height"=>image.height,
        "ColorSpace"=>image.color_space, "BitsPerComponent"=>8, "Interpolate"=>false};
        if let Some(intent) = image.srgb_intent {
            let profile = if let Some(profile) = srgb_profile {
                profile
            } else {
                let bytes = moxcms::ColorProfile::new_srgb()
                    .encode()
                    .map_err(|_| Error::Invalid("standard sRGB profile encoding failed"))?;
                let profile = document.add_object(Stream::new(
                    dictionary! {"N"=>3,"Alternate"=>"DeviceRGB"},
                    bytes,
                ));
                srgb_profile = Some(profile);
                profile
            };
            dictionary.set(
                "ColorSpace",
                vec![
                    Object::Name(b"ICCBased".to_vec()),
                    Object::Reference(profile),
                ],
            );
            dictionary.set("Intent", intent);
        }
        if let Some(mask) = mask {
            dictionary.set("SMask", mask);
        }
        let original_jpeg = image.format == "jpeg";
        if original_jpeg {
            dictionary.set("Filter", "DCTDecode");
            if let Some(transform) = image.jpeg_transform {
                dictionary.set("DecodeParms", dictionary! {"ColorTransform"=>transform});
            }
        }
        let mut stream = Stream::new(dictionary, image.pixels);
        if !original_jpeg {
            stream.compress()?;
        }
        stored_bytes = stored_bytes.saturating_add(stream.content.len());
        if stored_bytes > MAX_INPUT_BYTES {
            return Err(Error::Limit("stored image payloads exceed 128 MiB"));
        }
        let image_id = document.add_object(stream);
        let content = document.add_object(Stream::new(
            dictionary! {},
            format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", image.width, image.height).into_bytes(),
        ));
        let page = document.add_object(dictionary! {"Type"=>"Page", "Parent"=>parent,
        "MediaBox"=>vec![0.into(),0.into(),image.width.into(),image.height.into()],
        "Resources"=>dictionary! {"XObject"=>dictionary! {"Im0"=>image_id}}, "Contents"=>content});
        kids.push(Object::Reference(page));
        pages.push(ImagePageInfo {
            page: index as u32 + 1,
            format: image.format,
            width: image.width,
            height: image.height,
            color_space: if image.srgb_intent.is_some() {
                "ICCBased-sRGB"
            } else {
                image.color_space
            },
            alpha_mask,
            encoding: if original_jpeg {
                "original_dct"
            } else {
                "lossless_samples"
            },
            srgb_profile: image.srgb_intent.is_some(),
        });
    }
    document.objects.insert(
        parent,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>images.len() as i64}.into(),
    );
    let catalog = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>parent});
    document.trailer.set("Root", catalog);
    let mut output = BoundedOutput(Vec::new());
    if let Err(error) = document.save_to(&mut output) {
        if error.kind() == std::io::ErrorKind::FileTooLarge {
            return Err(Error::Limit("PDF output exceeds 128 MiB"));
        }
        return Err(error.into());
    }
    let document = PdfDocument::from_bytes(&output.0, None)?;
    if document.page_count() != pages.len() {
        return Err(Error::Invalid("generated page count changed"));
    }
    Ok(ImageConversion { document, pages })
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_INPUT_BYTES {
            return Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Decode one PNG/JPEG with the existing bounded color/orientation policy and
/// insert its real image XObject into a page. This adds content; it does not edit
/// or replace an existing image. The destination rectangle controls aspect ratio.
pub fn add_image_overlay(
    document: &mut PdfDocument,
    placement: &pdf_core::ImageOverlay,
    image: &[u8],
) -> Result<ImagePageInfo> {
    document.ensure_editable()?;
    let conversion = images_to_pdf(&[image])?;
    document.add_image_overlay(placement, &conversion.document)?;
    conversion
        .pages
        .into_iter()
        .next()
        .ok_or(Error::Invalid("converted image report missing"))
}

/// Replace exactly one source-bound direct page image Do using the validated PNG/JPEG
/// decoder. The original transform/clipping is retained; this is not secure erasure.
pub fn replace_image_instance(
    document: &mut PdfDocument,
    selection: &pdf_core::ImageSelection,
    image: &[u8],
) -> Result<ImagePageInfo> {
    document.ensure_editable()?;
    let conversion = images_to_pdf(&[image])?;
    document.replace_image_instance(selection, &conversion.document)?;
    conversion
        .pages
        .into_iter()
        .next()
        .ok_or(Error::Invalid("converted replacement image report missing"))
}
