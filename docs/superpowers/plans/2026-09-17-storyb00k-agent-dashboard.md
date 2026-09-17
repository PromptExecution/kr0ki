# storyb00k — agentic digital-thread query & storytelling panel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give kr0ki's `playbook/` a new panel, storyb00k, backed by a sidecar agent that
(a) queries the live SysML v2 model (via new `/model/*` routes wrapping
`kr0ki-sysmlv2-client`'s existing read API) and a new in-memory RDF triple graph, (b)
operates kr0ki's existing render tools, (c) narrates and assembles a multi-modal
(SVG/PNG + source text) dashboard, and (d) lets the user and agent iteratively
negotiate a session-scoped draft edit — never touching the authoritative model.

**Architecture:** Rust-side additions to `kr0ki-core`/`kr0ki-server` (new `McpTool`
variants, a new `graph_store.rs` in-memory `oxrdf` triple index, six new `/model/*`
routes) are pure extensions of the already-shipped mcp-http-parity manifest-dispatch
pattern. A new Python sidecar (`kr0ki-storyb00k-agent`, same-pod precedent as
`kr0ki-mcp`) speaks the AG-UI protocol over SSE, proxies to a generic
OpenAI-compatible LLM, dispatches tool calls via a manifest-dispatch module extracted
from `bridge.py`, and holds its own disposable `rdflib` draft graph. A new `playbook/`
panel consumes it via `ag-ui-vue`.

**Tech Stack:** Rust (axum, `oxrdf`/`oxttl`, `kr0ki-sysmlv2-client`), Python 3 stdlib +
`rdflib` (sidecar), Vue 3 + `ag-ui-vue` (frontend).

**Spec:** `docs/superpowers/specs/2026-09-17-storyb00k-agent-dashboard-design.md`
(revision 3). Companion context: `docs/DESIGN-NOTE-agentic-mbse-generation.md`,
`docs/PLAN-KR0KI-003-rust-source-frontend.md` (harmonization, spec §2),
`docs/EVAL-flexo.md`, `docs/EVAL-sysml-derive.md`.

## Prerequisite — branch base

**`crates/kr0ki-core/src/mcp_tool.rs`, `GET /mcp/tools`, and `POST /render/kubediagram`
do not exist on `main` yet** — they exist only on the still-open, unmerged
`mcp-http-parity` branch (PR #26). Task 1 below extends `McpTool`, which requires that
file to exist. **This plan's branch/worktree MUST fork from `mcp-http-parity`, not
`main`**, and should rebase onto `main` once PR #26 merges (a routine rebase — nothing
in this plan depends on anything PR #26 might change before merge, only on what it
already adds).

## Global Constraints

- One closed enum for the MCP-tool family (`McpTool`, extended, not replaced) — no
  general/dynamic tool-registration mechanism. YAGNI per the mcp-http-parity design's
  own precedent (spec §1 there).
- The new `graph_store.rs` in-memory triple index uses `oxrdf`/`oxttl` **directly** —
  no new crate dependency (`oxigraph`, `HelixDB`, or otherwise). A bounded,
  hand-rolled query surface (a closed set of query shapes), not general SPARQL.
  Likewise a hand-rolled, bounded SHACL-style shape checker — not a general SHACL
  engine. See design spec §10a for why.
- Every new `kr0ki-server` route follows the existing "unconfigured/unavailable is a
  clean 503, not a panic or 500" convention (`/b00t-graph`, `/capabilities`,
  `/render/kubediagram` all already do this).
- Every `AppState` struct literal across the codebase must be updated in the same task
  that adds a new field to `AppState` — list them explicitly per task.
- The sidecar's LLM backend config is **generic**: `OPENAI_API_KEY` / `OPENAI_API_URL`
  only. No hardcoded endpoint anywhere in sidecar code.
- The sidecar's session-scoped draft graph (`rdflib`, Task 8) is **never** written to
  `kr0ki-server`'s own graph store, never to Flexo, never to the authoritative model.
  It is disposable, in-memory, gone when the session ends.
- `containers/kr0ki-mcp/bridge.py`'s existing test coverage (`test_bridge.py`) must not
  regress when its dispatch logic is extracted in Task 6 — this is production code
  from an already-merged (once PR #26 lands) feature.
- Reuse `KR0KI_SYSMLV2_BASE_URL` / `KR0KI_SYSMLV2_TOKEN` (already established by
  `kr0ki-sysmlv2-client`'s own live test) for the new `/model/*` routes' backend
  config — do not invent new env var names for the same concept.

---

### Task 1: `McpTool` — seven new model-query/graph tool variants

**Files:**
- Modify: `crates/kr0ki-core/src/mcp_tool.rs`

**Interfaces:**
- Consumes: nothing new (extends the existing `McpTool`/`HttpBinding`/`ArgBinding`
  shapes from mcp-http-parity, unchanged).
- Produces: seven new `McpTool` variants in `McpTool::ALL`, each with a real
  `http_binding()` targeting the routes Tasks 3-4 add. Tasks 3-4 use these exact
  path templates.

- [ ] **Step 1: Write the failing tests**

Add to `crates/kr0ki-core/src/mcp_tool.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn all_ten_tools_have_unique_names() {
        let mut names: Vec<&str> = McpTool::ALL.iter().map(|t| t.name()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate McpTool name in ALL");
        assert_eq!(McpTool::ALL.len(), 10);
    }

    #[test]
    fn list_model_projects_binds_to_a_plain_get_with_no_args() {
        let binding = McpTool::ListModelProjects.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(binding.path_template, "/model/projects");
        assert!(binding.args.is_empty());
    }

    #[test]
    fn list_model_commits_binds_project_id_to_path() {
        let binding = McpTool::ListModelCommits.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(binding.path_template, "/model/projects/{project_id}/commits");
        assert_eq!(binding.args.len(), 1);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "project_id" && matches!(a.placement, ArgPlacement::Path)));
    }

    #[test]
    fn get_model_snapshot_binds_project_and_commit_to_path() {
        let binding = McpTool::GetModelSnapshot.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(
            binding.path_template,
            "/model/projects/{project_id}/commits/{commit_id}/snapshot"
        );
        assert_eq!(binding.args.len(), 2);
    }

    #[test]
    fn query_model_elements_binds_project_and_commit_to_path() {
        let binding = McpTool::QueryModelElements.http_binding();
        assert_eq!(
            binding.path_template,
            "/model/projects/{project_id}/commits/{commit_id}/elements"
        );
        assert_eq!(binding.args.len(), 2);
    }

    #[test]
    fn get_model_roots_binds_project_and_commit_to_path() {
        let binding = McpTool::GetModelRoots.http_binding();
        assert_eq!(
            binding.path_template,
            "/model/projects/{project_id}/commits/{commit_id}/roots"
        );
        assert_eq!(binding.args.len(), 2);
    }

    #[test]
    fn query_model_relationships_binds_element_id_and_direction_query() {
        let binding = McpTool::QueryModelRelationships.http_binding();
        assert_eq!(
            binding.path_template,
            "/model/projects/{project_id}/commits/{commit_id}/elements/{element_id}/relationships"
        );
        assert_eq!(binding.args.len(), 4);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "direction" && matches!(a.placement, ArgPlacement::Query)));
    }

    #[test]
    fn query_model_graph_binds_query_shape_and_subject_to_query_params() {
        let binding = McpTool::QueryModelGraph.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(binding.path_template, "/model/graph/query");
        assert_eq!(binding.args.len(), 2);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "shape" && matches!(a.placement, ArgPlacement::Query)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "subject" && matches!(a.placement, ArgPlacement::Query)));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kr0ki-core --lib mcp_tool`
Expected: FAIL to compile — the new variants don't exist yet.

- [ ] **Step 3: Write the implementation**

In `crates/kr0ki-core/src/mcp_tool.rs`, change the enum and every `match self` over it.
Replace:

```rust
pub enum McpTool {
    RenderDiagram,
    ListFormats,
    RenderKubeDiagram,
}
```

with:

```rust
pub enum McpTool {
    RenderDiagram,
    ListFormats,
    RenderKubeDiagram,
    ListModelProjects,
    ListModelCommits,
    GetModelSnapshot,
    QueryModelElements,
    GetModelRoots,
    QueryModelRelationships,
    QueryModelGraph,
}
```

Replace `ALL`:

```rust
    pub const ALL: &'static [McpTool] = &[
        Self::RenderDiagram,
        Self::ListFormats,
        Self::RenderKubeDiagram,
        Self::ListModelProjects,
        Self::ListModelCommits,
        Self::GetModelSnapshot,
        Self::QueryModelElements,
        Self::GetModelRoots,
        Self::QueryModelRelationships,
        Self::QueryModelGraph,
    ];
```

Extend `name()`'s match with:

```rust
            Self::ListModelProjects => "list_model_projects",
            Self::ListModelCommits => "list_model_commits",
            Self::GetModelSnapshot => "get_model_snapshot",
            Self::QueryModelElements => "query_model_elements",
            Self::GetModelRoots => "get_model_roots",
            Self::QueryModelRelationships => "query_model_relationships",
            Self::QueryModelGraph => "query_model_graph",
```

Extend `description()`'s match with:

```rust
            Self::ListModelProjects => "List SysML v2 projects on the configured model server.",
            Self::ListModelCommits => "List commits (immutable model snapshots) for a project.",
            Self::GetModelSnapshot => {
                "Fetch the full content-hashed element+root set for a project/commit."
            }
            Self::QueryModelElements => "List every element in a project/commit.",
            Self::GetModelRoots => "List the root element ids of a project/commit.",
            Self::QueryModelRelationships => {
                "List a model element's relationships (in/out/both direction)."
            }
            Self::QueryModelGraph => {
                "Query kr0ki-server's in-memory RDF graph (bounded query shapes, not SPARQL)."
            }
```

Extend `input_schema()`'s match with:

```rust
            Self::ListModelProjects => serde_json::json!({"type": "object", "properties": {}}),
            Self::ListModelCommits => serde_json::json!({
                "type": "object",
                "required": ["project_id"],
                "properties": {
                    "project_id": {"type": "string", "description": "SysML v2 project id."}
                }
            }),
            Self::GetModelSnapshot => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id"],
                "properties": {
                    "project_id": {"type": "string"},
                    "commit_id": {"type": "string"}
                }
            }),
            Self::QueryModelElements => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id"],
                "properties": {
                    "project_id": {"type": "string"},
                    "commit_id": {"type": "string"}
                }
            }),
            Self::GetModelRoots => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id"],
                "properties": {
                    "project_id": {"type": "string"},
                    "commit_id": {"type": "string"}
                }
            }),
            Self::QueryModelRelationships => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id", "element_id"],
                "properties": {
                    "project_id": {"type": "string"},
                    "commit_id": {"type": "string"},
                    "element_id": {"type": "string"},
                    "direction": {"type": "string", "enum": ["in", "out", "both"], "default": "both"}
                }
            }),
            Self::QueryModelGraph => serde_json::json!({
                "type": "object",
                "required": ["shape"],
                "properties": {
                    "shape": {
                        "type": "string",
                        "enum": ["triples_about", "related_via"],
                        "description": "Which bounded query shape to run — see graph_store.rs."
                    },
                    "subject": {"type": "string", "description": "Element id (IRI local name) to query about."}
                }
            }),
```

Extend `http_binding()`'s match with:

```rust
            Self::ListModelProjects => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects",
                args: &[],
            },
            Self::ListModelCommits => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects/{project_id}/commits",
                args: &[ArgBinding {
                    name: "project_id",
                    placement: ArgPlacement::Path,
                }],
            },
            Self::GetModelSnapshot => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects/{project_id}/commits/{commit_id}/snapshot",
                args: &[
                    ArgBinding {
                        name: "project_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "commit_id",
                        placement: ArgPlacement::Path,
                    },
                ],
            },
            Self::QueryModelElements => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects/{project_id}/commits/{commit_id}/elements",
                args: &[
                    ArgBinding {
                        name: "project_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "commit_id",
                        placement: ArgPlacement::Path,
                    },
                ],
            },
            Self::GetModelRoots => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects/{project_id}/commits/{commit_id}/roots",
                args: &[
                    ArgBinding {
                        name: "project_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "commit_id",
                        placement: ArgPlacement::Path,
                    },
                ],
            },
            Self::QueryModelRelationships => HttpBinding {
                method: HttpMethod::Get,
                path_template:
                    "/model/projects/{project_id}/commits/{commit_id}/elements/{element_id}/relationships",
                args: &[
                    ArgBinding {
                        name: "project_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "commit_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "element_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "direction",
                        placement: ArgPlacement::Query,
                    },
                ],
            },
            Self::QueryModelGraph => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/graph/query",
                args: &[
                    ArgBinding {
                        name: "shape",
                        placement: ArgPlacement::Query,
                    },
                    ArgBinding {
                        name: "subject",
                        placement: ArgPlacement::Query,
                    },
                ],
            },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kr0ki-core --lib mcp_tool`
Expected: PASS (all 12 tests: 5 pre-existing + 7 new).

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt -p kr0ki-core
cargo clippy -p kr0ki-core --all-targets -- -D warnings
git add crates/kr0ki-core/src/mcp_tool.rs
git commit -m "feat: add seven model-query/graph McpTool variants for storyb00k"
```

---

### Task 2: `graph_store.rs` — in-memory RDF triple index + bounded query + shape check

**Files:**
- Create: `crates/kr0ki-core/src/graph_store.rs`
- Modify: `crates/kr0ki-core/src/lib.rs` (add `pub mod graph_store;`, alphabetized
  between `format` and `k8s_recognizer`)

**Interfaces:**
- Produces: `kr0ki_core::graph_store::{GraphStore, QueryShape, ShapeViolation}`. Task 3
  consumes `GraphStore::insert_element_triples(&mut self, project_id: &str, commit_id:
  &str, element: &kr0ki_sysmlv2_client::Element)` to materialize query results. Task 4
  consumes `GraphStore::query(&self, shape: QueryShape, subject: Option<&str>) ->
  Vec<(String, String, String)>` (subject/predicate/object as plain strings, already
  IRI-local-name-shortened for JSON output) and `GraphStore::check_shapes(&self) ->
  Vec<ShapeViolation>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/kr0ki-core/src/graph_store.rs` with just the test module first:

```rust
//! An in-memory RDF triple index over SysML v2 model-query results, plus a bounded
//! query surface and a bounded set of SHACL-style structural shape checks.
//!
//! Not a general triplestore and not a SPARQL engine — a small, closed set of query
//! shapes (see [`QueryShape`]) over `oxrdf` term types, matching kr0ki's existing
//! "closed enum, not general machinery" posture (`docs/VOCABULARY.md`,
//! `docs/DESIGN-NOTE-typed-model-layer.md`). Built directly on the `oxrdf`/`oxttl`
//! crates kr0ki already vendors for `b00t_graph.rs`'s Turtle parsing — no new
//! dependency (see `docs/superpowers/specs/2026-09-17-storyb00k-agent-dashboard-design.md`
//! §10a for why `oxigraph`/HelixDB were considered and rejected for this).
//!
//! This store is a derived, disposable cache of the authoritative SysML v2 model
//! (sourced from `kr0ki-sysmlv2-client` query results) — never itself a source of
//! truth, same discipline as `cache::FsCache`.

use std::collections::BTreeSet;
use std::sync::Mutex;

use oxrdf::{NamedNode, Term, Triple};

/// A predicate-object-less identity every element gets — its `@type`.
const RDF_TYPE_PRED: &str = "http://kr0ki.promptexecution.com/ontology#type";
const KR0KI_NS: &str = "http://kr0ki.promptexecution.com/ontology#";

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_element() -> kr0ki_sysmlv2_client::Element {
        let json = serde_json::json!({
            "@id": "elem-1",
            "@type": "PartUsage",
            "name": "Engine"
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn inserting_an_element_materializes_its_type_and_name_as_triples() {
        let mut store = GraphStore::new();
        store.insert_element_triples("proj-1", "commit-1", &sample_element());
        let triples = store.query(QueryShape::TriplesAbout, Some("elem-1"));
        assert!(triples
            .iter()
            .any(|(_, p, o)| p == "type" && o == "PartUsage"));
        assert!(triples.iter().any(|(_, p, o)| p == "name" && o == "Engine"));
    }

    #[test]
    fn triples_about_an_unknown_subject_is_empty_not_an_error() {
        let store = GraphStore::new();
        let triples = store.query(QueryShape::TriplesAbout, Some("does-not-exist"));
        assert!(triples.is_empty());
    }

    #[test]
    fn related_via_finds_elements_linked_by_a_relation_predicate() {
        let mut store = GraphStore::new();
        store.insert_relationship_triple("elem-1", "Satisfy", "elem-2");
        let related = store.query(QueryShape::RelatedVia, Some("elem-1"));
        assert!(related
            .iter()
            .any(|(_, p, o)| p == "Satisfy" && o == "elem-2"));
    }

    #[test]
    fn check_shapes_flags_an_element_with_no_type_triple() {
        let mut store = GraphStore::new();
        // insert a bare relationship triple with no accompanying type triple for
        // "orphan-elem" — a structural violation the shape check should catch.
        store.insert_relationship_triple("elem-1", "Satisfy", "orphan-elem");
        let violations = store.check_shapes();
        assert!(violations
            .iter()
            .any(|v| v.subject == "orphan-elem" && v.rule == "must_have_type"));
    }

    #[test]
    fn a_fully_typed_element_has_no_shape_violations() {
        let mut store = GraphStore::new();
        store.insert_element_triples("proj-1", "commit-1", &sample_element());
        let violations = store.check_shapes();
        assert!(violations.iter().all(|v| v.subject != "elem-1"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-core --lib graph_store`
Expected: FAIL TO COMPILE — `GraphStore`/`QueryShape`/`ShapeViolation` don't exist yet.

- [ ] **Step 3: Write the implementation**

Append to `crates/kr0ki-core/src/graph_store.rs` (above the `#[cfg(test)]` module):

```rust
/// Which bounded query shape [`GraphStore::query`] runs. Mirrors
/// `McpTool::QueryModelGraph`'s `shape` argument enum — see `mcp_tool.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryShape {
    /// Every triple whose subject is the given element id.
    TriplesAbout,
    /// Every relationship triple whose subject is the given element id — i.e. what
    /// this element relates to, and via which relation kind.
    RelatedVia,
}

/// A structural violation found by [`GraphStore::check_shapes`] — the RDF-layer
/// analog of a `validate_sysml_v2` diagnostic, but over the materialized graph
/// rather than concrete SysML v2 text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeViolation {
    pub subject: String,
    /// A closed, small set of rule names — not a general SHACL constraint language.
    pub rule: &'static str,
    pub message: String,
}

fn local_node(local_name: &str) -> NamedNode {
    NamedNode::new(format!("{KR0KI_NS}{local_name}")).expect("local_name is a valid IRI segment")
}

fn short_name(iri: &str) -> String {
    iri.rsplit('#').next().unwrap_or(iri).to_string()
}

/// An in-memory, disposable RDF triple index. See module docs.
#[derive(Debug, Default)]
pub struct GraphStore {
    triples: Mutex<BTreeSet<Triple>>,
}

impl GraphStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Materialize one `kr0ki-sysmlv2-client::Element` as RDF triples: its `@type`,
    /// its `name` if present, and (project_id, commit_id) as provenance triples.
    pub fn insert_element_triples(
        &self,
        project_id: &str,
        commit_id: &str,
        element: &kr0ki_sysmlv2_client::Element,
    ) {
        let subject = local_node(element.id());
        let mut triples = self.triples.lock().expect("graph_store mutex poisoned");
        triples.insert(Triple::new(
            subject.clone(),
            local_node("type"),
            Term::Literal(element.ty().into()),
        ));
        if let Some(name) = element.name() {
            triples.insert(Triple::new(
                subject.clone(),
                local_node("name"),
                Term::Literal(name.into()),
            ));
        }
        triples.insert(Triple::new(
            subject.clone(),
            local_node("sourceProject"),
            Term::Literal(project_id.into()),
        ));
        triples.insert(Triple::new(
            subject,
            local_node("sourceCommit"),
            Term::Literal(commit_id.into()),
        ));
    }

    /// Materialize one relationship as a single directed triple
    /// `subject_id --relation_kind--> object_id`.
    pub fn insert_relationship_triple(&self, subject_id: &str, relation_kind: &str, object_id: &str) {
        let mut triples = self.triples.lock().expect("graph_store mutex poisoned");
        triples.insert(Triple::new(
            local_node(subject_id),
            local_node(relation_kind),
            Term::NamedNode(local_node(object_id)),
        ));
    }

    /// Run one of the two bounded query shapes. Returns
    /// `(subject_local_name, predicate_local_name, object_display_string)` triples —
    /// already shortened for direct JSON serialization by the `/model/graph/query`
    /// route (Task 4).
    pub fn query(&self, shape: QueryShape, subject: Option<&str>) -> Vec<(String, String, String)> {
        let Some(subject) = subject else {
            return Vec::new();
        };
        let subject_node = local_node(subject);
        let triples = self.triples.lock().expect("graph_store mutex poisoned");
        triples
            .iter()
            .filter(|t| match t.subject.clone() {
                oxrdf::NamedOrBlankNode::NamedNode(n) => n == subject_node,
                oxrdf::NamedOrBlankNode::BlankNode(_) => false,
            })
            .filter(|t| match shape {
                QueryShape::TriplesAbout => true,
                QueryShape::RelatedVia => {
                    !matches!(short_name(t.predicate.as_str()).as_str(), "type" | "name" | "sourceProject" | "sourceCommit")
                }
            })
            .map(|t| {
                let object = match &t.object {
                    Term::NamedNode(n) => short_name(n.as_str()),
                    Term::Literal(l) => l.value().to_string(),
                    Term::BlankNode(b) => b.to_string(),
                };
                (
                    short_name(t.subject.clone().to_string().trim_matches(['<', '>'])),
                    short_name(t.predicate.as_str()),
                    object,
                )
            })
            .collect()
    }

    /// Bounded SHACL-style structural check: every element referenced as an object of
    /// a relationship triple (i.e. every element another element points at) must
    /// itself carry a `type` triple. Catches a relationship into an element that was
    /// never actually materialized via [`insert_element_triples`] — a real structural
    /// gap (kr0ki queried a relationship but never fetched the target element), not a
    /// general SHACL shape language.
    pub fn check_shapes(&self) -> Vec<ShapeViolation> {
        let triples = self.triples.lock().expect("graph_store mutex poisoned");
        let typed_subjects: BTreeSet<String> = triples
            .iter()
            .filter(|t| short_name(t.predicate.as_str()) == "type")
            .map(|t| short_name(t.subject.clone().to_string().trim_matches(['<', '>'])))
            .collect();

        triples
            .iter()
            .filter_map(|t| match &t.object {
                Term::NamedNode(n) => Some(short_name(n.as_str())),
                _ => None,
            })
            .filter(|object_id| !typed_subjects.contains(object_id))
            .map(|object_id| ShapeViolation {
                subject: object_id.clone(),
                rule: "must_have_type",
                message: format!(
                    "{object_id} is the target of a relationship but was never itself \
                     materialized with a type triple — query it explicitly before relying on it"
                ),
            })
            .collect()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kr0ki-core --lib graph_store`
Expected: PASS (5 tests).

- [ ] **Step 5: Register the module**

In `crates/kr0ki-core/src/lib.rs`, add `pub mod graph_store;` alphabetized between
`format` and `k8s_recognizer` (check the existing `pub mod` list's ordering first and
match it exactly).

- [ ] **Step 6: Run the full kr0ki-core suite, format, lint, commit**

```bash
cargo test -p kr0ki-core --lib
cargo fmt -p kr0ki-core
cargo clippy -p kr0ki-core --all-targets -- -D warnings
git add crates/kr0ki-core/src/graph_store.rs crates/kr0ki-core/src/lib.rs
git commit -m "feat: add graph_store.rs, an in-memory RDF triple index over oxrdf"
```

---

### Task 3: `kr0ki-server` — six `/model/*` routes wrapping `kr0ki-sysmlv2-client`

**Files:**
- Modify: `crates/kr0ki-server/Cargo.toml` (add `kr0ki-sysmlv2-client` to
  `[dependencies]` — it's a workspace crate already built in Task 1's prerequisite
  branch; check its exact `path = "../kr0ki-sysmlv2-client"` form against
  `kr0ki-core/Cargo.toml`'s own intra-workspace dependency style first)
- Modify: `crates/kr0ki-server/src/app.rs` (`AppState` fields, six routes + handlers)
- Modify: `crates/kr0ki-server/src/main.rs` (env wiring: `KR0KI_SYSMLV2_BASE_URL`,
  `KR0KI_SYSMLV2_TOKEN`)
- Modify: `crates/kr0ki-server/tests/http.rs` (`test_state()` update, new tests)
- Modify: `crates/kr0ki-server/tests/b00t_graph_live.rs` (its one `AppState` literal)
- Modify: `crates/kr0ki-server/tests/kubediagram_live.rs` (its one `AppState` literal)

**Interfaces:**
- Consumes: `kr0ki_sysmlv2_client::{SysmlV2Client, Direction, Page}` (unmodified,
  read-only), `kr0ki_core::graph_store::GraphStore` (Task 2).
- Produces: `AppState.sysmlv2_client: Option<Arc<SysmlV2Client>>`,
  `AppState.model_graph: Arc<kr0ki_core::graph_store::GraphStore>` (always present,
  not optional — an empty store is a valid, safe starting state, unlike the client
  which genuinely may be unconfigured). Task 4 consumes `AppState.model_graph`.

- [ ] **Step 1: Write the failing tests**

In `crates/kr0ki-server/tests/http.rs`, first update `test_state()` — add the two new
fields:

```rust
    AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: None,
        sysmlv2_client: None,
        model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),
    }
}
```

Now add these tests, after the existing `render_kubediagram_*` tests:

```rust
#[tokio::test]
async fn list_model_projects_is_503_when_not_configured() {
    let app = test_app(test_state("model-unconfigured"));
    let resp = app
        .oneshot(Request::get("/model/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("sysmlv2_client_not_configured"));
}

fn test_state_with_sysmlv2_client(tag: &str, base_url: String) -> AppState {
    let mut state = test_state(tag);
    state.sysmlv2_client = Some(Arc::new(kr0ki_sysmlv2_client::SysmlV2Client::new(base_url)));
    state
}

#[tokio::test]
async fn list_model_projects_proxies_to_the_configured_client() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"@id": "proj-1", "name": "Toaster"}
        ])))
        .mount(&server)
        .await;

    let app = test_app(test_state_with_sysmlv2_client("model-projects", server.uri()));
    let resp = app
        .oneshot(Request::get("/model/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("proj-1"));
}

#[tokio::test]
async fn get_model_snapshot_materializes_elements_into_the_graph_store() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/proj-1/commits/c1/elements"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"@id": "elem-1", "@type": "PartUsage", "name": "Engine"}
        ])))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/projects/proj-1/commits/c1/roots"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!(["elem-1"])))
        .mount(&server)
        .await;

    let state = test_state_with_sysmlv2_client("model-snapshot", server.uri());
    let graph = state.model_graph.clone();
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::get("/model/projects/proj-1/commits/c1/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);

    let triples = graph.query(kr0ki_core::graph_store::QueryShape::TriplesAbout, Some("elem-1"));
    assert!(triples.iter().any(|(_, p, o)| p == "name" && o == "Engine"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kr0ki-server --test http model`
Expected: FAIL to compile — `AppState` has no `sysmlv2_client`/`model_graph` fields,
routes don't exist.

- [ ] **Step 3: Write the implementation**

In `crates/kr0ki-server/Cargo.toml`, add to `[dependencies]` (matching the existing
intra-workspace `kr0ki-core = { path = "../kr0ki-core" }` style):

```toml
kr0ki-sysmlv2-client = { path = "../kr0ki-sysmlv2-client" }
```

In `crates/kr0ki-server/src/app.rs`, add to `AppState`:

```rust
    /// Client for the configured OMG Systems Modeling API server (Flexo MMS, the OMG
    /// reference pilot, SysON, ...) — storyb00k design, 2026-09-17. `None` disables
    /// every `/model/*` route (503, not a panic).
    pub sysmlv2_client: Option<Arc<kr0ki_sysmlv2_client::SysmlV2Client>>,
    /// In-memory RDF triple index materialized from `/model/*` query results —
    /// storyb00k design, 2026-09-17 §10a. Always present (an empty store is a safe
    /// starting state); never itself a source of truth.
    pub model_graph: Arc<kr0ki_core::graph_store::GraphStore>,
```

Add the six routes (right after `.route("/render/kubediagram", post(render_kubediagram))`):

```rust
        .route("/model/projects", get(list_model_projects))
        .route("/model/projects/:project_id/commits", get(list_model_commits))
        .route(
            "/model/projects/:project_id/commits/:commit_id/snapshot",
            get(get_model_snapshot),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/elements",
            get(query_model_elements),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/roots",
            get(get_model_roots),
        )
        .route(
            "/model/projects/:project_id/commits/:commit_id/elements/:element_id/relationships",
            get(query_model_relationships),
        )
```

Add the handlers (near the other route handlers, e.g. after `render_kubediagram`):

```rust
/// Shared unconfigured-client guard every `/model/*` handler starts with.
fn require_sysmlv2_client(
    state: &AppState,
) -> Result<Arc<kr0ki_sysmlv2_client::SysmlV2Client>, Response> {
    state.sysmlv2_client.clone().ok_or_else(|| {
        error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "sysmlv2_client_not_configured",
            "KR0KI_SYSMLV2_BASE_URL is not set on this server",
        )
    })
}

fn client_error_response(e: kr0ki_sysmlv2_client::ClientError) -> Response {
    error_json(StatusCode::BAD_GATEWAY, "sysmlv2_upstream_error", &e.to_string())
}

/// `GET /model/projects` — storyb00k design, 2026-09-17.
async fn list_model_projects(State(state): State<AppState>) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match client.projects().await {
        Ok(projects) => Json(projects).into_response(),
        Err(e) => client_error_response(e),
    }
}

/// `GET /model/projects/:project_id/commits`.
async fn list_model_commits(State(state): State<AppState>, Path(project_id): Path<String>) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match client.commits(&project_id).await {
        Ok(commits) => Json(commits).into_response(),
        Err(e) => client_error_response(e),
    }
}

/// `GET /model/projects/:project_id/commits/:commit_id/snapshot` — also materializes
/// every returned element into `state.model_graph` (Task 2).
async fn get_model_snapshot(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match client.snapshot(&project_id, &commit_id).await {
        Ok(snapshot) => {
            for element in &snapshot.elements {
                state
                    .model_graph
                    .insert_element_triples(&project_id, &commit_id, element);
            }
            Json(snapshot).into_response()
        }
        Err(e) => client_error_response(e),
    }
}

/// `GET /model/projects/:project_id/commits/:commit_id/elements` — also materializes
/// every returned element into `state.model_graph`.
async fn query_model_elements(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match client.all_elements(&project_id, &commit_id).await {
        Ok(elements) => {
            for element in &elements {
                state
                    .model_graph
                    .insert_element_triples(&project_id, &commit_id, element);
            }
            Json(elements).into_response()
        }
        Err(e) => client_error_response(e),
    }
}

/// `GET /model/projects/:project_id/commits/:commit_id/roots`.
async fn get_model_roots(
    State(state): State<AppState>,
    Path((project_id, commit_id)): Path<(String, String)>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match client.roots(&project_id, &commit_id).await {
        Ok(roots) => Json(roots).into_response(),
        Err(e) => client_error_response(e),
    }
}

/// `GET /model/projects/:project_id/commits/:commit_id/elements/:element_id/relationships?direction=in|out|both`
/// — also materializes each relationship as a graph-store edge (Task 2).
async fn query_model_relationships(
    State(state): State<AppState>,
    Path((project_id, commit_id, element_id)): Path<(String, String, String)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let client = match require_sysmlv2_client(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let direction = match params.get("direction").map(String::as_str) {
        Some("in") => kr0ki_sysmlv2_client::Direction::In,
        Some("out") => kr0ki_sysmlv2_client::Direction::Out,
        _ => kr0ki_sysmlv2_client::Direction::Both,
    };
    match client
        .relationships(&project_id, &commit_id, &element_id, direction)
        .await
    {
        Ok(related) => {
            for element in &related {
                state.model_graph.insert_relationship_triple(
                    &element_id,
                    element.ty(),
                    element.id(),
                );
            }
            Json(related).into_response()
        }
        Err(e) => client_error_response(e),
    }
}
```

In `crates/kr0ki-server/src/main.rs`, add env wiring following the exact pattern
`KR0KI_KUBEDIAGRAM_WORKER_URL` already uses: read `KR0KI_SYSMLV2_BASE_URL` (`.ok()`),
build `Some(Arc::new(SysmlV2Client::new(base_url).with_token(...)))` when set (chaining
`.with_token()` only if `KR0KI_SYSMLV2_TOKEN` is also set), add both to the doc-comment
env-var table, the tracing `info!` line, and the `AppState` literal (with a fresh
`Arc::new(kr0ki_core::graph_store::GraphStore::new())` for `model_graph`).

- [ ] **Step 4: Update the two other `AppState` literals**

In `crates/kr0ki-server/tests/b00t_graph_live.rs` and
`crates/kr0ki-server/tests/kubediagram_live.rs`, add `sysmlv2_client: None,` and
`model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),` to each file's one
`AppState` literal.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p kr0ki-server`
Expected: PASS (all pre-existing tests plus the 3 new ones).

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/kr0ki-server/Cargo.toml crates/kr0ki-server/src/app.rs \
        crates/kr0ki-server/src/main.rs crates/kr0ki-server/tests/http.rs \
        crates/kr0ki-server/tests/b00t_graph_live.rs \
        crates/kr0ki-server/tests/kubediagram_live.rs Cargo.lock
git commit -m "feat: add six /model/* routes wrapping kr0ki-sysmlv2-client, materializing into graph_store"
```

---

### Task 4: `GET /model/graph/query` — the bounded graph-query route

**Files:**
- Modify: `crates/kr0ki-server/src/app.rs` (route + handler)
- Modify: `crates/kr0ki-server/src/app.rs` doc-comment on `mcp_tools()` if it lists
  route counts (check; update if so)
- Modify: `crates/kr0ki-server/tests/http.rs` (new tests, and the existing
  `mcp_tools_lists_all_three_tools_with_bindings` test needs renaming/updating for the
  new count of 10 — see Step 3)

**Interfaces:**
- Consumes: `AppState.model_graph` (Task 3), `kr0ki_core::graph_store::QueryShape`
  (Task 2).
- Produces: nothing new consumed by later tasks (a leaf route, like `/mcp/tools`
  itself).

- [ ] **Step 1: Write the failing tests**

In `crates/kr0ki-server/tests/http.rs`:

```rust
#[tokio::test]
async fn model_graph_query_returns_triples_about_a_subject() {
    let state = test_state("model-graph-query");
    state
        .model_graph
        .insert_element_triples("proj-1", "c1", &{
            let json = serde_json::json!({"@id": "elem-1", "@type": "PartUsage", "name": "Engine"});
            serde_json::from_value(json).unwrap()
        });
    let app = test_app(state);
    let resp = app
        .oneshot(
            Request::get("/model/graph/query?shape=triples_about&subject=elem-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Engine"));
}

#[tokio::test]
async fn model_graph_query_rejects_an_unknown_shape() {
    let app = test_app(test_state("model-graph-badshape"));
    let resp = app
        .oneshot(
            Request::get("/model/graph/query?shape=not_a_real_shape&subject=elem-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_shape"));
}
```

Also update the pre-existing `mcp_tools_lists_all_three_tools_with_bindings` test
(rename to `mcp_tools_lists_all_ten_tools_with_bindings`, change
`assert_eq!(tools.len(), 3)` to `assert_eq!(tools.len(), 10)`, add a
`names.contains(&"query_model_graph")` assertion alongside the existing name checks).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kr0ki-server --test http model_graph`
Expected: FAIL — 404 (no such route yet); the renamed mcp_tools test fails on the
count assertion.

- [ ] **Step 3: Write the implementation**

Add the route (alongside the other `/model/*` routes from Task 3):

```rust
        .route("/model/graph/query", get(query_model_graph))
```

Add the handler:

```rust
/// `GET /model/graph/query?shape=triples_about|related_via&subject=<id>` —
/// storyb00k design, 2026-09-17 §10a. Bounded query shapes over
/// `AppState.model_graph`, never SPARQL.
async fn query_model_graph(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let shape = match params.get("shape").map(String::as_str) {
        Some("triples_about") => kr0ki_core::graph_store::QueryShape::TriplesAbout,
        Some("related_via") => kr0ki_core::graph_store::QueryShape::RelatedVia,
        _ => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "invalid_shape",
                "shape must be triples_about or related_via",
            )
        }
    };
    let subject = params.get("subject").map(String::as_str);
    let triples = state.model_graph.query(shape, subject);
    Json(
        triples
            .into_iter()
            .map(|(s, p, o)| serde_json::json!({"subject": s, "predicate": p, "object": o}))
            .collect::<Vec<_>>(),
    )
    .into_response()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kr0ki-server --test http`
Expected: PASS (all tests, including the 2 new and the updated mcp_tools test).

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/kr0ki-server/src/app.rs crates/kr0ki-server/tests/http.rs
git commit -m "feat: add GET /model/graph/query, the bounded graph-query route"
```

---

### Task 5: extract `manifest_dispatch.py` from `bridge.py`

**Files:**
- Create: `containers/kr0ki-mcp/manifest_dispatch.py`
- Modify: `containers/kr0ki-mcp/bridge.py` (import from the new module instead of
  defining the same functions inline)
- Modify: `containers/kr0ki-mcp/test_bridge.py` (imports adjust; behavior must not
  change — this is a pure refactor)

**Interfaces:**
- Produces: `manifest_dispatch.py`'s `fetch_manifest(base_url, cache)`,
  `find_tool(manifest, name)`, `apply_binding(tool, arguments)`, `http_call(method,
  url, data=None)` — Task 7's sidecar imports these directly (same module, new
  consumer).

- [ ] **Step 1: Confirm current behavior is captured by existing tests**

Run: `cd containers/kr0ki-mcp && python3 test_bridge.py -v`
Expected: PASS (all 6 tests, pre-existing — this is the regression baseline Step 4
must still show green).

- [ ] **Step 2: Extract the module**

Create `containers/kr0ki-mcp/manifest_dispatch.py` containing exactly the
`http_call`, `fetch_manifest`, `find_tool`, and `apply_binding` functions currently
defined in `bridge.py` (copy their current bodies verbatim — this is a pure
extraction, not a rewrite). Module docstring:

```python
"""Generic MCP-tools/manifest-dispatch primitives, shared by containers/kr0ki-mcp/
bridge.py (stdio MCP) and containers/kr0ki-storyb00k-agent (AG-UI/HTTP) — storyb00k
design, 2026-09-17 §4/§7. Fetches kr0ki-server's GET /mcp/tools manifest once and maps
a tools/call's arguments onto an HTTP request per the tool's advertised httpBinding.
"""
```

Keep `_manifest_cache` as a module-level global in `manifest_dispatch.py` (moved from
`bridge.py`), and change `fetch_manifest()`'s signature to accept `base_url` as a
parameter rather than reading a `bridge.py`-local `BASE_URL` constant, so both callers
can pass their own configured URL:

```python
def fetch_manifest(base_url):
    global _manifest_cache
    if _manifest_cache is None:
        _, body = http_call("GET", f"{base_url}/mcp/tools")
        _manifest_cache = json.loads(body)
    return _manifest_cache
```

- [ ] **Step 3: Update `bridge.py` to import from the new module**

In `containers/kr0ki-mcp/bridge.py`, remove the now-duplicated function bodies and add:

```python
from manifest_dispatch import http_call, fetch_manifest, find_tool, apply_binding
```

Update every call site that previously called `fetch_manifest()` with no arguments to
`fetch_manifest(BASE_URL)` (passing `bridge.py`'s own `BASE_URL` module constant,
unchanged). `bridge.py`'s own `_manifest_cache` global and `MAX_INPUT_BYTES` check stay
exactly as they are — only the four extracted functions move.

- [ ] **Step 4: Run bridge.py's test suite to confirm no regression**

Run: `cd containers/kr0ki-mcp && python3 test_bridge.py -v`
Expected: PASS (all 6 tests, identical to Step 1's baseline — if any test needed a
source change beyond adjusting the `bridge.fetch_manifest` mock target to account for
the new signature, note it explicitly in your report; the test *behavior* must not
change).

- [ ] **Step 5: Commit**

```bash
git add containers/kr0ki-mcp/manifest_dispatch.py containers/kr0ki-mcp/bridge.py \
        containers/kr0ki-mcp/test_bridge.py
git commit -m "refactor: extract manifest_dispatch.py from bridge.py for reuse by storyb00k's sidecar"
```

---

### Task 6: `kr0ki-storyb00k-agent` sidecar — AG-UI SSE server + generic LLM client + skills bundle

**Files:**
- Create: `containers/kr0ki-storyb00k-agent/server.py`
- Create: `containers/kr0ki-storyb00k-agent/llm_client.py`
- Create: `containers/kr0ki-storyb00k-agent/skills/magicgrid.md`
- Create: `containers/kr0ki-storyb00k-agent/skills/requirements_quality.md`
- Create: `containers/kr0ki-storyb00k-agent/test_server.py`
- Create: `containers/kr0ki-storyb00k-agent/test_llm_client.py`

**Interfaces:**
- Consumes: `manifest_dispatch.{fetch_manifest, find_tool, apply_binding, http_call}`
  (Task 5) — this container's `Containerfile` (Task 9) copies
  `containers/kr0ki-mcp/manifest_dispatch.py` alongside this container's own files.
- Produces: an HTTP service on `0.0.0.0:{KR0KI_STORYB00K_PORT:-8789}` with `GET
  /health` and `POST /run` (AG-UI protocol: request body is `RunAgentInput`, response
  is an SSE event stream). Task 8 extends this same server with the draft-mutation
  tool/interrupt flow. Task 10's pod manifest wires this port.

- [ ] **Step 1: Write the failing tests**

Create `containers/kr0ki-storyb00k-agent/test_llm_client.py`:

```python
#!/usr/bin/env python3
"""Unit tests for llm_client.py's generic OpenAI-compatible client."""
import json
import os
import unittest
from unittest.mock import patch

import llm_client


class LlmClientTest(unittest.TestCase):
    def test_client_reads_api_key_and_url_from_env(self):
        with patch.dict(
            os.environ,
            {"OPENAI_API_KEY": "sk-test", "OPENAI_API_URL": "http://example.invalid/v1"},
        ):
            client = llm_client.OpenAICompatibleClient.from_env()
            self.assertEqual(client.api_key, "sk-test")
            self.assertEqual(client.base_url, "http://example.invalid/v1")

    def test_missing_env_vars_raises_a_clear_error(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(llm_client.ConfigError):
                llm_client.OpenAICompatibleClient.from_env()

    @patch("llm_client.http_call")
    def test_chat_completion_posts_messages_and_tools_to_chat_completions_endpoint(
        self, mock_http_call
    ):
        mock_http_call.return_value = (
            "application/json",
            json.dumps(
                {"choices": [{"message": {"role": "assistant", "content": "hello"}}]}
            ).encode(),
        )
        client = llm_client.OpenAICompatibleClient(
            api_key="sk-test", base_url="http://example.invalid/v1"
        )
        result = client.chat_completion(
            messages=[{"role": "user", "content": "hi"}], tools=[]
        )
        method, url, body = mock_http_call.call_args[0]
        self.assertEqual(method, "POST")
        self.assertEqual(url, "http://example.invalid/v1/chat/completions")
        payload = json.loads(body)
        self.assertEqual(payload["messages"], [{"role": "user", "content": "hi"}])
        self.assertEqual(result["choices"][0]["message"]["content"], "hello")


if __name__ == "__main__":
    unittest.main()
```

Create `containers/kr0ki-storyb00k-agent/test_server.py`:

```python
#!/usr/bin/env python3
"""Unit tests for server.py's AG-UI SSE endpoint."""
import json
import threading
import unittest
from unittest.mock import patch

import http.client

import server


class ServerTest(unittest.TestCase):
    def setUp(self):
        self.httpd = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        self.port = self.httpd.server_address[1]
        self.thread = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.httpd.shutdown()
        self.httpd.server_close()

    def _conn(self):
        return http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)

    def test_health_returns_ok(self):
        conn = self._conn()
        conn.request("GET", "/health")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        self.assertEqual(json.loads(resp.read())["status"], "ok")

    @patch("server.load_skills")
    @patch("server.fetch_manifest")
    @patch("server.llm_client.OpenAICompatibleClient.from_env")
    def test_run_emits_run_started_and_run_finished_events(
        self, mock_client_factory, mock_fetch_manifest, mock_load_skills
    ):
        mock_load_skills.return_value = {}
        mock_fetch_manifest.return_value = []
        mock_client = mock_client_factory.return_value
        mock_client.chat_completion.return_value = {
            "choices": [{"message": {"role": "assistant", "content": "Hello!", "tool_calls": []}}]
        }
        conn = self._conn()
        body = json.dumps(
            {"threadId": "t1", "runId": "r1", "messages": [{"role": "user", "content": "hi"}]}
        ).encode()
        conn.request("POST", "/run", body=body)
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        raw = resp.read().decode()
        self.assertIn('"type": "RUN_STARTED"', raw)
        self.assertIn('"type": "RUN_FINISHED"', raw)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd containers/kr0ki-storyb00k-agent && python3 test_llm_client.py && python3 test_server.py`
Expected: FAIL — `ModuleNotFoundError` for both `llm_client` and `server`.

- [ ] **Step 3: Write the implementation**

Create `containers/kr0ki-storyb00k-agent/llm_client.py`:

```python
#!/usr/bin/env python3
"""A generic OpenAI-compatible chat-completions client. Configured entirely via
OPENAI_API_KEY / OPENAI_API_URL — storyb00k design, 2026-09-17 §4. No hardcoded
endpoint anywhere in this file; the local ch0nky llama.cpp server is one valid
configuration among many, set in this pod's own .env, not in this code.
"""

import json
import os

from manifest_dispatch import http_call


class ConfigError(Exception):
    pass


class OpenAICompatibleClient:
    def __init__(self, api_key, base_url):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")

    @classmethod
    def from_env(cls):
        api_key = os.environ.get("OPENAI_API_KEY")
        base_url = os.environ.get("OPENAI_API_URL")
        if not api_key or not base_url:
            raise ConfigError(
                "OPENAI_API_KEY and OPENAI_API_URL must both be set — no default endpoint"
            )
        return cls(api_key=api_key, base_url=base_url)

    def chat_completion(self, messages, tools):
        payload = {"model": "default", "messages": messages}
        if tools:
            payload["tools"] = tools
        _, body = http_call(
            "POST",
            f"{self.base_url}/chat/completions",
            json.dumps(payload).encode("utf-8"),
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
        )
        return json.loads(body)
```

(This calls `http_call` with the `headers` keyword argument — see below for the small,
backward-compatible extension `manifest_dispatch.http_call` needs to accept it, which
must land before this code will actually work.)

**`http_call` needs one small, backward-compatible extension first** —
`manifest_dispatch.http_call` (Task 5's extraction) sends a fixed `Content-Type:
text/plain` and no `Authorization` header, which won't work against a real
OpenAI-compatible endpoint. Before writing `llm_client.py` above, make this change to
`containers/kr0ki-mcp/manifest_dispatch.py`:

Change:

```python
def http_call(method, url, data=None):
    headers = {"Accept": "application/json"}
    if AUTH_TOKEN:
        headers["Authorization"] = f"Bearer {AUTH_TOKEN}"
    if data is not None:
        headers["Content-Type"] = "text/plain; charset=utf-8"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as result:
        return result.headers.get_content_type(), result.read()
```

to:

```python
def http_call(method, url, data=None, headers=None):
    merged_headers = {"Accept": "application/json"}
    if AUTH_TOKEN:
        merged_headers["Authorization"] = f"Bearer {AUTH_TOKEN}"
    if data is not None:
        merged_headers["Content-Type"] = "text/plain; charset=utf-8"
    if headers:
        merged_headers.update(headers)
    req = urllib.request.Request(url, data=data, headers=merged_headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as result:
        return result.headers.get_content_type(), result.read()
```

(`headers`, when passed, overrides both the default `Authorization` and
`Content-Type` — exactly what `llm_client.py`'s `chat_completion` needs to send its
own `Authorization: Bearer {api_key}` and `Content-Type: application/json` instead of
`bridge.py`'s `AUTH_TOKEN`-based default.)

Add one test to `containers/kr0ki-mcp/test_bridge.py`, confirming the extension is
additive and doesn't change any existing caller's behavior:

```python
    @patch("bridge.urllib.request.urlopen")
    def test_http_call_headers_param_overrides_defaults_without_changing_existing_callers(
        self, mock_urlopen
    ):
        mock_urlopen.return_value.__enter__.return_value.headers.get_content_type.return_value = (
            "application/json"
        )
        mock_urlopen.return_value.__enter__.return_value.read.return_value = b"{}"
        bridge.http_call("POST", "http://example.invalid", b"x", headers={"Authorization": "Bearer custom"})
        sent_request = mock_urlopen.call_args[0][0]
        self.assertEqual(sent_request.get_header("Authorization"), "Bearer custom")
        # An existing no-headers call still works exactly as before.
        bridge.http_call("GET", "http://example.invalid")
```

Run `cd containers/kr0ki-mcp && python3 test_bridge.py -v` — expect 7/7 pass (6
pre-existing + this new one) before proceeding to `llm_client.py`.

`chat_completion` (in the `llm_client.py` code above) already passes
`headers={"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"}`
via `http_call`'s new parameter — no further change needed there once this extension
lands.

Create `containers/kr0ki-storyb00k-agent/skills/magicgrid.md`:

```markdown
# MagicGrid narrative scaffold

When narrating a whole-system walkthrough, organize your explanation around these
five areas, in this order, skipping any that have no model content rather than
inventing content to fill them:

1. **Stakeholders** — who cares about this system and why.
2. **Requirements** — what the system must do or be.
3. **System Context** — how this system relates to its environment/external systems.
4. **Logical Architecture** — the system's internal structure and interfaces.
5. **Physical Architecture** — how the logical architecture is realized.

Concept credit: `AI4MBSE/MBSE-AI-SysML-V2`'s MagicGrid convention (concepts only — no
dependency on Cameo/MagicDraw anywhere in this system).
```

Create `containers/kr0ki-storyb00k-agent/skills/requirements_quality.md`:

```markdown
# Requirements quality rubric (INCOSE/EARS)

When narrating or discussing a requirement found in the model, note (without
inventing values not present in the model) whether it appears to:

- be quantified (a number/threshold), not vague ("fast" vs. "responds within 200ms")
- avoid baking in a design/implementation constraint that belongs elsewhere
- have a clear trigger condition, if it's a behavioral requirement (EARS "When X, the
  system shall Y" shape)

Never fabricate a quality judgment as if it were model data — this rubric guides your
narration's framing, it is not itself a query result. Concept credit:
`AI4MBSE/MBSE-AI-SysML-V2`'s requirements-writing lab material (concepts only).
```

Create `containers/kr0ki-storyb00k-agent/server.py`:

```python
#!/usr/bin/env python3
"""AG-UI protocol SSE server for storyb00k's agent — storyb00k design, 2026-09-17
§4/§6. Fetches kr0ki-server's GET /mcp/tools manifest and this container's own bundled
skill-prompt fragments once at session start, dispatches every tool call generically
via manifest_dispatch (shared with containers/kr0ki-mcp/bridge.py), and streams AG-UI
events (RunStarted, TextMessage*, ToolCall*, StateDelta, RunFinished) over SSE.
"""

import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import llm_client
from manifest_dispatch import apply_binding, fetch_manifest, find_tool, http_call

KR0KI_URL = os.environ.get("KR0KI_URL", "http://127.0.0.1:8787")
SKILLS_DIR = Path(__file__).parent / "skills"


def load_skills():
    return {p.stem: p.read_text() for p in SKILLS_DIR.glob("*.md")}


def sse_event(event: dict) -> bytes:
    return f"data: {json.dumps(event)}\n\n".encode("utf-8")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def do_GET(self):
        if self.path == "/health":
            body = json.dumps({"status": "ok", "service": "kr0ki-storyb00k-agent"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        if self.path != "/run":
            self.send_response(404)
            self.end_headers()
            return

        length = int(self.headers.get("Content-Length", 0))
        run_input = json.loads(self.rfile.read(length)) if length else {}
        run_id = run_input.get("runId", "unknown")
        messages = run_input.get("messages", [])

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()

        self.wfile.write(sse_event({"type": "RUN_STARTED", "runId": run_id}))

        manifest = fetch_manifest(KR0KI_URL)
        skills = load_skills()
        system_prompt = "\n\n".join(skills.values())
        client = llm_client.OpenAICompatibleClient.from_env()

        tools = [
            {
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["inputSchema"],
                },
            }
            for t in manifest
        ]
        chat_messages = [{"role": "system", "content": system_prompt}] + messages
        result = client.chat_completion(messages=chat_messages, tools=tools)
        message = result["choices"][0]["message"]

        for tool_call in message.get("tool_calls") or []:
            tool_name = tool_call["function"]["name"]
            tool_args = json.loads(tool_call["function"]["arguments"])
            tool = find_tool(manifest, tool_name)
            if tool is None:
                continue
            method, url, body = apply_binding(tool, tool_args)
            _, response_body = http_call(method, url, body)
            self.wfile.write(
                sse_event(
                    {
                        "type": "STATE_DELTA",
                        "runId": run_id,
                        "delta": [
                            {
                                "op": "add",
                                "path": "/panels/-",
                                "value": {
                                    "kind": "render" if "render" in tool_name else "query-result",
                                    "toolName": tool_name,
                                    "content": response_body.decode("utf-8", "replace"),
                                },
                            }
                        ],
                    }
                )
            )

        if message.get("content"):
            self.wfile.write(
                sse_event(
                    {
                        "type": "TEXT_MESSAGE_CONTENT",
                        "runId": run_id,
                        "role": "assistant",
                        "content": message["content"],
                    }
                )
            )

        self.wfile.write(sse_event({"type": "RUN_FINISHED", "runId": run_id}))


def main():
    port = int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))
    httpd = ThreadingHTTPServer(("0.0.0.0", port), Handler)
    httpd.serve_forever()


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd containers/kr0ki-storyb00k-agent && python3 test_llm_client.py -v && python3 test_server.py -v`
Expected: PASS (3 + 2 tests). Note: this does NOT require a real LLM endpoint or a
real kr0ki-server — `fetch_manifest`, `load_skills`, and the LLM client are all mocked
in `test_server.py`'s tool-call test.

- [ ] **Step 5: Commit**

```bash
git add containers/kr0ki-storyb00k-agent/ containers/kr0ki-mcp/manifest_dispatch.py \
        containers/kr0ki-mcp/bridge.py
git commit -m "feat: add kr0ki-storyb00k-agent sidecar (AG-UI SSE server, generic OpenAI client, bundled skills)"
```

---

### Task 7: draft-mutation flow — `propose_draft_change`, AG-UI interrupts, `rdflib` session graph

**Files:**
- Create: `containers/kr0ki-storyb00k-agent/draft_graph.py`
- Modify: `containers/kr0ki-storyb00k-agent/server.py` (interrupt emission,
  `/respond-to-interrupt` endpoint, per-session `draft_graph.DraftGraph` instance)
- Create: `containers/kr0ki-storyb00k-agent/test_draft_graph.py`
- Modify: `containers/kr0ki-storyb00k-agent/test_server.py` (interrupt-flow test)
- Modify: `containers/kr0ki-storyb00k-agent/requirements.txt` (create if absent; add
  `rdflib`)

**Interfaces:**
- Produces: `draft_graph.DraftGraph` (`propose(subject, predicate, object) ->
  proposal_id`, `apply(proposal_id)`, `decline(proposal_id)`, `as_turtle()`) — a
  session-scoped, in-memory-only wrapper over an `rdflib.Graph`. Never written to
  `kr0ki-server`, Flexo, or any persistent store — see design spec §10a.

- [ ] **Step 1: Write the failing tests**

Create `containers/kr0ki-storyb00k-agent/test_draft_graph.py`:

```python
#!/usr/bin/env python3
"""Unit tests for draft_graph.py — the session-scoped, disposable rdflib draft."""
import unittest

from draft_graph import DraftGraph


class DraftGraphTest(unittest.TestCase):
    def test_a_proposed_change_is_not_applied_until_apply_is_called(self):
        draft = DraftGraph()
        proposal_id = draft.propose("elem-1", "name", "Engine v2")
        self.assertNotIn("Engine v2", draft.as_turtle())
        draft.apply(proposal_id)
        self.assertIn("Engine v2", draft.as_turtle())

    def test_a_declined_proposal_never_appears_in_the_graph(self):
        draft = DraftGraph()
        proposal_id = draft.propose("elem-1", "name", "Engine v2")
        draft.decline(proposal_id)
        self.assertNotIn("Engine v2", draft.as_turtle())

    def test_applying_an_unknown_proposal_id_raises(self):
        draft = DraftGraph()
        with self.assertRaises(KeyError):
            draft.apply("does-not-exist")

    def test_pending_proposals_lists_only_unresolved_ones(self):
        draft = DraftGraph()
        p1 = draft.propose("elem-1", "name", "Engine v2")
        p2 = draft.propose("elem-2", "name", "Wing")
        draft.apply(p1)
        pending = draft.pending_proposals()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0]["id"], p2)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd containers/kr0ki-storyb00k-agent && python3 test_draft_graph.py`
Expected: FAIL — `ModuleNotFoundError: No module named 'draft_graph'` (and likely
`rdflib` itself not yet installed — install it first: `pip install rdflib` in your
local dev environment, or note it needs adding to the container build in Task 9).

- [ ] **Step 3: Write the implementation**

Create `containers/kr0ki-storyb00k-agent/draft_graph.py`:

```python
#!/usr/bin/env python3
"""A session-scoped, in-memory-only draft graph. NEVER written to kr0ki-server's own
graph_store, never to Flexo, never to the authoritative model — storyb00k design,
2026-09-17 §10a. Gone when the process/session ends. Uses rdflib, the standard Python
RDF library — the Python-side analog of the Rust side's oxttl/oxrdf, not a new pattern.
"""

import uuid

import rdflib

KR0KI_NS = rdflib.Namespace("http://kr0ki.promptexecution.com/ontology#")


class DraftGraph:
    def __init__(self):
        self._graph = rdflib.Graph()
        self._pending = {}

    def propose(self, subject: str, predicate: str, obj: str) -> str:
        proposal_id = str(uuid.uuid4())
        self._pending[proposal_id] = (subject, predicate, obj)
        return proposal_id

    def apply(self, proposal_id: str) -> None:
        subject, predicate, obj = self._pending.pop(proposal_id)
        self._graph.add((KR0KI_NS[subject], KR0KI_NS[predicate], rdflib.Literal(obj)))

    def decline(self, proposal_id: str) -> None:
        self._pending.pop(proposal_id)

    def pending_proposals(self):
        return [
            {"id": pid, "subject": s, "predicate": p, "object": o}
            for pid, (s, p, o) in self._pending.items()
        ]

    def as_turtle(self) -> str:
        return self._graph.serialize(format="turtle")
```

In `containers/kr0ki-storyb00k-agent/server.py`, add a new `propose_draft_change`
handling path in `do_POST`'s `/run` handler: when the LLM's tool call is for
`propose_draft_change` (a tool name that does NOT come from kr0ki's `/mcp/tools`
manifest — it's local to this sidecar, never dispatched via `apply_binding`), call
`self.draft.propose(...)` and emit an AG-UI interrupt event instead of a normal
tool-call result:

```python
            if tool_name == "propose_draft_change":
                proposal_id = self.draft.propose(
                    tool_args["subject"], tool_args["predicate"], tool_args["object"]
                )
                self.wfile.write(
                    sse_event(
                        {
                            "type": "RUN_FINISHED",
                            "runId": run_id,
                            "interrupt": {
                                "type": "interrupt",
                                "interrupts": [
                                    {
                                        "id": proposal_id,
                                        "reason": f"Apply {tool_args['predicate']}={tool_args['object']!r} to {tool_args['subject']}?",
                                        "responseSchema": {
                                            "type": "object",
                                            "properties": {"approved": {"type": "boolean"}},
                                        },
                                    }
                                ],
                            },
                        }
                    )
                )
                return
```

Add a `draft` attribute (a per-connection `DraftGraph()` instance — session-scoping
across reconnects is explicitly out of scope for this task; one `DraftGraph` per
`Handler` instance is sufficient for the single-tab, single-session Phase 1 posture
already established in the design spec §8) and a new `/respond-to-interrupt` POST
endpoint in `do_POST`:

```python
        if self.path == "/respond-to-interrupt":
            length = int(self.headers.get("Content-Length", 0))
            payload = json.loads(self.rfile.read(length))
            proposal_id = payload["interruptId"]
            if payload.get("approved"):
                self.draft.apply(proposal_id)
            else:
                self.draft.decline(proposal_id)
            body = json.dumps({"status": "ok"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
```

(Placed as an early branch in `do_POST`, before the existing `/run` handling, and
initialize `self.draft = DraftGraph()` — note `BaseHTTPRequestHandler` creates a new
handler instance per connection by default with `ThreadingHTTPServer`, matching the
per-session scoping this task requires; add `from draft_graph import DraftGraph` to
the imports.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd containers/kr0ki-storyb00k-agent && python3 test_draft_graph.py -v && python3 test_server.py -v`
Expected: PASS (4 new + 5 pre-existing).

- [ ] **Step 5: Commit**

```bash
echo "rdflib" >> containers/kr0ki-storyb00k-agent/requirements.txt
git add containers/kr0ki-storyb00k-agent/
git commit -m "feat: add gated draft-mutation flow (propose_draft_change, AG-UI interrupts, rdflib session graph)"
```

---

### Task 8: `playbook/` — the storyb00k panel

**Files:**
- Create: `playbook/src/components/StoryB00k.vue`
- Create: `playbook/src/components/StoryB00kPanel.vue` (one dashboard panel, the four
  `kind`s)
- Modify: `playbook/src/App.vue` (add a route/nav entry to the new panel — check the
  existing routing approach in `App.vue` first, e.g. a simple `v-if`-switched view vs.
  vue-router, and match it; not decided here)
- Modify: `playbook/package.json` (add `ag-ui-vue` and `@ag-ui/client` dependencies)

**Interfaces:**
- Consumes: the sidecar's `/run` (AG-UI SSE) and `/respond-to-interrupt` endpoints
  (Task 6-7), reached at a base URL supplied via a `playbook/`-level env/config value
  (exact mechanism — build-time `.env` vs. runtime-injected — not decided here, match
  whatever convention `playbook/`'s existing API calls already use).

- [ ] **Step 1: Write the failing test**

Check `playbook/`'s existing test setup first (its `.story.vue` files suggest Histoire
— confirm whether there's also a unit-test runner, e.g. Vitest, configured in
`playbook/package.json`; if none exists, add Vitest following the minimal
"don't reinvent" path — a single new dev dependency, not a new framework choice
requiring its own design). Create `playbook/src/components/StoryB00kPanel.test.js` (or
`.spec.js`, matching whatever convention the chosen runner expects):

```javascript
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import StoryB00kPanel from './StoryB00kPanel.vue'

describe('StoryB00kPanel', () => {
  it('labels a query-result panel distinctly from a narration panel', () => {
    const queryPanel = mount(StoryB00kPanel, {
      props: { panel: { kind: 'query-result', toolName: 'query_model_elements', content: '[]' } },
    })
    expect(queryPanel.text()).toContain('Model data')

    const narrationPanel = mount(StoryB00kPanel, {
      props: { panel: { kind: 'narration', content: 'This system has one engine.' } },
    })
    expect(narrationPanel.text()).toContain('Agent narration')
    expect(narrationPanel.text()).not.toContain('Model data')
  })

  it('shows both rendered SVG and source text for a render-kind panel when both are present', () => {
    const panel = mount(StoryB00kPanel, {
      props: {
        panel: {
          kind: 'render',
          content: '<svg>...</svg>',
          source: { text: 'a -> b', format: 'd2' },
        },
      },
    })
    expect(panel.find('[data-testid="rendered-output"]').exists()).toBe(true)
    expect(panel.find('[data-testid="source-text"]').text()).toContain('a -> b')
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd playbook && npx vitest run src/components/StoryB00kPanel.test.js`
Expected: FAIL — `StoryB00kPanel.vue` doesn't exist yet.

- [ ] **Step 3: Write the implementation**

Create `playbook/src/components/StoryB00kPanel.vue`:

```vue
<script setup>
defineProps({
  panel: {
    type: Object,
    required: true,
  },
})

const kindLabel = {
  'query-result': 'Model data',
  render: 'Rendered diagram',
  'sysml-text': 'SysML v2 source',
  narration: 'Agent narration',
}
</script>

<template>
  <div class="storyb00k-panel" :data-kind="panel.kind">
    <span class="storyb00k-panel__label">{{ kindLabel[panel.kind] }}</span>
    <div v-if="panel.kind === 'render'" class="storyb00k-panel__render">
      <div data-testid="rendered-output" v-html="panel.content"></div>
      <pre v-if="panel.source" data-testid="source-text">{{ panel.source.text }}</pre>
    </div>
    <pre v-else-if="panel.kind === 'query-result'">{{ panel.content }}</pre>
    <p v-else>{{ panel.content }}</p>
  </div>
</template>

<style scoped>
.storyb00k-panel {
  border: 1px solid #ccc;
  border-radius: 4px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}
.storyb00k-panel__label {
  font-weight: bold;
  font-size: 0.8rem;
  text-transform: uppercase;
  opacity: 0.7;
}
.storyb00k-panel[data-kind='narration'] {
  background: #f0f4ff;
}
.storyb00k-panel[data-kind='query-result'] {
  background: #f4fff0;
}
</style>
```

Create `playbook/src/components/StoryB00k.vue` (the panel container — uses
`ag-ui-vue`'s `useChat`/`useAgentState`/`useFrontendTool` composables; exact composable
call shape to be verified against the installed `ag-ui-vue` package's actual
TypeScript definitions during implementation, since this plan's knowledge of its exact
API comes from a research summary, not the raw source):

```vue
<script setup>
import { ref } from 'vue'
import { useChat, useAgentState, useFrontendTool } from 'ag-ui-vue'
import StoryB00kPanel from './StoryB00kPanel.vue'

const agentUrl = import.meta.env.VITE_STORYB00K_AGENT_URL || 'http://localhost:8789'

const { messages, input, sendMessage } = useChat({ agentUrl: `${agentUrl}/run` })
const { state: dashboardState } = useAgentState({ agentUrl: `${agentUrl}/run`, initial: { panels: [] } })

useFrontendTool({
  name: 'propose_draft_change',
  requireConfirmation: true,
  onCall: async ({ interruptId, approved }) => {
    await fetch(`${agentUrl}/respond-to-interrupt`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ interruptId, approved }),
    })
  },
})
</script>

<template>
  <div class="storyb00k">
    <section class="storyb00k__transcript">
      <div v-for="(m, i) in messages" :key="i">{{ m.role }}: {{ m.content }}</div>
      <input v-model="input" @keyup.enter="sendMessage" placeholder="Ask about this system..." />
    </section>
    <section class="storyb00k__dashboard">
      <StoryB00kPanel v-for="(panel, i) in dashboardState.panels" :key="i" :panel="panel" />
    </section>
  </div>
</template>
```

Add a nav entry to `StoryB00k.vue` in `playbook/src/App.vue`, matching whatever
routing/view-switching convention `App.vue` already uses for `Gallery.vue` and
`RendererPanel.vue` (read `App.vue` first; this plan does not assume its exact shape).

In `playbook/package.json`, add `ag-ui-vue` and `@ag-ui/client` to `dependencies`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd playbook && npx vitest run src/components/StoryB00kPanel.test.js`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
cd playbook && npm install
git add playbook/src/components/StoryB00k.vue playbook/src/components/StoryB00kPanel.vue \
        playbook/src/components/StoryB00kPanel.test.js playbook/src/App.vue \
        playbook/package.json playbook/package-lock.json
git commit -m "feat: add storyb00k panel to playbook/ (ag-ui-vue integration, 4-kind dashboard)"
```

---

### Task 9: deploy — sidecar Containerfile + pod manifest

**Files:**
- Create: `containers/kr0ki-storyb00k-agent/Containerfile`
- Modify: `deploy/kr0ki-local.pod.yaml`
- Modify: `justfile` (`pod-build`/`k0s-load` need the new image)

**Interfaces:**
- Consumes: nothing new (packages Tasks 6-7's Python files).
- Produces: a fifth pod container, `kr0ki-storyb00k-agent`, on port
  `KR0KI_STORYB00K_PORT` (default 8789).

- [ ] **Step 1: Write the Containerfile**

Create `containers/kr0ki-storyb00k-agent/Containerfile`:

```dockerfile
FROM docker.io/library/python:3.13-alpine

RUN pip install --no-cache-dir rdflib

WORKDIR /opt/kr0ki-storyb00k-agent
COPY containers/kr0ki-storyb00k-agent/server.py /opt/kr0ki-storyb00k-agent/server.py
COPY containers/kr0ki-storyb00k-agent/llm_client.py /opt/kr0ki-storyb00k-agent/llm_client.py
COPY containers/kr0ki-storyb00k-agent/draft_graph.py /opt/kr0ki-storyb00k-agent/draft_graph.py
COPY containers/kr0ki-storyb00k-agent/skills/ /opt/kr0ki-storyb00k-agent/skills/
COPY containers/kr0ki-mcp/manifest_dispatch.py /opt/kr0ki-storyb00k-agent/manifest_dispatch.py

USER 65532:65532
ENTRYPOINT ["python3", "/opt/kr0ki-storyb00k-agent/server.py"]
```

- [ ] **Step 2: Update the pod manifest**

In `deploy/kr0ki-local.pod.yaml`, add a fifth container after `kr0ki-mcp`:

```yaml
    - name: kr0ki-storyb00k-agent
      image: localhost/kr0ki-storyb00k-agent:dev
      imagePullPolicy: Never
      env:
        - name: KR0KI_URL
          value: http://127.0.0.1:8787
        - name: OPENAI_API_KEY
          value: "unused-local-llamacpp-key"
        - name: OPENAI_API_URL
          value: http://127.0.0.1:8001/v1
      ports:
        - containerPort: 8789
      readinessProbe:
        httpGet:
          path: /health
          port: 8789
        initialDelaySeconds: 1
        periodSeconds: 2
        timeoutSeconds: 2
        failureThreshold: 15
      resources:
        limits:
          cpu: "250m"
          memory: 128Mi
      securityContext:
        allowPrivilegeEscalation: false
        capabilities:
          drop: ["ALL"]
        readOnlyRootFilesystem: true
```

Also add `KR0KI_SYSMLV2_BASE_URL` (and `KR0KI_SYSMLV2_TOKEN` if applicable — leave
unset if no target server is configured for local dev; `/model/*` routes correctly
503 per Task 3) to the `kr0ki` container's `env` list, right after
`KR0KI_KUBEDIAGRAM_WORKER_URL`.

**Note on `OPENAI_API_URL`'s value here:** this pod's own env config points it at the
already-running local `ch0nky` llama.cpp container (reachable at `127.0.0.1:8001` from
inside the pod's shared network namespace, same as every other intra-pod call
established this session) — matching design spec §4's framing that this is the pod's
own default configuration, not a hardcoded assumption in any code.

- [ ] **Step 3: Update the justfile**

In `justfile`'s `pod-build` recipe, add a fourth `podman build` line for the new image
(matching the existing three lines' exact style):

```
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-storyb00k-agent:dev -f containers/kr0ki-storyb00k-agent/Containerfile .
```

Add a fourth `just k0s-load` line to `pod-up`'s recipe body, matching the existing
three:

```
    just k0s-load localhost/kr0ki-storyb00k-agent:dev
```

- [ ] **Step 4: Commit**

```bash
git add containers/kr0ki-storyb00k-agent/Containerfile deploy/kr0ki-local.pod.yaml justfile
git commit -m "feat: add kr0ki-storyb00k-agent to the pod manifest and build/load recipes"
```

---

### Task 10: build, deploy to k0s, and validate live

This task has no code changes — it verifies Tasks 1-9 work together in the real
deployed pod, following the same process established for every prior kr0ki feature
this session (`just pod-build`, `just k0s-load`, `kubectl apply`).

- [ ] **Step 1: Full workspace verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd containers/kr0ki-mcp && python3 test_bridge.py && python3 test_http_worker.py && cd -
cd containers/kr0ki-storyb00k-agent && python3 test_llm_client.py && python3 test_server.py && python3 test_draft_graph.py && cd -
cd playbook && npx vitest run && cd -
```

Expected: all clean, all green.

- [ ] **Step 2: Build and deploy**

```bash
just pod-build
just k0s-load localhost/kr0ki-server:dev
just k0s-load localhost/kr0ki-mcp:dev
just k0s-load localhost/kr0ki-kroki-compat:dev
just k0s-load localhost/kr0ki-storyb00k-agent:dev
kubectl --context Default apply -f deploy/namespace.yaml
kubectl --context Default -n kr0ki delete pod --ignore-not-found kr0ki-local
kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml
kubectl --context Default -n kr0ki get pod kr0ki-local
```

Expected: `kr0ki-local` reaches `4/4 Running` with zero restarts. **Use `set -o
pipefail` on every build command** (a real pipe-masking failure was caught and fixed
during Task 7 of the mcp-http-parity work this session — don't repeat it) and confirm
`vendor/kubediagrams`/`vendor/kroki-mcp` submodules are initialized in whatever
worktree this build runs from before starting.

- [ ] **Step 3: Validate the new `/model/*` and `/model/graph/query` routes**

```bash
curl -fsS http://127.0.0.1:8787/mcp/tools | python3 -m json.tool | grep -c '"name"'
```

Expected: `10` (3 pre-existing + 7 new). If `KR0KI_SYSMLV2_BASE_URL` is set to a real
target server, also validate `curl -fsS http://127.0.0.1:8787/model/projects`; if
unset, validate the 503 path (`curl -sS -o /dev/null -w '%{http_code}'
http://127.0.0.1:8787/model/projects` → `503`).

- [ ] **Step 4: Validate the sidecar directly**

```bash
kubectl --context Default -n kr0ki port-forward pod/kr0ki-local 8789:8789 &
curl -fsS http://127.0.0.1:8789/health
```

Expected: `{"status":"ok",...}`.

- [ ] **Step 5: Validate a real end-to-end run against the local LLM**

With the port-forward from Step 4 still active:

```bash
curl -fsS -X POST http://127.0.0.1:8789/run \
  -H 'Content-Type: application/json' \
  -d '{"threadId":"t1","runId":"r1","messages":[{"role":"user","content":"List the formats kr0ki supports."}]}'
```

Expected: an SSE stream containing `RUN_STARTED`, at least one `STATE_DELTA` (from the
LLM calling `list_formats`), and `RUN_FINISHED` — proving the full chain (sidecar →
real `ch0nky` LLM → tool-call decision → manifest-dispatch → kr0ki-server → back)
works against the real local model, not just mocks.

- [ ] **Step 6: Validate the storyb00k panel in a browser**

Open `playbook/`'s served URL, navigate to the storyb00k panel, send a message, confirm
the dashboard populates with at least one panel and the narration transcript updates.

- [ ] **Step 7: Handoff to critical review**

Once all of the above is green, invoke the `code-review` skill (or equivalent) against
the full diff introduced by this plan (Tasks 1-9) for an independent critical pass
before considering this done — matching the process used for the mcp-http-parity work
this session (design → SDD execution → per-task review → final whole-branch review →
fix wave → re-review).
