//! Wiremock-backed tests for `SysmlV2Client`. No real network.

use kr0ki_sysmlv2_client::{
    CommitRequest, DataVersion, Direction, Page, PageParamStyle, Ref, SysmlV2Client,
};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn projects_parse() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "p1", "@type": "Project", "name": "Alpha", "description": "first" },
            { "@id": "p2", "@type": "Project", "name": "Beta" }
        ])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    let projects = client.projects().await.unwrap();

    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].at_id, "p1");
    assert_eq!(projects[0].name.as_deref(), Some("Alpha"));
    assert_eq!(projects[0].description.as_deref(), Some("first"));
    assert_eq!(projects[1].name.as_deref(), Some("Beta"));
    // unknown fields (here `@type`) are tolerated and captured
    assert_eq!(
        projects[0].extra.get("@type").and_then(|v| v.as_str()),
        Some("Project")
    );
}

#[tokio::test]
async fn trailing_slash_on_base_url_is_trimmed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(format!("{}/", server.uri()));
    assert!(!client.base_url().ends_with('/'));
    assert!(client.projects().await.unwrap().is_empty());
}

#[tokio::test]
async fn all_elements_concatenates_two_pages_and_snapshot_populates() {
    let server = MockServer::start().await;
    let base = server.uri();

    // Page 1: no `page-after`; server returns a Link: rel="next" pointing at page 2.
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .and(query_param_is_missing("page-after"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    format!(
                        "<{base}/projects/p1/commits/c1/elements?page-after=cursorA&page-size=2>; rel=\"next\""
                    )
                    .as_str(),
                )
                .set_body_json(json!([
                    { "@id": "e1", "@type": "PartUsage", "name": "pump" },
                    { "@id": "e2", "@type": "PartUsage", "name": "valve" }
                ])),
        )
        .mount(&server)
        .await;

    // Page 2: `page-after=cursorA`; no Link header, short page => exhausted.
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .and(query_param("page-after", "cursorA"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "e3", "@type": "PartDefinition", "name": "Assembly" }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/roots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(["e3"])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(&base);

    let all = client.all_elements("p1", "c1").await.unwrap();
    assert_eq!(
        all.iter().map(|e| e.id().to_string()).collect::<Vec<_>>(),
        ["e1", "e2", "e3"]
    );

    let snap = client.snapshot("p1", "c1").await.unwrap();
    assert_eq!(snap.project_id, "p1");
    assert_eq!(snap.commit_id, "c1");
    assert_eq!(snap.elements.len(), 3);
    assert_eq!(snap.roots, ["e3"]);
    assert_eq!(snap.content_hash.len(), 64);
    assert!(snap.content_hash.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[tokio::test]
async fn roots_tolerates_object_form() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/roots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "r1", "@type": "Package" },
            { "@id": "r2", "@type": "Package" }
        ])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    assert_eq!(client.roots("p1", "c1").await.unwrap(), ["r1", "r2"]);
}

#[tokio::test]
async fn json_api_bracket_style_sends_bracket_form_params_and_follows_link() {
    let server = MockServer::start().await;
    let base = server.uri();

    // Configured for bracket-form params: the request itself must carry
    // `page[size]`, not `page-size`, and the Link header's `page[after]` cursor
    // must be the one that gets picked up.
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .and(query_param("page[size]", "2"))
        .and(query_param_is_missing("page-size"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    format!(
                        "<{base}/projects/p1/commits/c1/elements?page[after]=cursorA&page[size]=2>; rel=\"next\""
                    )
                    .as_str(),
                )
                .set_body_json(json!([{ "@id": "e1", "@type": "PartUsage" }])),
        )
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(&base).with_page_param_style(PageParamStyle::JsonApiBracket);
    let page = client
        .elements(
            "p1",
            "c1",
            Page {
                after: None,
                before: None,
                size: Some(2),
            },
        )
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next_after.as_deref(), Some("cursorA"));
}

#[tokio::test]
async fn bearer_token_header_is_sent_when_configured() {
    let server = MockServer::start().await;
    // This mock only matches when the Authorization header is exactly right.
    Mock::given(method("GET"))
        .and(path("/projects"))
        .and(header("authorization", "Bearer testtok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "p1", "@type": "Project" }
        ])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri()).with_token("testtok");
    let projects = client.projects().await.unwrap();
    assert_eq!(projects.len(), 1);
}

#[tokio::test]
async fn missing_token_yields_no_auth_header_and_401_maps_to_status_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    match client.projects().await {
        Err(kr0ki_sysmlv2_client::ClientError::Status { code, body }) => {
            assert_eq!(code, 401);
            assert!(body.contains("unauthorized"));
        }
        other => panic!("expected Status error, got {other:?}"),
    }
}

#[tokio::test]
async fn relationships_sets_direction_query_param() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements/e1/relationships"))
        .and(query_param("direction", "out"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "rel1", "@type": "FeatureMembership" }
        ])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    let rels = client
        .relationships("p1", "c1", "e1", Direction::Out)
        .await
        .unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].ty(), "FeatureMembership");
}

#[tokio::test]
async fn branches_tags_commits_parse() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/branches"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "b1", "@type": "Branch", "name": "main" }
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "t1", "@type": "Tag", "name": "v1.0" }
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "@id": "c1", "@type": "Commit", "created": "2026-09-05T00:00:00Z",
              "owningProject": { "@id": "p1", "@type": "Project" } }
        ])))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    assert_eq!(
        client.branches("p1").await.unwrap()[0].name.as_deref(),
        Some("main")
    );
    assert_eq!(
        client.tags("p1").await.unwrap()[0].name.as_deref(),
        Some("v1.0")
    );
    let commits = client.commits("p1").await.unwrap();
    assert_eq!(commits[0].at_id, "c1");
    assert_eq!(
        commits[0].owning_project.as_ref().map(|r| r.at_id.as_str()),
        Some("p1")
    );
}

// --- determinism of ModelSnapshot.content_hash -------------------------------

async fn hash_for(elements_body: serde_json::Value, roots_body: serde_json::Value) -> String {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .respond_with(ResponseTemplate::new(200).set_body_json(elements_body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/roots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(roots_body))
        .mount(&server)
        .await;
    SysmlV2Client::new(server.uri())
        .snapshot("p1", "c1")
        .await
        .unwrap()
        .content_hash
}

#[tokio::test]
async fn content_hash_is_deterministic_regardless_of_element_order() {
    let order_a = json!([
        { "@id": "e1", "@type": "PartUsage", "name": "pump" },
        { "@id": "e2", "@type": "PartUsage", "name": "valve" },
        { "@id": "e3", "@type": "PartDefinition", "name": "Assembly" }
    ]);
    let order_b = json!([
        { "@id": "e3", "@type": "PartDefinition", "name": "Assembly" },
        { "@id": "e1", "@type": "PartUsage", "name": "pump" },
        { "@id": "e2", "@type": "PartUsage", "name": "valve" }
    ]);
    let roots = json!(["e3"]);

    let h_a = hash_for(order_a, roots.clone()).await;
    let h_b = hash_for(order_b, roots).await;

    assert_eq!(h_a.len(), 64);
    assert_eq!(
        h_a, h_b,
        "content_hash must be byte-identical across server orderings"
    );
}

#[tokio::test]
async fn content_hash_changes_when_a_field_value_changes() {
    let base = json!([
        { "@id": "e1", "@type": "PartUsage", "name": "pump" },
        { "@id": "e2", "@type": "PartUsage", "name": "valve" }
    ]);
    let mutated = json!([
        { "@id": "e1", "@type": "PartUsage", "name": "PUMP-X" },
        { "@id": "e2", "@type": "PartUsage", "name": "valve" }
    ]);
    let roots = json!(["e1"]);

    let h_base = hash_for(base, roots.clone()).await;
    let h_mut = hash_for(mutated, roots).await;

    assert_ne!(
        h_base, h_mut,
        "a changed element field must change content_hash"
    );
}

#[tokio::test]
async fn create_commit_posts_change_set_and_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .and(query_param("branchId", "main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "@id": "c2",
            "@type": "Commit",
            "owningProject": {"@id": "p1"}
        })))
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    let request = CommitRequest {
        type_: "Commit",
        change: vec![DataVersion {
            type_: "DataVersion",
            payload: Some(
                json!({"@type": "PartUsage", "name": "customers (marts)", "identifier": "dbt:model.jaffle_shop.customers"}),
            ),
            identity: None,
        }],
        previous_commit: Some(Ref {
            at_id: "c1".to_string(),
            extra: Default::default(),
        }),
    };

    let commit = client
        .create_commit("p1", Some("main"), request)
        .await
        .unwrap();

    assert_eq!(commit.at_id, "c2");
}

#[tokio::test]
async fn create_commit_omits_branch_id_query_param_when_none() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .and(query_param_is_missing("branchId"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"@id": "c1", "@type": "Commit"})),
        )
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    let request = CommitRequest {
        type_: "Commit",
        change: vec![],
        previous_commit: None,
    };

    client.create_commit("p1", None, request).await.unwrap();
}
