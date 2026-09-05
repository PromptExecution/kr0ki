//! Content-addressed artifact cache (PRD-KR0KI-001 FR5).
//!
//! P0 is a local filesystem store. The CDN tier (FR5's real target) is decision D5 —
//! `FsCache` is the fallback / origin store that a CDN would sit in front of, and the
//! key derivation here is what that CDN would key on too, so swapping the backend later
//! doesn't change cache identity.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::format::DiagramFormat;

/// Output artifact type. P0 renders SVG only; PNG/PDF are a later Kroki capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputKind {
    Svg,
}

impl OutputKind {
    pub const fn ext(self) -> &'static str {
        match self {
            Self::Svg => "svg",
        }
    }

    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml",
        }
    }
}

/// The cache key for one render request.
///
/// `key = SHA256("kr0ki/v1" ‖ 0x1f ‖ format_slug ‖ 0x1f ‖ output_ext ‖ 0x1f ‖ source)`,
/// lowercase hex. The domain prefix and the `0x1f` (ASCII Unit Separator, cannot appear
/// in a slug/ext) field delimiters make the concatenation unambiguous — no length
/// prefixing needed, no `("a","bc")` vs `("ab","c")` collision.
///
/// NOTE: keyed on the *raw* diagram source as given. The PRD's real key folds in
/// "ledgrrr's per-commit serialized KerML" — that only applies to the SysML-model
/// ingestion path, which P0 does not have. For raw Kroki-family text, the text itself
/// is the whole input.
pub fn cache_key(format: DiagramFormat, output: OutputKind, source: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kr0ki/v1");
    h.update([0x1f]);
    h.update(format.kroki_slug().as_bytes());
    h.update([0x1f]);
    h.update(output.ext().as_bytes());
    h.update([0x1f]);
    h.update(source.as_bytes());
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Filesystem-backed origin cache. Layout: `<root>/<key[0..2]>/<key>.<ext>` — the
/// two-char shard keeps directory fan-out sane at volume.
#[derive(Debug, Clone)]
pub struct FsCache {
    root: PathBuf,
}

impl FsCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, key: &str, output: OutputKind) -> PathBuf {
        let shard = &key[..2];
        self.root
            .join(shard)
            .join(format!("{key}.{}", output.ext()))
    }

    /// Return the cached bytes for this key, or `None` on a miss.
    pub async fn get(&self, key: &str, output: OutputKind) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.path_for(key, output);
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Store bytes for this key. Write is atomic (temp file + rename) so a concurrent
    /// `get` never sees a half-written artifact.
    pub async fn put(&self, key: &str, output: OutputKind, bytes: &[u8]) -> std::io::Result<()> {
        let path = self.path_for(key, output);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = path.with_extension(format!("{}.tmp.{}", output.ext(), std::process::id()));
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Whether a render served from cache or hit the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    Hit,
    Miss,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_deterministic() {
        let a = cache_key(DiagramFormat::D2, OutputKind::Svg, "a -> b");
        let b = cache_key(DiagramFormat::D2, OutputKind::Svg, "a -> b");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn key_varies_on_every_input_field() {
        let base = cache_key(DiagramFormat::D2, OutputKind::Svg, "a -> b");
        assert_ne!(
            base,
            cache_key(DiagramFormat::GraphViz, OutputKind::Svg, "a -> b")
        );
        assert_ne!(
            base,
            cache_key(DiagramFormat::D2, OutputKind::Svg, "a -> c")
        );
    }

    #[test]
    fn key_has_no_field_boundary_collision() {
        // "d2" + "x -> y"  vs  "d2x" + " -> y" style ambiguity — the 0x1f delimiter
        // means moving a byte across the format/source boundary changes the key.
        let k1 = cache_key(DiagramFormat::D2, OutputKind::Svg, "svgFOO");
        let k2 = cache_key(DiagramFormat::D2, OutputKind::Svg, "FOO");
        assert_ne!(k1, k2);
    }

    #[tokio::test]
    async fn put_then_get_round_trips_and_miss_is_none() {
        let dir = std::env::temp_dir().join(format!("kr0ki-cache-test-{}", std::process::id()));
        let cache = FsCache::new(&dir);
        let key = cache_key(DiagramFormat::D2, OutputKind::Svg, "a -> b");

        assert_eq!(cache.get(&key, OutputKind::Svg).await.unwrap(), None);

        cache.put(&key, OutputKind::Svg, b"<svg/>").await.unwrap();
        assert_eq!(
            cache.get(&key, OutputKind::Svg).await.unwrap(),
            Some(b"<svg/>".to_vec())
        );

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
