//! kr0ki-core — the P0 render loop, plus most of the SysML-model ingestion
//! pipeline (PLAN-KR0KI-002).
//!
//! Scope: raw Kroki-family diagram text -> rendered SVG, with a content-addressed
//! cache. This is PRD-KR0KI-001's decision-*independent* slice (FR2 + FR5).
//! [`ufo_graph`] builds box 2 of the five-box pipeline — the canonical UFO
//! semantic graph — from a `ModelSnapshot` (the SysML-v2 arm). [`k8s_recognizer`]
//! does the analogous normalization for the Kubernetes arm (kr0ki#12;
//! `docs/PATTERNS-kubernetes.md`). [`b00t_graph`] is a *separate* box-5 arm
//! (kr0ki#13): it reads an already-built `elasticdotventures/_b00t_` Turtle
//! graph and lowers it straight to D2 via `holon-viz`'s
//! `TypeRelationshipGraph`/`CytoscapeGraph` (D1/D2/D3/D6 all resolved
//! 2026-09-05..2026-09-10 — see `docs/PRD-KR0KI-001-foundational.md` §5) —
//! it does not go through `ufo_graph`/`k8s_recognizer`'s `OntologicalEdge`
//! pivot. [`sysml_lift`] is that further stage: it lifts a
//! `Vec<OntologicalEdge>` (from either `ufo_graph` or `k8s_recognizer`) into
//! `ufo_types::sysml_model::Relation` (box 4) per `docs/PATTERNS-kubernetes.md`
//! §4 — the step FR1/FR4 rendering for the SysML-v2/Kubernetes arms still
//! needs, previously design-only with no code. [`sysml_render`] is FR1
//! itself: `Vec<Relation>` (box 4, e.g. from [`sysml_lift`]) → D2 / Mermaid
//! diagram text, ready for the existing `render`/`RenderBackend` pipeline.
//! [`rust_recognizer`] is a third box-1→2 arm (`docs/PATTERNS-rust-source.md`,
//! `docs/PLAN-KR0KI-003-rust-source-frontend.md`): Rust source →
//! `ufo_types::iso_ir::{Node, Edge}`, a sibling of `docgen::harvest`'s
//! `SymbolVisitor` producing relationships instead of doc symbols.

pub mod b00t_graph;
pub mod cache;
pub mod catalog;
pub mod docgen;
pub mod examples;
pub mod flatten;
pub mod format;
pub mod graph_store;
pub mod k8s_recognizer;
pub mod mcp_tool;
pub mod probe;
pub mod render;
pub mod reqif_adapter;
/// Canonical home is `ufo_types::mbse::requirements` (kr0ki M1,
/// 2026-09-19) — re-exported here so existing `kr0ki_core::requirements::*`
/// call sites keep working without a local copy of the types.
pub use ufo_types::mbse::requirements;
pub mod requirements_render;
pub mod rust_recognizer;
pub mod sysml_lift;
pub mod sysml_render;
pub mod ufo_graph;

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

    /// Base URL of the render backend when the backend is HTTP-backed (health
    /// reporting). Non-HTTP test backends report `None`.
    pub fn backend_url(&self) -> Option<&str> {
        self.backend.describe_endpoint()
    }

    /// Filesystem root of the content-addressed cache.
    pub fn cache_root(&self) -> &std::path::Path {
        self.cache.root()
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
    /// `rule_set_version` is the recognizer rule-set version that produced `source`
    /// (e.g. [`crate::k8s_recognizer::KubernetesRecognizer::rule_set_version`]), or
    /// `""` for a source arm with no recognizer in its path — see
    /// [`cache::model_cache_key`]. A rule-set change invalidates the key too.
    pub async fn render_model(
        &self,
        view_kind: &str,
        format: DiagramFormat,
        output: OutputKind,
        source: &str,
        content_hash: &str,
        rule_set_version: &str,
    ) -> Result<Rendered, ServiceError> {
        let key = model_cache_key(
            view_kind,
            format.kroki_slug(),
            content_hash,
            rule_set_version,
        );

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
                "",
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
                "",
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
                "",
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
    async fn model_render_misses_again_when_recognizer_rule_set_version_changes() {
        let backend = CountingBackend {
            calls: Arc::new(AtomicUsize::new(0)),
            body: b"<svg>model</svg>".to_vec(),
        };
        let cache = tmp_cache("model-ruleset");
        let svc = RenderService::new(backend, cache.clone());

        let r1 = svc
            .render_model(
                "overview",
                DiagramFormat::D2,
                OutputKind::Svg,
                "a -> b",
                "hash-a",
                "rules-v1",
            )
            .await
            .unwrap();

        // Same view/format/content hash, but the recognizer's rule set changed:
        // must be a fresh key, not a hit against v1's cached artifact.
        let r2 = svc
            .render_model(
                "overview",
                DiagramFormat::D2,
                OutputKind::Svg,
                "a -> b",
                "hash-a",
                "rules-v2",
            )
            .await
            .unwrap();
        assert_ne!(r2.key, r1.key);
        assert_eq!(r2.status, CacheStatus::Miss);

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
