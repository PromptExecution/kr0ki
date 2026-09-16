//! Live happy-path test for `GET /b00t-graph/:tag` (kr0ki#13).
//!
//! Env-gated on KR0KI_TEST_BACKEND, same pattern as
//! `kr0ki-core/tests/template_render.rs`. Everything up to (not including)
//! the actual Kroki round-trip is already covered, always-on, by
//! `tests/http.rs`'s `b00t_graph_*` tests. To run locally:
//!   just kroki-up
//!   KR0KI_TEST_BACKEND=http://localhost:8000 cargo test -p kr0ki-server --test b00t_graph_live -- --ignored

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};
use tower::ServiceExt;

#[path = "../src/app.rs"]
mod app;
#[path = "../src/docs.rs"]
mod docs;
use app::{router, AppState};

const SAMPLE_TTL: &str = r#"
    @prefix b00t: <http://b00t.promptexecution.com/ontology#> .
    @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

    <http://b00t.promptexecution.com/ontology#datum/rust.cli>
        b00t:dependsOn <http://b00t.promptexecution.com/ontology#datum/cargo.cli> ;
        rdfs:label "Rust toolchain" .
"#;

#[tokio::test]
#[ignore = "needs KR0KI_TEST_BACKEND pointing at a live Kroki"]
async fn b00t_graph_renders_a_real_turtle_fixture_to_svg() {
    let Ok(backend_url) = std::env::var("KR0KI_TEST_BACKEND") else {
        eprintln!("KR0KI_TEST_BACKEND unset — skipping");
        return;
    };

    let base = std::env::temp_dir().join(format!("kr0ki-b00tgraph-live-{}", std::process::id()));
    let tag_dir = base.join("tags").join("v1");
    tokio::fs::create_dir_all(&tag_dir).await.unwrap();
    tokio::fs::write(tag_dir.join("kerml-view.ttl"), SAMPLE_TTL)
        .await
        .unwrap();

    let cache_dir = base.join(".cache");
    let service = RenderService::new(HttpKrokiBackend::new(backend_url), FsCache::new(&cache_dir));
    let state = AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: Some(base.clone()),
        capabilities_path: None,
        kubediagram_worker_url: None,
    };
    let app = router(state, None);

    let resp = app
        .oneshot(Request::get("/b00t-graph/v1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(status, StatusCode::OK, "response body: {text}");
    assert!(text.contains("<svg"), "expected SVG output, got: {text}");

    let _ = tokio::fs::remove_dir_all(&base).await;
}
