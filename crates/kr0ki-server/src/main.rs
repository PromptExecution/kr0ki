//! kr0ki server entrypoint.
//!
//! Config (env):
//!   KR0KI_BIND                 default 0.0.0.0:8787
//!   KR0KI_CACHE_DIR            default ./.kr0ki-cache
//!   KR0KI_BACKEND_URL          default https://kroki.io   (point at a SECURE-mode Kroki)
//!   KR0KI_AUTH_TOKEN           if set, require `Authorization: Bearer <token>` on every
//!                              request except /health (FR7 minimal implementation).
//!   B00T_GRAPH_ARTIFACTS_PATH  if set, enables GET /b00t-graph/:tag (kr0ki#13) —
//!                              a CSI-mounted base dir holding
//!                              tags/<tag>/kerml-view.ttl b00t-graph artifacts.
//!                              Unset disables the route with a 503, not a panic.
//!   KR0KI_CAPABILITIES_PATH    if set, enables GET /capabilities (kr0ki#20) —
//!                              the kroki container's self-reported
//!                              companion-required status, on a shared pod
//!                              volume it wrote at its own startup. Unset
//!                              disables the route with a 503, not a panic.
//!   KR0KI_KUBEDIAGRAM_WORKER_URL if set, enables POST /render/kubediagram —
//!                              the kr0ki-mcp sidecar's internal
//!                              http_worker.py listener, e.g.
//!                              http://127.0.0.1:8788. Unset disables the
//!                              route with a 503, not a panic.
//!   KR0KI_SYSMLV2_BASE_URL      if set, enables read-only `/model/*` routes.
//!   KR0KI_SYSMLV2_TOKEN         optional bearer token for that model server.

mod app;
mod dev_session;
mod docs;
mod phase0_fixture;
pub mod workspace_types;

use std::sync::Arc;

use anyhow::Context;
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};

use crate::app::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kr0ki=info,tower_http=info".into()),
        )
        .init();

    let bind = std::env::var("KR0KI_BIND").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let cache_dir = std::env::var("KR0KI_CACHE_DIR").unwrap_or_else(|_| "./.kr0ki-cache".into());
    let backend_url =
        std::env::var("KR0KI_BACKEND_URL").unwrap_or_else(|_| "https://kroki.io".into());
    let auth_token = std::env::var("KR0KI_AUTH_TOKEN").ok();
    let playbook_dir = std::env::var("KR0KI_PLAYBOOK_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./playbook/dist"));
    let b00t_graph_artifacts_path = std::env::var("B00T_GRAPH_ARTIFACTS_PATH")
        .ok()
        .map(std::path::PathBuf::from);
    let capabilities_path = std::env::var("KR0KI_CAPABILITIES_PATH")
        .ok()
        .map(std::path::PathBuf::from);
    let kubediagram_worker_url = std::env::var("KR0KI_KUBEDIAGRAM_WORKER_URL").ok();
    // Deep /health probes the storyb00k agent and the LLM endpoint it uses.
    // The LLM check enumerates models only — never an inference request.
    let storyb00k_agent_url = std::env::var("KR0KI_STORYB00K_AGENT_URL").ok();
    let llm_api_url = std::env::var("OPENAI_API_URL").ok();
    let llm_api_key = std::env::var("OPENAI_API_KEY").ok();
    let sysmlv2_client = std::env::var("KR0KI_SYSMLV2_BASE_URL")
        .ok()
        .filter(|base_url| !base_url.trim().is_empty())
        .map(|base_url| {
            let client = kr0ki_sysmlv2_client::SysmlV2Client::new(base_url);
            Arc::new(match std::env::var("KR0KI_SYSMLV2_TOKEN").ok() {
                Some(token) => client.with_token(token),
                None => client,
            })
        });

    let hostname = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "localhost".to_string());

    if auth_token.is_none() {
        tracing::warn!(
            "kr0ki: KR0KI_AUTH_TOKEN not set — no caller auth. Bind to localhost \
             or run behind a trusted proxy only."
        );
    } else {
        tracing::info!("caller auth enabled (FR7 minimal)");
    }
    tracing::info!(
        %bind, %cache_dir, %backend_url, %hostname,
        playbook_dir = %playbook_dir.display(),
        b00t_graph_artifacts_path = ?b00t_graph_artifacts_path.as_ref().map(|p| p.display().to_string()),
        capabilities_path = ?capabilities_path.as_ref().map(|p| p.display().to_string()),
        kubediagram_worker_url = ?kubediagram_worker_url,
        sysmlv2_configured = sysmlv2_client.is_some(),
        "starting kr0ki"
    );

    let service = RenderService::new(
        HttpKrokiBackend::new(&backend_url),
        FsCache::new(&cache_dir),
    );
    let state = AppState {
        service: Arc::new(service),
        playbook_dir,
        b00t_graph_artifacts_path,
        capabilities_path,
        kubediagram_worker_url,
        sysmlv2_client,
        model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),
        storyb00k_agent_url,
        llm_api_url,
        llm_api_key,
        started_at: std::time::Instant::now(),
        boot_wall_clock: std::time::SystemTime::now(),
        auth_token: auth_token.clone(),
    };

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    let local_addr = listener.local_addr()?;
    tracing::info!(
        "listening on {local_addr} — docs at http://{hostname}:{}/docs",
        local_addr.port()
    );

    axum::serve(listener, router(state, auth_token))
        .await
        .context("server error")?;
    Ok(())
}
