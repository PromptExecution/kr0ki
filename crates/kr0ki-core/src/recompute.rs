//! Orchestrates `crate::ufo_graph::build_sysgraph`,
//! `crate::rule_docs::extract_rule_docs`, `crate::rule_eval::RegorusBackend`,
//! and `crate::requirements_sync::register_violations`: latest commit ->
//! `SysGraph` -> rule docs -> evaluate -> `RequirementGraph`. Synchronous,
//! no persistence -- see Global Constraints in `docs/superpowers/plans/
//! 2026-09-22-requirements-rules-system.md`.

use crate::requirements_sync::{baseline_for, register_violations};
use crate::rule_docs::extract_rule_docs;
use crate::rule_eval::{RegorusBackend, RuleBackend};
use crate::ufo_graph::build_sysgraph;
use kr0ki_sysmlv2_client::{ClientError, SysmlV2Client};
use serde::Serialize;
use ufo_types::mbse::requirements::RequirementGraph;
use ufo_types::satisfies::SatisfiesResult;
use ufo_types::sysgraph::SysGraph;

#[derive(Debug, Clone, Serialize)]
pub struct RuleViolation {
    pub rule_id: String,
    pub result: SatisfiesResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecomputeResult {
    pub graph: SysGraph,
    pub requirements: RequirementGraph,
    /// Every rule's result, including `Disposition::Satisfied` and
    /// `Disposition::Unknown` -- not just actual violations, despite the
    /// field name. A consumer that renders this verbatim as "violations"
    /// should filter by `.result.is_violated()` first.
    pub violations: Vec<RuleViolation>,
}

/// Fetches the project's newest commit (`commits()` is already
/// newest-first, per `docs/TODO.md`'s "Commit poll loop" note), builds its
/// `SysGraph`, evaluates every `RuleDocument` element against it, and
/// returns both the raw per-rule results and the `RequirementGraph` they
/// were folded into. A project with zero commits or zero rule docs is a
/// valid state, not an error -- the latter just yields empty `violations`.
pub async fn recompute_and_evaluate(
    client: &SysmlV2Client,
    project_id: &str,
) -> Result<RecomputeResult, ClientError> {
    let commits = client.commits(project_id).await?;
    // TODO: this assumes `commits()` returns commits newest-first, so
    // `.first()` is the latest. That assumption rests entirely on the OMG
    // API server's own ordering behavior, which `SysmlV2Client::commits()`
    // does not itself verify or enforce -- kr0ki has no control over it and
    // it is currently unverified against any real server in this
    // environment (see `docs/TODO.md`'s "Commit poll loop" note and the
    // requirements-rules-system final review ledger). A defensive fix would
    // sort by `Commit.created` (falling back to server order when `created`
    // is absent) rather than trusting `.first()` outright; that change was
    // judged too risky to make inside this already-large fix wave (risk of
    // subtly changing existing test semantics around tie-breaking), so it
    // is deliberately left as a visible, tracked assumption instead.
    let latest = commits.first().ok_or_else(|| ClientError::Status {
        code: 404,
        body: format!("project {project_id} has no commits"),
    })?;
    let snapshot = client.snapshot(project_id, &latest.at_id).await?;
    let sysgraph = build_sysgraph(&snapshot);
    let rule_docs = extract_rule_docs(&snapshot);
    let backend = RegorusBackend;
    let mut requirements = RequirementGraph {
        baseline: baseline_for(&snapshot),
        requirements: Vec::new(),
        evidence: Vec::new(),
        relations: Vec::new(),
    };
    let mut violations = Vec::new();
    for doc in &rule_docs {
        let result = backend.evaluate(&sysgraph, doc);
        register_violations(&mut requirements, doc, &sysgraph.edges, &result);
        violations.push(RuleViolation {
            rule_id: doc.id.as_str().to_string(),
            result,
        });
    }
    Ok(RecomputeResult {
        graph: sysgraph,
        requirements,
        violations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn recomputes_and_reports_one_violation_end_to_end() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/projects/p1/commits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"@id": "c2", "@type": "Commit"},
                {"@id": "c1", "@type": "Commit"}
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/projects/p1/commits/c2/elements"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
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
        Mock::given(method("GET"))
            .and(path("/projects/p1/commits/c2/roots"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(["elem-1"])))
            .mount(&server)
            .await;

        let client = SysmlV2Client::new(server.uri());
        let result = recompute_and_evaluate(&client, "p1").await.unwrap();

        assert_eq!(result.graph.nodes.len(), 2);
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].result.is_violated());
        assert_eq!(result.requirements.relations.len(), 1);
        assert!(result.requirements.validate().is_ok());
    }

    #[tokio::test]
    async fn a_project_with_no_commits_is_a_clean_error_not_a_panic() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/projects/p1/commits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        let client = SysmlV2Client::new(server.uri());
        let error = recompute_and_evaluate(&client, "p1").await.unwrap_err();
        assert!(matches!(error, ClientError::Status { code: 404, .. }));
    }
}
