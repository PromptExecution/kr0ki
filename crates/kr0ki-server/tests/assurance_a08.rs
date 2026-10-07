//! KR-A08 acceptance case (VC-A08).
//!
//! "The audit service shall record each tool invocation with caller, operation, decision, model
//! revision and correlation identifier."
//!
//! Acceptance: retrieve records for both the permitted and denied KR-A06 calls.

use reqwest::Method;

#[path = "../src/app.rs"]
mod app;
#[path = "../src/contract.rs"]
mod contract;
#[path = "../src/docs.rs"]
mod docs;
#[path = "common/assurance_harness.rs"]
mod harness;

use harness::*;

fn records(body: &serde_json::Value) -> Vec<serde_json::Value> {
    body["records"].as_array().unwrap().clone()
}

#[tokio::test]
async fn records_exist_for_the_permitted_and_the_denied_calls_over_both_transports() {
    let h = start("a08").await;
    let base = h.base.clone();

    // Direct HTTP, with caller-supplied correlation ids.
    let read = format!("{base}/assurance/requirements/KR-A01");
    let commit = format!("{base}/assurance/changes/chg-any/commit?expected_revision=r0");
    let (s, _) = http(Method::GET, &read, READER, Some("corr-http-read"), None).await;
    assert_eq!(s, 200);
    let (s, _) = http(
        Method::POST,
        &commit,
        READER,
        Some("corr-http-commit"),
        None,
    )
    .await;
    assert_eq!(s, 403);

    // The MCP bridge, same identity.
    let b = base.clone();
    tokio::task::spawn_blocking(move || {
        let mut bridge = Bridge::start(&b, READER);
        bridge.initialize();
        bridge.call("get_requirement", serde_json::json!({"id": "KR-A01"}));
        bridge.call(
            "commit_change",
            serde_json::json!({"change_id": "chg-any", "expected_revision": "r0"}),
        );
    })
    .await
    .unwrap();

    // Retrieve the records (an identity with `audit.read`).
    let (s, body) = http(
        Method::GET,
        &format!("{base}/assurance/audit?caller=reader&phase=decision"),
        AUDITOR,
        None,
        None,
    )
    .await;
    assert_eq!(s, 200, "{body}");
    assert_eq!(body["chain"]["ok"], true, "{}", body["chain"]);
    let recs = records(&body);

    let find = |op: &str, decision: &str, transport: &str| {
        recs.iter()
            .find(|r| {
                r["operation"] == op && r["decision"] == decision && r["transport"] == transport
            })
            .unwrap_or_else(|| panic!("no {decision} record of {op} over {transport}: {recs:#?}"))
            .clone()
    };
    let permitted_http = find("get_requirement", "permit", "http");
    let denied_http = find("commit_change", "deny", "http");
    let permitted_mcp = find("get_requirement", "permit", "mcp");
    let denied_mcp = find("commit_change", "deny", "mcp");

    for r in [&permitted_http, &denied_http, &permitted_mcp, &denied_mcp] {
        // caller, operation, decision, model revision, correlation id: all five, every time.
        assert_eq!(r["caller"], "reader");
        assert!(r["operation"].as_str().unwrap().len() > 3);
        assert!(["permit", "deny"].contains(&r["decision"].as_str().unwrap()));
        assert!(
            !r["model_revision"].as_str().unwrap_or("").is_empty(),
            "{r}"
        );
        assert!(
            !r["correlation_id"].as_str().unwrap_or("").is_empty(),
            "{r}"
        );
        assert!(r["at"].as_str().unwrap().ends_with('Z'));
    }
    // The caller-supplied correlation ids are carried through.
    assert_eq!(permitted_http["correlation_id"], "corr-http-read");
    assert_eq!(denied_http["correlation_id"], "corr-http-commit");
    // A read is recorded at the service's model revision; a commit at the revision it expected.
    assert!(permitted_http["model_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(denied_http["model_revision"], "r0");
    // A denial says why, and names no token.
    assert!(denied_http["reason"]
        .as_str()
        .unwrap()
        .contains("model.commit"));
    assert!(permitted_http.get("reason").is_none());
}

#[tokio::test]
async fn each_call_has_a_decision_record_before_and_an_outcome_record_after_under_one_correlation_id(
) {
    let h = start("a08-phases").await;
    let read = format!("{}/assurance/requirements/KR-A01", h.base);
    let commit = format!(
        "{}/assurance/changes/chg-any/commit?expected_revision=r0",
        h.base
    );
    http(Method::GET, &read, READER, Some("corr-read"), None).await;
    http(Method::POST, &commit, READER, Some("corr-commit"), None).await;

    let all = h.audit.read_all().unwrap();
    let for_corr = |c: &str| {
        all.iter()
            .filter(|r| r.correlation_id == c)
            .collect::<Vec<_>>()
    };
    for (corr, status) in [("corr-read", 200u16), ("corr-commit", 403u16)] {
        let r = for_corr(corr);
        assert_eq!(r.len(), 2, "{corr}: {r:#?}");
        assert_eq!(
            format!("{:?}", r[0].phase),
            "Decision",
            "the decision is written first"
        );
        assert_eq!(format!("{:?}", r[1].phase), "Outcome");
        assert_eq!(r[1].status, Some(status));
        assert!(r[0].seq < r[1].seq);
    }
}

#[tokio::test]
async fn reading_the_audit_log_needs_the_audit_grant_and_the_attempt_is_itself_recorded() {
    let h = start("a08-grant").await;
    let url = format!("{}/assurance/audit", h.base);
    let (s, _) = http(Method::GET, &url, READER, Some("corr-snoop"), None).await;
    assert_eq!(s, 403, "a read-only identity must not read the audit log");
    let (s, body) = http(
        Method::GET,
        &format!("{url}?correlation_id=corr-snoop"),
        AUDITOR,
        None,
        None,
    )
    .await;
    assert_eq!(s, 200);
    let recs = records(&body);
    assert!(
        recs.iter().any(|r| r["operation"] == "get_audit_records"
            && r["decision"] == "deny"
            && r["caller"] == "reader"),
        "{recs:#?}"
    );
}

#[tokio::test]
async fn a_failing_audit_log_fails_closed() {
    let h = start("a08-closed").await;
    // Make the log unwritable: replace the file with a directory.
    let path = h.audit.path().to_path_buf();
    // A first call succeeds and creates the file.
    let (s, _) = http(
        Method::GET,
        &format!("{}/assurance/requirements", h.base),
        READER,
        None,
        None,
    )
    .await;
    assert_eq!(s, 200);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let (s, body) = http(
        Method::GET,
        &format!("{}/assurance/requirements", h.base),
        READER,
        None,
        None,
    )
    .await;
    assert_eq!(
        s, 503,
        "an operation that cannot be recorded must not run: {body}"
    );
    assert_eq!(body["error"], "audit_unavailable");
}
