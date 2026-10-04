//! Rasterize SVG bytes to PNG locally, in-process — no headless-Chromium companion
//! (PRD-KR0KI-001 NFR3), pure Rust via `resvg`/`usvg`/`tiny-skia`.
//!
//! Some Kroki-family formats only draw as SVG: Kroki itself rejects a PNG request
//! for `nomnoml`, `d2`, and `wavedrom` with "Unsupported output format". Rather than
//! tracking that support matrix by hand (and drifting when it changes upstream),
//! [`crate::RenderService`] always asks the backend for native PNG first and only
//! flattens here on rejection — see its module docs.

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

#[derive(Debug, thiserror::Error)]
pub enum FlattenError {
    #[error("invalid SVG: {0}")]
    InvalidSvg(String),
    #[error("SVG has zero-sized viewport")]
    ZeroSized,
    #[error("PNG encode failed: {0}")]
    Encode(String),
}

/// Render `svg` to PNG bytes at the SVG's own intrinsic pixel size (no upscaling).
pub fn svg_to_png(svg: &[u8]) -> Result<Vec<u8>, FlattenError> {
    // usvg starts with an empty font database and does not load SVG CSS
    // webfonts. In particular, D2's generated family names need an installed
    // fallback; otherwise valid <text> silently disappears from the PNG.
    let options = usvg::Options {
        fontdb: raster_fonts(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(svg, &options)
        .map_err(|e| FlattenError::InvalidSvg(e.to_string()))?;

    let size = tree.size().to_int_size();
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(FlattenError::ZeroSized)?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| FlattenError::Encode(e.to_string()))
}

fn raster_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTS.get_or_init(|| {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        let families: BTreeSet<String> = fonts
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
            .collect();
        // The OCI renderer supplies DejaVu. Elsewhere, choose an available
        // family deterministically rather than font-directory iteration order.
        let fallback = if families.contains("DejaVu Sans") {
            Some("DejaVu Sans".to_owned())
        } else {
            families.iter().next().cloned()
        };
        if let Some(fallback) = fallback {
            // usvg's default selector tries the generic serif family after
            // any unavailable named family, including D2's CSS webfont names.
            fonts.set_serif_family(fallback.clone());
            fonts.set_sans_serif_family(fallback.clone());
            fonts.set_monospace_family(if families.contains("DejaVu Sans Mono") {
                "DejaVu Sans Mono".to_owned()
            } else {
                fallback
            });
        }
        Arc::new(fonts)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#;

    #[test]
    fn flattens_a_simple_svg_to_a_valid_png() {
        let png = svg_to_png(SAMPLE_SVG.as_bytes()).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG signature");
    }

    #[test]
    fn rasterized_labels_use_installed_fonts_for_missing_svg_families() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="180" height="50">
            <rect width="180" height="50" fill="white"/>
            <text x="8" y="34" font-family="d2-generated-webfont-unavailable" font-size="24" fill="#0A0F25">Observe</text>
        </svg>"##;
        let png = svg_to_png(svg).unwrap();
        let pixmap = tiny_skia::Pixmap::decode_png(&png).unwrap();
        let dark_pixels = pixmap
            .pixels()
            .iter()
            .filter(|pixel| pixel.red() < 80 && pixel.green() < 80 && pixel.blue() < 80)
            .count();
        assert!(
            dark_pixels > 100,
            "SVG labels disappeared from PNG; install a system font for rasterization"
        );
    }

    #[test]
    fn invalid_svg_is_rejected() {
        let err = svg_to_png(b"not an svg").unwrap_err();
        assert!(matches!(err, FlattenError::InvalidSvg(_)));
    }

    #[test]
    fn zero_sized_svg_is_rejected() {
        // usvg itself rejects a degenerate 0x0 viewport at parse time, so this
        // surfaces as InvalidSvg rather than reaching the post-parse ZeroSized
        // check — either way it must not panic or silently produce a 0-byte PNG.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"></svg>"#;
        assert!(svg_to_png(svg.as_bytes()).is_err());
    }
}
