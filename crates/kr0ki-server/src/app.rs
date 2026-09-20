//! The axum router, split from `main` so tests can drive it in-process.

use std::str::FromStr;
use std::sync::Arc;
use std::{path::Component, path::PathBuf};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use kr0ki_core::{
    cache::{CacheStatus, OutputKind},
    format::DiagramFormat,
    render::{HttpKrokiBackend, RenderError},
    RenderService, Rendered, ServiceError,
};

pub type Service = RenderService<HttpKrokiBackend>;

// Declared here (not in main.rs) so every build context that includes app.rs
// — the bin crate and the integration-test binaries — resolves this module
// relative to src/, where health.rs lives.
#[path = "health.rs"]
pub mod health;

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
    /// Base URL of the `kr0ki-mcp` sidecar's internal KubeDiagrams-rendering
    /// listener (`http_worker.py`), e.g. `http://127.0.0.1:8788` — mcp-http-
    /// parity design, 2026-09-16. `None` disables `/render/kubediagram`
    /// (503, not a panic).
    pub kubediagram_worker_url: Option<String>,
    /// Read-only client for the configured OMG Systems Modeling API server. `None`
    /// deliberately makes every `/model/*` route return a retryable 503.
    pub sysmlv2_client: Option<Arc<kr0ki_sysmlv2_client::SysmlV2Client>>,
    /// Disposable RDF triples materialized from model-query results. This is a
    /// derived cache, never an authoritative model store.
    pub model_graph: Arc<kr0ki_core::graph_store::GraphStore>,
    /// AG-UI storyb00k sidecar base URL (deep /health probe). `None` skips it.
    pub storyb00k_agent_url: Option<String>,
    /// OpenAI-compatible LLM endpoint for the storyb00k agent. The /health LLM
    /// check only enumerates models (GET /models) — never an inference call.
    pub llm_api_url: Option<String>,
    pub llm_api_key: Option<String>,
    /// Process boot instant (uptime) and wall-clock boot time (RFC3339 reporting).
    pub started_at: std::time::Instant,
    pub boot_wall_clock: std::time::SystemTime,
    /// Caller-auth token presence for /health reporting (material never echoed).
    pub auth_token: Option<String>,
}

/// If `auth_token` is Some, inject a `RequireAuth` layer that rejects requests
/// missing `Authorization: Bearer <token>`.
pub fn router(state: AppState, auth_token: Option<String>) -> Router {
    let r = Router::new()
        .route("/", get(root))
        .route("/welcome", get(welcome))
        .route("/health", get(health))
        .route("/formats", get(formats))
        .route("/mcp/tools", get(mcp_tools))
        .route("/capabilities", get(capabilities))
        .route("/api/examples", get(examples))
        .route("/playbook/api/examples.json", get(examples))
        .route("/api/catalog", get(catalog))
        .route("/playbook", get(playbook_index))
        .route("/playbook/", get(playbook_index))
        .route("/playbook/*path", get(playbook_asset))
        .route(
            "/requirements/import",
            post(import_requirements).layer(DefaultBodyLimit::max(
                kr0ki_core::reqif_import::DEFAULT_MAX_REQIF_IMPORT_BYTES,
            )),
        )
        .route("/requirements/views", post(requirements_view))
        .route("/render/:format", post(render))
        .route("/render/requirements-view", post(render_requirements_view))
        .route("/render/kubediagram", post(render_kubediagram))
        .route("/render/k8s-topology", post(render_k8s_topology))
        .route(
            "/render/sysmlv2/projects/:project_id/commits/:commit_id",
            post(render_sysmlv2_snapshot),
        )
        .route("/model/projects", get(list_model_projects))
        .route(
            "/model/projects/:project_id/commits",
            get(list_model_commits),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/snapshot",
            get(get_model_snapshot),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/elements",
            get(query_model_elements),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/roots",
            get(get_model_roots),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/elements/:element_id/relationships",
            get(query_model_relationships),
        )
        .route("/model/graph/query", get(query_model_graph))
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

async fn health(State(state): State<AppState>) -> Json<health::HealthReport> {
    Json(health::collect(&state).await)
}

async fn root() -> Redirect {
    Redirect::temporary("/welcome")
}

async fn welcome() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Welcome to kr0ki</title>
<style>
:root { color-scheme: light dark; }
body { font-family: system-ui, sans-serif; line-height: 1.5; max-width: 56rem; margin: 3rem auto; padding: 0 1rem; }
h1 { margin-bottom: .25rem; }
.lede { font-size: 1.2rem; color: #666; }
.grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr)); gap: 1rem; margin: 2rem 0; }
article { border: 1px solid #8886; border-radius: .6rem; padding: 1rem; }
code { background: #8882; border-radius: .25rem; padding: .1rem .3rem; }
a { color: #5271ff; }
</style>
</head>
<body>
<main>
<h1>Welcome to kr0ki</h1>
<p class="lede">A small, cache-aware HTTP service that turns validated diagram models into portable artifacts.</p>
<div class="grid">
<article><h2>Render diagrams</h2><p>POST source to <code>/render/{format}</code> for standalone Kroki formats such as D2, GraphViz, and PlantUML.</p></article>
<article><h2>Reuse artifacts</h2><p>Content-addressed caching returns repeat renders efficiently; retrieve known artifacts at <code>/cache/{key}</code>.</p></article>
<article><h2>Explore the API</h2><p>Read the live <a href="/docs">documentation</a>, discover formats at <a href="/formats"><code>/formats</code></a>, or inspect MCP-compatible bindings at <a href="/mcp/tools"><code>/mcp/tools</code></a>.</p></article>
<article><h2>Try the playbook</h2><p>When bundled, the interactive <a href="/playbook/">playbook</a> provides test-backed diagram examples and previews.</p></article>
</div>
<p>For automation, check <a href="/health"><code>/health</code></a>. If this deployment uses caller authentication, include its configured bearer token for every route except health.</p>
<div id="health-badge" aria-live="polite" style="margin-top:1.5rem;font-size:.95rem;border:1px solid #8886;border-radius:.6rem;padding:.75rem 1rem;">Validating backend health…</div>
<script>
(function () {
  var el = document.getElementById('health-badge');
  fetch('/health').then(function (r) { return r.json(); }).then(function (h) {
    var parts = ['<strong>' + (h.status === 'ok' ? '\\u2705 healthy' : '\\u26a0 degraded') + '</strong>',
      'kr0ki v' + h.version,
      'uptime ' + Math.floor(h.uptime_secs / 60) + 'm ' + (h.uptime_secs % 60) + 's'];
    if (h.checks.llm.configured) {
      parts.push(h.checks.llm.ok ? 'LLM ok (' + h.checks.llm.model_count + ' model(s))' : 'LLM unreachable');
    } else {
      parts.push('LLM not configured');
    }
    if (!h.checks.kroki_backend.ok) { parts.push('render backend DOWN'); }
    if (h.status !== 'ok') {
      var failed = [];
      if (!h.checks.kroki_backend.ok) failed.push('kroki backend');
      if (h.checks.stores && !h.checks.stores.cache_dir.writable) failed.push('cache dir');
      parts.push('failing: ' + failed.join(', '));
    }
    el.innerHTML = parts.join(' &middot; ');
  }).catch(function (e) {
    el.innerHTML = '<strong>\\u274c backend unreachable</strong> &middot; ' + String(e);
  });
})();
</script>
</main>
</body>
</html>"#,
    )
}

async fn formats() -> Json<Vec<&'static str>> {
    Json(DiagramFormat::ALL.iter().map(|f| f.kroki_slug()).collect())
}

/// Backend-neutral input shared by the Playb00k HTTP surface, an MCP adapter,
/// and a future Flexo Web Modeler client. A Flexo/ReqIF adapter owns loading
/// the baseline; kr0ki receives the normalized graph only.
#[derive(Debug, serde::Deserialize)]
struct RequirementsViewInput {
    graph: kr0ki_core::requirements::RequirementGraph,
    request: kr0ki_core::requirements::ViewRequest,
}

/// `POST /requirements/import` — accept a raw ReqIF XML document or ReqIFz
/// archive, validate it under the bounded M2 intake policy, and return the
/// resulting normalized graphs.  This service never persists the source or
/// attachments: a requirements store (for example Flexo) owns retention.
///
/// The route accepts bytes rather than a `file://` or arbitrary URL so a
/// deployment cannot accidentally turn kr0ki into a server-side file/network
/// reader.  A future approved HTTPS fetcher and an allow-listed CLI/MCP local
/// loader can call the same core import boundary after acquiring bytes.
async fn import_requirements(body: Bytes) -> Response {
    match kr0ki_core::reqif_import::import_reqif_artifact(
        &body,
        &kr0ki_core::reqif_import::ReqIfImportConfig::default(),
    ) {
        Ok(imported) => Json(imported).into_response(),
        Err(
            kr0ki_core::reqif_import::ReqIfImportError::ArtifactTooLarge { .. }
            | kr0ki_core::reqif_import::ReqIfImportError::ExpandedTooLarge { .. },
        ) => error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            "reqif_import_too_large",
            "ReqIF import exceeds this deployment's size policy",
        ),
        Err(error) => error_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_reqif_import",
            &error.to_string(),
        ),
    }
}

/// `POST /requirements/views` — return an induced typed graph, not diagram
/// source. This is the stable view contract for requirements clients.
async fn requirements_view(Json(input): Json<RequirementsViewInput>) -> Response {
    match input.graph.view(&input.request) {
        Ok(view) => Json(view).into_response(),
        Err(error) => error_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_requirement_view",
            &error.to_string(),
        ),
    }
}

/// `GET /mcp/tools` — the MCP/HTTP capability manifest (mcp-http-parity
/// design, 2026-09-16). One entry per `McpTool`, each carrying both its MCP
/// schema and its HTTP binding — `containers/kr0ki-mcp/bridge.py` fetches
/// this once and dispatches every `tools/call` generically from it.
async fn mcp_tools() -> Json<Vec<serde_json::Value>> {
    Json(
        kr0ki_core::mcp_tool::McpTool::ALL
            .iter()
            .map(|t| t.to_manifest_json())
            .collect(),
    )
}

fn require_sysmlv2_client(
    state: &AppState,
) -> Result<Arc<kr0ki_sysmlv2_client::SysmlV2Client>, Box<Response>> {
    state.sysmlv2_client.clone().ok_or_else(|| {
        Box::new(error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "sysmlv2_client_not_configured",
            "KR0KI_SYSMLV2_BASE_URL is not set on this server",
        ))
    })
}

fn client_error_response(error: kr0ki_sysmlv2_client::ClientError) -> Response {
    error_json(
        StatusCode::BAD_GATEWAY,
        "sysmlv2_upstream_error",
        &error.to_string(),
    )
}

async fn list_model_projects(State(state): State<AppState>) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    match client.projects().await {
        Ok(projects) => Json(projects).into_response(),
        Err(error) => client_error_response(error),
    }
}

async fn list_model_commits(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    match client.commits(&project_id).await {
        Ok(commits) => Json(commits).into_response(),
        Err(error) => client_error_response(error),
    }
}

async fn get_model_snapshot(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    match client.snapshot(&project_id, &commit_id).await {
        Ok(snapshot) => {
            for element in &snapshot.elements {
                state
                    .model_graph
                    .insert_element_triples(&project_id, &commit_id, element);
            }
            Json(snapshot).into_response()
        }
        Err(error) => client_error_response(error),
    }
}

/// Render a content-addressed SysML v2 snapshot from the configured model
/// server. A compiler may produce the model upstream; kr0ki never ingests or
/// interprets the compiler's source language.
async fn render_sysmlv2_snapshot(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    let snapshot = match client.snapshot(&project_id, &commit_id).await {
        Ok(snapshot) => snapshot,
        Err(error) => return client_error_response(error),
    };
    let edges = kr0ki_core::ufo_graph::build_ufo_graph(&snapshot);
    let relations: Vec<_> = kr0ki_core::sysml_lift::lift_edges(&edges)
        .into_iter()
        .map(|lifted| lifted.relation)
        .collect();
    let d2 = kr0ki_core::sysml_render::to_d2(&relations);
    let output = params
        .get("output")
        .and_then(|value| OutputKind::from_param(value))
        .unwrap_or(OutputKind::Svg);

    match state
        .service
        .render_model(
            "sysmlv2-snapshot",
            DiagramFormat::D2,
            output,
            &d2,
            &snapshot.content_hash,
            "sysmlv2-ufo-graph-v1",
        )
        .await
    {
        Ok(rendered) => rendered_response(output, rendered),
        Err(error) => service_error_response(error),
    }
}

async fn query_model_elements(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    match client.all_elements(&project_id, &commit_id).await {
        Ok(elements) => {
            for element in &elements {
                state
                    .model_graph
                    .insert_element_triples(&project_id, &commit_id, element);
            }
            Json(elements).into_response()
        }
        Err(error) => client_error_response(error),
    }
}

async fn get_model_roots(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    match client.roots(&project_id, &commit_id).await {
        Ok(roots) => Json(roots).into_response(),
        Err(error) => client_error_response(error),
    }
}

async fn query_model_relationships(
    State(state): State<AppState>,
    Path((project_id, commit_id, element_id)): Path<(String, String, String)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(client) => client,
        Err(response) => return *response,
    };
    let direction = match params.get("direction").map(String::as_str) {
        Some("in") => kr0ki_sysmlv2_client::Direction::In,
        Some("out") => kr0ki_sysmlv2_client::Direction::Out,
        _ => kr0ki_sysmlv2_client::Direction::Both,
    };
    match client
        .relationships(&project_id, &commit_id, &element_id, direction)
        .await
    {
        Ok(elements) => {
            for element in &elements {
                state.model_graph.insert_relationship_triple(
                    &element_id,
                    element.ty(),
                    element.id(),
                );
            }
            Json(elements).into_response()
        }
        Err(error) => client_error_response(error),
    }
}

/// `GET /model/graph/query?shape=triples_about|related_via&subject=<id>`.
/// This is purposefully a closed query surface, never a general SPARQL endpoint.
async fn query_model_graph(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let shape = match params.get("shape").map(String::as_str) {
        Some("triples_about") => kr0ki_core::graph_store::QueryShape::TriplesAbout,
        Some("related_via") => kr0ki_core::graph_store::QueryShape::RelatedVia,
        _ => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "invalid_shape",
                "shape must be triples_about or related_via",
            )
        }
    };
    let triples = state
        .model_graph
        .query(shape, params.get("subject").map(String::as_str));
    Json(
        triples
            .into_iter()
            .map(|(subject, predicate, object)| {
                serde_json::json!({
                    "subject": subject, "predicate": predicate, "object": object
                })
            })
            .collect::<Vec<_>>(),
    )
    .into_response()
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

/// `GET /api/catalog` — the intent-first diagram-type taxonomy (Plan 005):
/// distinct addressible typeIds, use-case tags for the gallery filter, and
/// per-type sample prompts for the Agent handoff button.
async fn catalog() -> Json<serde_json::Value> {
    use kr0ki_core::catalog;
    Json(serde_json::json!({
        "useCases": catalog::used_use_cases(),
        "types": catalog::TYPES.iter().map(|t| serde_json::json!({
            "id": t.id,
            "syntax": t.syntax,
            "exampleId": t.example_id,
            "name": t.name,
            "useCases": t.use_cases,
            "blurb": t.blurb,
            "samplePrompt": t.sample_prompt,
        })).collect::<Vec<_>>(),
        "discoveryGuide": catalog::discovery_guide(),
    }))
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

/// `POST /render/requirements-view?output=svg|png` — the semantic graph
/// boundary for ReqIF/Flexo requirements. It first induces a typed view and
/// only then lowers that view to D2 for the existing cache-aware renderer.
/// A behaviour view must carry the user's explicit confirmation before it can
/// become an artifact; recommendations alone are intentionally non-rendering.
async fn render_requirements_view(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    Json(input): Json<RequirementsViewInput>,
) -> Response {
    if input.request.kind == kr0ki_core::requirements::ViewKind::Behaviour
        && input.request.confirmed_behaviour.is_none()
    {
        return error_json(
            StatusCode::CONFLICT,
            "behaviour_confirmation_required",
            "confirm the recommended behaviour view before rendering it",
        );
    }
    let view = match input.graph.view(&input.request) {
        Ok(view) => view,
        Err(error) => {
            return error_json(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_requirement_view",
                &error.to_string(),
            )
        }
    };
    let output = params
        .get("output")
        .and_then(|v| OutputKind::from_param(v))
        .unwrap_or(OutputKind::Svg);
    let d2 = kr0ki_core::requirements_render::to_d2(&view);
    match state.service.render(DiagramFormat::D2, output, &d2).await {
        Ok(rendered) => rendered_response(output, rendered),
        Err(error) => service_error_response(error),
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

/// The plan's Global Constraints mandate the 1 MiB manifest limit be
/// enforced exactly once, here in `kr0ki-server`'s HTTP handler — not
/// duplicated in `bridge.py` or `http_worker.py` (both of which also happen
/// to enforce it downstream, but this is the one authoritative check).
const MAX_MANIFEST_BYTES: usize = 1_048_576;

/// `POST /render/kubediagram?output=svg|dot_json` — mcp-http-parity design,
/// 2026-09-16. Body is a Kubernetes manifest (multi-doc YAML). Proxies to
/// the `kr0ki-mcp` sidecar's internal `/render` listener — never through
/// kr0ki-server's own content-addressed cache (backlog, see the design
/// doc's §1 — kube-diagrams' output isn't itself Kroki-renderable text, so
/// it needs its own cache-key derivation, deliberately deferred).
async fn render_kubediagram(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: Bytes,
) -> Response {
    let Some(worker_url) = state.kubediagram_worker_url.as_ref() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "kubediagram_worker_not_configured",
            "KR0KI_KUBEDIAGRAM_WORKER_URL is not set on this server",
        );
    };

    if body.is_empty() {
        return error_json(
            StatusCode::BAD_REQUEST,
            "empty_manifest",
            "kubernetes manifest body is empty",
        );
    }

    if body.len() > MAX_MANIFEST_BYTES {
        return error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            "manifest_too_large",
            "kubernetes manifest exceeds the 1 MiB limit",
        );
    }

    let output = params.get("output").map(String::as_str).unwrap_or("svg");
    if output != "svg" && output != "dot_json" {
        return error_json(
            StatusCode::BAD_REQUEST,
            "invalid_output",
            "output must be svg or dot_json",
        );
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{worker_url}/render?output={output}"))
        .body(body.to_vec())
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let content_type = if output == "svg" {
                "image/svg+xml"
            } else {
                "application/json"
            };
            match r.bytes().await {
                Ok(bytes) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, content_type)],
                    bytes,
                )
                    .into_response(),
                Err(e) => error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "kubediagram_worker_read_failed",
                    &e.to_string(),
                ),
            }
        }
        Ok(r) if r.status() == StatusCode::UNPROCESSABLE_ENTITY => {
            let body = r.text().await.unwrap_or_default();
            error_json(StatusCode::UNPROCESSABLE_ENTITY, "bad_manifest", &body)
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            error_json(
                StatusCode::BAD_GATEWAY,
                "kubediagram_worker_error",
                &format!("{status}: {body}"),
            )
        }
        Err(e) => error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "kubediagram_worker_unreachable",
            &e.to_string(),
        ),
    }
}

/// `POST /render/k8s-topology?output=svg|png` — the new native pipeline
/// (`docs/PATTERNS-kubernetes.md`, `crates/kr0ki-core/src/{k8s_recognizer,
/// sysml_lift,sysml_render}.rs`), distinct from `/render/kubediagram`'s proxy
/// to the vendored KubeDiagrams tool. Body is a multi-doc Kubernetes YAML
/// manifest bundle. `k8s_recognizer::KubernetesRecognizer::recognize` lifts
/// it to `OntologicalEdge`s (box 2/3), `sysml_lift::lift_edges` lifts those to
/// `sysml_model::Relation` (box 4), `sysml_render::to_d2` emits D2 text, and
/// — unlike `/render/kubediagram` — that text goes through the same
/// content-addressed `RenderService` cache every other `/render/*` route
/// uses (mirrors `b00t_graph`'s own parse-then-`state.service.render` shape).
async fn render_k8s_topology(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: Bytes,
) -> Response {
    if body.is_empty() {
        return error_json(
            StatusCode::BAD_REQUEST,
            "empty_manifest",
            "kubernetes manifest body is empty",
        );
    }
    if body.len() > MAX_MANIFEST_BYTES {
        return error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            "manifest_too_large",
            "kubernetes manifest exceeds the 1 MiB limit",
        );
    }
    let text = match std::str::from_utf8(&body) {
        Ok(t) => t,
        Err(_) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "invalid_utf8",
                "manifest is not UTF-8",
            )
        }
    };

    let manifests = match parse_multi_doc_yaml(text) {
        Ok(m) => m,
        Err(e) => {
            return error_json(
                StatusCode::UNPROCESSABLE_ENTITY,
                "bad_manifest",
                &e.to_string(),
            )
        }
    };
    if manifests.is_empty() {
        return error_json(
            StatusCode::BAD_REQUEST,
            "empty_manifest",
            "manifest contains no documents",
        );
    }

    let recognizer = kr0ki_core::k8s_recognizer::KubernetesRecognizer::new();
    let edges = recognizer.recognize(&manifests);
    let relations: Vec<_> = kr0ki_core::sysml_lift::lift_edges(&edges)
        .into_iter()
        .map(|lifted| lifted.relation)
        .collect();
    let d2 = kr0ki_core::sysml_render::to_d2(&relations);

    let output = params
        .get("output")
        .and_then(|v| OutputKind::from_param(v))
        .unwrap_or(OutputKind::Svg);

    match state.service.render(DiagramFormat::D2, output, &d2).await {
        Ok(r) => rendered_response(output, r),
        Err(e) => service_error_response(e),
    }
}

/// Split a multi-doc YAML manifest bundle (`---`-separated) into one
/// `serde_json::Value` per document, skipping empty/`null` documents (a
/// leading or trailing bare `---` produces one). Each document is
/// deserialized directly from the YAML `Deserializer` into `Value` — no
/// intermediate `serde_yaml::Value` — so nested numeric/string typing
/// matches what `k8s_recognizer` (built against real `kubectl get -o json`
/// shape) expects.
fn parse_multi_doc_yaml(text: &str) -> Result<Vec<serde_json::Value>, serde_yaml::Error> {
    use serde::Deserialize;
    serde_yaml::Deserializer::from_str(text)
        .map(serde_json::Value::deserialize)
        .collect::<Result<Vec<_>, _>>()
        .map(|docs| docs.into_iter().filter(|v| !v.is_null()).collect())
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
