//! Rasterize SVG bytes to PNG locally, in-process — no headless-Chromium companion
//! (PRD-KR0KI-001 NFR3), pure Rust via `resvg`/`usvg`/`tiny-skia`.
//!
//! Some Kroki-family formats only draw as SVG: Kroki itself rejects a PNG request
//! for `nomnoml`, `d2`, and `wavedrom` with "Unsupported output format". Rather than
//! tracking that support matrix by hand (and drifting when it changes upstream),
//! [`crate::RenderService`] always asks the backend for native PNG first and only
//! flattens here on rejection — see its module docs.

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
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
        .map_err(|e| FlattenError::InvalidSvg(e.to_string()))?;

    let size = tree.size().to_int_size();
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(FlattenError::ZeroSized)?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| FlattenError::Encode(e.to_string()))
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
