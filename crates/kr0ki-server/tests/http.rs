//! In-process HTTP tests for the paths that don't require a render backend.
//! The cache→backend happy path is covered by kr0ki-core's unit tests
//! (`RenderService` + a stub backend) and by `tests/live_render.rs` (env-gated).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};
use tower::ServiceExt; // oneshot

// The server crate is a bin; pull the router module in via path.
#[path = "../src/app.rs"]
mod app;
#[path = "../src/docs.rs"]
mod docs;
use app::{router, AppState};

fn test_state(tag: &str) -> AppState {
    let dir = std::env::temp_dir().join(format!("kr0ki-http-test-{}-{}", std::process::id(), tag));
    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"), // unreachable — must never be called by these tests
        FsCache::new(dir),
    );
    AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
    }
}

fn test_app(state: AppState) -> axum::Router {
    router(state, None)
}

async fn body_string(resp: axum::response::Response) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn health_ok() {
    let app = test_app(test_state("health"));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"status\":\"ok\""));
    assert!(body.contains("\"service\":\"kr0ki\""));
}

#[tokio::test]
async fn formats_lists_supported_slugs_only() {
    let app = test_app(test_state("formats"));
    let resp = app
        .oneshot(Request::get("/formats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("plantuml") && body.contains("d2"));
    assert!(
        !body.contains("mermaid"),
        "companion-only formats must not be advertised"
    );
}

#[tokio::test]
async fn examples_catalog_covers_every_advertised_format() {
    let app = test_app(test_state("examples"));
    let resp = app
        .oneshot(Request::get("/api/examples").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let examples: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(examples.len(), 8);
    assert!(examples.iter().all(|example| example["source"].is_string()));
    assert!(examples.iter().any(|example| example["format"] == "d2"));
}

#[tokio::test]
async fn playbook_serves_built_vue_assets() {
    let directory = std::env::temp_dir().join(format!("kr0ki-playbook-{}", std::process::id()));
    tokio::fs::create_dir_all(directory.join("assets"))
        .await
        .unwrap();
    tokio::fs::write(directory.join("index.html"), "<main>playb00k</main>")
        .await
        .unwrap();
    tokio::fs::write(directory.join("assets/app.js"), "export default 'playb00k'")
        .await
        .unwrap();

    let mut state = test_state("playbookassets");
    state.playbook_dir = directory.clone();
    let app = test_app(state);

    let response = app
        .clone()
        .oneshot(Request::get("/playbook/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("playb00k"));

    let response = app
        .oneshot(
            Request::get("/playbook/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("export default"));

    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn render_unknown_format_is_400_before_any_backend_call() {
    let app = test_app(test_state("badfmt"));
    let resp = app
        .oneshot(
            Request::post("/render/mermaid")
                .body(Body::from("graph TD; A-->B"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("unsupported_format"));
}

#[tokio::test]
async fn render_empty_body_is_400() {
    let app = test_app(test_state("empty"));
    let resp = app
        .oneshot(Request::post("/render/d2").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_source"));
}

#[tokio::test]
async fn auth_required_rejects_missing_token() {
    let app = router(test_state("authed"), Some("secret".to_string()));
    let resp = app
        .oneshot(Request::get("/formats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("unauthorized"));
}

#[tokio::test]
async fn auth_required_accepts_valid_bearer() {
    let app = router(test_state("authed-ok"), Some("secret".to_string()));
    let resp = app
        .oneshot(
            Request::get("/health")
                .header("Authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("status"));
}

#[tokio::test]
async fn render_png_query_param_rejected_without_backend() {
    // We can't hit a real backend in unit tests, but we can verify the route
    // accepts ?output=png and forwards it to the service (which will fail on the
    // unreachable backend address, returning 502).
    let app = test_app(test_state("png-route"));
    let resp = app
        .oneshot(
            Request::post("/render/graphviz?output=png")
                .body(Body::from("digraph { a -> b }"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("backend_unavailable"));
}

#[tokio::test]
async fn cache_get_rejects_malformed_key() {
    let app = test_app(test_state("badkey"));
    let resp = app
        .oneshot(
            Request::get("/cache/not-a-hash")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cache_get_miss_is_404() {
    let app = test_app(test_state("miss"));
    let key = "0".repeat(64);
    let resp = app
        .oneshot(
            Request::get(format!("/cache/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("not_found"));
}

#[tokio::test]
async fn docs_html_returns_valid_page() {
    let app = test_app(test_state("docshtml"));
    let resp = app
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<!DOCTYPE html>"));
    assert!(body.contains("kr0ki documentation"));
    assert!(body.contains("Diagram Example"));
    assert!(body.contains("/docs/examples/kr0ki-render-flow.svg"));
    assert!(body.contains("/docs/api.json"));
}

#[tokio::test]
async fn docs_rust_flow_uses_the_render_service_cache() {
    use kr0ki_core::{cache::OutputKind, format::DiagramFormat};

    let state = test_state("docsflow");
    let source = include_str!("../../../templates/kr0ki-render-flow.d2");
    let key = kr0ki_core::cache::cache_key(DiagramFormat::D2, OutputKind::Svg, source);
    state
        .service
        .cache()
        .put(&key, OutputKind::Svg, b"<svg id=\"cached-flow\"/>")
        .await
        .unwrap();

    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::get("/docs/examples/kr0ki-render-flow.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("cached-flow"));
}

#[tokio::test]
async fn docs_json_returns_symbol_array() {
    let app = test_app(test_state("docsjson"));
    let resp = app
        .oneshot(Request::get("/docs/api.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let arr = parsed.as_array().expect("json is an array");
    assert!(!arr.is_empty(), "harvested at least one symbol");
    let first = &arr[0];
    assert!(first.get("name").is_some());
    assert!(first.get("qualified_name").is_some());
    assert!(first.get("signature").is_some());
}

#[tokio::test]
async fn docs_tomllm_has_boilerplate() {
    let app = test_app(test_state("docstoml"));
    let resp = app
        .oneshot(
            Request::get("/docs/api.tomllm")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("b00t:map v1"));
    assert!(body.contains("[[kr0ki_core::"));
}

#[tokio::test]
async fn docs_rustdoc_has_source_marker() {
    let app = test_app(test_state("docsrust"));
    let resp = app
        .oneshot(
            Request::get("/docs/api.rustdoc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("/// # Source"));
    // The rustdoc format uses qualified_name in the header, not body text
    assert!(body.contains("/// `"));
}
