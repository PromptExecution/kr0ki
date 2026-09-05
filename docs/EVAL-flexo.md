# EVAL — Flexo MMS (`flexo-mms-sysmlv2`)

**Evaluated:** 2026-09-05 · **For:** kr0ki SysML-model ingestion path (PRD-KR0KI-001
FR1/FR4/FR5), `PLAN-KR0KI-002`. **Verdict:** primary integration target for the OMG
Systems Modeling API — but kr0ki targets the *generic* API, not Flexo specifically.

---

## What it is

**Flexo MMS** (Model Management System) — Apache-2.0, JPL-led (Jet Propulsion
Laboratory), Kotlin / Ktor. Storage is **RDF in an Apache Jena Fuseki triplestore** —
there is **no Postgres** anywhere in the stack. Layer 1 (`flexo-mms-layer1`) is a
generic quad-store versioning API (orgs / repos / branches / locks / commits over
RDF named graphs); the SysML surface is a microservice on top.

**`flexo-mms-sysmlv2`** is that microservice: an implementation of the **OMG "Systems
Modeling API and Services"** platform-specific model (PSM) — the standard REST API for
SysML v2 tools. Its own version is **0.2.0** (not tied to the OMG spec's version
number). Ships `openapi/openapi.yaml` (OpenAPI **3.0.3**) describing the surface.

- **Team / maturity:** JPL-led, roughly 5–6 human contributors, **beta**. Active but
  small.
- **Deploy:** Docker images, `docker compose` for the full stack, Kubernetes manifests
  exist. JVM services run ~256–512 MB each; Fuseki wants a real heap — **~1.5–3 GB for
  the full stack** (layer1 + sysmlv2 + Fuseki + auth/ldap bits).

## OMG API coverage — **partial**

| Area | Status in `flexo-mms-sysmlv2` 0.2.0 |
|---|---|
| Project | implemented |
| Branch | implemented |
| Tag | implemented |
| Commit | implemented (but see `previousCommit` below) |
| Element (get, list, paged) | implemented |
| Relationship navigation (`?direction=in\|out\|both`) | implemented |
| Query (POST a query object) | implemented |
| **Commit `changes` / element deltas** | **stubbed** — `NotImplementedError` |
| **Diff** (`GET .../diff`) | **stubbed** — `NotImplementedError` |
| **Merge** | **stubbed** — `NotImplementedError` |
| **Meta** endpoints | **stubbed** — `NotImplementedError` |
| `previousCommit` on a Commit | **returns `null`** — no parent-pointer chain exposed |

So: the read/navigate core kr0ki needs (Project → Commit → Element/Relationship, plus
`roots`) is there; anything that would give kr0ki a **cheap delta between two commits**
(Diff, commit `changes`, `previousCommit`) is not.

## What this means for kr0ki

- **`(projectId, commitId)` is a stable, immutable coordinate** — a commit's element set
  never changes. This is kr0ki's cache key material (PRD §3, FR5): a diagram rendered
  for a given `(projectId, commitId)` is valid forever.
- **No server-side content hash / ETag** on the element collection. kr0ki must derive
  its own — done: `kr0ki-sysmlv2-client`'s `ModelSnapshot.content_hash` (deterministic
  SHA-256 over the `@id`-sorted, canonically-serialized element set + root ids).
- **No webhooks / change-feed.** kr0ki discovers new commits by **polling
  `GET /projects/{id}/commits`** newest-first and snapshotting any `commitId` it has not
  cached. Cheap because each commit is rendered at most once, ever.
- **No PSM Diff / no `previousCommit`** ⇒ kr0ki cannot ask the server "what changed
  between commit A and B" to do incremental re-rendering. Every uncached commit is a
  full `snapshot()`. Acceptable given the render-once-per-commit model, but it rules out
  a "only re-render the sub-views whose elements moved" optimization on the Flexo side.

## Decision: target the generic OMG API, not Flexo

`kr0ki-sysmlv2-client` speaks the **OMG Systems Modeling API PSM**, not a Flexo-specific
protocol. The same client also works against:

- **`Systems-Modeling/SysML-v2-API-Services`** — the OMG **Java pilot** (Play framework
  + **Postgres**). The reference implementation of the same PSM.
- **`Open-MBEE/OpenSysML`** — a Go implementation of the same PSM.
- **Eclipse SysON** — partial, read-mostly (`docs/EVAL-syson.md`).

No Rust client for this API existed before `kr0ki-sysmlv2-client`. The crate names only
the fields it depends on and tolerates everything else, so a server implementing a
different slice of the spec (or adding fields) does not break it.

## Sizing — what is left after this PR

This PR's `kr0ki-sysmlv2-client` is **the bulk of the client-side work**. Remaining on
the model path:

1. a **poll loop** — periodically `commits()` newest-first, `snapshot()` the uncached
   ones. Small.
2. wiring **`ModelSnapshot.content_hash` into the render cache** — the FR5 *model* path,
   extending P0's `cache_key` with the view kind + notation (see `PLAN-KR0KI-002` §3).
3. the downstream chain **`ModelSnapshot` → canonical UFO-typed semantic graph →
   pattern recognizer → SysML v2 viewpoint → renderer adapter** (`PLAN-KR0KI-002` §2) —
   the real remaining design work. The UFO-graph layer is owned by `ufo-types` (PR in
   flight), not kr0ki; kr0ki MUST NOT lower `ModelSnapshot` straight to diagram syntax.

The **CDN tier** of the cache (FR5's real target) stays blocked on PRD decision **D5**
regardless.
