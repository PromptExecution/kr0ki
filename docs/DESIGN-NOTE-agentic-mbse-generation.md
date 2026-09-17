# DESIGN NOTE — agentic SysML v2 generation & shared multi-user digital engineering

**Status:** pre-decision input, research synthesis. **Not approved. Not a
plan.** This note exists to record what five external sources (two papers,
three repos) actually contain, cross-referenced against what kr0ki/ufo-types
already has, so the conclusions survive until a real brainstorm/plan is
warranted — not to propose an architecture.
**Created:** 2026-09-17 · **Owner:** PromptExecution (@elasticdotventures)
**Method:** MECE decomposition of the capability space + TRIZ-style
contradiction analysis, per explicit request. Five sources reviewed in
parallel by independent research agents; findings below are the controller's
synthesis, cross-checked for consistency across all five reports.
**Companion evaluations:** `docs/EVAL-sysml-derive.md`,
`docs/EVAL-reqif-opa-mcp.md`, `docs/EVAL-sysmlv2-learning-all-in-one.md` (new,
this batch), `docs/EVAL-flexo.md`, `docs/EVAL-syson.md` (existing).

---

## 0. Headline correction — fix first, read second

**D1 (`ledgrrr#202`, `sysml-derive` posture) was resolved 2026-09-10 — a week
before this research.** `docs/DESIGN-NOTE-typed-model-layer.md` still states
D1 as open/blocking in its status line and §4 table; that has been corrected
as part of this note (see that file's own changelog line). **kr0ki's SysML v2
authoring path is unblocked.** See `docs/EVAL-sysml-derive.md` for the full
resolution and why no new crate work is required on kr0ki's side.

This matters for reading everything below: the "generation" half of this
note's MECE map is **not** a wishlist blocked on an open decision — it is
already-available capability that nothing in kr0ki currently calls.

## 1. Sources reviewed

| Source | What it actually is | One-line verdict |
|---|---|---|
| arXiv 2506.21608 — **SysTemp** (Bouamra, Yun, Poisson, Armetta) | 4-agent sequential pipeline: NL → dict → Jinja2 template skeleton → fill → parser-repair loop | Validates kr0ki's existing template+validate-gate stance; no multi-user/concurrency content at all |
| arXiv 2604.25526 — **"AI as Consumer and Participant"** (Siyuan Ji) | Position/agenda paper, not a systems paper — three named principles for AI-MBSE, explicitly no existing implementation of any of them | Names the exact governance problem a shared multi-agent system needs; kr0ki has partial infrastructure (Provenance, Flexo) nobody has connected to these principles yet |
| `PromptExecution/sysml-derive` | Structural `#[derive(SysmlBlock)]` proc-macro | **Already resolved & shipped** — see §0 and `EVAL-sysml-derive.md` |
| `PromptExecution/reqif-opa-mcp` | ReqIF ingestion + OPA policy gate, production-grade | Reuse candidate for both ReqIF ingestion and multi-agent governance — see `EVAL-reqif-opa-mcp.md` |
| `FlyingCpp/sysmlv2-learning-all-in-one` (a.k.a. SynFeld) | Full MBSE learning platform, not a curated list | Its AI-teacher subsystem is the closest existing thing to a validated agentic-generation-with-commit-authority pattern — see `EVAL-sysmlv2-learning-all-in-one.md` |

**Note on the two arXiv links:** the user's first link (2604.25526) does not
resolve to SysTemp as its label suggested — direct fetch identified it as a
different, independently relevant paper (Ji's position paper). The second
link (2506.21608) is the real SysTemp paper. Both were fetched, read in full,
and are treated as two distinct sources throughout this note.

## 2. MECE decomposition of the capability space

Seven axes, each independently addressable, together covering the full
"shared multi-user agent-based digital engineering" problem the user
described. For each: what kr0ki/b00tyverse already has, what the five sources
offer, and what remains a genuine gap.

| Axis | kr0ki/b00tyverse already has | Prior art offers | Genuine gap |
|---|---|---|---|
| **A. Cognitive extraction** (unstructured/NL → structured claim) | Nothing | SysTemp's `SpecificationGeneratorAgent` (NL → typed dict); `reqif-opa-mcp`'s `docling`-based document ingestion (XLSX/PDF/DOCX/MD → requirement record) | A kr0ki-side mapping from either source's structured output into `ufo_types::sysml_model` types — small, not a research problem |
| **B. Procedural extraction** (unstructured → behavior/steps, not just structural facts) | Nothing — kr0ki has zero Action/State coverage (`EVAL-sysmlv2-learning-all-in-one.md`'s curriculum check) | Neither paper nor any of the three repos addresses this at all | **Real, unaddressed gap.** Everything surveyed (SysTemp, `sysml-derive`, `mbse.rs`) works at the structural (`part def`/attribute) level; none touch `ActionUsage`/`StateUsage` generation |
| **C. Structural generation** (typed value → spec-valid SysML v2 text) | `sysml-derive` + `ufo_types::mbse::MbseExport`, composed — **already shipped**, see §0 | SysTemp's Jinja2 skeleton (weaker: runtime string templating, not type-checked) | None — kr0ki's existing answer is stronger than the surveyed prior art |
| **D. Grammar/semantic validation** | `ufo_types::sysml::validate_sysml_v2` (wraps `sysml-v2-parser`, version-pinned, mandatory on every emit path per `DESIGN-NOTE-typed-model-layer.md` §2.7) | SysTemp's ParserAgent wraps the same underlying OMG parser lineage, but described as weaker/less-documented than kr0ki's own gate; SynFeld vendors the same Official Pilot Implementation, hash-verified | None — kr0ki's gate is already stricter (pinned, golden-fixture-backed) than anything surveyed |
| **E. Storage & versioning** (single source of truth, branches, commits) | Flexo MMS, target of `kr0ki-sysmlv2-client` — RDF/Jena, orgs/repos/branches/locks/commits (`EVAL-flexo.md`) | Nothing new — no source in this batch addresses model storage/versioning at all | Flexo's own known gap stands: **no diff, no merge, no webhooks, no `previousCommit` chain.** Nothing surveyed fixes this |
| **F. Governance** (who/what may commit; human vs. AI provenance; policy gates) | `Provenance` on every typed element (deterministic source-span/symbol-path — answers "where did this text come from," not "what is this claim's status") | `reqif-opa-mcp`'s "agents produce facts, a policy engine is the sole gate" (OPA/Rego, three-layer, audited); Ji's three principles (Engagement Readiness, Epistemic Accountability, Contribution Governance) name exactly this problem, with **no existing implementation anywhere**; SynFeld's teacher-agent binds delivery to a validator result + content hash before anything becomes authoritative | **Real gap, but the pieces to close it already exist separately** (Flexo's branch/commit model + `reqif-opa-mcp`'s OPA pattern + `Provenance`) — nobody has connected them. This is kr0ki's actual opportunity; see §4 |
| **G. Multi-agent concurrency** (multiple agents/humans editing the same model in parallel) | Nothing | **Nothing.** SysTemp is single-pipeline/single-model. Ji's paper explicitly declines to specify an architecture. SynFeld's teacher-agent is single-user/single-session. `reqif-opa-mcp`'s gate model assumes one proposal at a time | **The one axis truly unsolved by everything surveyed.** Flexo's branch+lock model gives eventual reconciliation via serialized human merge, not live concurrent multi-agent editing. This needs its own future decision, not a reuse — see §5 |

This partition is genuinely MECE for the scope the user described: extraction
(A, B) → generation (C) → validation (D) → persistence (E) → governance (F) →
coordination (G) is a linear pipeline with no axis overlapping another's
concern, and nothing outside these seven categories was surfaced by any of
the five sources.

## 3. TRIZ-style contradiction analysis

**Resolved contradictions found in prior art** (cited as external validation
of choices kr0ki has already made, not as new information requiring a design
change):

1. *"Generation must be flexible enough for open NL input" vs. "output must
   be syntactically guaranteed-valid."* SysTemp resolves this by separating
   unconstrained extraction (a dict) from constrained templated generation (a
   skeleton) — the same shape as `ufo_types`' split between the free-form
   `iso_ir` floor and the closed `ElementKind`/`Relation` typed layer above
   it. **Confirms the existing layering is the right shape.**
2. *"A domain type must stay stereotype-agnostic for structural derivation to
   work on it" vs. "it must carry classification metadata for full
   fidelity."* Resolved by D1 (§0): two independent, composable emitters
   (`sysml-derive` for shape, `ufo_types::mbse` for stereotype), neither aware
   of the other's mechanism.
3. *"Agents must generate freely" vs. "the model must stay a single verified
   source of truth."* SynFeld's teacher-agent resolves this by never treating
   LLM output as committed state until it passes the same validator a human
   edit would, and binding accepted output to a content hash before delivery
   — a stronger answer than SysTemp's syntax-only repair loop, because it also
   binds *delivery authority*, not just acceptance.

**The one contradiction nothing surveyed resolves:**

> *"Multiple agents (or agents + humans) need to work in parallel on the same
> model for throughput" vs. "the model must remain one consistent, auditable
> source of truth."*

SysTemp doesn't attempt it (single pipeline). Ji's paper names the *governance*
half of this problem (Contribution Governance) but explicitly states no
architecture exists anywhere. SynFeld's pattern handles one agent's proposal
against one validator, not two agents' concurrent proposals against each
other. `reqif-opa-mcp`'s OPA gate evaluates one candidate at a time. Flexo's
branch/lock model *sidesteps* the contradiction (serialize via human merge)
rather than solving live concurrent multi-agent editing.

This is not a "go build it" conclusion — a real answer here (optimistic
locking? CRDT-style merge over KerML's relationship set? a lock-broker in
front of Flexo? something else entirely) is its own research question and,
per `superpowers:brainstorming`'s classification, its own **architectural**
scope requiring its own approval gate. This note deliberately stops at naming
the contradiction, not resolving it.

## 4. Recommendations — reuse over rebuild, mapped to the MECE axes

Ordered by how settled each recommendation is, not by axis order:

- **C, D (generation, validation) — no action needed.** Already resolved,
  already stronger than anything surveyed. The only follow-up is the doc fix
  in §0 (done).
- **A (cognitive extraction) — reuse, thin adapter only.** Do not build a
  ReqIF parser or a document-ingestion pipeline. `reqif-opa-mcp`'s
  `reqif_parse`/`reqif_query` MCP tools, called as an upstream MCP server
  (composable with kr0ki's own `McpTool`/`GET /mcp/tools` dispatch pattern —
  see `EVAL-reqif-opa-mcp.md`), plus a small new mapping layer into
  `RequirementDefinition`/`RequirementUsage` + `Satisfy`/`Verify`, is the
  entire scope of closing this gap.
- **F (governance) — the real opportunity, composition not invention.**
  Nobody surveyed has connected Flexo's branch/commit/lock model +
  `reqif-opa-mcp`'s "agents produce facts, policy is the sole gate" pattern +
  `Provenance`'s existing per-element source tracking into one coherent
  answer to Ji's three principles. A future design (its own brainstorm,
  architectural scope) could plausibly be: an AI-generation agent's output
  lands as a Flexo branch/commit tagged AI-proposed (never a direct write to
  a shared branch); before merge, an OPA-style policy evaluates
  agent-supplied facts (did every element pass `validate_sysml_v2`? does it
  carry required `Provenance`?) as the sole gate; `Provenance` gains an
  orthogonal, additive epistemic-status/authorship axis (a closed enum, data
  not code — consistent with the house style `DESIGN-NOTE-typed-model-layer`
  already established, and explicitly NOT a `SchemaVersion`-style versioned
  field, which that note already rejected). **This is a real, citable, mostly
  novel contribution** — Ji's paper states no concrete architecture exists
  for these principles anywhere; kr0ki would be composing existing, already-
  evaluated pieces into a first instance, not duplicating prior art.
- **E (storage/versioning) — no fix available, track as a known limitation.**
  Flexo's missing diff/merge/webhooks remains unresolved by every source
  surveyed. Any multi-agent design must be built around this limitation
  (poll-based commit discovery, as kr0ki's read path already does), not
  assume it away.
- **B (procedural extraction) — flagged, not scoped.** A real, unaddressed
  gap (no source touches `ActionUsage`/`StateUsage` generation), but nothing
  surveyed offers a starting pattern for it either. Needs its own future
  investigation, not a recommendation here.
- **G (multi-agent concurrency) — explicitly deferred.** See §3's closing
  paragraph. Do not start implementation work here without a dedicated
  brainstorm.
- **Layout (a sub-concern of rendering, not its own MECE axis here since it's
  already kr0ki's box-5 territory) — low-priority evaluation candidate.**
  `EVAL-sysmlv2-learning-all-in-one.md` surfaces ELK.js as a genuine gap
  (kr0ki has no domain-aware auto-layout, fully delegates to Kroki backends).
  Worth a future look for per-`ViewDefinition` rendering (FR4), not urgent.

## 5. Explicitly out of scope / not decided here

- No implementation plan. No new Rust code beyond the doc corrections in §0.
- The governance composition sketched in §4 is a **direction**, not a design
  — it has not been through `superpowers:brainstorming`'s approval gate and
  should not be treated as approved.
- The multi-agent concurrency question (axis G) is named, not answered.
- Procedural/behavioral extraction (axis B) is named, not scoped.

## 6. If this note's direction is pursued next

Per `superpowers:brainstorming`, the governance composition in §4 and the
concurrency question in §3/G are each their own **architectural**-scope
decomposition (new subsystems, cross-component interfaces) — not a single
plan. Splitting them into separate sub-project specs, each through its own
brainstorm → design → plan cycle, is the expected next step if the user wants
to proceed past this research synthesis.
