//! Limits and safety of the upload path.

use image::{
    codecs::jpeg::JpegEncoder, DynamicImage, ImageEncoder, ImageFormat, Rgb, RgbImage, Rgba,
    RgbaImage,
};
use kr0ki_raster_ingest::*;
use std::io::Cursor;

fn encode(img: &DynamicImage, fmt: ImageFormat) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), fmt).unwrap();
    out
}
fn png(w: u32, h: u32) -> Vec<u8> {
    encode(
        &DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([10, 120, 200]))),
        ImageFormat::Png,
    )
}
fn cfg() -> NormalizeConfig {
    NormalizeConfig::default()
}
fn decode(png: &[u8]) -> DynamicImage {
    image::load_from_memory_with_format(png, ImageFormat::Png).unwrap()
}

#[test]
fn a_valid_png_is_normalized_and_identified_by_content() {
    let n = normalize(&png(40, 20), &cfg()).unwrap();
    assert_eq!(
        (n.width, n.height, n.source_format),
        (40, 20, SourceFormat::Png)
    );
    assert_eq!(n.sha256.len(), 64);
    assert_eq!(n.source_sha256.len(), 64);
    assert_eq!(decode(&n.png).width(), 40);
    // identical input -> identical identity; different pixels -> different identity
    assert_eq!(n.sha256, normalize(&png(40, 20), &cfg()).unwrap().sha256);
    assert_ne!(n.sha256, normalize(&png(41, 20), &cfg()).unwrap().sha256);
}

#[test]
fn jpeg_and_webp_are_accepted_and_come_back_as_png() {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 16, Rgb([90, 90, 90])));
    let jpeg = normalize(&encode(&img, ImageFormat::Jpeg), &cfg()).unwrap();
    assert_eq!(
        (jpeg.source_format, jpeg.width, jpeg.height),
        (SourceFormat::Jpeg, 32, 16)
    );
    let webp = normalize(&encode(&img, ImageFormat::WebP), &cfg()).unwrap();
    assert_eq!(
        (webp.source_format, webp.width, webp.height),
        (SourceFormat::Webp, 32, 16)
    );
    assert_eq!(decode(&jpeg.png).height(), 16);
}

#[test]
fn transparency_is_composited_onto_white() {
    let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0])); // fully transparent
    img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // opaque red
    img.put_pixel(1, 0, Rgba([0, 0, 0, 128])); // half-transparent black
    let n = normalize(
        &encode(&DynamicImage::ImageRgba8(img), ImageFormat::Png),
        &cfg(),
    )
    .unwrap();
    let out = decode(&n.png).to_rgb8();
    assert_eq!(
        out.get_pixel(3, 3).0,
        [255, 255, 255],
        "transparent -> white"
    );
    assert_eq!(out.get_pixel(0, 0).0, [255, 0, 0], "opaque is untouched");
    let half = out.get_pixel(1, 0).0;
    assert!(
        (120..=135).contains(&half[0]) && half[0] == half[1] && half[1] == half[2],
        "{half:?}"
    );
}

#[test]
fn large_images_are_downscaled_keeping_the_aspect_ratio() {
    let mut c = cfg();
    c.max_side = 50;
    let n = normalize(&png(200, 100), &c).unwrap();
    assert_eq!((n.width, n.height), (50, 25));
    let small = normalize(&png(30, 10), &c).unwrap();
    assert_eq!(
        (small.width, small.height),
        (30, 10),
        "images within the cap are never upscaled"
    );
}

#[test]
fn exif_orientation_is_applied_before_the_metadata_is_dropped() {
    // Minimal little-endian TIFF/EXIF block with Orientation = 6 (rotate 90 degrees clockwise).
    let exif = vec![
        0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, // TIFF header, IFD at 8
        0x01, 0x00, // one entry
        0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00,
        0x00, // tag 0x0112, SHORT, 1, value 6
        0x00, 0x00, 0x00, 0x00, // no next IFD
    ];
    let img = RgbImage::from_pixel(20, 10, Rgb([1, 2, 3]));
    let mut jpeg = Vec::new();
    let mut enc = JpegEncoder::new(&mut jpeg);
    enc.set_exif_metadata(exif).unwrap();
    enc.write_image(img.as_raw(), 20, 10, image::ExtendedColorType::Rgb8)
        .unwrap();

    let n = normalize(&jpeg, &cfg()).unwrap();
    assert_eq!(
        (n.width, n.height),
        (10, 20),
        "a 20x10 image tagged rotate-90 is stored 10x20"
    );
    assert!(
        !n.png.windows(4).any(|w| w == b"eXIf"),
        "no EXIF survives re-encoding"
    );
}

#[test]
fn unsupported_types_are_rejected_by_content_not_by_name() {
    for (label, bytes) in [
        ("gif", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec()),
        (
            "bmp",
            b"BM\x1e\x00\x00\x00\x00\x00\x00\x00\x1a\x00\x00\x00".to_vec(),
        ),
        ("pdf", b"%PDF-1.7\n".to_vec()),
        (
            "svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#.to_vec(),
        ),
        ("text", b"definitely not an image".to_vec()),
    ] {
        assert!(
            matches!(
                normalize(&bytes, &cfg()),
                Err(NormalizeError::UnsupportedFormat(_))
            ),
            "{label}"
        );
    }
}

#[test]
fn empty_oversized_and_over_pixel_inputs_are_typed_errors() {
    assert_eq!(normalize(&[], &cfg()).unwrap_err(), NormalizeError::Empty);

    let mut c = cfg();
    c.max_bytes = 100;
    let big = png(64, 64);
    assert!(big.len() > 100);
    assert_eq!(
        normalize(&big, &c).unwrap_err(),
        NormalizeError::TooLarge {
            actual: big.len(),
            max: 100
        }
    );

    let mut c = cfg();
    c.max_pixels = 50;
    assert_eq!(
        normalize(&png(10, 10), &c).unwrap_err(),
        NormalizeError::TooManyPixels {
            width: 10,
            height: 10,
            max: 50
        }
    );
    assert!(
        normalize(&png(7, 7), &c).is_ok(),
        "49 pixels is within a 50-pixel limit"
    );
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
    out
}

#[test]
fn the_pixel_limit_is_checked_from_the_header_before_any_decoding() {
    // A PNG that declares 100000 x 100000 pixels (~30 GB of RGB) but carries only a few junk IDAT bytes, as a
    // decompression bomb would. If the limit were applied after decoding, this would fail with a different
    // error (or try to allocate); the header check must reject it first.
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB, deflate, no filter, no interlace
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(chunk(b"IHDR", &ihdr));
    bytes.extend(chunk(b"IDAT", &[0x78, 0x9c, 0x00, 0x00, 0x00, 0x00]));
    bytes.extend(chunk(b"IEND", &[]));

    let err = normalize(&bytes, &cfg()).unwrap_err();
    assert!(
        matches!(
            err,
            NormalizeError::TooManyPixels {
                width: 100_000,
                height: 100_000,
                ..
            }
        ),
        "{err:?}"
    );
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[test]
fn truncated_and_corrupt_images_are_undecodable_not_panics() {
    let good = png(32, 32);
    let truncated = &good[..good.len() / 2];
    assert!(matches!(
        normalize(truncated, &cfg()).unwrap_err(),
        NormalizeError::Undecodable(_)
    ));
    let mut corrupt = good.clone();
    let mid = corrupt.len() / 2;
    for b in &mut corrupt[mid..mid + 8] {
        *b ^= 0xFF;
    }
    // Either the checksum catches it (Undecodable) or the damage is benign and it decodes; it must never panic.
    let _ = normalize(&corrupt, &cfg());
    assert!(matches!(
        normalize(&good[..12], &cfg()).unwrap_err(),
        NormalizeError::Undecodable(_) | NormalizeError::UnsupportedFormat(_)
    ));
}

#[test]
fn animated_png_is_rejected() {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 4, 4);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_animated(2, 0).unwrap();
        let mut w = enc.write_header().unwrap();
        let frame = vec![128u8; 4 * 4 * 3];
        w.write_image_data(&frame).unwrap();
        w.write_image_data(&frame).unwrap();
        w.finish().unwrap();
    }
    assert_eq!(
        normalize(&out, &cfg()).unwrap_err(),
        NormalizeError::Animated
    );
    // The same pixels as a still PNG are fine, so the rejection is about animation, not content.
    assert!(normalize(&png(4, 4), &cfg()).is_ok());
}

#[test]
fn the_pixel_cap_is_the_memory_bound_and_covers_every_accepted_format() {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 10, Rgb([5, 6, 7])));
    let mut tight = cfg();
    tight.max_pixels = 50; // the image has 100
    for (name, bytes) in [
        ("png", encode(&img, ImageFormat::Png)),
        ("jpeg", encode(&img, ImageFormat::Jpeg)),
        ("webp", encode(&img, ImageFormat::WebP)),
    ] {
        assert!(
            matches!(
                normalize(&bytes, &tight).unwrap_err(),
                NormalizeError::TooManyPixels {
                    width: 10,
                    height: 10,
                    ..
                }
            ),
            "{name} must be rejected from its header"
        );
        assert!(
            normalize(&bytes, &cfg()).is_ok(),
            "{name} is fine under the default cap"
        );
    }
}
