//! In-process HTTP tests for the paths that don't require a render backend.
//! The cache→backend happy path is covered by kr0ki-core's unit tests
//! (`RenderService` + a stub backend) and by `tests/live_render.rs` (env-gated).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};
use tower::ServiceExt; // oneshot

// The server crate is a bin; pull the router module in via path.
#[path = "../src/app.rs"]
mod app;
#[path = "../src/docs.rs"]
mod docs;
use app::{router, AppState};

fn test_state(tag: &str) -> AppState {
    let dir = std::env::temp_dir().join(format!("kr0ki-http-test-{}-{}", std::process::id(), tag));
    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"), // unreachable — must never be called by these tests
        FsCache::new(dir),
    );
    AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: None,
        sysmlv2_client: None,
        model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),
        storyb00k_agent_url: None,
        llm_api_url: None,
        llm_api_key: None,
        started_at: std::time::Instant::now(),
        boot_wall_clock: std::time::SystemTime::now(),
        auth_token: None,
    }
}

fn test_app(state: AppState) -> axum::Router {
    router(state, None)
}

async fn body_string(resp: axum::response::Response) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn health_ok() {
    let app = test_app(test_state("health"));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    // The test backend (127.0.0.1:1) is unreachable by design, so the report
    // must honestly say "degraded" while keeping every field present.
    assert!(body.contains("\"status\":\"degraded\""), "body: {body}");
    assert!(body.contains("\"service\":\"kr0ki\""));
    assert!(body.contains("\"version\""));
    assert!(body.contains("\"uptime_secs\""));
    assert!(body.contains("\"started_at\""));
    assert!(body.contains("\"kroki_backend\""));
    assert!(body.contains("\"stores\""));
    assert!(body.contains("\"cache_dir\""));
    assert!(body.contains("\"llm\""));
    assert!(body.contains("\"configured\":false"));
    assert!(body.contains("\"model_count\""));
}

#[tokio::test]
async fn root_redirects_to_a_feature_welcome_page() {
    let redirect = test_app(test_state("root-redirect"))
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(redirect.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(redirect.headers().get("location").unwrap(), "/welcome");

    let response = test_app(test_state("welcome"))
        .oneshot(Request::get("/welcome").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Welcome to kr0ki"));
    assert!(body.contains("Render diagrams"));
    assert!(body.contains("/docs"));
    assert!(body.contains("/mcp/tools"));
}

#[tokio::test]
async fn formats_lists_supported_slugs_only() {
    let app = test_app(test_state("formats"));
    let resp = app
        .oneshot(Request::get("/formats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("plantuml") && body.contains("d2"));
    assert!(
        !body.contains("mermaid"),
        "companion-only formats must not be advertised"
    );
}

#[tokio::test]
async fn mcp_tools_lists_all_thirteen_tools_with_bindings() {
    let app = test_app(test_state("mcp-tools"));
    let resp = app
        .oneshot(Request::get("/mcp/tools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let tools: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(tools.len(), 13);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"render_diagram"));
    assert!(names.contains(&"list_formats"));
    assert!(names.contains(&"render_kubernetes_manifest"));
    assert!(names.contains(&"render_kubernetes_topology"));
    assert!(names.contains(&"render_sysmlv2_snapshot"));
    assert!(names.contains(&"import_reqif"));
    assert!(names.contains(&"query_model_graph"));

    let render = tools
        .iter()
        .find(|t| t["name"] == "render_diagram")
        .unwrap();
    assert_eq!(render["httpBinding"]["method"], "POST");
    assert_eq!(render["httpBinding"]["pathTemplate"], "/render/{format}");
    assert!(render["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("format")));
}

const MINIMAL_REQIF: &str = include_str!("../../kr0ki-core/tests/fixtures/reqif/minimal.reqif");

#[tokio::test]
async fn requirements_import_normalizes_raw_reqif_with_digest_provenance() {
    let response = test_app(test_state("requirements-import"))
        .oneshot(
            Request::post("/requirements/import")
                .header("content-type", "application/reqif+xml")
                .body(Body::from(MINIMAL_REQIF))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    let imported: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(imported["documents"].as_array().unwrap().len(), 1);
    assert_eq!(
        imported["documents"][0]["graph"]["baseline"]["id"],
        "BL-MINIMAL"
    );
    assert_eq!(
        imported["documents"][0]["graph"]["baseline"]["import_artifact_sha256"],
        imported["artifact_sha256"]
    );
}

#[tokio::test]
async fn requirements_import_rejects_malformed_reqif_without_calling_a_renderer() {
    let response = test_app(test_state("requirements-import-malformed"))
        .oneshot(
            Request::post("/requirements/import")
                .body(Body::from("<REQ-IF><CORE-CONTENT>"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("invalid_reqif_import"), "body: {body}");
}

#[tokio::test]
async fn requirements_import_rejects_an_oversized_upload_at_the_http_boundary() {
    let too_large = vec![0_u8; kr0ki_core::reqif_import::DEFAULT_MAX_REQIF_IMPORT_BYTES + 1];
    let response = test_app(test_state("requirements-import-too-large"))
        .oneshot(
            Request::post("/requirements/import")
                .body(Body::from(too_large))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

fn requirement_view_payload(kind: &str, confirmed_behaviour: Option<&str>) -> String {
    serde_json::json!({
        "graph": {
            "baseline": {"id": "BL-1", "revision": "commit-1"},
            "requirements": [
                {"id": "r1", "title": "Top", "text": "", "baseline": {"id": "BL-1", "revision": "commit-1"}, "provenance": {"source_uri": "reqif://fixture"}, "attributes": {}},
                {"id": "r2", "title": "Child", "text": "", "baseline": {"id": "BL-1", "revision": "commit-1"}, "provenance": {"source_uri": "reqif://fixture"}, "attributes": {}}
            ],
            "evidence": [],
            "relations": [{"id": "contains", "source": "r1", "target": "r2", "kind": "contains", "authority": {"status": "asserted"}, "provenance": {"source_uri": "reqif://fixture"}}]
        },
        "request": {"kind": kind, "scope": "authoritative", "direction": "downstream", "confirmed_behaviour": confirmed_behaviour}
    }).to_string()
}

#[tokio::test]
async fn requirements_view_returns_typed_induced_graph_not_renderer_source() {
    let payload = requirement_view_payload("decomposition", None);
    let response = test_app(test_state("requirements-view"))
        .oneshot(
            Request::post("/requirements/views")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"relations\":[{"));
    assert!(body.contains("\"kind\":\"contains\""));
    assert!(
        !body.contains(" -> "),
        "view response must not contain D2 source"
    );
}

#[tokio::test]
async fn behaviour_render_requires_human_confirmation() {
    let payload = requirement_view_payload("behaviour", None);
    let response = test_app(test_state("requirements-behaviour-confirmation"))
        .oneshot(
            Request::post("/render/requirements-view")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body.contains("behaviour_confirmation_required"));
}

#[tokio::test]
async fn model_routes_return_503_when_no_client_is_configured() {
    let app = test_app(test_state("model-unconfigured"));
    let response = app
        .clone()
        .oneshot(Request::get("/model/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("sysmlv2_client_not_configured"));

    let response = app
        .oneshot(
            Request::post("/render/sysmlv2/projects/proj-1/commits/c1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("sysmlv2_client_not_configured"));
}

#[tokio::test]
async fn render_sysmlv2_snapshot_uses_the_model_api_not_source_language_input() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/elements",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": "assembly", "@type": "PartDefinition", "name": "Assembly"},
                {"@id": "engine", "@type": "PartUsage", "name": "Engine"},
                {"@id": "owns-engine", "@type": "FeatureMembership",
                 "owner": {"@id": "assembly"}, "member": {"@id": "engine"}}
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/roots",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!(["assembly"])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/d2/svg"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<svg/>"))
        .mount(&server)
        .await;

    let mut state = test_state_with_sysmlv2_client("render-sysmlv2", server.uri());
    state.service = Arc::new(RenderService::new(
        HttpKrokiBackend::new(server.uri()),
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-http-test-{}-render-sysmlv2-backend",
            std::process::id()
        ))),
    ));
    let response = test_app(state)
        .oneshot(
            Request::post("/render/sysmlv2/projects/proj-1/commits/c1?output=svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "<svg/>");
}

fn test_state_with_sysmlv2_client(tag: &str, base_url: String) -> AppState {
    let mut state = test_state(tag);
    state.sysmlv2_client = Some(Arc::new(kr0ki_sysmlv2_client::SysmlV2Client::new(base_url)));
    state
}

#[tokio::test]
async fn model_projects_proxy_and_snapshot_materializes_the_graph() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": "proj-1", "name": "Toaster"}
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/elements",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": "elem-1", "@type": "PartUsage", "name": "Engine"}
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/roots",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!(["elem-1"])),
        )
        .mount(&server)
        .await;

    let response = test_app(test_state_with_sysmlv2_client(
        "model-projects",
        server.uri(),
    ))
    .oneshot(Request::get("/model/projects").body(Body::empty()).unwrap())
    .await
    .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("proj-1"));

    let state = test_state_with_sysmlv2_client("model-snapshot", server.uri());
    let graph = state.model_graph.clone();
    let response = test_app(state)
        .oneshot(
            Request::get("/model/projects/proj-1/commits/c1/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(graph
        .query(
            kr0ki_core::graph_store::QueryShape::TriplesAbout,
            Some("elem-1")
        )
        .iter()
        .any(|(_, predicate, object)| predicate == "name" && object == "Engine"));
}

#[tokio::test]
async fn model_graph_query_is_bounded_and_validates_its_shape() {
    let state = test_state("model-graph");
    let element: kr0ki_sysmlv2_client::Element = serde_json::from_value(serde_json::json!({
        "@id": "elem-1", "@type": "PartUsage", "name": "Engine"
    }))
    .unwrap();
    state
        .model_graph
        .insert_element_triples("proj-1", "c1", &element);
    let app = test_app(state);
    let response = app
        .clone()
        .oneshot(
            Request::get("/model/graph/query?shape=triples_about&subject=elem-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Engine"));

    let response = app
        .oneshot(
            Request::get("/model/graph/query?shape=unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_shape"));
}

#[tokio::test]
async fn examples_catalog_covers_every_advertised_format() {
    let app = test_app(test_state("examples"));
    let resp = app
        .oneshot(Request::get("/api/examples").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let examples: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    // Every standalone renderer needs a fixture. Some renderers deliberately
    // have several fixtures because a catalog type must show its own syntax.
    let format_routed: std::collections::BTreeSet<_> = examples
        .iter()
        .filter(|example| example["route"].is_null())
        .map(|example| example["format"].as_str().unwrap())
        .collect();
    assert_eq!(
        format_routed.len(),
        kr0ki_core::format::DiagramFormat::ALL.len()
    );
    assert!(examples.iter().all(|example| example["source"].is_string()));
    assert!(examples.iter().any(|example| example["format"] == "d2"));
    assert!(examples
        .iter()
        .any(|example| example["route"] == "/render/k8s-topology"));
}

#[tokio::test]
async fn catalog_serves_taxonomy_for_gallery_and_discovery() {
    let app = test_app(test_state("examples"));
    let resp = app
        .oneshot(Request::get("/api/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let catalog: serde_json::Value = serde_json::from_str(&body).unwrap();
    let types = catalog["types"].as_array().unwrap();
    assert!(
        types.len() >= 20,
        "taxonomy must expand PlantUML top-level types"
    );
    // Distinct addressible names.
    let mut ids: Vec<&str> = types.iter().filter_map(|t| t["id"].as_str()).collect();
    ids.sort_unstable();
    let len = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len);
    // Filter vocabulary is non-empty and every type's tags come from it.
    let use_cases = catalog["useCases"].as_array().unwrap();
    assert!(use_cases.iter().any(|t| t == "process flow"));
    for t in types {
        for tag in t["useCases"].as_array().unwrap() {
            assert!(use_cases.contains(tag), "tag {tag} outside vocabulary");
        }
        assert!(t["samplePrompt"].as_str().unwrap().len() > 20);
        let example_id = t["exampleId"].as_str().unwrap();
        assert!(kr0ki_core::examples::ALL
            .iter()
            .any(|example| example.id == example_id));
    }
    assert!(catalog["discoveryGuide"]
        .as_str()
        .unwrap()
        .contains("sequence"));
}

#[tokio::test]
async fn playbook_serves_built_vue_assets() {
    let directory = std::env::temp_dir().join(format!("kr0ki-playbook-{}", std::process::id()));
    tokio::fs::create_dir_all(directory.join("assets"))
        .await
        .unwrap();
    tokio::fs::write(directory.join("index.html"), "<main>playb00k</main>")
        .await
        .unwrap();
    tokio::fs::write(directory.join("assets/app.js"), "export default 'playb00k'")
        .await
        .unwrap();

    let mut state = test_state("playbookassets");
    state.playbook_dir = directory.clone();
    let app = test_app(state);

    let response = app
        .clone()
        .oneshot(Request::get("/playbook/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("playb00k"));

    let response = app
        .oneshot(
            Request::get("/playbook/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("export default"));

    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn render_unknown_format_is_400_before_any_backend_call() {
    let app = test_app(test_state("badfmt"));
    let resp = app
        .oneshot(
            Request::post("/render/mermaid")
                .body(Body::from("graph TD; A-->B"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("unsupported_format"));
}

#[tokio::test]
async fn render_empty_body_is_400() {
    let app = test_app(test_state("empty"));
    let resp = app
        .oneshot(Request::post("/render/d2").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_source"));
}

#[tokio::test]
async fn b00t_graph_is_503_when_not_configured() {
    // test_state() leaves b00t_graph_artifacts_path: None.
    let app = test_app(test_state("b00tgraph-unconfigured"));
    let resp = app
        .oneshot(Request::get("/b00t-graph/v1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("b00t_graph_not_configured"));
}

fn test_state_with_b00t_graph_dir(tag: &str, dir: std::path::PathBuf) -> AppState {
    let mut state = test_state(tag);
    state.b00t_graph_artifacts_path = Some(dir);
    state
}

#[tokio::test]
async fn b00t_graph_rejects_path_traversal_in_tag() {
    let dir =
        std::env::temp_dir().join(format!("kr0ki-b00tgraph-traversal-{}", std::process::id()));
    let app = test_app(test_state_with_b00t_graph_dir("traversal", dir));
    let resp = app
        .oneshot(
            Request::get("/b00t-graph/..%2f..%2fetc%2fpasswd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_tag"));
}

#[tokio::test]
async fn b00t_graph_is_404_for_a_missing_tag() {
    let dir = std::env::temp_dir().join(format!("kr0ki-b00tgraph-missing-{}", std::process::id()));
    let app = test_app(test_state_with_b00t_graph_dir("missing", dir));
    let resp = app
        .oneshot(
            Request::get("/b00t-graph/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("b00t_graph_not_found"));
}

#[tokio::test]
async fn b00t_graph_is_422_for_malformed_turtle_before_any_backend_call() {
    let dir =
        std::env::temp_dir().join(format!("kr0ki-b00tgraph-malformed-{}", std::process::id()));
    let tag_dir = dir.join("tags").join("v1");
    tokio::fs::create_dir_all(&tag_dir).await.unwrap();
    tokio::fs::write(tag_dir.join("kerml-view.ttl"), b"not turtle {{{")
        .await
        .unwrap();

    let app = test_app(test_state_with_b00t_graph_dir("malformed", dir.clone()));
    let resp = app
        .oneshot(Request::get("/b00t-graph/v1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("b00t_graph_bad_turtle"));

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn capabilities_is_503_when_not_configured() {
    // test_state() leaves capabilities_path: None.
    let app = test_app(test_state("capabilities-unconfigured"));
    let resp = app
        .oneshot(Request::get("/capabilities").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("capabilities_not_configured"));
}

#[tokio::test]
async fn capabilities_is_503_when_configured_but_file_not_yet_written() {
    let dir =
        std::env::temp_dir().join(format!("kr0ki-capabilities-missing-{}", std::process::id()));
    let mut state = test_state("capabilities-missing");
    state.capabilities_path = Some(dir.join("capabilities.json"));
    let app = test_app(state);
    let resp = app
        .oneshot(Request::get("/capabilities").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("capabilities_not_ready"));
}

#[tokio::test]
async fn capabilities_returns_the_file_contents_as_json() {
    let dir = std::env::temp_dir().join(format!("kr0ki-capabilities-ok-{}", std::process::id()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let path = dir.join("capabilities.json");
    tokio::fs::write(
        &path,
        br#"{"mermaid":{"version":"11.16.0","companion_required":true,"companion_available":false}}"#,
    )
    .await
    .unwrap();

    let mut state = test_state("capabilities-ok");
    state.capabilities_path = Some(path);
    let app = test_app(state);
    let resp = app
        .oneshot(Request::get("/capabilities").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"companion_required\":true"));

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn render_kubediagram_is_503_when_not_configured() {
    // test_state() leaves kubediagram_worker_url: None.
    let app = test_app(test_state("kubediagram-unconfigured"));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("kubediagram_worker_not_configured"));
}

fn test_state_with_kubediagram_worker(tag: &str, worker_url: String) -> AppState {
    let mut state = test_state(tag);
    state.kubediagram_worker_url = Some(worker_url);
    state
}

#[tokio::test]
async fn render_kubediagram_empty_body_is_400_before_any_worker_call() {
    // http://127.0.0.1:1 is unreachable — if the handler validates before
    // proxying, this call never actually reaches it.
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-empty",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_manifest"));
}

#[tokio::test]
async fn render_kubediagram_rejects_invalid_output_before_any_worker_call() {
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-badoutput",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram?output=png")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_output"));
}

#[tokio::test]
async fn render_kubediagram_rejects_oversized_manifest_before_any_worker_call() {
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-oversized",
        "http://127.0.0.1:1".to_string(),
    ));
    let oversized = "a".repeat(1_048_577);
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from(oversized))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(body.contains("manifest_too_large"));
}

#[tokio::test]
async fn render_kubediagram_proxies_to_the_worker_and_returns_svg() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/render"))
        .and(wiremock::matchers::query_param("output", "svg"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_bytes(b"<svg>ok</svg>".to_vec()),
        )
        .mount(&server)
        .await;

    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-proxy",
        server.uri(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<svg>ok</svg>"));
}

#[tokio::test]
async fn render_kubediagram_maps_worker_422_to_bad_manifest() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/render"))
        .respond_with(
            wiremock::ResponseTemplate::new(422).set_body_string("kube-diagrams failed: bad yaml"),
        )
        .mount(&server)
        .await;

    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-badmanifest",
        server.uri(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("not: valid: yaml: at all"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("bad_manifest"));
}

#[tokio::test]
async fn render_kubediagram_is_503_when_worker_unreachable() {
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-unreachable",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("kubediagram_worker_unreachable"));
}

#[tokio::test]
async fn render_k8s_topology_empty_body_is_400() {
    let app = test_app(test_state("k8s-topology-empty"));
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_manifest"));
}

#[tokio::test]
async fn render_k8s_topology_rejects_oversized_manifest_before_parsing() {
    let app = test_app(test_state("k8s-topology-oversized"));
    let oversized = "a".repeat(1_048_577);
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology")
                .body(Body::from(oversized))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(body.contains("manifest_too_large"));
}

#[tokio::test]
async fn render_k8s_topology_rejects_malformed_yaml_before_any_backend_call() {
    // http://127.0.0.1:1 (test_state's fixed backend) is unreachable -- if
    // the handler validates/parses before rendering, this call never
    // reaches it.
    let app = test_app(test_state("k8s-topology-badyaml"));
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology")
                .body(Body::from("not: [valid yaml"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("bad_manifest"));
}

#[tokio::test]
async fn render_k8s_topology_all_null_documents_is_400_empty_manifest() {
    // A bare "---" with nothing else parses as a single null YAML document;
    // after filtering nulls, zero manifests remain.
    let app = test_app(test_state("k8s-topology-allnull"));
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology")
                .body(Body::from("---"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_manifest"));
}

#[tokio::test]
async fn render_k8s_topology_valid_manifest_reaches_the_unreachable_backend() {
    // A well-formed, recognizable manifest passes validation and parsing,
    // so the handler proceeds all the way to state.service.render() --
    // proving the recognize -> lift -> to_d2 pipeline itself didn't error
    // out before ever reaching the (deliberately unreachable) backend.
    let app = test_app(test_state("k8s-topology-valid"));
    let manifest = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n";
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology")
                .body(Body::from(manifest))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    // Not a validation error -- the pipeline ran and only the network call
    // to the unreachable stub backend failed (RenderError::Unavailable ->
    // 502, service_error_response).
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!body.contains("empty_manifest"));
    assert!(!body.contains("bad_manifest"));
}

#[tokio::test]
async fn auth_required_rejects_missing_token() {
    let app = router(test_state("authed"), Some("secret".to_string()));
    let resp = app
        .oneshot(Request::get("/formats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("unauthorized"));
}

#[tokio::test]
async fn auth_required_accepts_valid_bearer() {
    let app = router(test_state("authed-ok"), Some("secret".to_string()));
    let resp = app
        .oneshot(
            Request::get("/health")
                .header("Authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("status"));
}

#[tokio::test]
async fn render_png_query_param_rejected_without_backend() {
    // We can't hit a real backend in unit tests, but we can verify the route
    // accepts ?output=png and forwards it to the service (which will fail on the
    // unreachable backend address, returning 502).
    let app = test_app(test_state("png-route"));
    let resp = app
        .oneshot(
            Request::post("/render/graphviz?output=png")
                .body(Body::from("digraph { a -> b }"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("backend_unavailable"));
}

#[tokio::test]
async fn cache_get_rejects_malformed_key() {
    let app = test_app(test_state("badkey"));
    let resp = app
        .oneshot(
            Request::get("/cache/not-a-hash")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cache_get_miss_is_404() {
    let app = test_app(test_state("miss"));
    let key = "0".repeat(64);
    let resp = app
        .oneshot(
            Request::get(format!("/cache/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("not_found"));
}

#[tokio::test]
async fn docs_html_returns_valid_page() {
    let app = test_app(test_state("docshtml"));
    let resp = app
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<!DOCTYPE html>"));
    assert!(body.contains("kr0ki documentation"));
    assert!(body.contains("Diagram Example"));
    assert!(body.contains("/docs/examples/kr0ki-render-flow.svg"));
    assert!(body.contains("/docs/api.json"));
}

#[tokio::test]
async fn docs_rust_flow_uses_the_render_service_cache() {
    use kr0ki_core::{cache::OutputKind, format::DiagramFormat};

    let state = test_state("docsflow");
    let source = include_str!("../../../templates/kr0ki-render-flow.d2");
    let key = kr0ki_core::cache::cache_key(DiagramFormat::D2, OutputKind::Svg, source);
    state
        .service
        .cache()
        .put(&key, OutputKind::Svg, b"<svg id=\"cached-flow\"/>")
        .await
        .unwrap();

    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::get("/docs/examples/kr0ki-render-flow.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("cached-flow"));
}

#[tokio::test]
async fn docs_json_returns_symbol_array() {
    let app = test_app(test_state("docsjson"));
    let resp = app
        .oneshot(Request::get("/docs/api.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let arr = parsed.as_array().expect("json is an array");
    assert!(!arr.is_empty(), "harvested at least one symbol");
    let first = &arr[0];
    assert!(first.get("name").is_some());
    assert!(first.get("qualified_name").is_some());
    assert!(first.get("signature").is_some());
}

#[tokio::test]
async fn docs_tomllm_has_boilerplate() {
    let app = test_app(test_state("docstoml"));
    let resp = app
        .oneshot(
            Request::get("/docs/api.tomllm")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("b00t:map v1"));
    assert!(body.contains("[[kr0ki_core::"));
}

#[tokio::test]
async fn docs_rustdoc_has_source_marker() {
    let app = test_app(test_state("docsrust"));
    let resp = app
        .oneshot(
            Request::get("/docs/api.rustdoc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("/// # Source"));
    // The rustdoc format uses qualified_name in the header, not body text
    assert!(body.contains("/// `"));
}
