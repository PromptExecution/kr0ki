//! Template render test — validates that templates/b00t-stack-orchestration.d2
//! is executable by kr0ki-core against a live Kroki backend.
//!
//! Env-gated on KR0KI_TEST_BACKEND, same pattern as live_render.rs.
//! To run locally:
//!   just kroki-up
//!   KR0KI_TEST_BACKEND=http://localhost:8000 cargo test -p kr0ki-core --test template_render -- --ignored

use kr0ki_core::{
    cache::{CacheStatus, FsCache, OutputKind},
    format::DiagramFormat,
    render::HttpKrokiBackend,
    RenderService,
};

fn template_d2_source() -> String {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("templates")
        .join("b00t-stack-orchestration.d2");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("template file not found at {}: {e}", path.display()))
}

#[tokio::test]
#[ignore = "needs KR0KI_TEST_BACKEND pointing at a live Kroki"]
async fn renders_b00t_stack_template_d2_to_svg() {
    let Ok(base) = std::env::var("KR0KI_TEST_BACKEND") else {
        eprintln!("KR0KI_TEST_BACKEND unset — skipping");
        return;
    };

    let src = template_d2_source();
    // Sanity: the template must not be empty and must contain the expected header.
    assert!(
        src.contains("b00t Stack Orchestration Pattern"),
        "template source missing expected header"
    );

    let dir = std::env::temp_dir().join(format!("kr0ki-template-{}-live", std::process::id()));
    let svc = RenderService::new(HttpKrokiBackend::new(base), FsCache::new(&dir));

    let r1 = svc
        .render(DiagramFormat::D2, OutputKind::Svg, &src)
        .await
        .unwrap_or_else(|e| panic!("live render failed: {e}"));

    assert_eq!(
        r1.status,
        CacheStatus::Miss,
        "first render must be a cache miss"
    );
    let text = String::from_utf8_lossy(&r1.bytes);
    assert!(
        text.contains("<svg"),
        "expected SVG output, got: {}",
        &text[..text.len().min(200)]
    );

    // Determinism: same source → same key → byte-identical hit (PRD NFR1).
    let r2 = svc
        .render(DiagramFormat::D2, OutputKind::Svg, &src)
        .await
        .unwrap();
    assert_eq!(r2.status, CacheStatus::Hit);
    assert_eq!(r1.bytes, r2.bytes, "cache hit must be byte-identical");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
