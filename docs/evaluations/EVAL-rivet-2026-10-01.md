# G-rivet: evaluation of pulseengine/rivet (2026-10-01)

## 1. What it is / maturity
Rivet is a git-native, schema-driven artifact store and validator for safety-critical SDLC traceability, written in Rust. One binary has a CLI, an axum+HTMX dashboard, an LSP, and an MCP server. It targets AI-agent-authored artifacts gated by `rivet validate`. Artifacts are YAML in the repo.
Maturity (from `gh api` and the repo):
- Created 2026-03-07, so about 7 months old. Pushed today.
- v0.40.0 released 2026-10-01; v0.39.0 on 2026-09-25 and v0.38.0 on 2026-09-23. Releases are very fast and pre-1.0.
- 2 stars, 0 forks, 26 open issues.
- Contributors: avrabe 837 commits, "claude" 9, 1 bot. This is effectively a single-maintainer, heavily AI-assisted project.
- License: GitHub API reports none, there is no LICENSE file in the repo root, but Cargo.toml and the README badge say Apache-2.0. Treat the license text as missing (flag for legal).
- Tests: about 2,600 `#[test]` occurrences, 33 integration test files (proptest, yaml-test-suite, fuzz). CI claims Playwright, Kani, Verus, Rocq and mutation testing (README claim, UNVERIFIED).
- The tool self-declares "self-claimed" tool confidence, not qualified (`docs/rivet-is-not.md` section 4).
- Build: `cargo build --release -p rivet-cli` did NOT finish within my 10 minute cap (stopped while compiling deps). I did not run the CLI, so all CLI behaviour is UNVERIFIED by execution.

## 2. Data model
- Artifacts are YAML files under `artifacts/` (`id`, `type`, `title`, `status`, `fields`, `links: [{type, target}]`). IDs are free-form strings such as REQ-001 or DD-001, with `rivet next-id`. There is a provenance stamp, an `ai-session` record, and a sha256 `cited-source` field.
- Types and links are defined by YAML schemas. `schemas/dev.yaml` types: requirement (priority must/should/could/wont; category functional/non-functional/constraint/interface), design-decision, feature, verification.
- Link types in `schemas/common.yaml`: traces-to, satisfies, refines, verifies, implements, derives-from, derives-from-external, mitigates, allocated-to, constrained-by, and others.
- 28 schemas shipped in `schemas/`, verified by file listing: aspice, iso-26262, iec-61508, iec-62304, do-178c, en-50128, sotif, iso-pas-8800, eu-ai-act (plus bridge schemas), stpa, stpa-sec, stpa-ai, safety-case (GSN), cybersecurity, supply-chain, aadl, score, vv-coverage, dev, common, research. Schemas also carry traceability rules (`requirement-coverage`, etc.), oracles, pipelines and s-expression rules.
- Requirement semantics are thin by design: a flat typed record with a status. There is no requirement-definition/usage split, no typed constraint expressions and no SysML metamodel.

## 3. Capabilities
- Traceability and validation: `validate`, `coverage`, `matrix`, `trace`, `impact`, `diff`, `stats`, `baseline`, `lock`, `sync` (cross-repo externals), `commits` (commit-trailer to requirement check), `close-gaps`, `audit`. Source: `rivet-cli/src/main.rs` command list.
- ReqIF: YES. `rivet-core/src/reqif.rs` (3,467 lines) is a ReqIF 1.2 XML import/export adapter built on quick-xml. It maps Artifact to SPEC-OBJECT and Link to SPEC-RELATION. CHANGELOG records round-trip fixes. The README says ReqIF round-trip is one of three documented formal-verification gaps. The mapping is flat (all attributes become ATTRIBUTE-VALUE-STRING), so it is weaker typing than a full reqrs mapping.
- SysML v2: NO. No SysML code, import or export. The only `.sysml` mention is a design doc noting that the Eclipse SCORE corpus has zero `.sysml` files. Rivet's architecture modelling is AADL via its own `spar` crate (git dependency).
- Query: `rivet query` filter engine, an s-expression predicate language (`sexpr.rs`, `sexpr_eval.rs`), and an optional `sql` feature (gluesql). There is no SPARQL or Rego.
- UI: dashboard (`rivet serve`), VS Code extension (`vscode-rivet/`), LSP (`rivet lsp`), MCP (`rivet mcp`). Other formats: OSLC client (feature flag), Needs JSON, generic YAML, and Zola/Gherkin exporters.
- CI: `rivet validate` exit codes, commit-trailer checks, pre-commit docs. The project's own dashboard is published on every push to main.
- Embedding as a Rust crate: `rivet-core` is a lib with 59 public modules, built on salsa for incremental computation. BLOCKER: it depends on git forks (`pulseengine/rowan` branch `fix/miri-soundness-v3`, `pulseengine/spar` tag v0.10.0), so it is not crates.io-publishable as is. The API is unstable at v0.40 with a 1-2 release/week cadence. There is a heavy clippy-restriction posture but no semver promise.

## 4. Patterns worth borrowing
1. Schema-as-data traceability rules and "common-mistakes" with fix-commands per type. See `schemas/dev.yaml` (`requirement-coverage`, `approved-needs-description`, `common-mistakes`/`fix-command`). This is good for non-expert UX.
2. Coverage-gap closing loop: oracle, rank, close-by-link, emit commit trailer. See `rivet-cli/src/close_gaps.rs`, `agent_pipelines.rs`, and `pipelines` in the schemas.
3. Commit trailers tying every change to a requirement ID, enforced in CI. See `rivet-core/src/commits.rs` and `rivet commit-msg-check`.
4. `cited-source` as a sha256-pinned external reference (file/url/github/oslc/reqif/polarion) for drift detection, plus `rivet.lock` for baselines. See `rivet-core/src/cited_source.rs`, `externals.rs`, `schemas/common.yaml`.
5. `rivet-is-not.md` limit statements, `rivet docs` and `rivet quickstart` embedded in the binary, with the LSP serving the same docs on hover. See `docs/rivet-is-not.md` and `rivet-cli/src/docs.rs`. Also a provenance and `ai-session` stamp on every mutation (`common.yaml`).

## 5. Fit vs kr0ki
What Rivet adds: a ready UX layer (dashboard, matrix, coverage, LSP, MCP, docs), many compliance schemas (ASPICE, 26262, EU AI Act), commit-trailer enforcement and baseline/lock workflows.
Overlap: ReqIF import/export (kr0ki already has it via reqrs, with Flexo-backed RequirementUsage), traceability and coverage (kr0ki has Rego rules plus an RDF graph), and agent/MCP surfaces.
Gaps vs kr0ki's goal: no SysML v2 model, no RequirementDefinition/Usage, no satisfy/verify/derive as SysML relationships. Its truth store is YAML in git, a second source of truth next to Flexo. Its schema model is not UFO-typed.
Options:
- Embed: not recommended. It needs git-forked rowan and spar, the API churns weekly, the repo is a single maintainer with 2 stars, the license file is missing, and it would duplicate kr0ki's store.
- Interoperate via files: viable and cheap. kr0ki exports ReqIF or a Rivet-schema YAML projection so Rivet's dashboard, coverage and ASPICE schemas can run over it. Its ReqIF flattening needs a round-trip test before relying on it.
- Ignore: fine if no compliance-schema need exists.
Risks: Rivet's own limits doc says it is not a safety case and its tool-confidence claim is self-claimed. Bus factor 1. Pre-1.0 churn. Missing license file. Its CLI was not run by me (build cap), so quality is UNVERIFIED by execution.

RECOMMENDATION: Do not embed or adopt. Borrow patterns 1, 2, 3 and 5 into kr0ki's requirements surface (coverage-gap rules in Rego, per-type common-mistakes with fix-commands, trailer enforcement, embedded docs). Optionally run a time-boxed file-interop spike (ReqIF export from kr0ki into `rivet import`, then `rivet coverage`) only if a compliance schema such as ASPICE or EU AI Act becomes a requirement. Re-evaluate at v1.0 and once a LICENSE file and crates.io-publishable deps exist.
