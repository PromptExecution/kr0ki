# PLAN-KR0KI-008 — SysML v2 projects, requirements traceability and an MBSE "V" agent

_2026-10-01. Evidence: [`evaluations/`](evaluations/) (Rivet, RDF stores, inventory). **FACT** = verified; **UNVERIFIED** = not run. Target release: **0.1.0** (first minor)._

## 1. Goal
A non-systems-engineer can keep a **SysML v2 project** (textual `.sysml`/`.kerml`, diagram sources, ReqIF) in the playbook, **save (commit) it with a message**,
and see **which requirements each change affected**, the **graph of linked requirements**, and a **cost roll-up under different scenarios**. Local storage is one
backend; the OMG-API server (Flexo) is the other; the same commit shape serves both. An MBSE agent (INCOSE "V": needs -> requirements -> design -> verification -> validation) helps.

## 2. What the evaluations decided
| Question | Finding | Decision |
|---|---|---|
| **Rivet** (pulseengine) as the requirements surface? | Rust, YAML-in-git, ReqIF 1.2 in/out, 28 compliance schemas, CLI/dashboard/LSP/MCP; **no SysML v2, no requirement def/usage**; 7 months old, 1 maintainer, 2 stars, no LICENSE file, git-forked deps (not embeddable); CLI not run (build > 10 min) | **Do not embed or adopt.** Borrow patterns: coverage-gap rules, per-type "common mistakes + fix command", commit-trailer to requirement id, embedded docs. Optional ReqIF file-interop spike only if a compliance schema (ASPICE, EU AI Act) is required. Re-evaluate at v1.0 + licence |
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
