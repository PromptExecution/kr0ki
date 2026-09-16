//! The axum router, split from `main` so tests can drive it in-process.

use std::str::FromStr;
use std::sync::Arc;
use std::{path::Component, path::PathBuf};

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
    /// Built Vue/Vite assets. Empty in in-process tests unless a test supplies one.
    pub playbook_dir: PathBuf,
}

/// If `auth_token` is Some, inject a `RequireAuth` layer that rejects requests
/// missing `Authorization: Bearer <token>`.
pub fn router(state: AppState, auth_token: Option<String>) -> Router {
    let r = Router::new()
        .route("/health", get(health))
        .route("/formats", get(formats))
        .route("/api/examples", get(examples))
        .route("/playbook/api/examples.json", get(examples))
        .route("/playbook", get(playbook_index))
        .route("/playbook/", get(playbook_index))
        .route("/playbook/*path", get(playbook_asset))
        .route("/render/:format", post(render))
        .route("/cache/:key", get(cache_get))
        .merge(crate::docs::routes())
        .with_state(state);

    if let Some(token) = auth_token {
        r.layer(axum::middleware::from_fn(move |req, next| {
            require_bearer(req, next, token.clone())
        }))
    } else {
        r
    }
}

async fn require_bearer(
    req: axum::extract::Request,
    next: axum::middleware::Next,
    token: String,
) -> Response {
    let hdr = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    match hdr {
        Some(v) if v == format!("Bearer {token}") => next.run(req).await,
        _ => error_json(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid bearer token",
        ),
    }
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

/// `GET /api/examples` — the single example catalog shared by Rust tests,
/// mdb00k static export, and the Vue/Vite playb00k.
async fn examples() -> Json<&'static [kr0ki_core::examples::PlaybookExample]> {
    Json(kr0ki_core::examples::ALL)
}

async fn playbook_index(State(state): State<AppState>) -> Response {
    playbook_file(&state.playbook_dir, "index.html").await
}

async fn playbook_asset(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    playbook_file(&state.playbook_dir, &path).await
}

async fn playbook_file(root: &std::path::Path, requested: &str) -> Response {
    let relative = std::path::Path::new(requested);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "invalid_asset_path",
            "invalid playb00k asset",
        );
    }

    let path = root.join(relative);
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, playbook_content_type(&path))],
            bytes,
        )
            .into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => error_json(
            StatusCode::NOT_FOUND,
            "playbook_asset_not_found",
            "playb00k assets are not installed in this server image",
        ),
        Err(error) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "playbook_asset_read_failed",
            &error.to_string(),
        ),
    }
}

fn playbook_content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// `POST /render/{format}?output=svg|png` — body is the raw diagram source.
/// `X-Kr0ki-Cache: hit|miss` and `X-Kr0ki-Key: <sha256>` on success.
async fn render(
    State(state): State<AppState>,
    Path(format): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
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

    let output = params
        .get("output")
        .and_then(|v| OutputKind::from_param(v))
        .unwrap_or(OutputKind::Svg);

    match state.service.render(format, output, source).await {
        Ok(r) => {
            let cache_hdr = match r.status {
                CacheStatus::Hit => "hit",
                CacheStatus::Miss => "miss",
            };
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, output.content_type().to_string()),
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

/// `GET /cache/{key}?output=svg|png` — serve a previously rendered artifact.
async fn cache_get(
    State(state): State<AppState>,
    Path(key): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "bad_key",
            "key must be 64 hex chars",
        );
    }
    let output = params
        .get("output")
        .and_then(|v| OutputKind::from_param(v))
        .unwrap_or(OutputKind::Svg);

    match state.service.cache().get(&key, output).await {
        Ok(Some(bytes)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, output.content_type())],
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

pub(crate) fn error_json(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error": code, "message": message })),
    )
        .into_response()
}
