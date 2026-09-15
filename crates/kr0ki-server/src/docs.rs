//! `/docs` routes — serve kr0ki's own documentation, harvested from source.
//!
//! Endpoints:
//!   GET /docs            → HTML index
//!   GET /docs/api.json   → JSON symbol export
//!   GET /docs/api.tomllm → tomllm format
//!   GET /docs/api.rustdoc → rustdoc format

use std::sync::Arc;

use crate::app::error_json;
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use kr0ki_core::docgen::{format, harvest_kr0ki_workspace};
use tokio::sync::RwLock;

use crate::app::AppState;

/// Shared in-memory cache of harvested symbols.
pub type DocCache = Arc<RwLock<Vec<kr0ki_core::docgen::Symbol>>>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/docs", get(docs_html))
        .route("/docs/api.json", get(docs_json))
        .route("/docs/api.tomllm", get(docs_tomllm))
        .route("/docs/api.rustdoc", get(docs_rustdoc))
}

async fn docs_html() -> Response {
    let symbols = match harvest_or_cache().await {
        Ok(s) => s,
        Err(e) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "harvest_failed",
                &e.to_string(),
            )
        }
    };

    let d2_source = std::fs::read_to_string("templates/b00t-stack-orchestration.d2")
        .unwrap_or_else(|_| "# D2 template not found\na -> b".into());

    let html = format::format_html(&symbols, "kr0ki documentation", &d2_source);

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

async fn docs_json() -> Response {
    let symbols = match harvest_or_cache().await {
        Ok(s) => s,
        Err(e) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "harvest_failed",
                &e.to_string(),
            )
        }
    };
    match format::format_json(&symbols) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
        Err(e) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "format_failed",
            &e.to_string(),
        ),
    }
}

async fn docs_tomllm() -> Response {
    let symbols = match harvest_or_cache().await {
        Ok(s) => s,
        Err(e) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "harvest_failed",
                &e.to_string(),
            )
        }
    };
    let body = format::format_tomllm(&symbols);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn docs_rustdoc() -> Response {
    let symbols = match harvest_or_cache().await {
        Ok(s) => s,
        Err(e) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "harvest_failed",
                &e.to_string(),
            )
        }
    };
    let body = format::format_rustdoc(&symbols);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

static DOC_CACHE: std::sync::OnceLock<DocCache> = std::sync::OnceLock::new();

async fn harvest_or_cache() -> anyhow::Result<Vec<kr0ki_core::docgen::Symbol>> {
    if let Some(cache) = DOC_CACHE.get() {
        let read = cache.read().await;
        if !read.is_empty() {
            return Ok(read.clone());
        }
    }
    let symbols = harvest_kr0ki_workspace()?;
    let cache = DOC_CACHE.get_or_init(|| Arc::new(RwLock::new(Vec::new())));
    let mut write = cache.write().await;
    *write = symbols.clone();
    Ok(symbols)
}
