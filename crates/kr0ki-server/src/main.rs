//! kr0ki server entrypoint.
//!
//! Config (env):
//!   KR0KI_BIND         default 0.0.0.0:8787
//!   KR0KI_CACHE_DIR    default ./.kr0ki-cache
//!   KR0KI_BACKEND_URL  default https://kroki.io   (point at a SECURE-mode Kroki)
//!   KR0KI_AUTH_TOKEN   if set, require `Authorization: Bearer <token>` on every
//!                      request except /health (FR7 minimal implementation).

mod app;
mod docs;

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
    tracing::info!(%bind, %cache_dir, %backend_url, %hostname, "starting kr0ki");

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
