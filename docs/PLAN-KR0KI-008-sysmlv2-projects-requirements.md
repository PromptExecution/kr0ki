# PLAN-KR0KI-008 — SysML v2 projects, requirements traceability and an MBSE "V" agent

_2026-10-01 (decisions recorded the same day). Evidence: [`evaluations/`](evaluations/) (Rivet, RDF stores, inventory). **FACT** = verified; **UNVERIFIED** = not run. Target release: **0.1.0** (first minor)._

## 1. Goal
A non-systems-engineer can keep a **SysML v2 project** (textual `.sysml`/`.kerml`, diagram sources, ReqIF) in the playbook, **save (commit) it with a message**,
and see **which requirements each change affected**, the **graph of linked requirements**, and a **cost roll-up under different scenarios**. Local storage is one
backend; the OMG-API server (Flexo) is the other; the same commit shape serves both. An MBSE agent (INCOSE "V": needs -> requirements -> design -> verification -> validation) helps.

## 1a. Owner decisions (2026-10-01)
| # | Decision | Consequence |
|---|---|---|
| D1 | **Oxigraph approved** for the server graph/SPARQL store | WP7 proceeds with `oxigraph` (`default-features=false`); SHACL as SPARQL shapes first |
| D2 | **Compliance chain, not ASPICE/ISO 26262:** EU AI Act → Australian AI guardrails (and the Government *Guidance for AI Adoption*) → ISO/IEC 42001 → NIST AI RMF → AESCSF / CIRMP | The Rivet file-interop spike is dropped. New WP: **compliance tags + coverage** on requirements (`framework:control-id`), driven by a sourced crosswalk ([`evaluations/AI-GOVERNANCE-crosswalk-2026-10-01.md`](evaluations/AI-GOVERNANCE-crosswalk-2026-10-01.md)) |
| D3 | **Cost is attribution, not a number.** A requirement carries *accounting-style attribution codes* (like cost-centre / WBS / activity tracking codes), not a dollar amount or a bare numeric. Mapping requirements to a solution and turning that into budget estimates is **TBD** | The 0.1.0 numeric `attribute cost` becomes a temporary weight; replaced by an **attribution ledger** (below). Money is a *later, separate* layer keyed by code |
| D4 | **Priority: install and test a SysML v2 API server with an MCP interface as a kr0ki sidecar** | New WPs S1/S2 come first: they unblock live testing of everything that was fixture-only (WP6, WP9, WP10) |

### Cost attribution model (D3) — design
- **Attribution line** = `{ code, share, basis, status }`. `code` is a hierarchical accounting code such as `CC-4410/WBS-2.3/ACT-07` (cost centre / work breakdown / activity); `share` is the fraction (0–1) of the requirement attributed to it (a requirement's shares sum to ≤ 1; the rest is *unattributed*, which is reported, never hidden); `basis` = allocation | estimate | actual; `status` = planned | estimated | committed | actual.
- **Code book**: a project-level list of valid codes (name, owner, parent) so codes are checked, like a chart of accounts. Unknown codes are flagged.
- **SysML v2 native**: a user-defined `metadata def CostAttribution { attribute code : String; attribute share : Real; }` applied as `@CostAttribution { code = 'CC-4410/WBS-2.3'; share = 0.6; }` on a requirement (or on the `allocate` that maps it to a solution element). No bare dollar attribute.
- **Roll-up** groups by code and by code prefix (hierarchy) across derived requirements, per scenario; it shows *attribution coverage* (requirements with no code, shares that do not sum), not a total. Cost *amounts* appear only when a budget model (code → estimate, TBD) is supplied.
- **Scenarios** = sets of requirements and, later, alternative solution allocations; output is a code × scenario matrix.

## 2. What the evaluations decided
| Question | Finding | Decision |
|---|---|---|
| **Rivet** (pulseengine) as the requirements surface? | Rust, YAML-in-git, ReqIF 1.2 in/out, 28 compliance schemas (ASPICE, ISO 26262, EU AI Act ...), CLI/dashboard/LSP/MCP; **no SysML v2, no requirement def/usage**; 7 months old, 1 maintainer, 2 stars, no LICENSE file, git-forked deps (not embeddable); CLI not run (build > 10 min) | **Do not embed or adopt.** Borrow patterns: coverage-gap rules, per-type "common mistakes + fix command", commit-trailer to requirement id, embedded docs. Interop spike **dropped** (D2: our frameworks are the AI-governance chain, not ASPICE/26262) |
| **Graph store** | `graph_store.rs` is a `Vec<oxrdf::Triple>`, no SPARQL. **oxigraph 0.5.11** (`default-features=false`) was correct on every spike query: sub-select, `dependsOn+`, `SUM`, `GROUP BY`, `(a\|b)+`; keeps `xsd:integer`; 105 s build; wasm32 builds (`js` feature; browser run UNVERIFIED). grafeo 0.5.43: built-in SHACL, but SUM returned Float64, dropped a zero-length-path row, rejects `(a\|b)+`. **zu**: not RDF, "nothing is usable yet" | **Server store: oxigraph.** Reject zu; revisit grafeo later |
| **SHACL** | none in oxigraph. rudof `shacl_validation` works only with pinned `0.2.9` companions; its SPARQL mode disagreed with native | **Phase 1: shapes as SPARQL `ASK`/`SELECT` in oxigraph** (no extra crate). Phase 2: rudof (pinned) over a Turtle export |
| **ReqIF today** | import PARTIAL (parses via reqrs; attachments inventoried only), export PARTIAL (not 1.2-valid; drops attributes/hierarchy/attachments), **no HTTP route for export or baseline write/read** | WP4/WP8 |
| **SysML v2 requirements** | stored as one `RequirementUsage` per ReqIF requirement with relations in a JSON blob; **no `RequirementDefinition`, no Satisfy/Derive/Verify/Allocate elements, no VerificationCase**; no history/diff; no diagram<->requirement trace on the server | WP5/WP9 |

## 3. Architecture
```
 browser (playbook)                                   server (kr0ki)                          model server
 ┌──────────────────────────┐   commit shape    ┌───────────────────────┐   OMG API      ┌───────────┐
 │ Projects view            │ ────────────────► │ /projects/{id}/push   │ ─────────────► │ Flexo /   │
 │  files (.sysml,.d2,.reqif)│  (blobs + traces)│  parse (sysml-v2-parser)│  commits      │ SysML v2  │
 │  Quasar LocalStorage      │ ◄──────────────── │  oxigraph store        │ ◄───────────── │ pilot     │
 │  commits + traces + cost  │      pull         │  SPARQL / SHACL(ASK)   │                └───────────┘
 └──────────────────────────┘                    └───────────────────────┘
```
- **Commit** = `{id = sha256(body), parent, message, tree{path→blob}, changes, traces[], impact.requirements[]}` (implemented in `playbook/src/lib/projects.js`, browser backend).
- **Trace** = `{requirement, relation (declares|satisfy|verify|derive|refine|allocate|depicts|<reqif kind>), target, artifact}`. Requirement ids are SysML short names (`<'REQ-1'>`).
- **Impact** of a commit = requirements declared in, or linked from/to, a changed artifact (now or in the parent). A diagram can *depict* a requirement, so editing it flags that requirement.
- **Cost roll-up** (local first): numeric `attribute cost = N` on a requirement; `rollup(r) = own(r) + Σ rollup(derived-from-r)`, cycle-safe; a **scenario** is a set of included requirements. Server-side the same query is a SPARQL property-path + `SUM` (spiked and verified in oxigraph).
- **Quasar LocalStorage** is the browser backend (synchronous, ~5 MB/origin, shared): usage is reported, a commit that does not fit rolls back and says why, projects export/import as verified bundles. IndexedDB is the next backend if projects outgrow it.

## 4. INCOSE "V" coverage by release
| V stage | 0.1.0 (this release) | Later |
|---|---|---|
| Stakeholder needs | ReqIF import into a project (needs as requirements) | needs capture/elicitation skills; source-to-need links |
| System requirements | requirement scan + ids + derive links; quality hints | RequirementDefinition/subject/assume-require; EARS checks; Rivet-style "common mistakes" |
| Architecture / design | SysML text in the project, diagrams that *depict* requirements | allocation/satisfy as real elements (WP5); parts/ports authoring |
| Implementation | — | code-to-requirement links (commit trailers) |
| Verification | `verify` links, coverage gaps (requirements with no verify/satisfy) | VerificationCaseUsage; evidence ingestion |
| Validation | — | needs-vs-acceptance trace |

## 5. Work packages (ordered; each has an acceptance test)
**Priority block (D4): the SysML v2 sidecar.** Everything below it that touches a real model server depends on these.
| WP | What | Release | Acceptance |
|---|---|---|---|
| S1 | **SysML v2 API server as a kr0ki sidecar** (OMG Systems Modeling API; pilot implementation in a rootless podman container, in the kr0ki pod); `just` recipes to build/start/stop; seeded demo project | 0.1.x | server healthy in the pod; `GET /projects`, create project/commit/elements through the API; **kr0ki's existing live client tests pass against it** |
| S2 | **SysML v2 MCP sidecar** (parse/validate/diagnostics/symbols over MCP, exposed over localhost HTTP by a fixed-command bridge); registered in kr0ki's MCP manifest/agent | 0.1.x | MCP `initialize` + `tools/list` + `parse`/`validate` through the container; the agent can call it; the same package's LSP (if present) feeds the editor |
| S3 | kr0ki **sync/ReqIF/rules tests run live** against S1 (replaces fixture-only claims) | 0.1.x | live tests in CI-optional `just test-live-sysml` |
| | **Remaining work packages** | | |
| WP | What | Release | Acceptance |
|---|---|---|---|
| 1 | **Projects store** on Quasar LocalStorage: commit/log/checkout/diff/export/import, drafts, quota-safe | 0.1.0 | **DONE** 16 store tests incl. tamper rejection, rollback, chain order; mutation-checked |
| 2 | **SysML v2 text scanner** for requirements + links + numeric attributes (browser, heuristic; server parser is the authority) | 0.1.0 | **DONE** 8 scanner tests |
| 3 | **Projects view**: files, editor, Save (commit), history with impacted requirements, per-requirement timeline, usage meter, export/import | 0.1.0 | **DONE** 12 component tests + live browser check |
| 4 | **Requirements graph + cost roll-up** with scenarios, rendered through kr0ki's D2; coverage gaps; duplicate ids | 0.1.0 | **DONE** roll-up tests (cycles, scenarios); graph rendered live; D2 `$` escaping found by the renderer and fixed |
| 5 | **ReqIF import into a project** (existing `/requirements/import`) -> requirements as SysML text + traces | 0.1.0 | **DONE** verified live with the repo's ReqIF fixture |
| 6 | Server: **ReqIF export route**, requirement-baseline write/read routes (functions exist, no routes) | 0.2 | HTTP tests; round-trip through reqrs |
| 7 | Server: **oxigraph-backed `GraphStore`** + `POST /sparql` (read-only, bounded) + Turtle export of SysGraph/requirements | 0.2 | spike queries as tests; xsd numerics preserved |
| 8 | **SHACL as SPARQL shapes**: untested requirement, orphan requirement, missing cost/id; starter rule pack with fix hints (Rivet pattern) | 0.2 | violations on the spike data match |
| 9 | Write **Satisfy/Derive/Verify/Allocate and RequirementDefinition** as real SysML v2 elements (flexo_reqif_sync) | 0.3 | live round trip on a reference server (needs URL/token) |
| 10 | Server **push/pull** of project commits + SysML text parse endpoint (`sysml-v2-parser`) + trace store; conflict handling | 0.3 | two-client divergence test |
| 11 | **MBSE "V" agent skills** (see `REPORT-mbse-sysmlv2-assistant.md`; 14 skills) on the skills harness | 0.2-0.3 | per-skill acceptance prompts |
| 12 | rudof SHACL (pinned) over Turtle export; oxigraph WASM in the browser | 0.4 | only if SPARQL shapes prove too limiting |

## 6. Risks and open decisions
- **Heuristic scanner vs real parser:** the browser scanner can misread exotic SysML v2 (it is labelled a heuristic). Mitigation: WP10 parser endpoint; the scanner never blocks a save.
- **localStorage quota:** ~5 MB shared; big ReqIF/diagrams will not fit. Mitigation: quota meter, export, IndexedDB backend.
- **No live SysML v2 server yet:** WP9/10 need a URL/token for acceptance (owner).
- **Owner decisions:** (a) confirm oxigraph for the server store and SPARQL-ASK-first SHACL; (b) whether compliance schemas (ASPICE/ISO 26262/EU AI Act) are in scope (would justify the Rivet file-interop spike); (c) cost model: free-form numeric attribute (0.1.0) or typed quantities with units (SysML `MassValue`-style) later; (d) canonical brand/palette for the Projects UI.
