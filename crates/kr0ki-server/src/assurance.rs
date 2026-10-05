//! The `/assurance/*` routes (and the `/mcp/resources`, `/mcp/prompts` manifests): the HTTP face
//! of `kr0ki_core::assurance_service::AssuranceService`, and what the assurance MCP tools call.
//!
//! These handlers do **no authorisation of their own**. Every route here is classified in
//! `gateway::ROUTES`, and the gateway middleware has already authenticated the caller, checked
//! its grant and written the audit record before a handler runs. Keeping that in one place is
//! the point: a handler that checked for itself could drift from the table.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use kr0ki_core::assurance_service::{AssuranceService, RequirementFilter, ServiceError};
use kr0ki_core::assurance_view::{to_d2, to_table};
use kr0ki_core::audit_log::{AuditLog, AuditQuery, Decision, Phase};
use kr0ki_core::cache::OutputKind;
use kr0ki_core::change_service::{
    ChangeError, ChangeService, Committed, ModelBackend, ProposedChange, Validation,
};
use kr0ki_core::format::DiagramFormat;
use kr0ki_core::mcp_surface;
use kr0ki_core::sync_engine::DesiredElement;
use kr0ki_core::verification_runner::cases_from_graph;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use ufo_types::mbse::assurance::{statement_issues, RequirementStatus, SourceKind};

use crate::app::{error_json, AppState};

const SKILL_REQUIREMENTS_AUTHORING: &str =
    include_str!("../../../skills/requirements-authoring/SKILL.md");

/// Boxed future, so [`ChangeApi`] is object-safe.
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// `ChangeService<B>` behind a trait object, so `AppState` holds one concrete type whichever
/// model backend (the real server, or an in-memory one in tests) is behind it.
pub trait ChangeApi: Send + Sync {
    fn propose<'a>(
        &'a self,
        project: &'a str,
        branch: Option<&'a str>,
        prefix: &'a str,
        desired: &'a [DesiredElement],
    ) -> BoxFut<'a, Result<ProposedChange, ChangeError>>;
    fn validate<'a>(&'a self, change_id: &'a str) -> BoxFut<'a, Result<Validation, ChangeError>>;
    fn commit<'a>(
        &'a self,
        change_id: &'a str,
        expected: Option<&'a str>,
    ) -> BoxFut<'a, Result<Committed, ChangeError>>;
}

impl<B: ModelBackend + 'static> ChangeApi for ChangeService<B> {
    fn propose<'a>(
        &'a self,
        project: &'a str,
        branch: Option<&'a str>,
        prefix: &'a str,
        desired: &'a [DesiredElement],
    ) -> BoxFut<'a, Result<ProposedChange, ChangeError>> {
        Box::pin(ChangeService::propose(
            self, project, branch, prefix, desired,
        ))
    }

    fn validate<'a>(&'a self, change_id: &'a str) -> BoxFut<'a, Result<Validation, ChangeError>> {
        Box::pin(ChangeService::validate(self, change_id))
    }

    fn commit<'a>(
        &'a self,
        change_id: &'a str,
        expected: Option<&'a str>,
    ) -> BoxFut<'a, Result<Committed, ChangeError>> {
        Box::pin(ChangeService::commit(self, change_id, expected))
    }
}

pub struct AssuranceRuntime {
    pub service: Arc<AssuranceService>,
    /// `None` makes the change routes return 503 (no model backend configured).
    pub changes: Option<Arc<dyn ChangeApi>>,
    pub audit: Option<Arc<AuditLog>>,
    /// Identifier prefix of the elements this service manages on the model server.
    pub managed_prefix: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assurance/requirements", get(list_requirements))
        .route("/assurance/requirements/:id", get(get_requirement))
        .route("/assurance/requirements/:id/trace", get(trace_requirement))
        .route("/assurance/view", get(view))
        .route("/assurance/evidence", get(evidence))
        .route("/assurance/changes", post(propose_change))
        .route("/assurance/changes/:change_id", get(validate_change))
        .route("/assurance/changes/:change_id/commit", post(commit_change))
        .route("/assurance/verify/:case_id", post(run_verification))
        .route("/assurance/audit", get(audit_records))
        .route("/assurance/skills/:name", get(skill))
        .route("/mcp/resources", get(mcp_resources))
        .route("/mcp/prompts", get(mcp_prompts))
}

fn runtime(state: &AppState) -> Result<Arc<AssuranceRuntime>, Box<Response>> {
    state.assurance.clone().ok_or_else(|| {
        Box::new(error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "assurance_not_configured",
            "set KR0KI_ASSURANCE_BASELINE (and KR0KI_EVIDENCE_DIR) to enable /assurance",
        ))
    })
}

fn service_error(e: ServiceError) -> Response {
    match e {
        ServiceError::UnknownRequirement(_) | ServiceError::UnknownCase(_) => {
            error_json(StatusCode::NOT_FOUND, "not_found", &e.to_string())
        }
        ServiceError::RevisionMismatch {
            ref expected,
            ref actual,
        } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "revision_mismatch",
                "message": e.to_string(),
                "expected": expected,
                "actual": actual,
            })),
        )
            .into_response(),
        ServiceError::Runner(ref r) => error_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            "case_refused",
            &r.to_string(),
        ),
        other => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "assurance_unavailable",
            &other.to_string(),
        ),
    }
}

fn change_error(e: ChangeError) -> Response {
    match e {
        ChangeError::StaleBase {
            ref expected,
            ref current,
        } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "stale_base",
                "message": e.to_string(),
                "expected": expected,
                "current": current,
            })),
        )
            .into_response(),
        ChangeError::ProposalMismatch { .. } => {
            error_json(StatusCode::CONFLICT, "proposal_mismatch", &e.to_string())
        }
        ChangeError::UnknownChange(_) => {
            error_json(StatusCode::NOT_FOUND, "unknown_change", &e.to_string())
        }
        ChangeError::NoChanges(_) => error_json(StatusCode::OK, "no_changes", &e.to_string()),
        ChangeError::Sync(_) => error_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_change",
            &e.to_string(),
        ),
        ChangeError::Backend(_) => error_json(
            StatusCode::BAD_GATEWAY,
            "model_backend_error",
            &e.to_string(),
        ),
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
) -> Result<T, Box<Response>> {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(Box::new(service_error(e))),
        Err(e) => Err(Box::new(error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "task_failed",
            &e.to_string(),
        ))),
    }
}

#[derive(Deserialize, Default)]
struct ListParams {
    status: Option<String>,
    owner: Option<String>,
    state: Option<String>,
    gap: Option<String>,
}

async fn list_requirements(State(state): State<AppState>, Query(p): Query<ListParams>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let filter = RequirementFilter {
        status: p.status,
        owner: p.owner,
        state: p.state,
        gap: p.gap,
    };
    match blocking(move || rt.service.list_requirements(&filter)).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => *r,
    }
}

async fn get_requirement(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    match blocking(move || rt.service.get_requirement(&id)).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => *r,
    }
}

async fn trace_requirement(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    match blocking(move || rt.service.trace_requirement(&id)).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => *r,
    }
}

#[derive(Deserialize, Default)]
struct ViewParams {
    format: Option<String>,
}

async fn view(State(state): State<AppState>, Query(p): Query<ViewParams>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let snap = match blocking(move || rt.service.snapshot()).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match p.format.as_deref().unwrap_or("json") {
        "json" => Json(&snap.view).into_response(),
        "table" => (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            to_table(&snap.view),
        )
            .into_response(),
        "d2" => (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            to_d2(snap.graph(), &snap.view),
        )
            .into_response(),
        "svg" => {
            let d2 = to_d2(snap.graph(), &snap.view);
            match state
                .service
                .render(DiagramFormat::D2, OutputKind::Svg, &d2)
                .await
            {
                Ok(r) => ([(header::CONTENT_TYPE, "image/svg+xml")], r.bytes).into_response(),
                Err(e) => error_json(StatusCode::BAD_GATEWAY, "render_failed", &e.to_string()),
            }
        }
        other => error_json(
            StatusCode::BAD_REQUEST,
            "bad_format",
            &format!("unknown format `{other}` (json|table|d2|svg)"),
        ),
    }
}

#[derive(Deserialize, Default)]
struct EvidenceParams {
    requirement: Option<String>,
    case: Option<String>,
}

async fn evidence(State(state): State<AppState>, Query(p): Query<EvidenceParams>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    match blocking(move || {
        rt.service
            .evidence(p.requirement.as_deref(), p.case.as_deref())
    })
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(r) => *r,
    }
}

// ── changes ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedRequirement {
    id: String,
    title: String,
    statement: String,
    source: String,
    /// Parsed only to reject an unknown vocabulary value; the model server does not store it.
    #[allow(dead_code)]
    source_kind: SourceKind,
    owner: String,
    rationale: String,
    verification_id: String,
    acceptance: String,
    /// Parsed only to reject an unknown vocabulary value; the model server does not store it.
    #[allow(dead_code)]
    status: RequirementStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeBody {
    project_id: String,
    #[serde(default)]
    branch_id: Option<String>,
    requirements: Vec<ProposedRequirement>,
}

#[derive(Serialize)]
struct Diagnostic {
    requirement_id: String,
    severity: &'static str,
    code: &'static str,
    message: String,
}

fn diagnose(body: &ProposeBody, known_cases: &BTreeSet<String>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for r in &body.requirements {
        let mut err = |code: &'static str, message: String| {
            out.push(Diagnostic {
                requirement_id: r.id.clone(),
                severity: "error",
                code,
                message,
            });
        };
        if !seen.insert(r.id.as_str()) {
            err(
                "duplicate_id",
                format!("`{}` appears more than once in this proposal", r.id),
            );
        }
        for (field, value) in [
            ("id", &r.id),
            ("title", &r.title),
            ("statement", &r.statement),
            ("source", &r.source),
            ("owner", &r.owner),
            ("rationale", &r.rationale),
            ("verification_id", &r.verification_id),
            ("acceptance", &r.acceptance),
        ] {
            if value.trim().is_empty() {
                err("empty_field", format!("`{field}` must not be empty"));
            }
        }
    }
    for r in &body.requirements {
        for issue in statement_issues(&r.statement) {
            out.push(Diagnostic {
                requirement_id: r.id.clone(),
                severity: "warning",
                code: "statement_lint",
                message: issue.to_string(),
            });
        }
        if !r.verification_id.trim().is_empty() && !known_cases.contains(&r.verification_id) {
            out.push(Diagnostic {
                requirement_id: r.id.clone(),
                severity: "warning",
                code: "unknown_verification_case",
                message: format!(
                    "`{}` is not a verification case in the baseline",
                    r.verification_id
                ),
            });
        }
    }
    out
}

async fn propose_change(State(state): State<AppState>, body: Bytes) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let Some(changes) = rt.changes.clone() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_backend_not_configured",
            "KR0KI_SYSMLV2_BASE_URL is not set on this server",
        );
    };
    let parsed: ProposeBody = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => return error_json(StatusCode::BAD_REQUEST, "bad_change", &e.to_string()),
    };
    let svc = rt.service.clone();
    let known_cases: BTreeSet<String> = match blocking(move || {
        let snap = svc.snapshot()?;
        Ok(cases_from_graph(snap.graph())
            .into_iter()
            .map(|c| c.id)
            .collect())
    })
    .await
    {
        Ok(c) => c,
        Err(r) => return *r,
    };
    let diagnostics = diagnose(&parsed, &known_cases);
    if diagnostics.iter().any(|d| d.severity == "error") {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "invalid_change",
                "message": "the proposal has errors; nothing was proposed",
                "diagnostics": diagnostics,
            })),
        )
            .into_response();
    }
    let desired: Vec<DesiredElement> = parsed
        .requirements
        .iter()
        .map(|r| {
            let mut fields = Map::new();
            fields.insert("name".into(), json!(r.title));
            fields.insert("text".into(), json!([r.statement]));
            DesiredElement {
                identifier: format!("{}{}", rt.managed_prefix, r.id),
                type_: "RequirementUsage",
                fields,
            }
        })
        .collect();
    match changes
        .propose(
            &parsed.project_id,
            parsed.branch_id.as_deref(),
            &rt.managed_prefix,
            &desired,
        )
        .await
    {
        Ok(change) => Json(json!({
            "change": change,
            "diagnostics": diagnostics,
            // The model server keeps the title and statement; the other profile fields are
            // authored in the baseline file, not stored on the server.
            "persisted_fields": ["name", "text"],
        }))
        .into_response(),
        Err(ChangeError::NoChanges(base)) => Json(json!({
            "change": null,
            "no_changes": true,
            "base_revision": base,
            "diagnostics": diagnostics,
        }))
        .into_response(),
        Err(e) => change_error(e),
    }
}

async fn validate_change(State(state): State<AppState>, Path(change_id): Path<String>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let Some(changes) = rt.changes.clone() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_backend_not_configured",
            "KR0KI_SYSMLV2_BASE_URL is not set on this server",
        );
    };
    match changes.validate(&change_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => change_error(e),
    }
}

#[derive(Deserialize)]
struct CommitParams {
    expected_revision: String,
}

async fn commit_change(
    State(state): State<AppState>,
    Path(change_id): Path<String>,
    Query(p): Query<CommitParams>,
) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let Some(changes) = rt.changes.clone() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_backend_not_configured",
            "KR0KI_SYSMLV2_BASE_URL is not set on this server",
        );
    };
    // An empty expected revision means "the branch has no commits yet".
    let expected = (!p.expected_revision.is_empty()).then_some(p.expected_revision.as_str());
    match changes.commit(&change_id, expected).await {
        Ok(c) => Json(c).into_response(),
        Err(e) => change_error(e),
    }
}

#[derive(Deserialize)]
struct VerifyParams {
    revision: String,
}

async fn run_verification(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
    Query(p): Query<VerifyParams>,
) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    match blocking(move || rt.service.run_verification(&case_id, &p.revision)).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => *r,
    }
}

#[derive(Deserialize, Default)]
struct AuditParams {
    caller: Option<String>,
    operation: Option<String>,
    decision: Option<String>,
    correlation_id: Option<String>,
    phase: Option<String>,
    limit: Option<usize>,
}

async fn audit_records(State(state): State<AppState>, Query(p): Query<AuditParams>) -> Response {
    let rt = match runtime(&state) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let Some(audit) = rt.audit.clone() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "audit_not_configured",
            "no audit log is configured",
        );
    };
    let decision = match p.decision.as_deref() {
        None => None,
        Some("permit") => Some(Decision::Permit),
        Some("deny") => Some(Decision::Deny),
        Some(o) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "bad_decision",
                &format!("`{o}` (permit|deny)"),
            )
        }
    };
    let phase = match p.phase.as_deref() {
        None => None,
        Some("decision") => Some(Phase::Decision),
        Some("outcome") => Some(Phase::Outcome),
        Some(o) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "bad_phase",
                &format!("`{o}` (decision|outcome)"),
            )
        }
    };
    let q = AuditQuery {
        caller: p.caller,
        operation: p.operation,
        decision,
        correlation_id: p.correlation_id,
        phase,
    };
    let limit = p.limit.unwrap_or(200).min(1000);
    let result = tokio::task::spawn_blocking(move || {
        let chain = audit.verify()?;
        let mut records = audit.query(&q)?;
        if records.len() > limit {
            records.drain(..records.len() - limit);
        }
        Ok::<_, kr0ki_core::audit_log::AuditError>((chain, records))
    })
    .await;
    match result {
        Ok(Ok((chain, records))) => Json(json!({
            "chain": match chain {
                Ok(n) => json!({"ok": true, "records": n}),
                Err(b) => json!({"ok": false, "line": b.line, "seq": b.seq, "reason": b.reason}),
            },
            "records": records,
        }))
        .into_response(),
        Ok(Err(e)) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "audit_unreadable",
            &e.to_string(),
        ),
        Err(e) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "task_failed",
            &e.to_string(),
        ),
    }
}

async fn skill(Path(name): Path<String>) -> Response {
    match name.as_str() {
        "requirements-authoring" => (
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            SKILL_REQUIREMENTS_AUTHORING,
        )
            .into_response(),
        _ => error_json(
            StatusCode::NOT_FOUND,
            "unknown_skill",
            &format!("no skill `{name}`"),
        ),
    }
}

async fn mcp_resources() -> Json<Value> {
    Json(mcp_surface::resources_manifest_json())
}

async fn mcp_prompts() -> Json<Value> {
    Json(mcp_surface::prompts_manifest_json())
}
