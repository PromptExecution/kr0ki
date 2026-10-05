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
#[path = "../src/contract.rs"]
mod contract;
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
        ui_bus: Arc::new(kr0ki_core::ui_bus::UiBus::new()),
        brand_dir: std::env::temp_dir().join("kr0ki-no-brands"),
        sysml_mcp: None,
        storyb00k_agent_url: None,
        llm_api_url: None,
        llm_api_key: None,
        started_at: std::time::Instant::now(),
        boot_wall_clock: std::time::SystemTime::now(),
        auth_token: None,
        contract: Arc::new(contract::ContractReference::default()),
        assurance: None,
    }
}

fn test_app(state: AppState) -> axum::Router {
    let contract = state.contract.clone();
    router(state, None, contract)
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
async fn contract_headers_present_on_all_responses() {
    let app = test_app(test_state("contract-headers"));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let headers = resp.headers();
    assert!(
        headers.contains_key("x-kr0ki-contract"),
        "missing x-kr0ki-contract header"
    );
    assert!(
        headers.contains_key("x-kr0ki-request-id"),
        "missing x-kr0ki-request-id header"
    );

    let contract = headers.get("x-kr0ki-contract").unwrap().to_str().unwrap();
    assert_eq!(contract, "ledgrrr://state-machines/sysml-render/v1");

    let request_id = headers.get("x-kr0ki-request-id").unwrap().to_str().unwrap();
    assert!(!request_id.is_empty(), "request-id should not be empty");
    assert!(
        request_id.len() == 36,
        "request-id should be UUID v7 format (36 chars)"
    );
}

#[tokio::test]
async fn request_id_adopted_from_header() {
    let app = test_app(test_state("request-id-adopted"));
    let custom_id = "my-custom-correlation-id-12345";
    let resp = app
        .oneshot(
            Request::get("/health")
                .header("x-request-id", custom_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let returned_id = resp
        .headers()
        .get("x-kr0ki-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        returned_id, custom_id,
        "should adopt caller-provided request-id"
    );
}

#[tokio::test]
async fn health_includes_contract_field() {
    let app = test_app(test_state("health-contract"));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\"contract\":\"ledgrrr://state-machines/sysml-render/v1\""),
        "health should include contract field, body: {body}"
    );
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
async fn mcp_tools_lists_all_tools_with_bindings() {
    let app = test_app(test_state("mcp-tools"));
    let resp = app
        .oneshot(Request::get("/mcp/tools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let tools: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(tools.len(), 32);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"render_diagram"));
    assert!(names.contains(&"list_formats"));
    assert!(names.contains(&"render_kubernetes_manifest"));
    assert!(names.contains(&"render_kubernetes_topology"));
    assert!(names.contains(&"render_sysmlv2_snapshot"));
    assert!(names.contains(&"import_reqif"));
    assert!(names.contains(&"import_reqif_url"));
    assert!(names.contains(&"query_model_graph"));
    assert!(names.contains(&"recompute_and_evaluate"));
    assert!(names.contains(&"sync_digital_thread"));

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
async fn model_recompute_evaluates_rule_docs_and_returns_violations() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/p1/commits"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": "c1", "@type": "Commit"}
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/p1/commits/c1/elements",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"@id": "elem-1", "@type": "PartUsage", "name": "BadPart"},
            {
                "@id": "rule:no-bad-parts",
                "@type": "RuleDocument",
                "name": "No BadPart allowed",
                "rego": "package kr0ki\n\nviolations := [v |\n    some n\n    input.nodes[n].label == \"BadPart\"\n    v := {\"element_id\": input.nodes[n].id, \"reason\": \"BadPart is not allowed\"}\n]\n"
            }
        ])))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/p1/commits/c1/roots"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!(["elem-1"])),
        )
        .mount(&server)
        .await;

    let response = test_app(test_state_with_sysmlv2_client("recompute", server.uri()))
        .oneshot(
            Request::post("/model/projects/p1/recompute")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"disposition\""));
    assert!(body.contains("BadPart is not allowed"));
}

const ONE_DBT_NODE_GRAPH: &str = r#"{"nodes":[{"id":"dbt:model.a","stereotype":{"Kind":"DbtModel"},"label":"A (marts)"}],"edges":[]}"#;

async fn mount_sync_project(server: &wiremock::MockServer, elements: serde_json::Value) {
    for (p, body) in [
        (
            "/projects/p1/commits",
            serde_json::json!([{"@id": "c1", "@type": "Commit"}]),
        ),
        ("/projects/p1/commits/c1/elements", elements),
    ] {
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(p))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }
}

async fn post_sync(
    server: &wiremock::MockServer,
    tag: &str,
    body: &'static str,
) -> (StatusCode, String) {
    let response = test_app(test_state_with_sysmlv2_client(tag, server.uri()))
        .oneshot(
            Request::post("/model/projects/p1/sync")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    body_string(response).await
}

#[tokio::test]
async fn model_sync_commits_the_graph_and_returns_the_commit() {
    let server = wiremock::MockServer::start().await;
    mount_sync_project(&server, serde_json::json!([])).await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/projects/p1/commits"))
        .and(wiremock::matchers::body_partial_json(
            serde_json::json!({"previousCommit": {"@id": "c1"}}),
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"@id": "c2", "@type": "Commit"})),
        )
        .mount(&server)
        .await;

    let (status, body) = post_sync(&server, "sync-ok", ONE_DBT_NODE_GRAPH).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("\"@id\":\"c2\""), "{body}");
}

#[tokio::test]
async fn model_sync_persistent_conflict_is_409() {
    let server = wiremock::MockServer::start().await;
    mount_sync_project(&server, serde_json::json!([])).await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(409))
        .mount(&server)
        .await;
    let (status, body) = post_sync(&server, "sync-conflict", ONE_DBT_NODE_GRAPH).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("sync_conflict"));
}

#[tokio::test]
async fn model_sync_duplicate_server_identifiers_are_422() {
    let server = wiremock::MockServer::start().await;
    mount_sync_project(
        &server,
        serde_json::json!([
            {"@id": "s1", "@type": "PartUsage", "identifier": "dbt:model.a", "name": "A"},
            {"@id": "s2", "@type": "PartUsage", "identifier": "dbt:model.a", "name": "A"}
        ]),
    )
    .await;
    let (status, body) = post_sync(&server, "sync-dupes", ONE_DBT_NODE_GRAPH).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body.contains("duplicate_identifier"));
}

#[tokio::test]
async fn model_sync_upstream_failure_is_502() {
    let server = wiremock::MockServer::start().await;
    mount_sync_project(&server, serde_json::json!([])).await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (status, _) = post_sync(&server, "sync-502", ONE_DBT_NODE_GRAPH).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn model_sync_rejects_a_body_that_is_not_a_sysgraph() {
    let server = wiremock::MockServer::start().await;
    let (status, body) = post_sync(&server, "sync-bad", r#"{"nodes": "nope"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("invalid_graph"));
}

#[tokio::test]
async fn model_sync_without_a_configured_client_is_503() {
    let response = test_app(test_state("sync-unconfigured"))
        .oneshot(
            Request::post("/model/projects/p1/sync")
                .body(Body::from(ONE_DBT_NODE_GRAPH))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn model_recompute_without_a_configured_client_is_503() {
    let response = test_app(test_state("recompute-unconfigured"))
        .oneshot(
            Request::post("/model/projects/p1/recompute")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
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
async fn render_kubediagram_second_call_with_same_body_hits_cache_not_worker() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/render"))
        .and(wiremock::matchers::query_param("output", "svg"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_bytes(b"<svg>ok</svg>".to_vec()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-cache-hit",
        server.uri(),
    ));

    let manifest = "apiVersion: v1\nkind: Pod";

    let resp1 = app
        .clone()
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from(manifest))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status1, body1) = body_string(resp1).await;
    assert_eq!(status1, StatusCode::OK);

    let resp2 = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from(manifest))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status2, body2) = body_string(resp2).await;
    assert_eq!(status2, StatusCode::OK);

    assert_eq!(body1, body2);
    assert_eq!(body1, "<svg>ok</svg>");

    // wiremock's `.expect(1)` is verified on `server` drop -- if the second
    // request had hit the worker again, this would panic.
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
async fn render_k8s_topology_unrecognized_view_query_param_is_400() {
    // The view-kind check happens after recognize/lift but before the
    // (deliberately unreachable) backend is ever called, so this 400 is
    // reachable even with test_state's fixed-unreachable backend.
    let app = test_app(test_state("k8s-topology-badview"));
    let manifest = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n";
    let resp = app
        .oneshot(
            Request::post("/render/k8s-topology?view=not_a_real_view")
                .body(Body::from(manifest))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("unknown_view_kind"), "body: {body}");
}

#[tokio::test]
async fn render_k8s_topology_recognized_view_query_param_renders_only_that_view() {
    // A ReplicaSet with an ownerReferences pointer to a Deployment lifts to
    // exactly one HasPart edge (k8s_recognizer::tests::
    // owner_reference_lifts_to_has_part_from_owner_to_owned), which
    // sysml_lift maps to Relation::FeatureMembership in SysmlViewKind::Tree
    // (sysml_lift.rs's mapping table). `?view=tree` should therefore reach
    // the render backend instead of 400ing.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/d2/svg"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<svg/>"))
        .mount(&server)
        .await;

    let mut state = test_state("k8s-topology-goodview");
    state.service = Arc::new(RenderService::new(
        HttpKrokiBackend::new(server.uri()),
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-http-test-{}-k8s-topology-goodview-backend",
            std::process::id()
        ))),
    ));
    let manifest = "apiVersion: apps/v1\nkind: ReplicaSet\nmetadata:\n  name: web-abc123\n  namespace: ns1\n  ownerReferences:\n    - kind: Deployment\n      name: web\n      apiVersion: apps/v1\n";
    let resp = test_app(state)
        .oneshot(
            Request::post("/render/k8s-topology?view=tree")
                .body(Body::from(manifest))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body, "<svg/>");
}

#[tokio::test]
async fn render_rust_source_empty_body_is_400() {
    let app = test_app(test_state("rust-source-empty"));
    let resp = app
        .oneshot(
            Request::post("/render/rust-source")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_source"));
}

#[tokio::test]
async fn render_rust_source_rejects_oversized_source_before_parsing() {
    let app = test_app(test_state("rust-source-oversized"));
    let oversized = "a".repeat(1_048_577);
    let resp = app
        .oneshot(
            Request::post("/render/rust-source")
                .body(Body::from(oversized))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(body.contains("source_too_large"));
}

#[tokio::test]
async fn render_rust_source_rejects_invalid_rust_before_any_backend_call() {
    // http://127.0.0.1:1 (test_state's fixed backend) is unreachable -- if
    // the handler validates/parses before rendering, this call never
    // reaches it.
    let app = test_app(test_state("rust-source-badsyntax"));
    let resp = app
        .oneshot(
            Request::post("/render/rust-source")
                .body(Body::from("struct Unclosed {"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("bad_rust_source"));
}

#[tokio::test]
async fn render_rust_source_valid_source_reaches_the_unreachable_backend() {
    // A struct field of another local struct type lifts to exactly one
    // HasPart edge (rust_recognizer's own struct_field_of_local_type_is_has_part
    // test), which rust_lift maps straight through to UfoRelation::HasPart --
    // proving the recognize -> lift -> to_d2 pipeline itself didn't error
    // out before ever reaching the (deliberately unreachable) backend.
    let app = test_app(test_state("rust-source-valid"));
    let source = "struct Engine {}
struct Car { engine: Engine }
";
    let resp = app
        .oneshot(
            Request::post("/render/rust-source")
                .body(Body::from(source))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    // Not a validation error -- the pipeline ran and only the network call
    // to the unreachable stub backend failed (RenderError::Unavailable ->
    // 502, service_error_response).
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!body.contains("empty_source"));
    assert!(!body.contains("bad_rust_source"));
}

#[tokio::test]
async fn render_rust_source_end_to_end_through_a_real_backend() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/d2/svg"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<svg/>"))
        .mount(&server)
        .await;

    let mut state = test_state("rust-source-e2e");
    state.service = Arc::new(RenderService::new(
        HttpKrokiBackend::new(server.uri()),
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-http-test-{}-rust-source-e2e-backend",
            std::process::id()
        ))),
    ));
    let source = "struct Engine {}
struct Car { engine: Engine }
";
    let resp = test_app(state)
        .oneshot(
            Request::post("/render/rust-source")
                .body(Body::from(source))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body, "<svg/>");
}

#[tokio::test]
async fn auth_required_rejects_missing_token() {
    let app = router(
        test_state("authed"),
        Some("secret".to_string()),
        Arc::new(contract::ContractReference::default()),
    );
    let resp = app
        .oneshot(Request::get("/formats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("unauthorized"));
}

/// Contract/correlation headers are part of every response, including the
/// 401 the auth layer produces itself (auth must not sit outside the contract layer).
#[tokio::test]
async fn unauthorized_responses_still_carry_contract_headers() {
    let app = router(
        test_state("authed-headers"),
        Some("secret".to_string()),
        Arc::new(contract::ContractReference::default()),
    );
    let resp = app
        .oneshot(
            Request::get("/formats")
                .header("x-request-id", "corr-401")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("x-kr0ki-contract")
            .map(|v| v.to_str().unwrap()),
        Some("ledgrrr://state-machines/sysml-render/v1")
    );
    assert_eq!(
        resp.headers()
            .get("x-kr0ki-request-id")
            .map(|v| v.to_str().unwrap()),
        Some("corr-401")
    );
}

/// `/health` is always reachable without a token (pod readiness probes, `just validate-server`);
/// every other route stays guarded.
#[tokio::test]
async fn health_is_exempt_from_bearer_auth_but_other_routes_are_not() {
    let app = router(
        test_state("health-exempt"),
        Some("secret".to_string()),
        Arc::new(contract::ContractReference::default()),
    );
    let health = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert!(health.headers().contains_key("x-kr0ki-contract"));

    for guarded in ["/formats", "/healthz", "/health/extra", "/cache/abc"] {
        let resp = app
            .clone()
            .oneshot(Request::get(guarded).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{guarded} must stay guarded"
        );
    }
}

#[tokio::test]
async fn auth_required_accepts_valid_bearer() {
    let app = router(
        test_state("authed-ok"),
        Some("secret".to_string()),
        Arc::new(contract::ContractReference::default()),
    );
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

#[tokio::test]
async fn import_requirements_url_rejects_non_https_scheme() {
    let response = test_app(test_state("reqif-url-scheme"))
        .oneshot(
            Request::post("/requirements/import/url")
                .header("content-type", "text/plain")
                .body(Body::from("http://example.com/doc.reqif"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body.contains("reqif_fetch_rejected"), "body: {body}");
}

#[tokio::test]
async fn import_requirements_url_rejects_disallowed_address() {
    let response = test_app(test_state("reqif-url-ssrf"))
        .oneshot(
            Request::post("/requirements/import/url")
                .header("content-type", "text/plain")
                .body(Body::from("https://127.0.0.1/doc.reqif"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body.contains("reqif_fetch_rejected"), "body: {body}");
}

// ---- planning agent: catalog tools + UI steering ---------------------------------------------

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

async fn post_json(app: axum::Router, uri: &str, body: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(
            Request::post(uri)
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn catalog_can_be_narrowed_to_one_use_case_and_rejects_unknown_ones() {
    let state = test_state("catalog-filter");
    let (s, all) = get_json(test_app(state.clone()), "/api/catalog").await;
    assert_eq!(s, StatusCode::OK);
    let (s, narrowed) = get_json(
        test_app(state.clone()),
        "/api/catalog?use_case=data%20model",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let n = narrowed["types"].as_array().unwrap();
    assert!(!n.is_empty() && n.len() < all["types"].as_array().unwrap().len());
    assert!(n.iter().all(|t| t["useCases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "data model")));
    // 'All' and empty mean no filter
    let (_, same) = get_json(test_app(state.clone()), "/api/catalog?use_case=All").await;
    assert_eq!(same["types"], all["types"]);
    let (s, err) = get_json(test_app(state), "/api/catalog?use_case=vibes").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err["error"], "unknown_use_case");
}

#[tokio::test]
async fn suggest_returns_an_explained_ranking_and_validates_input() {
    let state = test_state("suggest");
    let (s, v) = post_json(
        test_app(state.clone()),
        "/api/catalog/suggest?limit=3",
        "show the database tables and their foreign key relationships",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let sug = v["suggestions"].as_array().unwrap();
    assert!(!sug.is_empty() && sug.len() <= 3);
    assert!(sug[0]["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.as_str().unwrap().contains("data model")));
    assert!(sug[0]["name"].is_string());
    let (s, v) = post_json(test_app(state.clone()), "/api/catalog/suggest", "zzz qqq").await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["suggestions"].as_array().unwrap().is_empty());
    let (s, _) = post_json(
        test_app(state.clone()),
        "/api/catalog/suggest?use_cases=vibes",
        "x",
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = post_json(
        test_app(state),
        "/api/catalog/suggest",
        &"x".repeat(17 * 1024),
    )
    .await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn navigate_reaches_a_connected_ui_over_sse_and_reports_zero_when_none_is_connected() {
    use http_body_util::BodyExt;
    let state = test_state("ui-nav");
    // Nobody listening yet: honest zero, nothing allocated.
    let (s, v) = post_json(
        test_app(state.clone()),
        "/ui/sess-1/navigate?view=gallery",
        "",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["delivered"], 0);

    // The UI connects.
    let events = test_app(state.clone())
        .oneshot(
            Request::get("/ui/sess-1/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    assert_eq!(events.headers()["content-type"], "text/event-stream");
    let mut body = events.into_body();

    let type_id = kr0ki_core::catalog::TYPES[0].id;
    let (s, v) = post_json(
        test_app(state.clone()),
        &format!("/ui/sess-1/navigate?view=gallery&use_case=process%20flow&type_id={type_id}"),
        "",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["delivered"], 1);
    assert_eq!(v["commands"], 3);

    let mut text = String::new();
    while text.matches("event: ui").count() < 3 {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.frame())
            .await
            .expect("SSE frame in time")
            .expect("stream open")
            .unwrap();
        if let Some(data) = frame.data_ref() {
            text.push_str(&String::from_utf8_lossy(data));
        }
    }
    assert!(
        text.contains(r#""type":"open_view""#) && text.contains(r#""view":"gallery""#),
        "{text}"
    );
    assert!(
        text.contains(r#""type":"filter_gallery""#) && text.contains("process flow"),
        "{text}"
    );
    assert!(
        text.contains(r#""type":"select_type""#) && text.contains(type_id),
        "{text}"
    );
}

#[tokio::test]
async fn navigate_rejects_anything_outside_the_catalog_and_bad_session_ids() {
    let state = test_state("ui-nav-bad");
    for q in [
        "view=nowhere",
        "use_case=vibes",
        "type_id=%3Cscript%3E",
        "suggest=,",
        "",
    ] {
        let (s, v) = post_json(
            test_app(state.clone()),
            &format!("/ui/sess-1/navigate?{q}"),
            "",
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{q}");
        assert_eq!(v["error"], "bad_ui_command");
    }
    let (s, v) = post_json(
        test_app(state.clone()),
        "/ui/bad%20id/navigate?view=gallery",
        "",
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"], "bad_session");
    let resp = test_app(state)
        .oneshot(
            Request::get("/ui/bad%20id/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mcp_manifest_advertises_the_planner_tools() {
    let (s, v) = get_json(test_app(test_state("mcp-planner")), "/mcp/tools").await;
    assert_eq!(s, StatusCode::OK);
    let names: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in ["list_diagram_types", "suggest_diagram_type", "navigate_ui"] {
        assert!(names.contains(&n), "{n} missing from {names:?}");
    }
}

#[tokio::test]
async fn sysmlv2_render_labels_nodes_with_names_but_keys_them_by_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/elements",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": "0000-assembly", "@type": "PartDefinition", "name": "Assembly"},
                {"@id": "0000-engine", "@type": "PartUsage", "name": "Engine"},
                {"@id": "owns-engine", "@type": "FeatureMembership",
                 "owner": {"@id": "0000-assembly"}, "member": {"@id": "0000-engine"}}
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/projects/proj-1/commits/c1/roots",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!(["0000-assembly"])),
        )
        .mount(&server)
        .await;
    // The backend only answers when the D2 it receives has id keys AND name labels.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/d2/svg"))
        .and(wiremock::matchers::body_string_contains(
            "\"0000-engine\": \"Engine\"",
        ))
        .and(wiremock::matchers::body_string_contains(
            "\"0000-assembly\" -> \"0000-engine\"",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<svg>named</svg>"))
        .mount(&server)
        .await;
    let mut state = test_state_with_sysmlv2_client("render-sysmlv2-names", server.uri());
    state.service = Arc::new(RenderService::new(
        HttpKrokiBackend::new(server.uri()),
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-http-test-{}-render-sysmlv2-names",
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
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "<svg>named</svg>");
}

// ---- brand overlay on SysML renders --------------------------------------------------------------

const SYSML_D2_FIXTURE: &str = include_str!("../../kr0ki-svg/tests/fixtures/sysml-d2.svg");

fn brand_state(tag: &str, backend: &str, sysml: &str) -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("kr0ki-brands-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("test")).unwrap();
    std::fs::write(
        dir.join("test/brand.json"),
        r##"{"version":1,"icons":{"part":{"view_box":"0 0 24 24","svg":"<rect width=\"24\" height=\"24\"/>"}},
            "rules":[{"select":"g[data-kr0ki-type=\"PartUsage\"]","class":"k-part","icon":"part"}],"css":".k-part rect{stroke:#0ea5e9 !important}"}"##,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("broken")).unwrap();
    std::fs::write(
        dir.join("broken/brand.json"),
        r#"{"icons":{"x":{"svg":"<script/>"}}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("garbage")).unwrap();
    std::fs::write(dir.join("garbage/brand.json"), "not json").unwrap();
    let mut state = test_state_with_sysmlv2_client(tag, sysml.to_owned());
    state.service = Arc::new(RenderService::new(
        HttpKrokiBackend::new(backend),
        FsCache::new(std::env::temp_dir().join(format!(
            "kr0ki-http-test-{}-{tag}-cache",
            std::process::id()
        ))),
    ));
    state.brand_dir = dir.clone();
    (state, dir)
}

async fn brand_fixture_server() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    let u = |n: u32| format!("00000000-0000-4000-8000-{n:012}");
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/p1/commits/c1/elements"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"@id": u(1), "@type": "PartDefinition", "name": "Vehicle"},
                {"@id": u(2), "@type": "PartUsage", "name": "engine"},
                {"@id": u(3), "@type": "PartUsage", "name": "transmission"},
                {"@id": u(4), "@type": "PartUsage", "name": "battery"},
                {"@id": u(5), "@type": "RequirementUsage", "name": "range"},
            ])),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/p1/commits/c1/roots"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([u(1)])))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/d2/svg"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(SYSML_D2_FIXTURE))
        .mount(&server)
        .await;
    server
}

async fn post_render(state: AppState, query: &str) -> axum::response::Response {
    test_app(state)
        .oneshot(
            Request::post(format!(
                "/render/sysmlv2/projects/p1/commits/c1?output=svg{query}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn brand_overlay_stamps_identifiers_and_applies_the_package_to_a_sysml_render() {
    let server = brand_fixture_server().await;
    let (state, _dir) = brand_state("brand-ok", &server.uri(), &server.uri());
    let plain = post_render(state.clone(), "").await;
    let (status, plain_body) = body_string(plain).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !plain_body.contains("data-kr0ki-id"),
        "no brand, no overlay"
    );

    let branded = post_render(state, "&brand=test").await;
    assert_eq!(branded.status(), StatusCode::OK);
    assert_eq!(branded.headers()["x-kr0ki-brand"], "test");
    assert_eq!(
        branded.headers()["x-kr0ki-enhance"],
        "indexed=4;unindexed=1"
    ); // the requirement is not drawn
    let (_, body) = body_string(branded).await;
    assert_eq!(body.matches("data-kr0ki-type=\"PartUsage\"").count(), 3);
    assert_eq!(body.matches("<use ").count(), 3);
    assert!(body.contains("id=\"kr0ki-brand\"") && body.contains("data-d2-version"));
}

#[tokio::test]
async fn brand_names_are_validated_and_bad_packages_are_rejected_not_applied() {
    let server = brand_fixture_server().await;
    let (state, _dir) = brand_state("brand-bad", &server.uri(), &server.uri());
    for (query, expect, code) in [
        ("&brand=nope", StatusCode::NOT_FOUND, "unknown_brand"),
        ("&brand=..%2Fetc", StatusCode::BAD_REQUEST, "bad_brand_name"),
        ("&brand=A%20B", StatusCode::BAD_REQUEST, "bad_brand_name"),
        (
            "&brand=garbage",
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_brand",
        ),
        (
            "&brand=broken",
            StatusCode::UNPROCESSABLE_ENTITY,
            "brand_rejected",
        ),
    ] {
        let (status, body) = body_string(post_render(state.clone(), query).await).await;
        assert_eq!(status, expect, "{query}: {body}");
        assert!(body.contains(code), "{query}: {body}");
    }
}

#[tokio::test]
async fn brand_listing_shows_only_valid_installed_packages() {
    let server = brand_fixture_server().await;
    let (state, dir) = brand_state("brand-list", &server.uri(), &server.uri());
    std::fs::create_dir_all(dir.join("Not Valid")).unwrap();
    std::fs::write(dir.join("Not Valid/brand.json"), "{}").unwrap();
    std::fs::create_dir_all(dir.join("empty")).unwrap(); // no brand.json
    let resp = test_app(state)
        .oneshot(Request::get("/brand").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["brands"],
        serde_json::json!(["broken", "garbage", "test"])
    );
}

#[test]
fn the_shipped_example_brand_is_a_valid_package() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../brand/example/brand.json"
    );
    let text = std::fs::read_to_string(path).expect("brand/example/brand.json ships with the repo");
    let brand: kr0ki_svg::enhance::Brand = serde_json::from_str(&text).unwrap();
    let svg = SYSML_D2_FIXTURE;
    kr0ki_svg::enhance::enhance(svg, &[], &brand)
        .expect("the shipped placeholder passes the layer's own validation");
}

// ---- SysML v2 MCP sidecar routes ------------------------------------------------------------------

async fn sysml_sidecar(tool_result: serde_json::Value) -> wiremock::MockServer {
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "S")
                .set_body_json(serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "notifications/initialized"}),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "tools/call"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"jsonrpc": "2.0", "id": 2, "result": tool_result}),
            ),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&s)
        .await;
    s
}

fn state_with_sysml_mcp(tag: &str, url: &str) -> AppState {
    let mut state = test_state(tag);
    state.sysml_mcp = Some(Arc::new(kr0ki_core::sysml_mcp::SysmlMcpClient::new(url)));
    state
}

async fn post_text(app: axum::Router, uri: &str, body: Vec<u8>) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(Request::post(uri).body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn sysml_routes_answer_from_the_mcp_sidecar() {
    let sidecar = sysml_sidecar(serde_json::json!({"structuredContent": {"valid": false, "syntaxErrors": [{"line": 1, "message": "extraneous input"}]}})).await;
    let state = state_with_sysml_mcp("sysml-ok", &sidecar.uri());
    for route in ["validate", "parse", "symbols", "summary"] {
        let (status, body) = post_text(
            test_app(state.clone()),
            &format!("/sysml/{route}"),
            b"package P { part x : ; }".to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{route}: {body}");
        assert_eq!(body["syntaxErrors"][0]["line"], 1);
    }
    let sent = sidecar.received_requests().await.unwrap();
    let names: Vec<String> = sent
        .iter()
        .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
        .filter(|v| v["method"] == "tools/call")
        .map(|v| v["params"]["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        ["validate", "parse", "getSymbols", "getModelSummary"]
    );
}

#[tokio::test]
async fn sysml_routes_say_so_when_unconfigured_and_validate_their_input() {
    let (s, b) = post_text(
        test_app(test_state("sysml-off")),
        "/sysml/validate",
        b"package P {}".to_vec(),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("sysml_mcp_not_configured")
        )
    );
    let sidecar = sysml_sidecar(serde_json::json!({"structuredContent": {}})).await;
    let state = state_with_sysml_mcp("sysml-in", &sidecar.uri());
    let (s, b) = post_text(
        test_app(state.clone()),
        "/sysml/validate",
        vec![0xff, 0xfe, 0x00],
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_utf8"))
    );
    let (s, b) = post_text(
        test_app(state),
        "/sysml/validate",
        vec![b'x'; 256 * 1024 + 1],
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Some("sysml_source_too_large")
        )
    );
    assert!(
        sidecar.received_requests().await.unwrap().is_empty(),
        "refused input must never reach the sidecar"
    );
}

#[tokio::test]
async fn sysml_routes_map_sidecar_failures_to_distinct_statuses() {
    let down = state_with_sysml_mcp("sysml-down", "http://127.0.0.1:1");
    let (s, b) = post_text(test_app(down), "/sysml/validate", b"package P {}".to_vec()).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("sysml_mcp_unavailable"))
    );
    let failing = sysml_sidecar(serde_json::json!({"isError": true, "content": [{"type": "text", "text": "parser crashed"}]})).await;
    let (s, b) = post_text(
        test_app(state_with_sysml_mcp("sysml-err", &failing.uri())),
        "/sysml/validate",
        b"package P {}".to_vec(),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("sysml_mcp_error"))
    );
    assert!(b["message"].as_str().unwrap().contains("parser crashed"));
}

#[tokio::test]
async fn health_reports_the_sysml_sidecar_when_configured_and_omits_it_otherwise() {
    let sidecar = sysml_sidecar(serde_json::json!({})).await;
    let (_, with) = get_json(
        test_app(state_with_sysml_mcp("sysml-health", &sidecar.uri())),
        "/health",
    )
    .await;
    assert_eq!(with["checks"]["sysml_mcp"]["ok"], true);
    let (_, without) = get_json(test_app(test_state("sysml-nohealth")), "/health").await;
    assert!(without["checks"]["sysml_mcp"].is_null());
}

#[tokio::test]
async fn mcp_manifest_advertises_the_sysml_tools_bound_to_the_sysml_routes() {
    let (_, v) = get_json(test_app(test_state("mcp-sysml")), "/mcp/tools").await;
    for (name, path) in [
        ("validate_sysml", "/sysml/validate"),
        ("sysml_symbols", "/sysml/symbols"),
        ("sysml_summary", "/sysml/summary"),
    ] {
        let t = v
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(t["httpBinding"]["pathTemplate"], path);
        assert_eq!(t["httpBinding"]["args"][0]["name"], "code");
        assert_eq!(t["httpBinding"]["args"][0]["placement"], "body");
    }
}

fn sparql_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            {"id": "R1", "satisfiedBy": ["Engine"], "verifiedBy": ["T1"], "attributions": [{"code": "CC-1/WBS-1", "share": 1.0}]},
            {"id": "R2", "attributions": [{"code": "CC-1/WBS-2", "share": 0.7}, {"code": "CC-2", "share": 0.5}]}
        ],
        "edges": [{"from": "R2", "to": "R1", "kind": "derive"}]
    })
}

#[tokio::test]
async fn sparql_routes_answer_over_the_requirements_graph() {
    let state = state_with_sysml_mcp("sparql", "http://127.0.0.1:1");
    let post = |path: &'static str, body: serde_json::Value| {
        let app = test_app(state.clone());
        async move { post_text(app, path, serde_json::to_vec(&body).unwrap()).await }
    };

    let (s, b) = post("/sparql", serde_json::json!({"graph": sparql_graph(), "query": "PREFIX r: <urn:kr0ki:req#> SELECT ?id WHERE { ?q r:derivedFrom+ ?p . ?p r:id 'R1' . ?q r:id ?id }"})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["rows"][0]["id"]["value"], "R2");

    let (s, b) = post(
        "/sparql/shapes",
        serde_json::json!({"graph": sparql_graph()}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let over = b["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["shape"] == "over-allocated")
        .unwrap();
    assert_eq!(over["violations"], serde_json::json!(["R2"]));

    let (s, b) = post(
        "/sparql/rollup",
        serde_json::json!({"graph": sparql_graph(), "prefixDepth": 1}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let cc1 = b["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == "CC-1")
        .unwrap();
    assert!((cc1["total"].as_f64().unwrap() - 1.7).abs() < 1e-9);
}

#[tokio::test]
async fn sparql_refuses_updates_bad_queries_and_malformed_bodies() {
    let state = state_with_sysml_mcp("sparql-bad", "http://127.0.0.1:1");
    for q in [
        "INSERT DATA { <x:a> <x:b> <x:c> }",
        "SELECT nonsense",
        "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
    ] {
        let body =
            serde_json::to_vec(&serde_json::json!({"graph": sparql_graph(), "query": q})).unwrap();
        let (s, b) = post_text(test_app(state.clone()), "/sparql", body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{q}: {b}");
        assert_eq!(b["error"], "invalid_sparql");
    }
    let (s, b) = post_text(test_app(state), "/sparql", b"not json".to_vec()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["error"], "invalid_request");
}

#[tokio::test]
async fn requirements_export_is_the_inverse_of_import() {
    let state = test_state("requirements-export");
    let imported = test_app(state.clone())
        .oneshot(
            Request::post("/requirements/import")
                .body(Body::from(include_str!(
                    "../../kr0ki-core/tests/fixtures/reqif/roundtrip.reqif"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(imported).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    let graph = serde_json::to_vec(&first["documents"][0]["graph"]).unwrap();

    let exported = test_app(state.clone())
        .oneshot(
            Request::post("/requirements/export")
                .body(Body::from(graph))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exported.status(), StatusCode::OK);
    assert!(exported.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/reqif+xml"));
    let (_, xml) = body_string(exported).await;
    assert!(xml.contains("<REQ-IF"), "{xml}");

    let again = test_app(state)
        .oneshot(
            Request::post("/requirements/import")
                .body(Body::from(xml))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(again).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second: serde_json::Value = serde_json::from_str(&body).unwrap();
    let texts = |v: &serde_json::Value| {
        let mut t: Vec<(String, String)> = v["documents"][0]["graph"]["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["id"].to_string(), r["text"].to_string()))
            .collect();
        t.sort();
        t
    };
    assert_eq!(
        texts(&first).len(),
        2,
        "fixture must carry requirements or the round trip proves nothing"
    );
    assert_eq!(texts(&first), texts(&second));
}

#[tokio::test]
async fn requirements_export_rejects_a_body_that_is_not_a_requirement_graph() {
    let (s, b) = post_text(
        test_app(test_state("requirements-export-bad")),
        "/requirements/export",
        b"{\"nodes\": []}".to_vec(),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["error"], "invalid_requirement_graph");
}

#[tokio::test]
async fn requirements_sync_writes_an_imported_baseline_into_a_project() {
    let state = test_state("requirements-sync-import");
    let imported = test_app(state)
        .oneshot(
            Request::post("/requirements/import")
                .body(Body::from(include_str!(
                    "../../kr0ki-core/tests/fixtures/reqif/roundtrip.reqif"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, body) = body_string(imported).await;
    let first: serde_json::Value = serde_json::from_str(&body).unwrap();
    let graph = serde_json::to_string(&first["documents"][0]["graph"]).unwrap();

    let server = wiremock::MockServer::start().await;
    mount_sync_project(&server, serde_json::json!([])).await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/projects/p1/commits"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "previousCommit": {"@id": "c1"},
            "change": [{"payload": {"@type": "RequirementUsage"}}, {"payload": {"@type": "RequirementUsage"}}]
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({"@id": "c2", "@type": "Commit"})))
        .expect(1)
        .mount(&server)
        .await;
    let response = test_app(test_state_with_sysmlv2_client(
        "requirements-sync",
        server.uri(),
    ))
    .oneshot(
        Request::post("/model/projects/p1/requirements")
            .body(Body::from(graph))
            .unwrap(),
    )
    .await
    .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("\"@id\":\"c2\""), "{body}");
}

#[tokio::test]
async fn requirements_sync_rejects_bad_bodies_and_needs_a_configured_server() {
    let server = wiremock::MockServer::start().await;
    let response = test_app(test_state_with_sysmlv2_client(
        "requirements-sync-bad",
        server.uri(),
    ))
    .oneshot(
        Request::post("/model/projects/p1/requirements")
            .body(Body::from("{\"nodes\": 1}"))
            .unwrap(),
    )
    .await
    .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("invalid_requirement_graph"));

    let response = test_app(test_state("requirements-sync-unconfigured"))
        .oneshot(
            Request::post("/model/projects/p1/requirements")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
