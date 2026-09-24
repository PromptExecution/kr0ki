# CRITICAL REVIEW — kr0ki roadmap & existing functionality

**Reviewed:** 2026-09-24 · **Scope:** `PRD-KR0KI-001`, `PLAN-KR0KI-002` through
`-006`, `AGENTS.md`, `docs/TODO.md`, and the actual shipped code surface in
`crates/kr0ki-core`, `crates/kr0ki-server`, `crates/kr0ki-sysmlv2-client`, cross-checked
against `git log` on `main` and this session's four just-reviewed PRs (#51–#53, #55).
**Verdict:** the implemented core (Box 5 render loop, plus Box 2/3's Kubernetes arm) is
solid and well-tested; the project's own status-tracking documents have real,
independently-confirmed drift — including one factual error in `AGENTS.md` itself, the
primary orientation document every agent session reads first. Two of the drift items
found here are fixed in this same PR; the rest are flagged for a human decision.

---

## 1. Confirmed drift, fixed in this PR

### 1a. `AGENTS.md`'s five-box table (§1) had two wrong rows

- **Box 3 (pattern recognizers)** was marked `Design only — ... no code`. This is
  false: `crates/kr0ki-core/src/k8s_recognizer.rs` (955 lines) is implemented, wired
  into `kr0ki-server/src/app.rs`'s actual route handler, and has a differential oracle
  test against real KubeDiagrams (`tests/kubediagrams_oracle.rs`). The Rust arm
  (`rust_recognizer.rs`) is also implemented, though not yet wired to any route.
- **Box 2 (canonical UFO-typed semantic graph)** was marked `Design only — ... no
  container type`. Also false: `ufo-types` has shipped the `SysGraph` container (its
  own `lib.rs` doc comment: *"`SysGraph` typed envelope (`sysgraph`): a box-2
  snapshot of..."*, present across every checked-out revision inspected, not a
  recent addition) and kr0ki's own `ufo_graph.rs` builds it for the SysML-v2 arm.
  Only the dbt arm's builder (`PLAN-KR0KI-006` piece 1) remains spec-only.

Both rows corrected in this PR. This matters more than an ordinary stale doc: `AGENTS.md`
is injected into every agent session's context at startup — an agent given a task
touching Box 2/3 would previously have been told, incorrectly, that no code exists to
build on, and risked re-implementing what's already there and tested.

### 1b. `PLAN-KR0KI-006`'s piece 2 said "not designed. Needs its own brainstorm pass"

False as of this session: PR #52 (`kr0ki_core::digital_thread_sync::sync_dbt_graph`)
fully designed and implemented this piece, including a critical-review-driven fix
(2026-09-23) for `DataVersion.payload`'s full-replace semantics (researched against
the OMG reference implementation's own source, since the formal spec is silent on it).
Corrected in this PR to point at the real spec/plan/PR, with a note that the PR isn't
merged yet.

## 2. Drift found, not fixed here (needs a decision, not just a wording fix)

### 2a. `PLAN-KR0KI-002` is stale on Box 2/3 status in the same way `AGENTS.md` was

`PLAN-KR0KI-002.md` (2026-09-05) still frames FR1/FR4 as blocked on "the UFO semantic-
graph layer + a pattern recognizer" that don't exist. Per §1 above, both now exist for
at least the Kubernetes/SysML-v2 arms. This plan doc wasn't touched in this PR — it's
the origin document for the five-box framing and deserves its own pass rather than a
drive-by edit alongside `AGENTS.md`'s summary table.

### 2b. `PLAN-KR0KI-004`'s Phase 0 exit gate: status genuinely unclear

`PLAN-KR0KI-004-revisioned-procedural-workspace.md` still reads `Status: proposed` and
gates real orchestration-framework work on a "Phase 0 exit gate." `git log` shows real
StoryB00k work has shipped (PRs referenced: #34, #35, #37 — `dev_session.rs`,
`workspace_types.rs`, `phase0_fixture.rs` all exist in `kr0ki-server`). What was **not**
confirmed in this review: whether the Phase 0 exit gate itself has ever been formally
declared passed anywhere. This is a decision for whoever owns Plan 004, not something
this review can settle by reading code — recommend an explicit status line update (or
confirmation that "proposed" is still accurate) rather than leaving it ambiguous.

### 2c. `PRD-KR0KI-001`'s FR6 (intra-ecosystem reference resolver) has zero presence anywhere

Searched `docs/TODO.md`, every `PLAN-KR0KI-*.md`, and the actual code — FR6 is not
mentioned once outside the PRD itself. This isn't stale-documentation drift; it's an
apparent scope gap between a committed functional requirement and everything that
tracks work against it. Two readings are both plausible and this review can't
adjudicate between them: (a) FR6 was quietly descoped/deferred and the PRD itself is
what's now stale, or (b) it's a genuinely dropped requirement nobody re-surfaced.
Recommend an explicit call on which.

### 2d. Two architecture framings coexist without one superseding the other

`PRD-KR0KI-001` frames kr0ki via a "MODEL SIDE / RENDER SIDE" split; `AGENTS.md` and
`PLAN-KR0KI-002` frame it via the five-box pipeline. Both are internally coherent and
cross-referenced, but they describe the same system at different abstraction levels
with no explicit mapping between them. Not a defect — the five-box framing has clearly
become the operative one in practice — but worth an explicit "the five-box pipeline is
PRD-001's MODEL/RENDER split refined into five stages" note somewhere, so a new
contributor reading the PRD first doesn't have to reverse-engineer the correspondence.

## 3. Strengths worth naming (not just gaps)

- **`docs/TODO.md` actively self-corrects.** It contains inline notes like *"This
  checkbox and the two below were stale — the work already shipped,"* and git history
  has a dedicated commit (`8561f3f docs: correct stale TODO.md checkboxes for boxes
  2/3 and ledgrrr seam`) for exactly this. Stale-doc correction is a named, recurring
  pattern in this project's own history (also `77a5ad2`), not a one-off — this review
  extends that pattern to `AGENTS.md` and `PLAN-KR0KI-006`, it doesn't introduce it.
- **Module-level provenance discipline is high.** Every surveyed module in
  `kr0ki-core`/`kr0ki-server` has a doc comment naming its exact box/plan/PR origin
  and explicitly stating what it deliberately does *not* do (e.g. `sysml_lift.rs`
  names exactly which two upstream modules stop short of producing its input type).
  No dead code or abandoned half-features found in this pass.
- **Git history shows real plan-then-code discipline**: `docs:` commits with spec/plan
  content consistently precede the feature commits that implement them, across the
  whole recent history, not just this session's work.

## 4. Recommendations

1. Give `PLAN-KR0KI-002` its own drift-correction pass (§2a) — out of scope for this
   PR, which only touched the two documents this review could fix with full confidence
   from direct code verification.
2. Get an explicit answer on Plan 004's Phase 0 exit-gate status (§2b) and PRD-001's
   FR6 (§2c) — both need a decision from whoever owns those documents, not a doc edit.
3. Consider a short cross-reference note resolving the two architecture framings (§2d).
4. No code changes are recommended by this review — the gaps found are entirely in
   status-tracking documentation, not in the implemented surface's correctness.
