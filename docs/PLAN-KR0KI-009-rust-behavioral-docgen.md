# PLAN-KR0KI-009 — validated Rust behavioral models and generated views

**Status (2026-10-04):** implemented and validated for the supported compiler contracts. [Delivery evidence](behavioral-docgen-delivery.md) records the exact tested generator revision, source/container gates and full artifact digests. Static compiler evidence, unresolved dispatch and notation validation levels remain explicit; arbitrary runtime equivalence is not claimed.

**Parent plans:** [PLAN-KR0KI-002](PLAN-KR0KI-002.md) (five-box model/render pipeline), [PLAN-KR0KI-003](PLAN-KR0KI-003-rust-source-frontend.md) (Rust source front end), and [PLAN-KR0KI-004](PLAN-KR0KI-004-revisioned-procedural-workspace.md) (procedural workspace).

## 1. Goal

Generate repeatable, reviewable documentation and diagrams from Rust structures, implemented traits, dispatch, loops, and state machines. Reuse the ontology and SysML v2 view machinery already present in `ufo-types` and KR0KI. Every derived node and edge must retain its source symbol/location and the exact source revision used to derive it.

Treat generated diagrams as a view of the executable source model, not a second hand-maintained description. For state machines and dispatch tables, prefer one declarative transition/dispatch definition consumed by both the runtime and the generator. For arbitrary existing functions, label results as static analysis and preserve unresolved behavior instead of claiming exact runtime semantics.

## 2. Current baseline

- KR0KI `main` has a `syn`-based Rust recognizer, `rust_lift`, `sysml_lift`, `sysml_render`, and `POST /render/rust-source`. The route can filter the resulting relations by a `SysmlViewKind` and render D2. Current recognition includes struct/enum relationships, trait implementation and generic-bound edges, and a limited set of path-resolved function calls.
- The Rust recognizer deliberately does not resolve method calls, trait dispatch, function pointers/closures, or all cross-file module paths. Its current AST output is not a complete control-flow graph; branches and loops must not be inferred from call edges.
- `ufo-types` provides `SysGraph`, the UFO semantic vocabulary, `ViewDefinition`/`Expose` selection data, SysML model types, `MbseExport`, and a feature-gated OODA-to-SCXML exporter. The SCXML module preserves a particular Rust state machine's transition shape; it does not replace that machine's Rust executor.
- `kr0ki` also has public-symbol docgen. Keep symbol documentation and semantic/behavioral graph extraction as separate products with a shared parser/cache layer where useful.
- `sysml-derive::SysmlBlock` maps named struct fields to a SysML v2 `part def`. It is useful for type-level export, but is not a trait, dispatch, or behavior generator.
- The live SysML view route currently filters by view kind. Model-authored `ViewDefinition`/`Expose` selection should become an explicit end-to-end acceptance target before claiming full model-authored view execution.

This plan follows the implemented source on KR0KI `main`; older local checkouts and historical plan text may not reflect that current surface.

## 3. Canonical representation and storage

Use the existing `ufo-types::SysGraph` / ontology / SysML model types as the typed semantic contract. Add a Rust behavior IR in KR0KI for source-level facts that do not fit the current structural recognizer. Keep the IR typed in Rust; if it is serialized between build stages, define a versioned schema, parser, validator, and linter together. No unvalidated intermediate JSON or diagram text may enter rendering or caching.

**Storage decision for this work:**

- The SysML v2 model server / versioned source revision remains the authoritative model history. A derived artifact is immutable and keyed by source revision, toolchain, configuration, and rule/schema digests.
- Use the existing typed `SysGraph` for in-process semantic modeling and the approved Oxigraph 0.5.x, `default-features = false`, for RDF/SPARQL graph queries and validation queries. Keep it in memory by default. Add RocksDB or another persistence layer only if an explicit retention, scale, or multi-writer requirement needs it.
- HelixDB was evaluated and rejected for this graph: its property-graph model does not provide the RDF/Turtle/SPARQL semantics this system already uses.
- Do not add ClickHouse or pgwire as a competing semantic store. DuckDB may be evaluated later as an optional consumer for aggregate analytics over exported Parquet/CSV; it is not a source of truth and is not required for this plan.
- Do not persist duplicate canonical graphs in both Oxigraph and a property/column store. OCI images carry reproducible derived artifacts; they do not become the model database.

## 4. Rust-to-view pipeline

```text
Cargo workspace + exact revision/configuration
    → compiler-aware symbol/type/impl facts
    → typed RustBehaviorIR + source provenance
    → validated UFO SysGraph (structural + behavioral edges)
    → SysML v2 model elements and selected ViewDefinition
    → linted D2/Mermaid/SCXML/SysML text
    → Kroki render + compressed OCI artifact bundle
```

Each edge records its source file and symbol, source span where available, commit/tree digest, analysis mode, and confidence/status (`resolved`, `inferred`, or `unresolved`). Preserve trait and method resolution provenance so a reviewer can traverse from a rendered edge to the exact `impl` or call site.

### Rust facts to recognize

- **Type shape:** modules, structs, enums, fields, associated types, trait definitions, supertraits, and generic constraints.
- **Ontology:** explicit UFO/SysML attributes and known implemented traits map to reviewed stereotypes/relations. `impl Trait for Type` is a `satisfies` assertion; do not infer that a trait name alone proves behavior. Preserve blanket/generic implementation conditions.
- **Dispatch:** resolve static calls and concrete trait implementations with compiler type information. Represent `dyn Trait`, plugin registries, callbacks, and function pointers as sets of possible targets or explicit unresolved dispatch nodes when proof is incomplete.
- **Control flow:** represent `if`/`match` as guarded decision/merge paths; loops as loop nodes with back edges and exit edges; `break`, `continue`, `return`, `?`, and `await` as explicit control effects. Collapse nested bodies behind stable function/action nodes by default; allow bounded expansion through a view option.
- **State machines:** use a typed transition table or a state-machine trait contract that the runtime and exporter both consume. Include state, event, guard, transition effect, and terminal-state semantics. Generate SCXML and SysML views from the shared transition model, and compare generated transitions against execution traces in fixtures.

Select a compiler-aware extraction approach in Phase 1. The existing `syn` walker is a useful syntax front end but cannot safely resolve method/trait dispatch. Spike rust-analyzer semantic APIs and Rust compiler/HIR/MIR extraction against the target Rust toolchains; record the chosen version and supported language/features. If compiler-private extraction is required, isolate it behind the extractor interface and pin the Rust toolchain. Never silently downgrade a failed type-resolution pass to confident edges.

## 5. Intermediate syntax, validation, and anti-pattern lints

Every intermediate representation must have a machine-enforced contract:

1. **Rust source:** parse with the selected Rust parser/compiler; report file, span, edition, cfg/features, and unsupported syntax. Reject partial workspaces unless explicitly requested.
2. **RustBehaviorIR:** deserialize into typed Rust structures with a versioned schema. Lint duplicate IDs, dangling endpoints, invalid spans, unknown relation names, impossible transition targets, missing loop exits, ambiguous dispatch, and inconsistent confidence/provenance. Unresolved facts remain explicit findings.
3. **UFO/SysML graph:** validate type/relation compatibility, identifier uniqueness, referential integrity, view filters, and supported KerML/SysML mappings. Apply SHACL/SPARQL checks through the approved Oxigraph path where they fit the graph contract.
4. **Generated DSL:** parse and lint D2/Mermaid/SCXML/SysML output before render. SysML v2 text must pass the real grammar validator. Use the available renderer/compiler parser for other notations; if a format has no parser, mark validation as structural-only and include adversarial fixtures rather than calling it grammar-validated.
5. **Anti-pattern report:** fail the self-test on invalid syntax or invariant violations. Warnings such as unresolved dynamic dispatch, reflection/macros, recursion, or a loop with unknown termination must be surfaced in a deterministic report and artifact manifest; strict mode may promote selected warnings to errors.

Generated text must use stable ordering, escaped labels, and byte-identical output for identical inputs. No timestamps, random IDs, machine paths, or nondeterministic map iteration in content-addressed model artifacts. Put wall-clock build time only in optional external build metadata, not in hashed semantic output.

## 6. OCI artifact and local workflow

All generated outputs are derived data. Do not commit them to Git. Store any host-side cache/export beneath a dedicated ignored directory such as `.kr0ki-generated/`; add it to `.gitignore` before generation. Cargo `target/`, caches, and temporary source bundles remain ignored too.

Build artifacts within a multi-stage `Containerfile` using Podman. The artifact stage should include the compressed bundle and a manifest inside an OCI layer, keyed by source tree, Cargo lockfile/toolchain, extractor/config, ontology mapping, and linter/schema digests. Bundle contents should include the typed IR, graph/view JSON, generated source notation, rendered SVG/PNG where configured, lint report, and provenance manifest. Use a deterministic archive (for example, sorted tar + zstd with normalized metadata) so equivalent input yields the same bundle digest.

Add thin `just` recipes, with no docker-compose:

- `just docgen-image [revision]` builds the OCI artifact image from the selected source tree.
- `just docgen-self-test` runs the image's extraction, IR/schema checks, graph/view checks, notation validation, and render smoke test under Podman resource limits.
- `just docgen-artifacts [revision]` builds or selects the image and prints its digest/location; `podman run`/`podman cp` provides the preferred way to inspect or extract a bundle. Any extracted host copy goes only under `.kr0ki-generated/`.
- CI may run the same recipes when Podman is available; it must not commit generated files. Container labels and manifest identify the source revision and every generator/schema/rule version.

Keep builds serial by default and make memory/CPU limits configurable. Do not build the full Rust workspace, SysML API server, UI, and renderer images concurrently on the constrained host. Cache immutable OCI build layers by lockfile and generator inputs. The image must be reproducible from tracked source and pinned dependencies; it must not rely on an untracked local artifact or hidden host state.

## 7. Delivery phases

| Phase | Work | Exit criteria |
|---|---|---|
| **0 — contract and fixtures** | Freeze the IR vocabulary, source-provenance fields, confidence semantics, schema version, and representative Rust fixtures. Document compiler/extractor candidates. | Fixtures cover structs/enums, trait/supertrait/generic impls, concrete and dynamic dispatch, match/branch, nested loop, early return, async call, and OODA-like state transitions. Expected resolved/unresolved facts reviewed. |
| **1 — typed traits and dispatch** | Add compiler-aware extractor behind a narrow interface; merge it with current `syn` recognizer; map trait impls and UFO annotations to typed graph edges. | Resolved impl/call edges match compiler facts on fixtures; dynamic dispatch is explicit; unsupported resolution emits lint findings; no guessed target edges. |
| **2 — executable control flow** | Build behavior IR for branches, loops, exits, async boundaries, and state transition tables. Keep code paths summarized and expandable. | Generated flow preserves guarded branches, loop back/exit edges, and terminal behavior; tests compare state-transition graphs with the runtime's transition table and execution traces. |
| **3 — SysML views and docs** | Connect validated Rust behavior graphs to model-authored `ViewDefinition`/`Expose`, renderable SysML constructs, and stable D2/Mermaid/SCXML outputs. Extend live Rust route with documented view selection. | View filtering changes only selected graph content; all output passes syntax/semantic lints; each diagram edge resolves back to source evidence; golden output is deterministic. |
| **4 — OCI build and self-test** | Add Podman `Containerfile` artifact stage, compressed bundle/manifest, ignored host extraction path, and `just` recipes. | Fresh build/self-test succeeds without prior generated host files; OCI contains compressed outputs and digest manifest; `git status` remains free of derived artifacts; repeated build has matching semantic and bundle hashes. |

## 8. Non-goals and safeguards

- Do not turn this into a replacement Rust compiler, runtime tracer, general-purpose graph database, or a claim that static analysis proves all dynamic behavior.
- Do not add a second AST-only call graph and present it as trait-dispatch resolution.
- Do not put generated artifacts, model caches, or test exports in Git.
- Do not make AI classification or inferred ontology mappings authoritative. Suggestions require explicit provenance and review.
- Do not broaden container permissions, run arbitrary code from source inputs, or expose Kroki include/network features while rendering untrusted generated diagrams.

## 9. First implementation issue set

1. Reconcile the current recognizer/view status in `AGENTS.md`, `PLAN-KR0KI-002`, `PLAN-KR0KI-003`, and `TODO.md` before adding new code.
2. Create Phase 0 fixtures and choose the compiler-aware extractor based on small, measured spikes—one at a time to protect the host from resource exhaustion.
3. Implement and lint the typed behavior IR, then connect one trait-dispatch example and one executable state machine end-to-end.
4. Add the OCI artifact image and `just` self-test only after the first end-to-end graph and view contract is stable.
