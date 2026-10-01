# kr0ki requirements / ReqIF / SysML v2 inventory (read-only audit, 2026-10-01)
`cargo test -p kr0ki-core --lib`: 236 passed, 0 failed. Paths below are under crates/kr0ki-core/src unless noted. RequirementGraph types live in upstream ufo-types (rev ee87348, mbse/requirements.rs), re-exported as `kr0ki_core::requirements`.

## 1. Capability table
| Capability | Status | Evidence |
|---|---|---|
| ReqIF import (XML) | PARTIAL | reqif_import::import_reqif_artifact: parses via reqrs 0.2.2, lowers to RequirementGraph via ufo_types::reqif. Kept: id, title, text, `attributes` map, relations, provenance, baseline hash (SHA-256). Specification/SpecHierarchy handling, DatatypeDefinition detail, XHTML rich text: UNVERIFIED (lowering is upstream; not read) |
| ReqIFz | PARTIAL | reqif_import::import_reqifz: ZIP bomb limits (32 MiB, 4096 entries), multiple .reqif docs; attachments are inventoried only (ReqIfAttachment), "kr0ki does not make attachments durable" |
| ReqIF fetch by URL | BUILT | reqif_fetch::fetch_reqif_url (HTTPS-only, SSRF-hardened); route POST /requirements/import/url |
| ReqIF export | PARTIAL | reqif_export::export_bundle_to_xml. Self-declared "round-trip-grade for kr0ki/reqrs, not OMG ReqIF-1.2 valid": omits LAST-CHANGE, MAX-LENGTH, header fields, SPECIFICATIONS. Drops Requirement.attributes, promotion audit trail, baseline revision, evidence endpoints, attachments. One synthesized SpecObjectType + relation types ST-DERIVES etc. Only asserted relations. No HTTP route found for export (UNVERIFIED: grep of app.rs routes shows none) |
| Requirement graph + relation kinds | BUILT (upstream) | RequirementRelationKind: Contains, Derives, Refines, Requires, Satisfies, Verifies, Implements, Traces, AllocatedTo, Precedes. RelationAuthority: Asserted / Inferred / Proposed (+Promotion actor/rationale). Evidence refs (EvidenceRef) |
| Views | BUILT | ViewKind: Decomposition, Traceability, Impact, Behaviour, VerificationCoverage; TraversalDirection Upstream/Downstream/Both; POST /requirements/views, POST /render/requirements-view -> D2 (requirements_render::to_d2) |
| SysML v2 requirement elements written | PARTIAL | flexo_reqif_sync::sync_requirement_baseline writes one `RequirementUsage` per requirement, identifier `reqif:<baseline>:<req>`, custom fields name/text/reqif_* (provenance, attributes, evidence, relations). No RequirementDefinition, no subject, no ConstraintUsage/assume/require, no reqId/documentation fields. Relations stored as JSON in `reqif_relations`, NOT as SysML Satisfy/Derive/Verify elements. No HTTP route found for this write (only /sync for dbt: nodes, digital_thread_sync) |
| SysML v2 requirement elements read | PARTIAL | flexo_reqif_sync::fetch_requirement_baseline / requirement_graph_from_elements reads back its own managed elements only |
| satisfy/verify/refine/dependency/allocation read | PARTIAL | ufo_graph.rs: KerML Satisfy->Satisfies, Verify->Verifies, Refine->TracesTo, Dependency->Requires; sysml_lift.rs lifts to Relation::Satisfy/Verify/Allocation; sysml_render.rs draws D2/Mermaid. Allocation deliberately NOT lifted in ufo_graph. Derive (DeriveRequirementUsage): not found (grep) -> NOT BUILT. VerificationCaseUsage: not found -> NOT BUILT |
| Writing satisfy/verify/derive/allocate as SysML elements | NOT BUILT | digital_thread_sync header: "Nodes only -- no edge/relationship sync" |
| Rules (Rego) | BUILT | rule_docs::extract_rule_docs (`@type: RuleDocument`, `rego` field); rule_eval::RegorusBackend (500 ms timeout, entry `data.kr0ki.violations` -> {element_id, reason}; errors -> unknown). Checks run against the SysGraph (structure of the SysML model), not requirement text. Backend trait is a JEV seam. POST /model/projects/{id}/recompute |
| Rules -> requirements | PARTIAL | requirements_sync::register_violations adds Inferred Satisfies edges; known limitation: re-register overwrites promoted authority. No persistence (recompute.rs "no persistence") |
| Baselines / versioning | PARTIAL | BaselineIdentity {id, revision} + sha256; sync_engine: branch-head resolve, stale-head check, 409 retry (3 attempts), full-replace payload; client create_commit, branches, tags (read) |
| History / traceability over time | NOT BUILT | Server routes list commits and snapshots (GET /model/projects/{id}/commits, .../snapshot) but no baseline diff, no per-requirement history, no change-impact-over-time (UNVERIFIED beyond route list) |
| Diagram <-> requirement links | NOT BUILT server-side | No code ties a rendered diagram/cache key to requirement ids. Browser-side only: playbook/src/lib/projects.js commits carry `traces` and `impact.requirements` (see section 4) |
| Cost/mass/other attributes | PARTIAL | only the free-form `Requirement.attributes: BTreeMap<String,String>`; no typed values/units, no roll-up, no SysML AttributeUsage/quantities (grep found none) |
| Impact / dependency traversal | BUILT (graph-level) | ViewKind::Impact + TraversalDirection on RequirementGraph; verification coverage view. Not over SysML-native links unless lifted through RequirementGraph |
| Graph/RDF store | PARTIAL | graph_store::GraphStore: in-memory triple cache, QueryShape TriplesAbout, RelatedVia, check_shapes; GET /model/graph/query. "neither triplestore nor SPARQL"; disposable, not persisted |

## 2. INCOSE V mapping
- Stakeholder needs: ReqIF/ReqIFz import is the only intake (reqif_import). Missing: needs capture/elicitation, source-to-need links, stakeholder entities, validation criteria.
- System requirements: RequirementGraph + RequirementUsage store; Derives/Refines/Contains kinds exist. Missing: RequirementDefinition, reqId, quality checks (ambiguity, EARS), ConstraintUsage/assume/require, derive links as SysML.
- Architecture/design: SysML v2 read via snapshot -> SysGraph -> D2/Mermaid (sysml_lift/render). Missing: authoring of parts/ports/allocation, SysML text parse/validate in-service (parser is conformance dev-dep only).
- Implementation: Rust-source and k8s recognizers (rust_recognizer, k8s_recognizer) lift code to graph; dbt digital thread write. Missing: link of code elements to requirement ids.
- Verification: Verifies relation + VerificationCoverage view; Rego rules as automated checks over the model. Missing: VerificationCaseUsage, test results/evidence ingestion beyond EvidenceRef, verification methods (analysis/test/inspection/demo).
- Validation: NOT BUILT (no stakeholder acceptance, no validation-vs-needs trace).
- Cross-cutting: baselines exist; change history/impact over time, approvals/promotion workflow only partly (Promotion struct, no route).

## 3. Top 8 gaps (for an MBSE agent serving non-SE users)
1. Requirement relations not persisted as SysML Satisfy/Derive/Verify/Allocate elements (JSON blob only) -> write real relationship elements in flexo_reqif_sync.
2. No HTTP routes for ReqIF export or for the requirement-baseline write/read -> expose export_bundle_to_xml and sync/fetch_requirement_baseline.
3. No requirement<->diagram/model-element traceability on the server -> add a trace store keyed by requirement id and artifact content hash.
4. No history/diff of requirements across commits -> baseline diff + per-requirement timeline over the commit list.
5. No RequirementDefinition/subject/assume-require/reqId/doc and no VerificationCase -> extend element mapping to the full SysML v2 requirement constructs.
6. Requirement quality and completeness checks absent (only Rego over model structure) -> ship starter rule pack (untested req, orphan req, unallocated req).
7. Typed attributes (cost, mass, priority, status) and roll-ups absent -> typed attribute schema + aggregation views.
8. ReqIF export not schema-valid; attachments, SpecHierarchy fidelity lost -> emit XSD-required fields, specification tree, durable attachments.

## 4. Backend needs for a browser "projects" store (commit diagrams + SysML text with requirement traces)
Already in the working tree: playbook/src/lib/projects.js (localStorage, content-addressed sha256 commits: tree, changes, `traces`, `impact.requirements`; export bundle `kr0ki-project/1`), sysmlText.js (`tracesOf`), revisionGraph.js. Its header says a server backend via /model/projects/{id}/sync "shares the same commit shape" -- but that is not true of the current route (UNVERIFIED claim).
Backend EXISTS: GET /model/projects, /commits, /commits/{id}/snapshot, /elements, /roots, /relationships; POST /model/projects/{id}/sync (accepts only a SysGraph, writes `dbt:` nodes); POST /render/*; client create_commit with branchId; read of branches/tags.
MISSING: (a) a push/pull route accepting arbitrary files (SysML text, diagram source) as blobs with commit metadata; (b) project/branch create (client has no create_project/create_branch -- only reads); (c) SysML v2 text -> element parse/validate endpoint; (d) trace storage mapping requirement id <-> file/element, queryable both ways; (e) conflict/merge for diverging browser and server heads (sync_engine handles server-side 409 only); (f) authentication per user (single shared bearer token); (g) quota/large-blob handling beyond browser 5 MB.
