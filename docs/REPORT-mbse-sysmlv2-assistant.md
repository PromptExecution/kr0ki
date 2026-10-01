# What an AI SysML v2 Modeling Assistant Can Do: Deliverable Now vs Later

_Report prepared 2026-10-01 from the repo's design notes and module headers; no tests were run for it. Status labels reflect what the code and docs state._

Basis: kr0ki design notes, evaluations, module headers and signatures. Status labels reflect what the code and docs state. I did not run the test suite, and the write path has not been confirmed against a live reference server (PLAN-006 §2 says so). SysML v2 facts from the web are marked [web].

## 1. Executive summary
An assistant can read a SysML v2 model from a server, check it against written rules, draw it, import and export requirements, and propose edits for human approval. In kr0ki today, reading, rule checking, requirement round-trips, a guarded write path and a few relationship diagrams are built; AI-driven authoring is not. The sound design has the AI propose and a validator and a human decide, so every change is diffed, checked and approved before it reaches the model. It will not replace engineering judgment: it cannot decide what the system should do, certify that requirements are correct or complete, or guarantee a vendor tool accepts its output. It does not yet draw block, internal-block, state or sequence diagrams. SysML v2 and its API were formally adopted by OMG on 30 June 2025 [web: omg.org news release, 21 Jul 2025], so tooling is young, and slow progress by any provider is not unusual.

## 2. Action catalog
"Auto" means a machine can do it unattended. "Human" means a person must decide or approve.

| Group / action | Input -> Output | Auto | Human | kr0ki status (evidence) |
|---|---|---|---|---|
| **Understand/query**: list projects, branches, tags, commits, elements, relationships; snapshot with content hash | Server URL, token -> typed snapshot | All | Choose project | **BUILT**: `kr0ki-sysmlv2-client/src/lib.rs` (`projects`, `branches`, `commits`, `elements`, `relationships`, `snapshot`); HTTP `/model/projects*`, `/model/graph/query` in `kr0ki-server/src/app.rs`. Live test exists but is `#[ignore]`d (`tests/live.rs`) |
| Lift model to a canonical typed graph | Snapshot -> graph | Yes | Review mapping | **PARTIAL**: `ufo_graph.rs`; the ontology layer is upstream (`ufo-types`) |
| **Author/edit elements**: write elements to a project | Typed graph -> commit | Mechanics yes | Approve content | **PARTIAL**: `SysmlV2Client::create_commit`; `sync_engine.rs` (branch-head resolution, stale-head check, 409/412 retry, diff). Only for kr0ki-owned `dbt:`/`reqif:` identifiers (`digital_thread_sync.rs`). No relationship sync. Live OMG-server run still pending |
| Natural language -> new SysML v2 text | Prose -> validated `.sysml` | Draft only | Everything semantic | **NOT BUILT** (DESIGN-NOTE §2 axis A "nothing"). Structural text emission from Rust types is shipped upstream (`sysml-derive`, `ufo_types::mbse`; EVAL-sysml-derive). Behavior (Action/State) generation is an unaddressed gap (axis B) |
| **Requirements & ReqIF**: import ReqIF/ReqIFz; export; store as `RequirementUsage` | ReqIF file <-> requirement graph <-> project | Parse, hash, diff | Decide what a requirement means | **BUILT, with limits**: `reqif_import.rs`, `reqif_export.rs`, `flexo_reqif_sync.rs`, `reqif_fetch.rs`; HTTPS fetch hardened. Export is round-trip-grade for kr0ki only and is not guaranteed to be accepted by DOORS/Polarion (own header) |
| Extract requirements from documents (PDF/Word) | Doc -> candidate requirements | Draft | Accept/reject each | **NOT BUILT** (reuse candidate: `reqif-opa-mcp` docling, EVAL-reqif-opa-mcp) |
| **Traceability & verification**: satisfy/verify/allocation | Requirements <-> design <-> evidence | Propose links, find gaps | Confirm links | **PARTIAL**: `requirements_sync.rs` writes inferred Satisfies edges from rule results; promotion to asserted exists but is not persisted (own header). `Satisfy`/`Verify` vocabulary exists; no verification-evidence ingestion |
| **Consistency & rule checking**: Rego rules stored in the model | Snapshot + RuleDocument -> violations | Yes | Write and own the rules | **BUILT**: `rule_docs.rs`, `rule_eval.rs` (regorus backend), `recompute.rs`. Rules are the team's to author; SSRF-capable builtins disabled |
| **View generation**: relationship diagrams (D2/Mermaid), requirements view, topology | Relations -> diagram | Yes | Judge usefulness | **PARTIAL**: `sysml_lift.rs`, `sysml_render.rs` (`to_d2`, `to_mermaid`), `requirements_render.rs`; render service with cache. **BDD, IBD, state, sequence, `ViewDefinition`: NOT BUILT** (AGENTS §1 Box 4; behavior views are a named gap) |
| **Change impact**: what is affected by a change | Two snapshots -> affected set | Graph walk, diff | Interpret | **PARTIAL**: content-hashed snapshots and `diff_managed` in `sync_engine.rs` give the diff; no impact-report feature |
| **Baselines/commits/review** | Commit, tag, compare | Mechanics | Approve, release | **PARTIAL**: commits and tags readable; baseline commit write for ReqIF; no diff/merge/webhooks in Flexo (EVAL-flexo, per DESIGN-NOTE §2 E); no review UI |
| **Reporting**: coverage, violations, freshness | Snapshot -> report | Yes | Narrative | **NOT BUILT** as a feature; inputs exist (violation lists, requirement graph) |

## 3. Skills design
One Markdown file per task. Each file lists trigger, knowledge to load, and guardrails. Global rules for all skills: (G1) no write without a draft the user approved; (G2) always show a diff against current branch head before commit; (G3) every AI-authored element must pass the SysML v2 parser/validator first (the pattern EVAL-sysmlv2-learning calls "bind hash + validator result, then deliver"); (G4) commit to a review branch, never the shared branch; (G5) record provenance (who/which model/which source) on each element; (G6) cite source text for every extracted claim.

| # | Skill | Trigger | Loads | Specific guardrails |
|---|---|---|---|---|
| 1 | model-orientation | "what is in this model?" | Snapshot, element-kind glossary (`VOCABULARY.md`) | Read-only |
| 2 | element-lookup | "find / explain element X" | Element, its relationships, glossary | Read-only; quotes the model verbatim, labels any inference |
| 3 | requirement-extract | Upload of spec document | Docling output, house requirement style | Output is a draft list with quoted source; no auto-merge |
| 4 | reqif-roundtrip | Import/export a ReqIF | `reqif_import/export` limits | Warn that export is not vendor-certified; hash check on re-import |
| 5 | requirement-quality | "review requirements" | Rule set (shall, atomic, testable) | Flags only; never rewrites silently |
| 6 | trace-proposer | "link requirements to design" | Requirement graph, part tree | Proposed links marked inferred until a human promotes |
| 7 | verification-gap | "what is unverified?" | Satisfy/Verify edges | Reports gaps; does not invent evidence |
| 8 | rule-author | "add a rule: every requirement needs an owner" | Rego examples, `RuleDocument` shape | Test rule on the live snapshot and show hits before saving |
| 9 | violation-triage | A rule reports a violation | Violation, element, rule text | Proposes a fix or a waiver with reason; a human chooses; never edits the model itself |
| 10 | structure-drafter | "add a subsystem" | Parent element, naming conventions | Emits SysML text, validates, shows diff; commits to review branch |
| 11 | view-generator | "draw the allocation of X" | Relations, supported diagram kinds | Say which views are unsupported rather than approximating |
| 12 | impact-analysis | "what if we change X?" | Two snapshots, trace links | Distinguishes direct links from inferred ones |
| 13 | baseline-reviewer | Before a release | Commit diff, open violations | Produces review pack; a human signs |
| 14 | status-report | Weekly | Coverage, violations, freshness | Numbers computed from the model, not generated text |

## 4. Evaluation in 30 days
Judge any provider (consultant, tool or AI) by artifacts, not hours, and ask every provider for the same ones. Agree the numbers on a baseline snapshot in week 1.

| Acceptance test | Measure | Pass signal (set your own thresholds) |
|---|---|---|
| Requirements captured | N requirements in the model with source, owner, ID | Count and % reconciled to the source document |
| Traceability | % requirements with at least one satisfy link and one verify link | Reported weekly; gaps listed by name |
| Rule violations | M violations found by written rules; % resolved or waived with reason | Rules readable by your staff, reproducible by rerunning |
| Diagram freshness | Diagrams regenerated from the live model; age of latest vs. last model commit | Zero hand-drawn diagrams without a model source |
| Model health | Elements passing validator; open unnamed/orphan elements | Trend downward |
| Change history | Commits with description, author (human/AI) and approver | 100% of AI changes reviewable |


## 5. Risks
- **Hallucinated model content.** Plausible but wrong elements, or invented trace links. Mitigation: G1-G6; parser validation proves syntax only, never business correctness (EVAL-sysmlv2-learning). Spot audits.
- **Tool lock-in (two ways).** Dependence on the AI vendor, and on a model format. Mitigation: the OMG API and ReqIF are open interfaces, and kr0ki keeps its own typed layer upstream; export remains partial (own header).
- **API variance across vendors.** The standard is new [web: adopted June 2025]. kr0ki has `PageParamStyle` to cope with paging differences, is not yet proven against a live reference server in this repo's own notes, and Flexo lacks diff/merge/webhooks.
- **Confidentiality.** Models and requirements may be sensitive. Prefer a self-hosted or private model endpoint; send nothing to public services; a bearer token is the only server auth today ("FR7 minimal", AGENTS §7).

## 6. 30/60/90-day plan
**First three demonstrable deliverables**
1. **Model health report (week 2-3).** Connect read-only to your existing SysML v2 server, take a snapshot, run a first rule set, and deliver a violations list plus dependency diagram. Mostly built; the effort is connection and rule writing.
2. **Requirements round-trip (week 3-4).** Import your current ReqIF or document requirements, show them as model elements with a requirements diagram, and export back. Built, with the export caveat; test with your actual tool before relying on it.
3. **Traceability gap report (week 4-5).** Coverage of satisfy/verify links with proposed links for human confirmation. Partially built; link proposal is the new work.

**Days 1-30:** read-only access, baseline metrics (section 4), first rules, deliverables 1-2. **Days 31-60:** traceability, skills 3-9, review-branch workflow, weekly report. **Days 61-90:** AI-drafted structure with validator and approval (skill 10), first state/block views, live test against your vendor's server, decision on scope. Do not promise behavior modeling or concurrent multi-user editing (unsolved, DESIGN-NOTE axis G) inside 90 days.

Sources: OMG news release on adoption (omg.org/news/releases/pr2025/07-21-25.htm); github.com/systems-modeling/sysml-v2-release (OpenAPI REST implementation).
