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
pub mod examples;
pub mod flatten;
pub mod format;
pub mod render;

use cache::{cache_key, model_cache_key, CacheStatus, FsCache, OutputKind};
use format::DiagramFormat;
use render::{RenderBackend, RenderError};

/// Colour-blind SVG→PNG fallback: some formats Kroki only draws as SVG (`nomnoml`,
/// `d2`, `wavedrom`, at least) and reject a native PNG request with "Unsupported
/// output format". Rather than hand-maintain that support matrix, always try the
/// backend's native PNG first; only on rejection, render SVG and flatten it locally
/// via [`flatten::svg_to_png`]. A genuinely bad diagram source fails identically on
/// the SVG attempt too, so this never masks a real syntax error — it costs one extra
/// backend round-trip on the first (uncached) request for such a format, never on a
/// cache hit.
async fn render_bytes_with_png_fallback<B: RenderBackend>(
    backend: &B,
    format: DiagramFormat,
    output: OutputKind,
    source: &str,
) -> Result<Vec<u8>, RenderError> {
    match backend.render(format, output, source).await {
        Ok(bytes) => Ok(bytes),
        Err(RenderError::BadSource { .. }) if output == OutputKind::Png => {
            let svg = backend.render(format, OutputKind::Svg, source).await?;
            flatten::svg_to_png(&svg).map_err(|e| RenderError::Flatten(e.to_string()))
        }
        Err(other) => Err(other),
    }
}

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

        let bytes = render_bytes_with_png_fallback(&self.backend, format, output, source).await?;
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

    /// Render a SysML-model diagram: `source` is the lowered diagram text for the
    /// given view, and `content_hash` is the `ModelSnapshot.content_hash` that the
    /// key is derived from (PLAN-KR0KI-002 §3). A new commit changes the hash and
    /// therefore the key, so a stale derived diagram can never be served.
    pub async fn render_model(
        &self,
        view_kind: &str,
        format: DiagramFormat,
        output: OutputKind,
        source: &str,
        content_hash: &str,
    ) -> Result<Rendered, ServiceError> {
        let key = model_cache_key(view_kind, format.kroki_slug(), content_hash);

        if let Some(bytes) = self.cache.get(&key, output).await? {
            return Ok(Rendered {
                key,
                bytes,
                output,
                status: CacheStatus::Hit,
            });
        }

        let bytes = render_bytes_with_png_fallback(&self.backend, format, output, source).await?;
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
    async fn model_render_caches_on_content_hash() {
        let calls = Arc::new(AtomicUsize::new(0));
        let backend = CountingBackend {
            calls: calls.clone(),
            body: b"<svg>model</svg>".to_vec(),
        };
        let cache = tmp_cache("model");
        let svc = RenderService::new(backend, cache.clone());

        // Same view + same commit hash -> second call is a hit.
        let r1 = svc
            .render_model(
                "overview",
                DiagramFormat::D2,
                OutputKind::Svg,
                "a -> b",
                "hash-a",
            )
            .await
            .unwrap();
        assert_eq!(r1.status, CacheStatus::Miss);

        let r2 = svc
            .render_model(
                "overview",
                DiagramFormat::D2,
                OutputKind::Svg,
                "a -> b",
                "hash-a",
            )
            .await
            .unwrap();
        assert_eq!(r2.status, CacheStatus::Hit);
        assert_eq!(r2.key, r1.key);

        // Same lowered source, new commit hash -> miss again (key differs).
        let r3 = svc
            .render_model(
                "overview",
                DiagramFormat::D2,
                OutputKind::Svg,
                "a -> b",
                "hash-b",
            )
            .await
            .unwrap();
        assert_eq!(r3.status, CacheStatus::Miss);
        assert_ne!(r3.key, r1.key);

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "backend hit once per distinct key"
        );

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

    /// A backend like real Kroki for `nomnoml`/`d2`/`wavedrom`: it rejects a native
    /// PNG request but renders SVG fine.
    #[derive(Clone)]
    struct SvgOnlyBackend {
        svg_calls: Arc<AtomicUsize>,
        png_calls: Arc<AtomicUsize>,
    }

    impl RenderBackend for SvgOnlyBackend {
        async fn render(
            &self,
            _f: DiagramFormat,
            output: OutputKind,
            _s: &str,
        ) -> Result<Vec<u8>, RenderError> {
            match output {
                OutputKind::Svg => {
                    self.svg_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#.to_vec())
                }
                OutputKind::Png => {
                    self.png_calls.fetch_add(1, Ordering::SeqCst);
                    Err(RenderError::BadSource {
                        status: 400,
                        body: "Unsupported output format: png for nomnoml. Must be one of svg."
                            .into(),
                    })
                }
            }
        }
    }

    #[tokio::test]
    async fn png_falls_back_to_flattened_svg_when_backend_rejects_native_png() {
        let backend = SvgOnlyBackend {
            svg_calls: Arc::new(AtomicUsize::new(0)),
            png_calls: Arc::new(AtomicUsize::new(0)),
        };
        let cache = tmp_cache("pngfallback");
        let svc = RenderService::new(backend.clone(), cache.clone());

        let r1 = svc
            .render(DiagramFormat::Nomnoml, OutputKind::Png, "[a]->[b]")
            .await
            .unwrap();
        assert_eq!(r1.status, CacheStatus::Miss);
        assert!(
            r1.bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "expected a real PNG, got {:?}",
            &r1.bytes[..r1.bytes.len().min(16)]
        );
        assert_eq!(backend.png_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.svg_calls.load(Ordering::SeqCst), 1);

        // The flattened result is cached under the original PNG key: a second call
        // is a hit and never touches the backend again.
        let r2 = svc
            .render(DiagramFormat::Nomnoml, OutputKind::Png, "[a]->[b]")
            .await
            .unwrap();
        assert_eq!(r2.status, CacheStatus::Hit);
        assert_eq!(r2.bytes, r1.bytes);
        assert_eq!(backend.png_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.svg_calls.load(Ordering::SeqCst), 1);

        let _ = tokio::fs::remove_dir_all(cache.root()).await;
    }

    #[tokio::test]
    async fn genuinely_bad_source_still_fails_through_the_png_fallback() {
        struct AlwaysBadSource;
        impl RenderBackend for AlwaysBadSource {
            async fn render(
                &self,
                _f: DiagramFormat,
                _o: OutputKind,
                _s: &str,
            ) -> Result<Vec<u8>, RenderError> {
                Err(RenderError::BadSource {
                    status: 400,
                    body: "actually broken syntax".into(),
                })
            }
        }
        let cache = tmp_cache("pngfallback-badsource");
        let svc = RenderService::new(AlwaysBadSource, cache.clone());

        let err = svc
            .render(DiagramFormat::Nomnoml, OutputKind::Png, "not valid")
            .await
            .unwrap_err();
        assert!(
            matches!(err, ServiceError::Render(RenderError::BadSource { .. })),
            "a real syntax error must still surface as BadSource, not be swallowed: {err:?}"
        );

        let _ = tokio::fs::remove_dir_all(cache.root()).await;
    }
}
