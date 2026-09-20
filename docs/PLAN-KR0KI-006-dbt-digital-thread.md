# PLAN-KR0KI-006 — dbt as a digital-thread source

**Parent:** [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md) (the five-box pipeline; this plan
adds dbt as a second box-1 front-end, alongside the SysML-v2-API-client arm and
[`PLAN-KR0KI-003`](PLAN-KR0KI-003-rust-source-frontend.md)'s Rust-source arm).
**Reads against:** [`DESIGN-NOTE-versioned-dialects-and-b00t-loader.md`](DESIGN-NOTE-versioned-dialects-and-b00t-loader.md)
§4 (the `b00t://` loader this deliberately does not build yet — see §2 below).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-20.

**Status:** proposed. Piece 1 has an approved design and is ready for
`writing-plans`; piece 2 is tracked, not designed.

---

## 0. One paragraph

Getting ahead of a real near-term need, not solving one today: connecting the
SysML-v2 digital thread to actual data infrastructure (dbt-modeled tables/columns,
eventually live databases and other schema sources) so requirements, code, and data
assets are traceable in one graph. Split into two independently-implementable pieces
that share one identity/versioning discipline, tracked together here so the second
piece isn't lost once the first ships.

## 1. Piece 1 — dbt manifest → canonical UFO graph

Spec: [`docs/superpowers/specs/2026-09-20-dbt-manifest-digital-thread-design.md`](superpowers/specs/2026-09-20-dbt-manifest-digital-thread-design.md).
New `ufo_types::dbt` module, mirrors `ufo_types::reqif`'s exact shape: an `Upgrade`
impl for dbt's own versioned `manifest.json` wire format, an ordinary lowering
function into `ufo_types::sysgraph::SysGraph`, zero-drift identity via a `dbt:`-prefixed
`ElementId` (no new provenance field on existing types). Bytes in, `SysGraph` out —
acquisition-neutral, same split `kr0ki_core::reqif_import` already established.
**Status: spec approved, ready for an implementation plan.**

## 2. Piece 2 — the Flexo write path (tracked, not designed)

How `kr0ki-sysmlv2-client` — currently read-only (`ListModelProjects`,
`ListModelCommits`, `GetModelSnapshot`) — commits a `SysGraph` (from piece 1, or any
other box-1 front-end) into a live Flexo-backed SysML v2 project, and how the
`dbt:`-prefixed `ElementId` convention from piece 1 lets a re-ingest reconcile against
Flexo's commit history (diff, update, remove-stale) without ever touching an element
it doesn't own. **Status: not designed. Needs its own brainstorm pass before either a
spec or code.**

## 3. Explicitly deferred, not forgotten

Per the design note (§4) and this plan's own "getting ahead of a real need" framing:

- **The `b00t://` loader / live acquisition.** Piece 1 takes bytes directly — no URI
  resolution, no credential handling, no native database client. The design note's
  `b00t://<datum-name>` scheme (sponsored by b00t, not kr0ki) stays the documented
  future extensibility point for *how* bytes get acquired (a `postgres://`-flavored
  live source would be another pluggable acquisition type under the same scheme) —
  not something either piece here builds.
- **Column-level lineage.** Node-level only in piece 1 (§7 of its spec). Real future
  need, needs its own design once there's a concrete consumer.
- **Source-code-level discovery** (the other half of "discovery and mapping of
  systems using schema & source code") is [`PLAN-KR0KI-003`](PLAN-KR0KI-003-rust-source-frontend.md)'s
  job, already planned, unstarted — this plan doesn't duplicate it, just notes both
  arms feed the same canonical graph.

## 4. Open decisions (not this plan's to make)

- **Which dbt schema versions piece 1 actually supports at launch** — v12 is current;
  how far back (v9? v10?) to support in the first `Upgrade` impl is a real scoping
  call for whoever writes the implementation plan, not fixed here.
- **Piece 2's timing** relative to piece 1 shipping — piece 1 is independently useful
  (a testable, in-memory `SysGraph` from a real dbt project) even before piece 2
  exists; sequencing them is an operator call, not an architectural constraint.
