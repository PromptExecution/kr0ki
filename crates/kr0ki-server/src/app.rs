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
    RenderService, Rendered, ServiceError,
};
use serde::Serialize;

pub type Service = RenderService<HttpKrokiBackend>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<Service>,
    /// Built Vue/Vite assets. Empty in in-process tests unless a test supplies one.
    pub playbook_dir: PathBuf,
    /// CSI-mounted base directory holding `tags/<tag>/kerml-view.ttl` b00t-graph
    /// artifacts (kr0ki#13). `None` disables `/b00t-graph/:tag` (503, not a panic).
    pub b00t_graph_artifacts_path: Option<PathBuf>,
    /// Path to the `kroki` container's self-reported `capabilities.json`
    /// (kr0ki#20), written to a shared pod volume at its startup — see
    /// `deploy/kr0ki-local.pod.yaml`. `None` disables `/capabilities` (503).
    pub capabilities_path: Option<PathBuf>,
}

/// If `auth_token` is Some, inject a `RequireAuth` layer that rejects requests
/// missing `Authorization: Bearer <token>`.
pub fn router(state: AppState, auth_token: Option<String>) -> Router {
    let r = Router::new()
        .route("/health", get(health))
        .route("/formats", get(formats))
        .route("/capabilities", get(capabilities))
        .route("/api/examples", get(examples))
        .route("/playbook/api/examples.json", get(examples))
        .route("/playbook", get(playbook_index))
        .route("/playbook/", get(playbook_index))
        .route("/playbook/*path", get(playbook_asset))
        .route("/render/:format", post(render))
        .route("/b00t-graph/:tag", get(b00t_graph))
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

/// `GET /capabilities` — the `kroki` container's own self-reported
/// companion-required status per converter (kr0ki#20), read from a shared
/// pod volume the `kroki` container wrote at its startup. 503 when
/// `KR0KI_CAPABILITIES_PATH` isn't configured; 503 (not 404) when it's
/// configured but the file isn't there yet — a benign, retryable pod-startup
/// race (the two containers in `deploy/kr0ki-local.pod.yaml` aren't
/// ordered), not a hard error.
async fn capabilities(State(state): State<AppState>) -> Response {
    let Some(path) = state.capabilities_path.as_ref() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "capabilities_not_configured",
            "KR0KI_CAPABILITIES_PATH is not set on this server",
        );
    };

    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => (StatusCode::OK, Json(value)).into_response(),
            Err(e) => error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "capabilities_invalid_json",
                &e.to_string(),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "capabilities_not_ready",
            "capabilities.json not written yet — the kroki container may still be starting",
        ),
        Err(e) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "capabilities_read_failed",
            &e.to_string(),
        ),
    }
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
        Ok(r) => rendered_response(output, r),
        Err(e) => service_error_response(e),
    }
}

/// `GET /b00t-graph/{tag}?output=svg|png` — kr0ki#13. Reads a b00t-graph
/// artifact's Turtle dump (`{base}/tags/{tag}/kerml-view.ttl`) from a
/// CSI-mounted local path — no S3 SDK / HTTP fetch, and no signature
/// verification (the mount is the trust boundary for v1; both are this
/// issue's own decided scope, not oversights). Converts it through
/// `holon_viz::type_graph::TypeRelationshipGraph` to D2 text and renders it
/// via the same cache-in-front-of-a-backend path as `/render/{format}`, so
/// the response shape (headers, cache semantics) matches that route exactly.
async fn b00t_graph(
    State(state): State<AppState>,
    Path(tag): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let Some(base) = state.b00t_graph_artifacts_path.as_ref() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "b00t_graph_not_configured",
            "B00T_GRAPH_ARTIFACTS_PATH is not set on this server",
        );
    };

    let tag_path = std::path::Path::new(&tag);
    if tag.is_empty()
        || tag_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return error_json(StatusCode::BAD_REQUEST, "invalid_tag", "invalid tag");
    }

    let ttl_path = base.join("tags").join(&tag).join("kerml-view.ttl");
    let ttl = match tokio::fs::read(&ttl_path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return error_json(
                StatusCode::NOT_FOUND,
                "b00t_graph_not_found",
                &format!("no b00t-graph artifact for tag {tag:?}"),
            )
        }
        Err(e) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "b00t_graph_read_failed",
                &e.to_string(),
            )
        }
    };

    let type_graph = match kr0ki_core::b00t_graph::parse_turtle_to_type_graph(&ttl) {
        Ok(g) => g,
        Err(e) => {
            return error_json(
                StatusCode::UNPROCESSABLE_ENTITY,
                "b00t_graph_bad_turtle",
                &e.to_string(),
            )
        }
    };
    let d2 = kr0ki_core::b00t_graph::D2Emitter::emit(&type_graph.to_cytoscape());

    let output = params
        .get("output")
        .and_then(|v| OutputKind::from_param(v))
        .unwrap_or(OutputKind::Svg);

    match state.service.render(DiagramFormat::D2, output, &d2).await {
        Ok(r) => rendered_response(output, r),
        Err(e) => service_error_response(e),
    }
}

fn rendered_response(output: OutputKind, r: Rendered) -> Response {
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

fn service_error_response(e: ServiceError) -> Response {
    match e {
        ServiceError::Render(RenderError::BadSource { body, .. }) => {
            error_json(StatusCode::UNPROCESSABLE_ENTITY, "bad_source", &body)
        }
        ServiceError::Render(RenderError::Unavailable(msg)) => {
            error_json(StatusCode::BAD_GATEWAY, "backend_unavailable", &msg)
        }
        ServiceError::Render(RenderError::Flatten(msg)) => {
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "flatten_failed", &msg)
        }
        ServiceError::CacheIo(e) => error_json(
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
