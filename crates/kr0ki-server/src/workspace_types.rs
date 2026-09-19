//! Plan 004 Phase 0 — domain type contracts for the revisioned procedural
//! workspace. Schema-first: these types are the Rust mirror of
//! `docs/schemas/plan-004/*.schema.json` and carry the same field names on the
//! wire. Serialization is the contract; store logic comes in Phase 1
//! (`crates/kr0ki-server/src/workspace/{store,service,routes,policy}.rs`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Canonical revision status. State-transition table (Plan 004 §7):
/// proposed -> accepted | rejected; accepted/rejected are terminal and
/// immutable. `merged` is reserved for Phase 4 two-parent revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionStatus {
    Proposed,
    Accepted,
    Rejected,
    #[serde(skip)]
    #[allow(dead_code)]
    MergedReservedPhase4,
}

/// Canonical input lane (Plan 004 §2.1). Only `DiagramSource` is accepted in
/// Phase 0/1; `ProceduralModel` is named so the schema rejects unknown lanes
/// rather than silently widening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    DiagramSource,
    #[serde(rename = "procedural_model_reserved")]
    ProceduralModelReservedPhase4,
}

/// Actor identity. Phase 0 has no production IdP: a signed dev session is a
/// `DevSubject`, and a bare thread ID is explicitly NOT a session (Plan 004
/// §6 Phase 0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// Human, authenticated via a granted session.
    Human { subject: String },
    /// Agent identity, e.g. "storyb00k-sidecar".
    Agent { subject: String },
    /// Local-development signed session stand-in. Must be rejected outside
    /// KR0KI_DEV_MODE.
    DevSession { subject: String },
}

/// One attempted procedural state (Plan 004 §2 `Revision` record).
/// `canonical_input` is validated diagram-as-code or a typed model snapshot —
/// never a rendered artifact. `artifact_hash` is derived, server-computed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: String,
    pub workspace_id: String,
    pub branch_id: String,
    /// Empty for the initial revision; otherwise one or more parent revision
    /// IDs. The full history is a DAG; Phase 0/1 expose linear only.
    #[serde(default)]
    pub parent_ids: Vec<String>,
    pub status: RevisionStatus,
    pub author: Actor,
    /// Human/agent statement of why this revision was proposed.
    pub intent: String,
    pub input_kind: InputKind,
    /// Canonical, validated, normalized input text (e.g. D2 source).
    pub canonical_input: String,
    /// SHA-256 of `canonical_input` (hex). Computed server-side; a client-
    /// supplied value is never trusted (Plan 004 §2.1).
    pub input_hash: String,
    /// Pinned renderer recipe, e.g. "d2/svg@kr0ki-core-0.x+kroki-backend".
    /// Regeneration must be reproducible from (canonical_input, recipe).
    pub renderer_recipe: String,
    /// SHA-256 of the rendered artifact bytes (hex); present only after a
    /// successful render. A failed attempt stays auditable without promotion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_hash: Option<String>,
    pub created_at: String,
}

impl Revision {
    /// Server-side canonical-input hash. The single hashing site so the
    /// schema, store, and replay tests agree.
    pub fn compute_input_hash(input: &str) -> String {
        let mut h = Sha256::new();
        h.update(input.as_bytes());
        hex(&h.finalize())
    }
}

/// A single ordered, validated tool operation inside a revision
/// (Plan 004 §2 `RevisionOperation`). This is the replay/audit trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionOperation {
    pub revision_id: String,
    /// 0-based order within the revision's operation list.
    pub sequence: u64,
    pub tool_name: String,
    pub tool_version: String,
    /// Schema-validated arguments. Secrets must be redacted before storage.
    pub arguments: serde_json::Value,
    /// Digest of the tool result; large bodies stored by content reference.
    pub result_digest: String,
    pub actor: Actor,
}

/// The required human promotion decision (Plan 004 §2 `Approval`).
/// Only a user action may move a `proposed` revision to branch head.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    pub revision_id: String,
    pub approver: Actor,
    pub decision: ApprovalDecision,
    pub reason: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Accept,
    Reject,
}

/// A mutable pointer to the current head revision. Revisions themselves are
/// immutable; only the branch pointer moves (Plan 004 §2 `Branch`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub workspace_id: String,
    /// MVP exposes `main` only.
    pub name: String,
    pub head_revision_id: Option<String>,
}

/// Names the development project a session may see. Not a shell grant;
/// no filesystem/shell/network authority (Plan 004 invariant 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectGrant {
    pub project_id: String,
    /// Declared root/worktree reference the grant scopes to.
    pub project_root: String,
    /// Allowed render formats, e.g. ["d2", "graphviz"].
    pub allowed_formats: Vec<String>,
    pub can_read: bool,
    pub can_write: bool,
    pub can_materialize: bool,
}

/// Bounded collaboration unit (Plan 004 §2 `Workspace`). Its project grant is
/// immutable except through an audited admin action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub owner_id: String,
    pub project_id: String,
    pub active_branch_id: String,
    pub grant: ProjectGrant,
    pub created_at: String,
}

/// Redacted browser snapshot projection (Plan 004 §2.2 wire state). This is
/// what AG-UI STATE_SNAPSHOT carries; project roots, credentials, raw tool
/// results, and policy fields never appear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub workspace: WorkspaceView,
    pub branches: Vec<BranchView>,
    pub active_revision: Option<RevisionView>,
    pub revision_graph: RevisionGraph,
    pub selection: Selection,
    pub pending_approval: Option<PendingApproval>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceView {
    pub id: String,
    pub project: ProjectView,
    /// Capability strings, e.g. ["diagram.read", "diagram.propose"].
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectView {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchView {
    pub id: String,
    pub name: String,
    pub head_revision_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionView {
    pub id: String,
    pub status: RevisionStatus,
    pub input_kind: InputKind,
    pub source: String,
    pub artifact: Option<ArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// URL the browser may fetch the artifact from.
    pub url: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RevisionGraph {
    pub nodes: Vec<RevisionNode>,
    pub edges: Vec<RevisionEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionNode {
    pub id: String,
    pub status: RevisionStatus,
    pub author_kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingApproval {
    pub revision_id: String,
    pub intent: String,
    pub proposed_by: Actor,
}

/// Stale-head write guard (Plan 004 invariant 4): every mutation carries
/// `workspace_id` + `expected_revision_id`; a mismatch is this typed conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub workspace_id: String,
    pub expected_revision_id: String,
    pub actual_head_revision_id: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const D2_A: &str = "x -> y";

    #[test]
    fn input_hash_is_deterministic_sha256() {
        let h1 = Revision::compute_input_hash(D2_A);
        let h2 = Revision::compute_input_hash(D2_A);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        // Known SHA-256 of "x -> y".
        assert_eq!(
            h1,
            "4784d08a8ba81e79a9dd30d4cd8a7ea09c5373c7c2022673a45be14f9448a2a4"
        );
    }

    #[test]
    fn revision_serializes_snake_case_with_all_contract_fields() {
        let rev = Revision {
            id: "rev_1".into(),
            workspace_id: "ws_1".into(),
            branch_id: "branch_main".into(),
            parent_ids: vec![],
            status: RevisionStatus::Proposed,
            author: Actor::Agent {
                subject: "storyb00k-sidecar".into(),
            },
            intent: "group services by bounded context".into(),
            input_kind: InputKind::DiagramSource,
            canonical_input: D2_A.into(),
            input_hash: Revision::compute_input_hash(D2_A),
            renderer_recipe: "d2/svg@kr0ki-core+http-kroki-backend".into(),
            artifact_hash: None,
            created_at: "2026-09-18T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&rev).unwrap();
        for field in [
            "id",
            "workspace_id",
            "branch_id",
            "parent_ids",
            "status",
            "author",
            "intent",
            "input_kind",
            "canonical_input",
            "input_hash",
            "renderer_recipe",
            "created_at",
        ] {
            assert!(v.get(field).is_some(), "missing contract field: {field}");
        }
        assert_eq!(v["status"], "proposed");
        assert_eq!(v["input_kind"], "diagram_source");
        assert_eq!(v["author"]["kind"], "agent");
    }

    #[test]
    fn status_is_terminal_snake_case() {
        assert_eq!(
            serde_json::to_value(RevisionStatus::Accepted).unwrap(),
            "accepted"
        );
        assert_eq!(
            serde_json::to_value(RevisionStatus::Rejected).unwrap(),
            "rejected"
        );
    }

    #[test]
    fn snapshot_projection_roundtrips() {
        let snap = WorkspaceSnapshot {
            workspace: WorkspaceView {
                id: "ws_1".into(),
                project: ProjectView {
                    id: "p1".into(),
                    name: "demo".into(),
                },
                capabilities: vec!["diagram.read".into(), "diagram.propose".into()],
            },
            branches: vec![BranchView {
                id: "branch_main".into(),
                name: "main".into(),
                head_revision_id: Some("rev_1".into()),
            }],
            active_revision: Some(RevisionView {
                id: "rev_1".into(),
                status: RevisionStatus::Accepted,
                input_kind: InputKind::DiagramSource,
                source: D2_A.into(),
                artifact: Some(ArtifactRef {
                    url: "/cache/abc".into(),
                    hash: "deadbeef".into(),
                }),
            }),
            revision_graph: RevisionGraph::default(),
            selection: Selection {
                entity_id: None,
                revision_id: Some("rev_1".into()),
            },
            pending_approval: None,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: WorkspaceSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
    }
}
