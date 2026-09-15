//! kr0ki-core — the P0 render loop.
//!
//! Scope: raw Kroki-family diagram text -> rendered SVG, with a content-addressed
//! cache. This is PRD-KR0KI-001's decision-*independent* slice (FR2 + FR5). The
//! SysML-model ingestion path (FR1/FR3/FR4 — `iso_ir`, `systhread-core` isometric,
//! the typed KerML view model) is **not here**: it's blocked on §5 decisions D1-D6
//! (sysml-derive posture, holon-viz dependency, the typed model's crate home,
//! vocabulary collisions). Nothing in this crate depends on `ufo-types`,
//! `systhread-core`, or `holon-viz`.

pub mod cache;
pub mod docgen;
pub mod format;
pub mod render;

use cache::{cache_key, CacheStatus, FsCache, OutputKind};
use format::DiagramFormat;
use render::{RenderBackend, RenderError};

/// The render service: a cache in front of a render backend.
#[derive(Debug, Clone)]
pub struct RenderService<B> {
    backend: B,
    cache: FsCache,
}

/// One completed render.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub key: String,
    pub bytes: Vec<u8>,
    pub output: OutputKind,
    pub status: CacheStatus,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("cache io: {0}")]
    CacheIo(#[from] std::io::Error),
}

impl<B: RenderBackend> RenderService<B> {
    pub fn new(backend: B, cache: FsCache) -> Self {
        Self { backend, cache }
    }

    /// Render `source` as `format` to `output`, serving from cache on a hit.
    ///
    /// Deterministic: the same `(format, output, source)` always yields the same
    /// `key`, and a cache hit returns byte-identical output to the render that
    /// populated it (PRD NFR1).
    pub async fn render(
        &self,
        format: DiagramFormat,
        output: OutputKind,
        source: &str,
    ) -> Result<Rendered, ServiceError> {
        let key = cache_key(format, output, source);

        if let Some(bytes) = self.cache.get(&key, output).await? {
            return Ok(Rendered {
                key,
                bytes,
                output,
                status: CacheStatus::Hit,
            });
        }

        let bytes = self.backend.render(format, output, source).await?;
        // A backend failure above short-circuits; only a real artifact is cached, so a
        // transient outage never poisons the cache with an error page.
        self.cache.put(&key, output, &bytes).await?;

        Ok(Rendered {
            key,
            bytes,
            output,
            status: CacheStatus::Miss,
        })
    }

    pub fn cache(&self) -> &FsCache {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A backend that records how many times it was hit and returns a canned body.
    #[derive(Clone)]
    struct CountingBackend {
        calls: Arc<AtomicUsize>,
        body: Vec<u8>,
    }

    impl RenderBackend for CountingBackend {
        async fn render(
            &self,
            _f: DiagramFormat,
            _o: OutputKind,
            _s: &str,
        ) -> Result<Vec<u8>, RenderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.body.clone())
        }
    }

    fn tmp_cache(tag: &str) -> FsCache {
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-svc-test-{}-{}",
            std::process::id(),
            tag
        )))
    }

    #[tokio::test]
    async fn first_render_misses_then_second_hits_backend_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let backend = CountingBackend {
            calls: calls.clone(),
            body: b"<svg>ok</svg>".to_vec(),
        };
        let cache = tmp_cache("hitmiss");
        let svc = RenderService::new(backend, cache.clone());

        let r1 = svc
            .render(DiagramFormat::D2, OutputKind::Svg, "a -> b")
            .await
            .unwrap();
        assert_eq!(r1.status, CacheStatus::Miss);
        assert_eq!(r1.bytes, b"<svg>ok</svg>");

        let r2 = svc
            .render(DiagramFormat::D2, OutputKind::Svg, "a -> b")
            .await
            .unwrap();
        assert_eq!(r2.status, CacheStatus::Hit);
        assert_eq!(r2.bytes, r1.bytes);
        assert_eq!(r2.key, r1.key);

        assert_eq!(calls.load(Ordering::SeqCst), 1, "backend hit exactly once");

        let _ = tokio::fs::remove_dir_all(cache.root()).await;
    }

    #[tokio::test]
    async fn backend_failure_is_not_cached() {
        struct AlwaysDown;
        impl RenderBackend for AlwaysDown {
            async fn render(
                &self,
                _f: DiagramFormat,
                _o: OutputKind,
                _s: &str,
            ) -> Result<Vec<u8>, RenderError> {
                Err(RenderError::Unavailable("down".into()))
            }
        }
        let cache = tmp_cache("nofail");
        let svc = RenderService::new(AlwaysDown, cache.clone());

        assert!(svc
            .render(DiagramFormat::D2, OutputKind::Svg, "x")
            .await
            .is_err());
        let key = cache_key(DiagramFormat::D2, OutputKind::Svg, "x");
        assert_eq!(cache.get(&key, OutputKind::Svg).await.unwrap(), None);

        let _ = tokio::fs::remove_dir_all(cache.root()).await;
    }
}
