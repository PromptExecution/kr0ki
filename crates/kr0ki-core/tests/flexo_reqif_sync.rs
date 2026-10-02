//! Orchestration tests for `flexo_reqif_sync` against a stub SysML v2 server, plus one
//! `#[ignore]`d live round trip (design doc §3: commit <-> graph <-> export equivalence).
//! The element mapping itself is unit-tested in `src/flexo_reqif_sync.rs`.

use kr0ki_core::digital_thread_sync::SyncConfig;
use kr0ki_core::flexo_reqif_sync::{
    baseline_prefix, fetch_requirement_baseline, requirement_identifier, sync_requirement_baseline,
};
use kr0ki_core::requirements::{BaselineIdentity, RelationAuthority, RequirementGraph};
use kr0ki_sysmlv2_client::SysmlV2Client;
use serde_json::{json, Value};
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE: &[u8] = include_bytes!("fixtures/reqif/roundtrip.reqif");

fn fixture_graph() -> RequirementGraph {
    parse_and_lower(
        FIXTURE,
        &ReqIfAdapterConfig {
            source_uri: "test:roundtrip".into(),
            revision: "r1".into(),
            import_artifact_sha256: Some("abc123".into()),
        },
    )
    .expect("fixture lowers")
}

fn config() -> SyncConfig {
    SyncConfig {
        project_id: "p1".into(),
        branch_id: None,
    }
}

async fn get_json(server: &MockServer, p: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn accept_posts(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"@id": "new"})))
        .mount(server)
        .await;
}

async fn posted_changes(server: &MockServer) -> Vec<Vec<Value>> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .map(|r| {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            body["change"].as_array().cloned().unwrap_or_default()
        })
        .collect()
}

/// What the server would return for `payloads` after a commit created them.
fn as_stored(payloads: &[Value]) -> Value {
    Value::Array(
        payloads
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut e = p["payload"].clone();
                e["@id"] = json!(format!("srv-{i}"));
                e
            })
            .collect(),
    )
}

#[tokio::test]
async fn first_sync_creates_one_requirement_usage_per_requirement_with_no_previous_commit() {
    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([])).await;
    accept_posts(&server).await;

    let graph = fixture_graph();
    let commit = sync_requirement_baseline(&SysmlV2Client::new(server.uri()), &graph, &config())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(commit.at_id, "new");

    let body: Value = serde_json::from_slice(
        &server
            .received_requests()
            .await
            .unwrap()
            .last()
            .unwrap()
            .body,
    )
    .unwrap();
    assert!(
        body.get("previousCommit").is_none(),
        "empty project has no previous commit"
    );
    let changes = body["change"].as_array().unwrap();
    assert_eq!(changes.len(), graph.requirements.len());
    for (change, req) in changes.iter().zip(&graph.requirements) {
        assert_eq!(change["payload"]["@type"], "RequirementUsage");
        assert_eq!(
            change["payload"]["identifier"],
            requirement_identifier(&graph.baseline.id, &req.id)
        );
        assert_eq!(change["payload"]["reqif_baseline_id"], graph.baseline.id);
        assert_eq!(change["payload"]["reqif_source_sha256"], "abc123");
    }
}

#[tokio::test]
async fn resyncing_what_was_stored_posts_nothing_and_fetch_reads_it_back() {
    // Round 1: capture what a first sync writes.
    let first = MockServer::start().await;
    get_json(&first, "/projects/p1/commits", json!([])).await;
    accept_posts(&first).await;
    let graph = fixture_graph();
    sync_requirement_baseline(&SysmlV2Client::new(first.uri()), &graph, &config())
        .await
        .unwrap();
    let written = posted_changes(&first).await.remove(0);

    // Round 2: a server holding exactly that. No POST mock: any write fails the test.
    let second = MockServer::start().await;
    get_json(&second, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(
        &second,
        "/projects/p1/commits/c1/elements",
        as_stored(&written),
    )
    .await;
    let client = SysmlV2Client::new(second.uri());

    let head = sync_requirement_baseline(&client, &graph, &config())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        head.at_id, "c1",
        "unchanged baseline returns the existing head"
    );
    assert!(posted_changes(&second).await.is_empty());

    // Fetch reconstructs the same requirements and asserted relations.
    let back = fetch_requirement_baseline(&client, &config(), &graph.baseline.id)
        .await
        .unwrap()
        .unwrap();
    let mut expected = graph.requirements.clone();
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(back.requirements, expected);
    assert_eq!(
        back.baseline.revision, graph.baseline.revision,
        "the import's own revision survives, not the commit it was read at"
    );
    let asserted = |g: &RequirementGraph| {
        let mut v: Vec<_> = g
            .relations
            .iter()
            .filter(|r| r.authority == RelationAuthority::Asserted)
            .map(|r| (r.id.clone(), r.source.clone(), r.target.clone()))
            .collect();
        v.sort();
        v
    };
    assert_eq!(asserted(&back), asserted(&graph));
}

#[tokio::test]
async fn update_keeps_other_tools_fields_and_never_touches_other_baselines() {
    let graph = fixture_graph();
    let baseline_id = &graph.baseline.id;
    let mut changed = graph.clone();
    changed.requirements[0].title = "A retitled requirement".into();
    let retitled_identifier = requirement_identifier(baseline_id, &changed.requirements[0].id);

    // Server state = the original baseline, but another tool added `owner` to the first
    // requirement; plus a stale requirement no longer in the graph; plus another baseline.
    let seed = MockServer::start().await;
    get_json(&seed, "/projects/p1/commits", json!([])).await;
    accept_posts(&seed).await;
    sync_requirement_baseline(&SysmlV2Client::new(seed.uri()), &graph, &config())
        .await
        .unwrap();
    let mut stored = as_stored(&posted_changes(&seed).await.remove(0));
    stored[0]["owner"] = json!("team-x");
    let stale = format!("{}STALE", baseline_prefix(baseline_id));
    stored.as_array_mut().unwrap().push(json!({
        "@id": "srv-stale", "@type": "RequirementUsage", "identifier": stale, "name": "old"
    }));
    stored.as_array_mut().unwrap().push(json!({
        "@id": "srv-other", "@type": "RequirementUsage",
        "identifier": "reqif:OTHER:R1", "name": "another baseline"
    }));

    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(&server, "/projects/p1/commits/c1/elements", stored).await;
    accept_posts(&server).await;
    sync_requirement_baseline(&SysmlV2Client::new(server.uri()), &changed, &config())
        .await
        .unwrap();

    let changes = posted_changes(&server).await.remove(0);
    assert_eq!(
        changes.len(),
        2,
        "one update + one delete, nothing else: {changes:#?}"
    );

    let update = changes.iter().find(|c| !c["payload"].is_null()).unwrap();
    assert_eq!(update["payload"]["identifier"], retitled_identifier);
    assert_eq!(update["payload"]["name"], "A retitled requirement");
    assert_eq!(
        update["payload"]["owner"], "team-x",
        "another tool's field must survive"
    );
    assert_eq!(update["identity"]["@id"], "srv-0");

    let delete = changes.iter().find(|c| c["payload"].is_null()).unwrap();
    assert_eq!(delete["identity"]["@id"], "srv-stale");
    assert!(
        changes.iter().all(|c| c["identity"]["@id"] != "srv-other"),
        "another baseline's requirement must never be touched"
    );
}

/// Live round trip: import -> commit -> fetch -> equal. Not run in CI (needs an OMG-API
/// server). Uses a throwaway baseline id so it never collides with real data, and leaves
/// its requirements in the project (sync an empty graph for the same id to remove them).
#[tokio::test]
#[ignore = "requires KR0KI_SYSMLV2_BASE_URL and KR0KI_SYSMLV2_TEST_PROJECT_ID (+ optional KR0KI_SYSMLV2_TOKEN)"]
async fn live_commit_graph_roundtrip() {
    let base = std::env::var("KR0KI_SYSMLV2_BASE_URL").expect("KR0KI_SYSMLV2_BASE_URL");
    let project_id =
        std::env::var("KR0KI_SYSMLV2_TEST_PROJECT_ID").expect("KR0KI_SYSMLV2_TEST_PROJECT_ID");
    let mut client = SysmlV2Client::new(base);
    if let Ok(token) = std::env::var("KR0KI_SYSMLV2_TOKEN") {
        client = client.with_token(token);
    }
    let config = SyncConfig {
        project_id,
        branch_id: None,
    };

    let mut graph = fixture_graph();
    graph.baseline = BaselineIdentity {
        id: format!("kr0ki-live-{}", std::process::id()),
        ..graph.baseline
    };

    let first = sync_requirement_baseline(&client, &graph, &config)
        .await
        .unwrap()
        .unwrap();
    let again = sync_requirement_baseline(&client, &graph, &config)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first.at_id, again.at_id,
        "second sync of identical data must not commit"
    );

    // What every server must round-trip: id (as the alias marker), name and text.
    let mut expected = graph.requirements.clone();
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    let ids = |v: &[kr0ki_core::requirements::Requirement]| {
        v.iter()
            .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
            .collect::<Vec<_>>()
    };
    match fetch_requirement_baseline(&client, &config, &graph.baseline.id).await {
        Ok(back) => assert_eq!(ids(&back.unwrap().requirements), ids(&expected)),
        // The OMG pilot server is strictly typed and drops extension fields: full graph read-back is then refused
        // loudly (never invented). That is the documented, expected outcome there.
        Err(e) => assert!(
            e.to_string().contains("does not persist"),
            "unexpected fetch failure: {e}"
        ),
    }
    let stored = client
        .all_elements(&config.project_id, &again.at_id)
        .await
        .unwrap();
    let prefix = format!("reqif:{}:", graph.baseline.id);
    let mut got: Vec<(String, String)> = stored
        .iter()
        .filter(|e| {
            e.fields
                .get("aliasIds")
                .and_then(|a| a.as_array())
                .is_some_and(|a| {
                    a.iter()
                        .any(|x| x.as_str().is_some_and(|x| x.starts_with(&prefix)))
                })
        })
        .map(|e| {
            (
                e.fields["name"].as_str().unwrap().to_string(),
                e.fields["text"][0].as_str().unwrap().to_string(),
            )
        })
        .collect();
    got.sort();
    let mut want: Vec<(String, String)> = expected
        .iter()
        .map(|r| (r.title.clone(), r.text.clone()))
        .collect();
    want.sort();
    assert_eq!(got, want);
}
