//! Live PNG render test against a real Kroki backend.
//!
//! Env-gated on KR0KI_TEST_BACKEND (same pattern as live_render.rs).
//! Kroki supports PNG for PlantUML / GraphViz / C4 / Ditaa / Nomnoml / WaveDrom
//! but not for D2 (returns 400). We test GraphViz which is known to work.

use kr0ki_core::{
    cache::{CacheStatus, FsCache, OutputKind},
    format::DiagramFormat,
    render::HttpKrokiBackend,
    RenderService,
};

#[tokio::test]
#[ignore = "needs KR0KI_TEST_BACKEND pointing at a live Kroki"]
async fn renders_graphviz_png_and_cache_hit_is_byte_identical() {
    let Ok(base) = std::env::var("KR0KI_TEST_BACKEND") else {
        eprintln!("KR0KI_TEST_BACKEND unset — skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("kr0ki-png-live-{}", std::process::id()));
    let svc = RenderService::new(HttpKrokiBackend::new(base), FsCache::new(&dir));

    let src = "digraph { a -> b -> c }";
    let r1 = svc
        .render(DiagramFormat::GraphViz, OutputKind::Png, src)
        .await
        .unwrap_or_else(|e| panic!("live PNG render failed: {e}"));
    assert_eq!(r1.status, CacheStatus::Miss);
    assert!(
        r1.bytes.starts_with(b"\x89PNG\r\n"),
        "expected PNG magic bytes, got: {:?}",
        &r1.bytes[..8.min(r1.bytes.len())]
    );

    let r2 = svc
        .render(DiagramFormat::GraphViz, OutputKind::Png, src)
        .await
        .unwrap();
    assert_eq!(r2.status, CacheStatus::Hit);
    assert_eq!(
        r1.bytes, r2.bytes,
        "PNG cache hit must be byte-identical (NFR1)"
    );

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
