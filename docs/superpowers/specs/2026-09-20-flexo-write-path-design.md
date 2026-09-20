# Digital-thread write path: syncing a `SysGraph` into a live SysML v2 project

**Parent:** [`PLAN-KR0KI-006-dbt-digital-thread.md`](../../PLAN-KR0KI-006-dbt-digital-thread.md)
§2 (piece 2 — the Flexo write path, previously tracked but not designed).
**Reads against:** [`2026-09-20-dbt-manifest-digital-thread-design.md`](2026-09-20-dbt-manifest-digital-thread-design.md)
(piece 1 — merged, `ufo-types#30`; this spec consumes its `SysGraph` output and its
`dbt:{unique_id}` `ElementId` convention verbatim), `crates/kr0ki-sysmlv2-client/src/lib.rs`
(the client this spec extends), `docs/EVAL-flexo.md` (Flexo MMS's commit-changes
endpoint is stubbed server-side — see §0).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-20.

**Status:** approved for implementation (brainstorm session, 2026-09-20) — proceed to
`writing-plans`.

---

## 0. What changed the plan mid-brainstorm

Piece 2 was originally framed as "commit `SysGraph` elements into a live Flexo
project." Investigation found `flexo-mms-sysmlv2` 0.2.0's commit `changes`/element-
delta endpoint is `NotImplementedError` — stubbed, not built — per this repo's own
prior evaluation (`docs/EVAL-flexo.md`). The write path this spec needs cannot be
tested against Flexo at all right now, regardless of client-side correctness.

The OMG's own **Java pilot reference implementation**
([`Systems-Modeling/SysML-v2-API-Services`](https://github.com/Systems-Modeling/SysML-v2-API-Services))
does implement it — confirmed by reading `CommitController.java` (`postCommitByProject`
routes to a real `commitService::create`, not a stub) and the official
[`SysML-v2-API-Cookbook`](https://github.com/Systems-Modeling/SysML-v2-API-Cookbook)'s
`Element_Create_Update_Delete.ipynb`, which is the source of the request shape in §2.
`kr0ki-sysmlv2-client` already targets the generic OMG PSM, not Flexo specifically
(`docs/EVAL-flexo.md`'s own decision), so this substitution needs no client-side
compromise — it's the intended usage.

## 1. One paragraph

A new reconciliation layer takes a `SysGraph` (from piece 1, or any future box-1
front-end) and a target SysML v2 project, and syncs the graph's nodes into that
project as a single commit: elements the project doesn't have yet get created,
elements whose content changed get updated, elements the graph no longer has get
deleted, and elements that are unchanged produce no commit content at all — the
zero-drift promise piece 1 set up, delivered. Split across two layers matching this
crate's existing boundary: `kr0ki-sysmlv2-client` gains generic, spec-shaped write
capability (no domain awareness); `kr0ki-core` owns the dbt-specific diff/reconcile
policy. Edge (lineage) sync is explicitly deferred — see §7.

## 2. Client layer — `kr0ki-sysmlv2-client`

New types, matching the confirmed real wire shape verbatim (§0's cookbook source):

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ElementRef {
    #[serde(rename = "@id")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataVersion {
    #[serde(rename = "@type")]
    pub type_: &'static str, // always "DataVersion"
    /// `Some(payload)` for create/update; `None` for delete.
    pub payload: Option<serde_json::Value>,
    /// Absent for create (server assigns `@id`); present for update/delete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<ElementRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitRequest {
    #[serde(rename = "@type")]
    pub type_: &'static str, // always "Commit"
    pub change: Vec<DataVersion>,
    /// Omitted for a project's first-ever commit.
    #[serde(skip_serializing_if = "Option::is_none", rename = "previousCommit")]
    pub previous_commit: Option<ElementRef>,
}
```

`payload` stays `serde_json::Value`, not a typed SysML-element enum — the client
doesn't and shouldn't know about specific element kinds (`PartUsage` vs. anything
else); that's the reconciliation layer's concern (§3), matching the same
acquisition/domain-neutral split `reqif_import.rs` and `ufo_types::dbt` already use.

New method:

```rust
impl SysmlV2Client {
    /// POST /projects/{project_id}/commits[?branchId={branch_id}]
    pub async fn create_commit(
        &self,
        project_id: &str,
        branch_id: Option<&str>,
        request: CommitRequest,
    ) -> Result<Commit, ClientError> { .. }
}
```

Reuses the existing `Commit` response type (already defined for the read-side
`commits()`/`commit()` methods) and the existing `ClientError` — no new error type at
this layer; a POST failure is the same class of failure a GET failure already is.

## 3. Reconciliation layer — new `kr0ki-core` module

`kr0ki_core::digital_thread_sync` (name open to bikeshedding at plan-writing time):

```rust
pub struct SyncConfig {
    pub project_id: String,
    pub branch_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Client(#[from] kr0ki_sysmlv2_client::ClientError),
}

pub async fn sync_dbt_graph(
    client: &SysmlV2Client,
    graph: &SysGraph,
    config: &SyncConfig,
) -> Result<Commit, SyncError> { .. }
```

Algorithm:

1. Fetch the project's latest commit (`client.commits()`, newest first — same
   polling pattern `docs/EVAL-flexo.md` already documents for the read side; the
   existing client's `commits()` is project-scoped, not branch-filtered, so this reads
   project history regardless of `config.branch_id`) and its elements
   (`client.elements(...)`). `None` if the project has no commits yet (first-sync case
   — no `previousCommit` in the request). `config.branch_id`, when set, is used only
   as `create_commit`'s `branchId` query param in step 5 below (associating the *new*
   commit with a branch) — it plays no role in step 1's read.
2. Filter fetched elements to those whose `identifier` field starts with `dbt:`,
   building `identifier -> @id`. Everything else (human-authored elements, elements
   from a different front-end's `identifier` prefix) is invisible to this pass — never
   read, never touched. This is the entire zero-drift guarantee, inherited unchanged
   from piece 1's design (its spec §5), just exercised against a real server now
   instead of only against an in-memory `SysGraph`.
3. For each node in the fresh `SysGraph` (`node.id.0`, already `dbt:{unique_id}` per
   piece 1): if its identifier isn't in the fetched map, emit a create `DataVersion`
   (payload only, no `identity`). If it is, and the fetched element's `name` differs
   from `dbt_node_label`'s current output for this node (piece 1's `lower_nodes`/label
   logic — this spec doesn't re-derive it, it's the input), emit an update
   `DataVersion` (payload + `identity`). If the name matches, emit nothing — no commit
   content, matching zero-drift's actual point (a re-ingest of an unchanged manifest
   produces zero server writes, not a no-op write).
4. For every `dbt:`-prefixed identifier in the fetched map with no corresponding node
   in the fresh `SysGraph` (the dbt node was removed from the source manifest), emit a
   delete `DataVersion` (`payload: None`, `identity` set).
5. If the changeset is empty, return the *previous* commit unchanged rather than
   posting an empty commit (an OMG API commit with zero `change` entries is
   pointless — avoid the round-trip and the noise in the project's commit history).
   Otherwise, POST via `create_commit`.

Element payload shape for a create/update, matching piece 1's own already-decided
node classification (its spec §1: *"a dbt model becomes a SysML v2 `PartUsage`"*):

```json
{"@type": "PartUsage", "name": "customers (marts)", "identifier": "dbt:model.jaffle_shop.customers"}
```

## 4. Testing

Matching `kr0ki-sysmlv2-client`'s own existing convention exactly (its current tests
are stub-backed; a separate `#[ignore]`d test is gated on `KR0KI_SYSMLV2_BASE_URL`):

- **Unit tests, no network** — the diff/changeset-building logic in
  `sync_dbt_graph` (or a pure helper it calls) is a function from
  `(fetched elements, fresh SysGraph)` to `Vec<DataVersion>`; test it directly against
  hand-built inputs covering create / update / no-op / delete, and the "empty
  changeset skips the POST" case. No client, no server, no async runtime needed for
  this part.
- **Stub-backed integration test** for `create_commit` itself — a local HTTP stub
  (matching `kr0ki-server`'s own "in-process HTTP tests, no backend needed" pattern)
  verifying the request is well-formed (`@type`, `change` shape, `previousCommit`
  presence/absence) and the response deserializes into `Commit`.
- **One new `#[ignore]`d live test** in `kr0ki-sysmlv2-client/tests/live.rs`, gated on
  the same `KR0KI_SYSMLV2_BASE_URL`/`KR0KI_SYSMLV2_TOKEN` env vars the existing live
  test uses — but pointed at a running **OMG Java pilot** instance (§0), not Flexo.
  Standing one up: clone `Systems-Modeling/SysML-v2-API-Services`, run a local
  PostgreSQL container (`docker run --name sysml2-postgres -p 5432:5432 -e
  POSTGRES_PASSWORD=... -e POSTGRES_DB=sysml2 -d postgres` — from that repo's own
  README), then `sbt run` (JDK 11 + sbt required; no Dockerfile exists in that repo
  today for the app itself, only the Postgres dependency is containerized). This is a
  real "on-demand instance" in the sense the goal asked for, but it's a manual
  developer step, not something this spec automates — containerizing the pilot app
  itself (a `Containerfile` for it) is a reasonable fast-follow, not blocking this spec.

## 5. Error handling

No new failure modes beyond what the client already has (`ClientError` covers
transport/HTTP/deserialize failures uniformly for reads and writes). A partial-commit
failure (the POST fails after some but not all intended changes) is not something this
API gives a client any way to detect or recover from differently than any other failed
request — the OMG API models a commit as atomic; if the POST fails, nothing was
written, and a retry re-runs the same diff (safe, since the diff is computed fresh
from the server's actual current state each time, not from a locally cached belief
about it).

## 6. Explicit non-goals

- **Edge/lineage sync.** Piece 1's `SysGraph` edges (dbt `depends_on` lineage,
  `UfoRelation::Requires`) are not synced by this spec — only nodes. Relationship
  creation via this API needs its own investigation (the cookbook recipes pulled for
  this spec covered element create/update/delete only, not relationship creation) and
  its own design pass. Tracked as a fast-follow to this piece, not blocking it.
- **Flexo itself**, until it implements the commit-changes endpoint (§0) — this spec
  targets the generic OMG PSM (already this client's stated design), which happens to
  currently mean testing against the pilot instead.
- **Conflict resolution beyond "diff against current state each time."** No optimistic
  locking, no merge, no branch-divergence handling — `previousCommit` is set to
  whatever the latest commit was at diff-time; if another writer committed between the
  fetch and the POST, this spec doesn't detect or handle that race. Real risk only
  under concurrent writers to the same project, which isn't this integration's use
  case yet (a periodic solo sync job).
- **Containerizing the OMG pilot app itself** — a real, valuable fast-follow (§4) but
  not part of this spec's deliverable.
- **A `Containerfile`/CI job that stands the pilot up automatically for the live
  test** — same reasoning; the live test stays a manual, `#[ignore]`d, developer-run
  check, matching the existing one's own posture.
