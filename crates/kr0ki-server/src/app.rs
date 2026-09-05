//! The axum router, split from `main` so tests can drive it in-process.

use std::str::FromStr;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use kr0ki_core::{
    cache::{CacheStatus, OutputKind},
    format::DiagramFormat,
    render::{HttpKrokiBackend, RenderError},
    RenderService, ServiceError,
};
use serde::Serialize;

pub type Service = RenderService<HttpKrokiBackend>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<Service>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/formats", get(formats))
        .route("/render/:format", post(render))
        .route("/cache/:key", get(cache_get))
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        service: "kr0ki",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn formats() -> Json<Vec<&'static str>> {
    Json(DiagramFormat::ALL.iter().map(|f| f.kroki_slug()).collect())
}

/// `POST /render/{format}` — body is the raw diagram source. Returns SVG.
/// `X-Kr0ki-Cache: hit|miss` and `X-Kr0ki-Key: <sha256>` on success.
async fn render(
    State(state): State<AppState>,
    Path(format): Path<String>,
    body: Bytes,
) -> Response {
    let format = match DiagramFormat::from_str(&format) {
        Ok(f) => f,
        Err(e) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "unsupported_format",
                &e.to_string(),
            )
        }
    };

    let source = match std::str::from_utf8(&body) {
        Ok(s) if !s.trim().is_empty() => s,
        Ok(_) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "empty_source",
                "diagram source is empty",
            )
        }
        Err(_) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "invalid_utf8",
                "diagram source is not UTF-8",
            )
        }
    };

    match state.service.render(format, OutputKind::Svg, source).await {
        Ok(r) => {
            let cache_hdr = match r.status {
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
                        cache_hdr.to_string(),
                    ),
                    (header::HeaderName::from_static("x-kr0ki-key"), r.key),
                ],
                r.bytes,
            )
                .into_response()
        }
        Err(ServiceError::Render(RenderError::BadSource { body, .. })) => {
            error_json(StatusCode::UNPROCESSABLE_ENTITY, "bad_source", &body)
        }
        Err(ServiceError::Render(RenderError::Unavailable(msg))) => {
            error_json(StatusCode::BAD_GATEWAY, "backend_unavailable", &msg)
        }
        Err(ServiceError::CacheIo(e)) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cache_io",
            &e.to_string(),
        ),
    }
}

/// `GET /cache/{key}` — serve a previously rendered artifact by its content hash.
/// P0 serves SVG only, so the key alone is enough.
async fn cache_get(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "bad_key",
            "key must be 64 hex chars",
        );
    }
    match state.service.cache().get(&key, OutputKind::Svg).await {
        Ok(Some(bytes)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, OutputKind::Svg.content_type())],
            bytes,
        )
            .into_response(),
        Ok(None) => error_json(
            StatusCode::NOT_FOUND,
            "not_found",
            "no artifact for that key",
        ),
        Err(e) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cache_io",
            &e.to_string(),
        ),
    }
}

fn error_json(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error": code, "message": message })),
    )
        .into_response()
}
