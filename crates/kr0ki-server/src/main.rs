//! kr0ki P0 server entrypoint.
//!
//! Config (env):
//!   KR0KI_BIND         default 0.0.0.0:8787
//!   KR0KI_CACHE_DIR    default ./.kr0ki-cache
//!   KR0KI_BACKEND_URL  default https://kroki.io   (point at a SECURE-mode Kroki)
//!
//! Auth (PRD FR7) is NOT in P0 — this is bind-to-localhost / behind-a-trusted-proxy
//! only until the auth boundary lands. Logged loudly at startup.

mod app;

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

    tracing::warn!(
        "kr0ki P0: no caller auth (PRD FR7 not implemented) — bind to localhost or run \
         behind a trusted authenticating proxy only"
    );
    tracing::info!(%bind, %cache_dir, %backend_url, "starting kr0ki");

    let service = RenderService::new(
        HttpKrokiBackend::new(&backend_url),
        FsCache::new(&cache_dir),
    );
    let state = AppState {
        service: Arc::new(service),
    };

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, router(state))
        .await
        .context("server error")?;
    Ok(())
}
