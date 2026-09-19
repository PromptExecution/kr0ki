//! Live happy-path test for `POST /render/kubediagram` — mcp-http-parity
//! design, 2026-09-16 (docs/superpowers/specs/2026-09-16-mcp-http-parity-design.md).
//!
//! Env-gated on KR0KI_TEST_KUBEDIAGRAM_WORKER, same pattern as
//! `b00t_graph_live.rs`. Renders kr0ki's own deployment manifest
//! (deploy/kr0ki-local.pod.yaml) through the route — kr0ki drawing a
//! picture of its own deployment. To run against the deployed k0s pod:
//!   kubectl --context Default -n kr0ki port-forward pod/kr0ki-local 8788:8788
//!   KR0KI_TEST_KUBEDIAGRAM_WORKER=http://127.0.0.1:8788 \
//!     cargo test -p kr0ki-server --test kubediagram_live -- --ignored --nocapture

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

const KR0KI_OWN_DEPLOYMENT_MANIFEST: &str = include_str!("../../../deploy/kr0ki-local.pod.yaml");

#[tokio::test]
#[ignore = "needs KR0KI_TEST_KUBEDIAGRAM_WORKER pointing at a live kubediagram worker"]
async fn kr0ki_renders_its_own_deployment_manifest() {
    let Ok(worker_url) = std::env::var("KR0KI_TEST_KUBEDIAGRAM_WORKER") else {
        eprintln!("KR0KI_TEST_KUBEDIAGRAM_WORKER unset — skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("kr0ki-kubediagram-live-{}", std::process::id()));
    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"), // this route never uses the Kroki backend
        FsCache::new(&dir),
    );
    let state = AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: Some(worker_url),
        sysmlv2_client: None,
        model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),
        storyb00k_agent_url: None,
        llm_api_url: None,
        llm_api_key: None,
        started_at: std::time::Instant::now(),
        boot_wall_clock: std::time::SystemTime::now(),
        auth_token: None,
    };
    let app = router(state, None);

    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from(KR0KI_OWN_DEPLOYMENT_MANIFEST))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(status, StatusCode::OK, "response body: {text}");
    assert!(text.contains("<svg"), "expected SVG output, got: {text}");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
