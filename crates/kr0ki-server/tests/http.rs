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
use app::{router, AppState};

fn test_state(tag: &str) -> AppState {
    let dir = std::env::temp_dir().join(format!("kr0ki-http-test-{}-{}", std::process::id(), tag));
    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"), // unreachable — must never be called by these tests
        FsCache::new(dir),
    );
    AppState {
        service: Arc::new(service),
    }
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
    let app = router(test_state("health"));
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
    let app = router(test_state("formats"));
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
async fn render_unknown_format_is_400_before_any_backend_call() {
    let app = router(test_state("badfmt"));
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
    let app = router(test_state("empty"));
    let resp = app
        .oneshot(Request::post("/render/d2").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_source"));
}

#[tokio::test]
async fn cache_get_rejects_malformed_key() {
    let app = router(test_state("badkey"));
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
    let app = router(test_state("miss"));
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
