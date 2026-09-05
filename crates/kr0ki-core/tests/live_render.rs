//! Live render test against a real Kroki backend.
//!
//! Env-gated (same pattern as b00t's nats integration tests): skipped unless
//! `KR0KI_TEST_BACKEND` is set to a reachable Kroki base URL, e.g.
//!   KR0KI_TEST_BACKEND=http://localhost:8000 cargo test -p kr0ki-core -- --ignored
//! Run a local one first:  podman run -d -p 8000:8000 ghcr.io/yuzutech/kroki

use kr0ki_core::{
    cache::{CacheStatus, FsCache, OutputKind},
    format::DiagramFormat,
    render::HttpKrokiBackend,
    RenderService,
};

#[tokio::test]
#[ignore = "needs KR0KI_TEST_BACKEND pointing at a live Kroki"]
async fn renders_graphviz_to_svg_and_second_call_hits_cache() {
    let Ok(base) = std::env::var("KR0KI_TEST_BACKEND") else {
        eprintln!("KR0KI_TEST_BACKEND unset — skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("kr0ki-live-{}", std::process::id()));
    let svc = RenderService::new(HttpKrokiBackend::new(base), FsCache::new(&dir));

    let src = "digraph { a -> b -> c }";
    let r1 = svc
        .render(DiagramFormat::GraphViz, OutputKind::Svg, src)
        .await
        .unwrap();
    assert_eq!(r1.status, CacheStatus::Miss);
    let text = String::from_utf8_lossy(&r1.bytes);
    assert!(
        text.contains("<svg"),
        "expected SVG, got: {}",
        &text[..text.len().min(200)]
    );

    let r2 = svc
        .render(DiagramFormat::GraphViz, OutputKind::Svg, src)
        .await
        .unwrap();
    assert_eq!(r2.status, CacheStatus::Hit);
    assert_eq!(
        r1.bytes, r2.bytes,
        "cache hit must be byte-identical (PRD NFR1)"
    );

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
