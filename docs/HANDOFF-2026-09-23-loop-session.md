# HANDOFF — 2026-09-23 autonomous /loop session

**Audience:** the next agent (or Brian) picking this up.
**Session:** `https://claude.ai/code/session_01STeK9pUZ8Cg3iPKQvazc3r`

## Executive state

A single long `/loop` session shipped 6 items against `docs/TODO.md`'s
backlog, found and fixed a real security disclosure bug along the way,
diagnosed a live infra outage (not a kr0ki bug), and surfaced a
cross-repo (`kr0ki` ↔ `ledgrrr`) type-fragmentation problem that is now
its own tracked PR. One architectural item (Flexo baseline adapter) has
an **approved spec but no implementation plan yet** — that's the cleanest
re-entry point for whoever continues.

## What shipped (all reviewed, all gate-clean)

| # | What | Where | Status |
|---|---|---|---|
| 1 | Requirements-centric rules system (rules-as-Flexo-elements, regorus/OPA v1) | `feat/requirements-rules-system` | [PR #51](https://github.com/PromptExecution/kr0ki/pull/51), open, green, unreviewed |
| 2 | Digital-thread write path (`sync_dbt_graph`, generic `SysGraph`→Flexo commit) | `feat/flexo-write-path` | [PR #52](https://github.com/PromptExecution/kr0ki/pull/52), open, green, unreviewed |
| 3 | SSRF-hardened ReqIF HTTPS-fetch (`reqif_fetch`) | `feat/reqif-https-fetch` | [PR #53](https://github.com/PromptExecution/kr0ki/pull/53), open, green, unreviewed |
| 4 | Per-`ViewDefinition` rendering (`?view=` query param) | `main` directly (`9f1796a`) | Shipped, merged |
| 5 | KubeDiagrams response caching (`FsCache::get_raw`/`put_raw`) | `main` directly (`e981e3e`) | Shipped, merged |
| 6 | Flexo baseline adapter (M3) — **spec only** | `main` (`025bba0`) | Spec approved, **plan not written** |

Every one of 1–5 went through brainstorm → (spec+plan for 1–3, short
bounded design for 4–5) → subagent-driven-development (fresh implementer
+ independent reviewer per task, final whole-branch review on the most
capable model for anything security-relevant) → tests/clippy/fmt clean
at every stage, verified independently, not trusted from implementer
self-reports. Full per-task rulings live in each worktree's
`.superpowers/sdd/<plan-name>/progress.md` (git-ignored, still on disk).

**Real bugs caught, not just process theater:**
- PR #53's final review found rejection error messages were leaking
  resolved internal IPs and raw DNS errors back to the caller — an
  internal-network reconnaissance oracle, even though the SSRF
  connection-blocking logic itself was airtight. Fixed, re-verified.
- Item 2's async orchestration had a real bug in the *plan's own*
  verbatim code (`reqwest::Url::host_str()` returns IPv6 literals
  bracketed, breaking the DNS-literal fast path) — caught by the
  implementer, independently re-verified against real `hyper-util`
  source by the reviewer.

## PRs needing a human decision (not mine to make)

PRs #51, #52, #53 are all open, green, no reviews yet. Nobody's blocking
on them; they just need someone to actually read/merge/close them.
[`PromptExecution/ledgrrr#249`](https://github.com/PromptExecution/ledgrrr/pull/249)
(see below) is the same — open, needs ledgrrr-side review.

## The Flexo baseline adapter (M3) — where to pick up

**Spec:** `docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md`
(approved by Brian). **Plan: not started.**

The design: a new `kr0ki_core::reqif_export` module (`RequirementGraph` →
`reqrs::model::ReqIfBundle` → XML, using `reqrs` as a **direct**
`kr0ki-core` dependency — it's already transitively present via
`ufo-types`' `reqif` feature, this only promotes it to direct), storing
imported requirements as `RequirementUsage` elements in a live Flexo/
SysML-v2 project via the already-shipped `create_commit` write path, and
a commit↔graph↔export round-trip contract test using
`BaselineIdentity.exported_baseline_sha256` (a field that already exists
on the type for exactly this purpose — confirmed against real
`ufo_types::mbse::requirements` source).

**Before writing the plan**, verify these `reqrs` types against the real
pinned source
(`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reqrs-0.2.2/src/model/`)
— this session got as far as `SpecObject`/`ReqIfBundle`'s shape but did
**not** yet check: `CoreContent`, `ReqIfContent`, `Specification`,
`SpecHierarchy`, `ObjectLookup::build`'s exact signature, `ReqIfHeader`'s
required fields, `SpecType`/`SpecRelationType`, `AttributeValue`'s
variants, or `reqrs::unparse::FormatMode`. Building a `ReqIfBundle` from
scratch (not just parsing one) touches all of these — do not guess their
shapes, this session stopped specifically to avoid writing a plan against
unverified types.

**⚠️ Read the next section before touching this at all.**

## Cross-repo finding: `Requirement` type fragmentation (ledgrrr)

While scoping the Flexo adapter's export direction, discovered three
independent `Requirement` type representations across the b00tyverse with
no canonical designation and no translation between them:

1. `PromptExecution/ufo-types` (standalone, kr0ki's dependency) —
   `ufo_types::mbse::requirements::Requirement`.
2. `ledgrrr/crates/ufo-types` (local, independently drifted fork, missing
   `mbse::requirements`/`reqif` entirely).
3. `ledgrrr/crates/arc-kit-au::Requirement` (a third, unrelated shape).

Full diagnosis + recommended path:
[`PromptExecution/ledgrrr#249`](https://github.com/PromptExecution/ledgrrr/pull/249)
(`docs/ufo-types-requirement-fragmentation-scoping.md` in that repo).

**Brian added a hard requirement mid-session, not yet resolved:** any
unified `Requirement` must be attributable back to ledgrrr via a
`ledgrrr://` URI scheme (a "meta-dataframe interface"). The PR proposes
`ledgrrr://<NodeId>` (zero new machinery — `arc-kit-au::NodeId` already
stringifies as `"{prefix}:{hash}"`) but **explicitly leaves the resolver
semantics as ledgrrr's own open decision** — what dereferencing that URI
actually does (MCP call, HTTP route, or purely opaque-stable-identifier-
with-no-resolver) is not decided. **Brian said he cannot approve the
unification path without this being answered** — treat PR #249 as
blocking any further ReqIF-export work that would deepen the
fragmentation, until it's resolved or Brian says otherwise.

**Practical implication for the Flexo baseline adapter above:** building
`kr0ki_core::reqif_export` using `reqrs` directly does NOT touch
`arc-kit-au` or ledgrrr's `ufo-types` at all (it's kr0ki-local, self-
contained), so per Brian's own explicit call ("push the doc PR, and
resume kr0ki's original plan") it does not need to wait on PR #249's
resolution. But if the M3 work later needs to reference ledgrrr (e.g. a
`RequirementUsage` element's provenance pointing back to a decision
ledgrrr made), that's exactly when PR #249 becomes load-bearing — check
its status before assuming a `ledgrrr://` URI convention exists yet.

## Infra: `ch0nky` (storyb00k agent's LLM backend) — unresolved, not kr0ki's to fix

Brian reported "the agent chat capability doesn't work" mid-session.
Diagnosed: **not a kr0ki code bug.** Full details:
`.claude/projects/-home-brianh-promptexecution-kr0ki/memory/
kr0ki_ch0nky_inference_broken.md` (this session's own memory file).
Short version: the `b00t-inference/ch0nky` k8s deployment (namespace
`b00t-inference`) was scaled to 0, and after Brian approved scaling it
back to 1, it immediately `CrashLoopBackOff`s because its configured
model file
(`models--unsloth--Qwen3.6-35B-A3B-GGUF/.../Qwen3.6-35B-A3B-UD-IQ4_XS.gguf`)
does not exist anywhere on the node's HuggingFace cache. Left at
`replicas: 1` (as instructed) but the model path was **not** touched —
re-downloading a 35B model or repointing at a different cached one
(several exist, e.g. `unsloth/Qwen3.6-27B-GGUF`) is Brian's call, not
made this session.

## Uncommitted, pre-existing, NOT touched this session

`README.md`, `containers/kr0ki-storyb00k-agent/project_store.py`,
`containers/kr0ki-storyb00k-agent/server.py` have substantial uncommitted
changes (a redesign of the agent's clarification/exploration flow —
`MAX_TOOL_ROUNDS` split into an internal model/tool ceiling vs. a
separate user-facing clarifying-question budget, explore-before-ask
candidate rendering). These predate this session and were deliberately
left alone throughout — every implementer this session was explicitly
told not to touch them and to `git add` only intended files, never `-A`
or `.`. They pass their own test suite (43/43, verified this session)
but were never committed. Not this handoff's problem to solve, just
don't lose track of them.

## Process notes for whoever continues

- Every implementation this session followed: brainstorming (classify
  bounded vs. architectural — don't over-ceremony a small change, don't
  under-ceremony a real one) → spec+plan (architectural) or short
  in-chat design (bounded) → **human approval before any code** → fresh
  subagent per task → independent review per task → final whole-branch
  review on the most capable model for anything security/architecture-
  sensitive → human decision on push/merge/keep, never automated.
- Verify third-party crate APIs against real pinned source before
  writing code or plans that assume a shape — this caught real bugs
  multiple times this session (stdlib `IpAddr` stability, `reqwest`/
  `hyper-util` DNS-resolution internals, `axum`'s default body limit,
  `ufo-types`' actual field shapes). Don't trust a memory, a doc
  comment, or an implementer's self-report without an independent check
  when it matters.
- `.worktrees/` (kr0ki) and (as of this session) `ledgrrr/.worktrees/`
  are the established pattern for isolated work — check `git worktree
  list` and `git status` on any shared checkout before committing
  anything; this session found the main `ledgrrr` checkout in a detached
  HEAD state with unrelated in-progress work from elsewhere and had to
  route around it with a fresh worktree from `origin/main`.
