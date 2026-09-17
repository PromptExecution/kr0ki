# EVAL — `reqif-opa-mcp`

**Evaluated:** 2026-09-17 · **For:** a ReqIF ingestion path into
`ufo_types::sysml_model::{RequirementDefinition, RequirementUsage}` (currently
nonexistent in the b00tyverse stack), and a governance-layer pattern for a
future shared multi-agent modeling system. **Verdict:** reuse via its MCP
interface for ingestion; adopt its OPA governance pattern as a design
reference — do not vendor its Python.

---

## What it is

`PromptExecution/reqif-opa-mcp` — a deterministic **requirements-governance
pipeline**, not a modeling tool. Python/`uv`, `fastmcp`, `returns` (Rust-style
`Result`), `jsonschema`, `ulid`. Actively maintained (24 merged PRs, last push
2026-08-24), 17 test files, `justfile`-driven, CI-integrated, Azure DevOps
deployment guide, a NATS micro-service wrapper for Docling extraction. **Real
and production-oriented, not a prototype.**

Two independent pipelines behind a hard control boundary:

1. **Ingestion** (`reqif_ingest_cli/`) — source artifact (XLSX / PDF / DOCX /
   Markdown) → document graph → requirement candidate → derived **ReqIF 1.2**
   XML. Deterministic-first; an *optional* Azure Foundry LLM review hook is
   explicitly kept out of the deterministic path.
2. **Gate** (`reqif_mcp/`) — derived ReqIF + agent-supplied facts → **OPA**
   policy decision (three layers: `meta_policy` / `processing` / `policy`) →
   SARIF + an audit evidence log.

Stated house rule, verbatim from their own README: **"agents produce facts, not
pass/fail decisions — OPA remains the gate and policy authority."**

## Data model

`schemas/requirement-record.schema.json` — `uid`, `key` (human-readable, e.g.
`CYBER-AC-001`), `subtypes[]`, `status` (`active`/`obsolete`/`draft`),
`policy_baseline{id,version,hash}`, `rubrics[]` (each `{engine, bundle,
package, rule}` — a requirement carries a pointer to which OPA policy
evaluates it), `text`. **Flat and governance-shaped, not KerML-shaped** — no
containment, no typed relationships, no `Satisfy`/`Verify` edges. It answers
"is this requirement compliant," not "how does this requirement fit into a
system model."

## MCP tool surface

`reqif_mcp/server.py`, FastMCP 3.0, HTTP + STDIO transports, five
`@mcp.tool()`-decorated functions: `reqif_parse` (base64 ReqIF XML → in-memory
handle), `reqif_validate`, `reqif_query`, `reqif_export_req_set`,
`reqif_write_verification`.

Compared to kr0ki's own `McpTool` pattern (a closed Rust enum as the single
source of truth for name/schema/HTTP-binding, served via `GET /mcp/tools`,
dispatched generically by `bridge.py`): this repo predates and does not overlap
with that pattern — five hand-coded, individually-decorated tools, no separate
manifest, no generic dispatch layer. **No convention clash.** If kr0ki's
`kr0ki-mcp` bridge ever calls `reqif-mcp`'s tools as an upstream MCP server, it
composes the same way any other upstream tool would — no redesign needed on
either side.

## OPA / policy mechanism

Real OPA binary invoked as a **subprocess** (`reqif_mcp/opa_evaluator.py`),
loading Rego bundle directories from the filesystem (`.manifest`-driven) — not
embedded WASM, not a remote OPA server. Two working sample bundles ship
(OWASP ASVS, NIST SSDF). Every decision is JSON-Schema-validated on both the
input and output side and logged to an evidence store for audit.

## Maturity

Real, active, well-tested. Created 2026-01-30. Has its own roadmap doc with
explicit near/far-term sequencing; ingest-as-MCP-tools, normalized diffing, and
persistent baseline storage are self-reported as **not yet done** — declared
gaps, not hidden ones.

## Reuse candidates (not reinvented here)

ReqIF 1.2, SARIF v2.1.0, OPA/Rego, FastMCP, `docling` (DOCX/Markdown/rich-PDF
extraction), `pypdf` (offline text-layer PDF), `jsonschema`. Nothing here
reinvents ReqIF parsing, SARIF emission, or policy evaluation from scratch —
it composes existing OSS standards tooling.

## What this means for kr0ki

**(a) A ready-made ReqIF front door.** `ufo_types::sysml_model::ElementKind`
already includes `RequirementDefinition | RequirementUsage`, and `Relation`
already includes `Satisfy { requirement, subject }` and `Verify { requirement,
by }` — but there is **no ReqIF ingestion path anywhere in the b00tyverse stack
today**; requirements only exist if already hand-modeled as SysML v2. This
repo's `reqif_ingest_cli` (XLSX/PDF/DOCX/Markdown → derived ReqIF) plus
`reqif_mcp`'s `reqif_parse`/`reqif_query` gives a ready front door:

```
ReqIF (via reqif-opa-mcp) → its flat requirement-record → a thin new mapping
layer → ufo_types::sysml_model::{RequirementDefinition, RequirementUsage} +
Satisfy/Verify edges
```

That thin mapping layer is the *only* new code kr0ki-side would need — call
`reqif-mcp`'s tools as an upstream MCP server rather than vendoring its Python.

**(b) A directly transplantable governance pattern.** "Agents produce facts;
a policy engine is the sole gate" is exactly the missing primitive for a
shared multi-agent modeling system: multiple AI agents proposing model changes
concurrently need a single, auditable, non-agent authority deciding what's
accepted. This repo's three-layer gate (`meta_policy`/`processing`/`policy`)
plus its evidence/decision log is a directly reusable pattern — possibly the
literal same OPA-subprocess mechanism — for gating agent-proposed SysML v2
edits before they land as a Flexo commit, not just for compliance
requirements. See `docs/DESIGN-NOTE-agentic-mbse-generation.md` §4 (Governance)
for how this composes with Flexo's own branch/commit model.
