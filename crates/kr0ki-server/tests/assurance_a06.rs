//! KR-A06 acceptance case (VC-A06).
//!
//! "The tool gateway shall deny operations outside the caller identity's capability grant."
//!
//! Acceptance: permit model reads; deny a commit from the same read-only identity through MCP
//! and direct HTTP.
//!
//! Both transports are real: plain HTTP with the identity's bearer token, and the actual
//! `bridge.py` MCP bridge started with the *same* token. The refusal comes from the server's
//! gateway on both, because nothing in the bridge or in a tool's description is enforcement.

use axum::http::StatusCode;
use reqwest::Method;

// The server crate is a bin; pull its modules in via path (as `tests/http.rs` does).
#[path = "../src/app.rs"]
mod app;
#[path = "../src/contract.rs"]
mod contract;
#[path = "../src/docs.rs"]
mod docs;
#[path = "common/assurance_harness.rs"]
mod harness;

use harness::*;

#[tokio::test]
async fn a_read_only_identity_reads_but_cannot_commit_over_http() {
    let h = start("a06-http").await;

    // Permitted: a read within its grant.
    let (status, body) = http(
        Method::GET,
        &format!("{}/assurance/requirements/KR-A01", h.base),
        READER,
        None,
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["requirement"]["id"], "KR-A01");
    assert!(body["revisions"]["model_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    // Denied: the same identity trying to commit.
    let commit = format!(
        "{}/assurance/changes/chg-any/commit?expected_revision=r0",
        h.base
    );
    let (status, body) = http(Method::POST, &commit, READER, None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN.as_u16(), "{body}");
    assert_eq!(body["error"], "forbidden");
    assert!(
        body["message"].as_str().unwrap().contains("model.commit"),
        "{body}"
    );

    // The grant is what decided it: an identity that holds `model.commit` reaches the handler
    // (which then says the change is unknown: a 404, not a 403).
    let (status, body) = http(Method::POST, &commit, COMMITTER, None, None).await;
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["error"], "unknown_change");
}

#[tokio::test]
async fn the_same_identity_is_denied_through_the_mcp_bridge_too() {
    let h = start("a06-mcp").await;
    let base = h.base.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut bridge = Bridge::start(&base, READER);
        bridge.initialize();
        // The commit tool is *listed*: visibility is not what protects it.
        assert!(bridge.tool_names().contains(&"commit_change".to_string()));

        // Permitted read.
        let (is_error, text) = bridge.call("get_requirement", serde_json::json!({"id": "KR-A01"}));
        assert!(!is_error, "{text}");
        assert!(text.contains("\"KR-A01\""));

        // Denied commit with the same token.
        bridge.call(
            "commit_change",
            serde_json::json!({"change_id": "chg-any", "expected_revision": "r0"}),
        )
    })
    .await
    .unwrap();
    let (is_error, text) = result;
    assert!(is_error, "the commit must be refused: {text}");
    assert!(text.contains("403"), "{text}");
    assert!(text.contains("forbidden"), "{text}");
    drop(h);
}

#[tokio::test]
async fn an_unknown_token_is_unauthenticated_not_merely_unauthorised() {
    let h = start("a06-unauth").await;
    let (status, body) = http(
        Method::GET,
        &format!("{}/assurance/requirements", h.base),
        "tok-nobody",
        None,
        None,
    )
    .await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(body["error"], "unauthorized");
}

#[tokio::test]
async fn an_unclassified_route_requires_admin_so_a_new_route_is_never_open_by_accident() {
    let h = start("a06-default").await;
    // `/nonexistent` matches no route, so it is classified `admin`: a non-admin is refused
    // before the router can say 404.
    let (status, _) = http(
        Method::GET,
        &format!("{}/nonexistent", h.base),
        READER,
        None,
        None,
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn health_needs_no_token_and_is_not_audited() {
    let h = start("a06-health").await;
    let resp = reqwest::get(format!("{}/health", h.base)).await.unwrap();
    assert!(resp.status().as_u16() < 500);
    assert!(
        h.audit.read_all().unwrap().is_empty(),
        "a readiness probe is not a tool invocation"
    );
}

#[tokio::test]
async fn a_token_is_never_logged_and_the_audit_chain_verifies() {
    let h = start("a06-secret").await;
    http(
        Method::GET,
        &format!("{}/assurance/requirements", h.base),
        READER,
        None,
        None,
    )
    .await;
    http(
        Method::POST,
        &format!("{}/assurance/changes/x/commit?expected_revision=r", h.base),
        READER,
        None,
        None,
    )
    .await;
    let raw = std::fs::read_to_string(h.audit.path()).unwrap();
    for token in [READER, COMMITTER, AUDITOR] {
        assert!(
            !raw.contains(token),
            "a bearer token leaked into the audit log"
        );
    }
    assert!(h.audit.verify().unwrap().is_ok());
}

#[tokio::test]
async fn resource_reads_pass_through_the_same_gateway_as_the_equivalent_http_request() {
    let h = start("a06-resources").await;
    let base = h.base.clone();
    let (reader, auditor, prompts) = tokio::task::spawn_blocking(move || {
        let mut as_reader = Bridge::start(&base, READER);
        as_reader.initialize();
        let reader = as_reader.request(
            "resources/read",
            serde_json::json!({"uri": "kr0ki://requirement/KR-A01"}),
        );
        let skill = as_reader.request(
            "resources/read",
            serde_json::json!({"uri": "kr0ki://skill/requirements-authoring"}),
        );
        let prompts = as_reader.request(
            "prompts/get",
            serde_json::json!({"name": "explain_gap", "arguments": {"id": "KR-A06"}}),
        );
        // The auditor has `audit.read` only: no `assurance.read`.
        let mut as_auditor = Bridge::start(&base, AUDITOR);
        as_auditor.initialize();
        let auditor = as_auditor.request(
            "resources/read",
            serde_json::json!({"uri": "kr0ki://requirement/KR-A01"}),
        );
        (reader, auditor, (skill, prompts))
    })
    .await
    .unwrap();

    let text = reader["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("\"KR-A01\""), "{reader}");
    assert!(
        auditor["error"]["message"]
            .as_str()
            .unwrap()
            .contains("403"),
        "{auditor}"
    );

    let (skill, prompt) = prompts;
    assert!(skill["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("name: requirements-authoring"));
    let p = prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(p.contains("trace_requirement with id=KR-A06"), "{prompt}");
}
