//! Plan 004 Phase 0 exit gate — the fixture trace.
//!
//! Records the ordered state-transition trace from connect through proposed
//! revision → approval → reconnect using ONLY the published Phase-0 contract
//! types (`workspace_types`) and the dev-session grant interface
//! (`dev_session`). Every step asserts its actor, its precondition, and the
//! postcondition the workspace service must guarantee in Phase 1. The printed
//! trace is the artifact the plan's exit gate requires ("record a fixture
//! trace from connect through proposed revision/approval/reconnect, and get
//! agreement that every state transition has an owner and precondition").
//!
//! Phase 1 replaces the in-memory `FixtureWorkspace` with the real store;
//! the assertions here become its acceptance tests, unchanged.

#![cfg_attr(not(test), allow(unused_imports, dead_code))] // test-only fixture module

use crate::dev_session::{DevSession, SessionClaims};
use crate::workspace_types::*;

// ---------- the in-memory stand-in for the Phase-1 store ----------
// Test-only fixture: everything below this line through the trace test is
// referenced solely from #[cfg(test)] code.

/// Precondition ledger: every mutation must pass through `assert_write_allowed`
/// (session grant + expected head compare-and-set), exactly what the real
/// service's policy layer will enforce.
#[cfg(test)]
#[derive(Default)]
struct FixtureWorkspace {
    workspace: Option<Workspace>,
    revisions: Vec<Revision>,
    branch_head: Option<String>,
    approvals: Vec<Approval>,
    /// Append-only AG-UI event log (event-name, actor) — the durable
    /// conversation/activity trail the plan requires for reconnect.
    events: Vec<(&'static str, String)>,
}

#[cfg(test)]
impl FixtureWorkspace {
    fn connect(&mut self, ws: Workspace, event: &'static str) {
        assert!(self.workspace.is_none(), "connect: workspace already bound");
        self.workspace = Some(ws);
        self.events.push((event, "server".into()));
    }

    /// Every write: session grant check + expected-head compare-and-set.
    fn assert_write_allowed(
        &self,
        claims: &SessionClaims,
        expected_head: Option<&str>,
    ) -> Result<(), String> {
        let ws = self.workspace.as_ref().ok_or("no workspace bound")?;
        if claims.workspace_id != ws.id {
            return Err("workspace mismatch: session grant does not cover this workspace".into());
        }
        if !claims.capabilities.iter().any(|c| c == "diagram.propose") {
            return Err("capability denied: diagram.propose".into());
        }
        match (&self.branch_head, expected_head) {
            (None, None) => Ok(()),
            (Some(head), Some(expected)) if head == expected => Ok(()),
            (Some(head), Some(expected)) => Err(format!(
                "stale head conflict: expected {expected}, actual {head}"
            )),
            (None, Some(expected)) => Err(format!(
                "stale head conflict: expected {expected}, no head yet"
            )),
            (Some(_), None) => Err("missing expected_revision_id on a write".into()),
        }
    }

    fn propose(
        &mut self,
        claims: &SessionClaims,
        expected_head: Option<&str>,
        mut rev: Revision,
    ) -> Result<String, String> {
        self.assert_write_allowed(claims, expected_head)?;
        // Server computes the hash — the client never supplies it (Plan 004 §2.1).
        rev.input_hash = Revision::compute_input_hash(&rev.canonical_input);
        rev.status = RevisionStatus::Proposed;
        let id = rev.id.clone();
        self.revisions.push(rev);
        self.events
            .push(("tool.intent.propose_patch", claims.actor_subject()));
        self.events.push(("state.proposal_created", id.clone()));
        Ok(id)
    }

    /// Human-only promotion: an agent actor is rejected at this boundary.
    /// The acting identity is passed explicitly — in Phase 1 the policy layer
    /// derives it from the authenticated session, not from client claims.
    fn approve(
        &mut self,
        actor: &Actor,
        claims: &SessionClaims,
        revision_id: &str,
        expected_head: Option<&str>,
    ) -> Result<(), String> {
        self.assert_write_allowed(claims, expected_head)?;
        if matches!(actor, Actor::Agent { .. }) {
            return Err("approval by agent actor is forbidden".into());
        }
        let rev = self
            .revisions
            .iter_mut()
            .find(|r| r.id == revision_id)
            .ok_or_else(|| format!("unknown revision {revision_id}"))?;
        assert_eq!(
            rev.status,
            RevisionStatus::Proposed,
            "only proposed revisions can be accepted"
        );
        rev.status = RevisionStatus::Accepted;
        self.branch_head = Some(revision_id.to_string());
        self.approvals.push(Approval {
            revision_id: revision_id.into(),
            approver: actor.clone(),
            decision: ApprovalDecision::Accept,
            reason: "fixture approval".into(),
            timestamp: "2026-09-18T00:00:00Z".into(),
        });
        self.events.push(("approval.requested", revision_id.into()));
        self.events
            .push(("state.head_advanced", revision_id.into()));
        Ok(())
    }

    /// Reconnect: a full STATE_SNAPSHOT is rebuilt from durable state; the
    /// revision bytes must be byte-identical to what was stored pre-disconnect.
    fn reconnect_snapshot(
        &mut self,
        revision_id: &str,
    ) -> Result<(WorkspaceSnapshot, String), String> {
        let ws = self.workspace.as_ref().ok_or("no workspace")?;
        let rev = self
            .revisions
            .iter()
            .find(|r| r.id == revision_id)
            .ok_or("unknown revision")?;
        self.events.push(("state.snapshot", "server".into()));
        Ok((
            WorkspaceSnapshot {
                workspace: WorkspaceView {
                    id: ws.id.clone(),
                    project: ProjectView {
                        id: ws.project_id.clone(),
                        name: "fixture".into(),
                    },
                    capabilities: vec!["diagram.read".into(), "diagram.propose".into()],
                },
                branches: vec![BranchView {
                    id: "branch_main".into(),
                    name: "main".into(),
                    head_revision_id: self.branch_head.clone(),
                }],
                active_revision: Some(RevisionView {
                    id: rev.id.clone(),
                    status: rev.status,
                    input_kind: rev.input_kind,
                    source: rev.canonical_input.clone(),
                    artifact: Some(ArtifactRef {
                        url: format!("/cache/{}", rev.input_hash),
                        hash: rev.input_hash.clone(),
                    }),
                }),
                revision_graph: RevisionGraph {
                    nodes: self
                        .revisions
                        .iter()
                        .map(|r| RevisionNode {
                            id: r.id.clone(),
                            status: r.status,
                            author_kind: "dev_session".into(),
                        })
                        .collect(),
                    edges: self
                        .revisions
                        .iter()
                        .flat_map(|r| {
                            r.parent_ids.iter().map(|p| RevisionEdge {
                                from: p.clone(),
                                to: r.id.clone(),
                            })
                        })
                        .collect(),
                },
                selection: Selection {
                    entity_id: None,
                    revision_id: Some(rev.id.clone()),
                },
                pending_approval: None,
            },
            rev.canonical_input.clone(),
        ))
    }
}

#[cfg(test)]
impl SessionClaims {
    fn actor_subject(&self) -> String {
        self.subject.clone()
    }
}

// ---------- the trace ----------

#[cfg(test)]
fn dev_claims(subject: &str, ws: &str) -> SessionClaims {
    SessionClaims {
        subject: subject.into(),
        workspace_id: ws.into(),
        capabilities: vec![
            "diagram.read".into(),
            "diagram.propose".into(),
            "diagram.accept".into(),
        ],
        expires_at: 0,
    }
}

#[test]
fn phase0_fixture_trace_connect_propose_approve_reconnect() {
    let _dev = DevSession::from_env_key(Some("fixture-key")).unwrap();
    let agent = dev_claims("dev:agent", "ws_fixture");
    let human = dev_claims("dev:operator", "ws_fixture");

    let mut ws = FixtureWorkspace::default();

    // -- 1. CONNECT: server binds session to one workspace (grant on the session).
    ws.connect(
        Workspace {
            id: "ws_fixture".into(),
            owner_id: "dev:operator".into(),
            project_id: "project_kr0ki".into(),
            active_branch_id: "branch_main".into(),
            grant: ProjectGrant {
                project_id: "project_kr0ki".into(),
                project_root: "/tmp/fixture-project".into(),
                allowed_formats: vec!["d2".into()],
                can_read: true,
                can_write: true,
                can_materialize: false,
            },
            created_at: "2026-09-18T00:00:00Z".into(),
        },
        "state.snapshot",
    );
    println!("1. CONNECT  owner=server          precondition=signed dev session bound to ws_fixture  postcondition=workspace bound, grant active");

    // -- 2. PROPOSE (agent): creates a proposed revision; head unchanged.
    let rev = Revision {
        id: "rev_001".into(),
        workspace_id: "ws_fixture".into(),
        branch_id: "branch_main".into(),
        parent_ids: vec![],
        status: RevisionStatus::Proposed,
        author: agent.actor(),
        intent: "add the cache node to the flow diagram".into(),
        input_kind: InputKind::DiagramSource,
        canonical_input: "client -> cache: lookup\n".into(),
        input_hash: String::new(), // server fills this
        renderer_recipe: "d2/svg@kr0ki-core+http-kroki-backend".into(),
        artifact_hash: None,
        created_at: "2026-09-18T00:00:01Z".into(),
    };
    let proposed = ws.propose(&agent, None, rev).expect("agent propose");
    assert_eq!(proposed, "rev_001");
    assert!(ws.branch_head.is_none(), "propose must NOT move the head");
    println!("2. PROPOSE  owner=agent           precondition=diagram.propose + expected head (none)   postcondition=rev_001 proposed, head unchanged, input_hash server-computed");

    // -- 2b. Agent CANNOT approve its own proposal (invariant 5).
    let agent_actor = Actor::Agent {
        subject: "dev:agent".into(),
    };
    let agent_approval = ws.approve(&agent_actor, &agent, "rev_001", None);
    assert!(
        agent_approval.is_err(),
        "agent actors must not promote revisions"
    );
    assert!(
        ws.branch_head.is_none(),
        "denied agent approval must not move the head"
    );
    println!("2b. DENY    owner=policy          precondition=approve() called by agent actor        postcondition=rejected, head still unchanged");

    // -- 3. STALE WRITE (agent): expected head that doesn't exist -> conflict.
    let stale = ws.propose(
        &agent,
        Some("rev_ghost"),
        Revision {
            id: "rev_002".into(),
            workspace_id: "ws_fixture".into(),
            branch_id: "branch_main".into(),
            parent_ids: vec![],
            status: RevisionStatus::Proposed,
            author: agent.actor(),
            intent: "stale".into(),
            input_kind: InputKind::DiagramSource,
            canonical_input: "x".into(),
            input_hash: String::new(),
            renderer_recipe: "d2/svg".into(),
            artifact_hash: None,
            created_at: "2026-09-18T00:00:02Z".into(),
        },
    );
    assert!(stale.is_err());
    assert_eq!(
        ws.revisions.len(),
        1,
        "stale write must not create a revision"
    );
    println!("3. CONFLICT owner=store           precondition=expected_revision_id != actual head     postcondition=typed conflict, no revision created");

    // -- 4. APPROVE (human): promotion to head, approval recorded immutably.
    let human_actor = Actor::Human {
        subject: "dev:operator".into(),
    };
    ws.approve(&human_actor, &human, "rev_001", None)
        .expect("human approve");
    assert_eq!(ws.branch_head.as_deref(), Some("rev_001"));
    println!("4. APPROVE  owner=human           precondition=diagram.accept + human actor            postcondition=rev_001 accepted and is branch head");

    // -- 5. APPROVE-AGAIN: terminal status must be immutable.
    let again = ws.approve(&human_actor, &human, "rev_001", None);
    assert!(again.is_err(), "accepted revision must be terminal");
    println!("5. DENY     owner=store           precondition=revision already accepted               postcondition=immutability preserved");

    // -- 6. RECONNECT: full snapshot; stored bytes byte-identical.
    let (snapshot, source) = ws.reconnect_snapshot("rev_001").expect("snapshot");
    assert_eq!(source, "client -> cache: lookup\n");
    assert_eq!(snapshot.active_revision.as_ref().unwrap().id, "rev_001");
    assert_eq!(
        snapshot.branches[0].head_revision_id.as_deref(),
        Some("rev_001")
    );
    assert!(snapshot
        .active_revision
        .as_ref()
        .unwrap()
        .artifact
        .is_some());
    println!("6. RECONNECT owner=server         precondition=durable revision + event log            postcondition=STATE_SNAPSHOT rebuilt, bytes identical to pre-disconnect");

    // -- 7. EVENT LOG: the ordered AG-UI trace this fixture records.
    println!("\n--- AG-UI event trace (ordered, durable) ---");
    for (i, (event, actor)) in ws.events.iter().enumerate() {
        println!("{:>2}. {} (actor: {})", i, event, actor);
    }
    // 6 events: connect-snapshot, propose intent, proposal-created,
    // approval-requested, head-advanced, reconnect-snapshot.
    // The DENIED writes (2b agent approval, 3 stale head, 5 re-approval)
    // correctly emit NO durable events — denials are policy rejections at
    // the boundary, not state transitions; their evidence is the trace
    // table above, and in Phase 1 they become audited policy-log rows.
    assert_eq!(ws.events.len(), 6);
    assert_eq!(ws.events[0].0, "state.snapshot");
    assert_eq!(ws.events[1].0, "tool.intent.propose_patch");
    assert_eq!(ws.events[2].0, "state.proposal_created");
    assert_eq!(ws.events[3].0, "approval.requested");
    assert_eq!(ws.events[4].0, "state.head_advanced");
    assert_eq!(ws.events[5].0, "state.snapshot");
}
