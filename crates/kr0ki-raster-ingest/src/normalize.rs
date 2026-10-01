//! Turn an untrusted upload into a bounded, metadata-free PNG the model can be shown.
//!
//! Order matters: cheap checks first (size, sniffed format, header-declared dimensions, animation), and only
//! then a decode under [`image::Limits`]. The result is re-encoded as PNG, which drops EXIF/ICC/text chunks;
//! JPEG orientation is applied *before* that so a rotated phone photo is not stored sideways.

use image::{
    codecs::{png::PngDecoder, webp::WebPDecoder},
    imageops::FilterType,
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbImage,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizeConfig {
    /// Reject anything larger before looking inside it.
    pub max_bytes: usize,
    /// Reject anything declaring more pixels than this, from the header, before decoding.
    pub max_pixels: u64,
    /// Downscale so the longest side is at most this (aspect ratio kept).
    pub max_side: u32,
}

impl Default for NormalizeConfig {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024,
            max_pixels: 16_000_000,
            max_side: 2048,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone)]
pub struct NormalizedImage {
    /// The PNG to show the model.
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// SHA-256 (hex) of `png`: the content identity used in cache keys.
    pub sha256: String,
    /// SHA-256 (hex) of the bytes as uploaded.
    pub source_sha256: String,
    pub source_format: SourceFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NormalizeError {
    #[error("image is empty")]
    Empty,
    #[error("image is {actual} bytes; the limit is {max}")]
    TooLarge { actual: usize, max: usize },
    #[error("unsupported image type: {0} (PNG, JPEG and WebP only)")]
    UnsupportedFormat(String),
    #[error("animated images are not supported")]
    Animated,
    #[error("image declares {width}x{height} pixels; the limit is {max}")]
    TooManyPixels { width: u32, height: u32, max: u64 },
    #[error("image could not be decoded: {0}")]
    Undecodable(String),
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn undecodable(e: impl std::fmt::Display) -> NormalizeError {
    NormalizeError::Undecodable(e.to_string())
}

///
/// **Cost.** CPU-bound and synchronous: async callers must run it under `spawn_blocking`. Peak memory is roughly
/// 11 bytes per declared pixel (decode buffer, RGBA copy for alpha images, flattened RGB), so the default 16 MP cap
/// is about 180 MB per upload in the worst case; cap concurrent uploads accordingly.
pub fn normalize(bytes: &[u8], cfg: &NormalizeConfig) -> Result<NormalizedImage, NormalizeError> {
    if bytes.is_empty() {
        return Err(NormalizeError::Empty);
    }
    if bytes.len() > cfg.max_bytes {
        return Err(NormalizeError::TooLarge {
            actual: bytes.len(),
            max: cfg.max_bytes,
        });
    }

    // Content sniffing, never the caller's Content-Type or file extension.
    let (format, source_format) = match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => (ImageFormat::Png, SourceFormat::Png),
        Ok(ImageFormat::Jpeg) => (ImageFormat::Jpeg, SourceFormat::Jpeg),
        Ok(ImageFormat::WebP) => (ImageFormat::WebP, SourceFormat::Webp),
        Ok(other) => return Err(NormalizeError::UnsupportedFormat(format!("{other:?}"))),
        Err(_) => return Err(NormalizeError::UnsupportedFormat("unrecognized".into())),
    };

    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(undecodable)?;
    if width == 0 || height == 0 {
        return Err(NormalizeError::Undecodable("zero-sized image".into()));
    }
    if u64::from(width) * u64::from(height) > cfg.max_pixels {
        return Err(NormalizeError::TooManyPixels {
            width,
            height,
            max: cfg.max_pixels,
        });
    }

    let mut limits = Limits::default();
    limits.max_alloc = Some(512 * 1024 * 1024);

    let (image, orientation) = match format {
        ImageFormat::Png => {
            let decoder =
                PngDecoder::with_limits(Cursor::new(bytes), limits).map_err(undecodable)?;
            if decoder.is_apng().map_err(undecodable)? {
                return Err(NormalizeError::Animated);
            }
            decode(decoder)?
        }
        ImageFormat::WebP => {
            let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(undecodable)?;
            decoder.set_limits(limits).map_err(undecodable)?;
            if decoder.has_animation() {
                return Err(NormalizeError::Animated);
            }
            decode(decoder)?
        }
        _ => {
            let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
            reader.limits(limits);
            decode(reader.into_decoder().map_err(undecodable)?)?
        }
    };
    let mut image = image;
    image.apply_orientation(orientation);

    // Opaque images skip the alpha pass and its extra RGBA copy.
    let mut rgb = if image.color().has_alpha() {
        flatten_onto_white(&image)
    } else {
        image.to_rgb8()
    };
    drop(image);
    if rgb.width().max(rgb.height()) > cfg.max_side {
        rgb = DynamicImage::ImageRgb8(rgb)
            .resize(cfg.max_side, cfg.max_side, FilterType::Lanczos3)
            .to_rgb8();
    }

    let (width, height) = (rgb.width(), rgb.height());
    let mut png = Vec::new();
    rgb.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(undecodable)?;

    Ok(NormalizedImage {
        sha256: hex(&png),
        source_sha256: hex(bytes),
        png,
        width,
        height,
        source_format,
    })
}

fn decode(
    mut decoder: impl ImageDecoder,
) -> Result<(DynamicImage, image::metadata::Orientation), NormalizeError> {
    let orientation = decoder.orientation().map_err(undecodable)?;
    let image = DynamicImage::from_decoder(decoder).map_err(undecodable)?;
    Ok((image, orientation))
}

/// Composite over white so transparent diagrams are legible and every model sees the same background.
fn flatten_onto_white(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = u32::from(p[3]);
        let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    out
}
