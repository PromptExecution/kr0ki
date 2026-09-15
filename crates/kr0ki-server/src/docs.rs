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
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use kr0ki_core::{
    cache::{CacheStatus, OutputKind},
    docgen::{format, harvest_kr0ki_workspace},
    format::DiagramFormat,
    render::RenderError,
    ServiceError,
};
use tokio::sync::RwLock;

use crate::app::AppState;

/// Shared in-memory cache of harvested symbols.
pub type DocCache = Arc<RwLock<Vec<kr0ki_core::docgen::Symbol>>>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/docs", get(docs_html))
        .route(
            "/docs/examples/kr0ki-render-flow.svg",
            get(docs_rust_flow_svg),
        )
        .route("/docs/api.json", get(docs_json))
        .route("/docs/api.tomllm", get(docs_tomllm))
        .route("/docs/api.rustdoc", get(docs_rustdoc))
}

const RUST_FLOW_D2: &str = include_str!("../../../templates/kr0ki-render-flow.d2");

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

    let html = format::format_html_with_live_flow(
        &symbols,
        "kr0ki documentation",
        &d2_source,
        Some("/docs/examples/kr0ki-render-flow.svg"),
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

/// Render the playb00k's executable D2 fixture through the same service used by
/// callers. This makes the live documentation an end-to-end visual harness,
/// including cache-hit behaviour, rather than a hand-drawn approximation.
async fn docs_rust_flow_svg(State(state): State<AppState>) -> Response {
    match state
        .service
        .render(DiagramFormat::D2, OutputKind::Svg, RUST_FLOW_D2)
        .await
    {
        Ok(rendered) => {
            let cache = match rendered.status {
                CacheStatus::Hit => "hit",
                CacheStatus::Miss => "miss",
            };
            (
                StatusCode::OK,
                [
                    (
                        header::CONTENT_TYPE,
                        OutputKind::Svg.content_type().to_string(),
                    ),
                    (
                        header::HeaderName::from_static("x-kr0ki-cache"),
                        cache.to_string(),
                    ),
                    (header::HeaderName::from_static("x-kr0ki-key"), rendered.key),
                ],
                rendered.bytes,
            )
                .into_response()
        }
        Err(ServiceError::Render(RenderError::BadSource { body, .. })) => {
            error_json(StatusCode::UNPROCESSABLE_ENTITY, "bad_source", &body)
        }
        Err(ServiceError::Render(RenderError::Unavailable(message))) => {
            error_json(StatusCode::BAD_GATEWAY, "backend_unavailable", &message)
        }
        Err(ServiceError::CacheIo(error)) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cache_io",
            &error.to_string(),
        ),
    }
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
