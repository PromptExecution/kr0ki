# Earlier plans: what was proposed, and what exists

Root-level planning and summary documents from September 2026 (the four `KR0KI-EVOLUTION-*` files,
`SVG-ENRICHMENT-PRIORITY`, `PHASE-1-COMPLETE`, `IMPLEMENTATION-SUMMARY`, the two `VUE-FLOW-*` files, the two
handoff notes and the two b00t MCP-ergonomics files) are folded into this page. Each proposal is marked with
what the repository contains **today**, checked against the code. The originals remain in git at `8a5a5b7`
(`git show 8a5a5b7:KR0KI-EVOLUTION-REFINED.md`). The authoritative, current documents are
[`PRD-KR0KI-001`](PRD-KR0KI-001-foundational.md), the numbered `PLAN-KR0KI-*` files, [`TODO.md`](TODO.md),
and [`../AGENTS.md`](../AGENTS.md).

Status key: **built**, **partial**, **scoped out** (belongs to another project by design), **not started**.

## The "evolution" plan (system-status dashboard, WASM SVG layer, plugins)

The plan went Plan → Challenge → Refined → Summary. Its refinement moved to an MVP of phases 1–3 with early
validation and fallbacks. What exists:

| Proposal | Status | Evidence |
|---|---|---|
| Quasar/Vue system-status dashboard | **not started** | the playbook is Vue 3 + Vite with no Quasar dependency. The nearest thing is the deep `/health` report and the welcome page's live badge |
| Rust/WASM SVG post-processing: core parser (phase 1) | **built** | `crates/kr0ki-svg`: SVG parse/enrich on `usvg`, exported through `wasm-bindgen` |
| Graph solver with constraints (phase 2) | **built** | `kr0ki-svg` constraint layout on `fdg-sim`; see [`DESIGN-NOTE-svg-layout-solver.md`](DESIGN-NOTE-svg-layout-solver.md) |
| Wiring the WASM layer into the playbook or a consumer (ledgrrr, app4dog) | **not started** | nothing in `playbook/` imports `kr0ki-svg` |
| 3D isometric renderer (phase 3) | **scoped out** | `systhread-core` owns isometric rendering: call it, don't port it ([`AGENTS.md` §4](../AGENTS.md)) |
| Enrichment engine (phase 4) | **not started** | no code |
| Plugin registry / b00t plugin integration, WebSocket push | **not started** | no WebSocket route; agents reach kr0ki through `GET /mcp/tools` and the `containers/kr0ki-mcp` bridge |
| Semantic visualization (t-SNE/UMAP) | **not started** | it was an optional phase |

## Playbook features described in summaries and handoffs

| Item | Status | Evidence |
|---|---|---|
| Version shown in the sidebar, derived from `Cargo.toml` | **built** | Vite `define` → `__APP_VERSION__`; `cog.toml` bumps with `cargo set-version` |
| Requirements documents and the "valigate" circuit | **partial** | `docs/requirements/`, [`VALIGATE.md`](VALIGATE.md), `scripts/valigate.sh`, `scripts/validate-requirements.sh` exist; no `just` recipe or CI job runs them |
| Code Editor → Agent handoff (`editorHandoff`, `prefill`, per-type prompts) and the *EDIT* button | **built** | `playbook/src/App.vue`, `StoryB00k.vue`, `Gallery.vue` |
| Setup tab with connection tests and dynamic model list | **built** | `playbook/src/components/Setup.vue`; behaviour in [`OPERATIONS.md` §5](OPERATIONS.md) |
| `@assistant-ui/vue` built from the vendored submodule | **built** | `just build-assistant-ui-vue`, run by `just test` and `just build`; the built `dist/` is not committed |
| Revision DAG (prompt / edit / fork, checkout, regenerate) drawn with vue-flow | **built** | `playbook/src/lib/revisionGraph.js`, `RevisionFlow.vue` |
| "Git-based redesign": real version control for revisions | **partial, done differently** | persistence is server-side in the agent through `jj` (`chart_store.py`: save, restore, history); not git-in-the-browser |
| Branch-management UI and remote sync for revisions | **not started** | PLAN-004 phases 4–5 |

## Proposals aimed at another repository

`AGENT-STORIES-B00T-MCP-ERGONOMICS` and `IMPLEMENTATION-GUIDE-B00T-MCP-ERGONOMICS` propose changes to the b00t
CLI (`elasticdotventures/_b00t_`) after friction installing the Chrome DevTools MCP server. Nothing in this repo
implements them, and whether they were filed or adopted is not tracked here. The working outcome for kr0ki is
the recipe in [`OPERATIONS.md` §7](OPERATIONS.md): a Chrome with `--remote-debugging-port=9222` plus
`chrome-devtools-mcp`.

## Status of the numbered plans (for orientation)

| Plan | Where it stands |
|---|---|
| PLAN-KR0KI-002 (source-to-render pipeline, five boxes) | Box 5 built; Box 3 built for Kubernetes and Rust source; Box 2 built for the SysML-v2 arm; see `AGENTS.md` §1 |
| PLAN-KR0KI-003 (Rust-source front-end) | recognizer + `POST /render/rust-source` built; method/trait-dispatch call resolution deferred |
| PLAN-KR0KI-004 (revisioned procedural workspace) | Phase 0 (contracts, ADR, schemas, dev session, exit-gate fixture) complete; durable revisions exist only as the agent's `jj` chart store; later phases not started |
| PLAN-KR0KI-005 (gallery catalog, type discovery) | built: `kr0ki_core::catalog`, `GET /api/catalog`, use-case filter, *Agent* button, `recommend_diagram_type`, `/projects/lock-type`, per-type skills |
| PLAN-KR0KI-006 (dbt digital thread) | write path built and hardened (`sync_engine`, `POST /model/projects/{id}/sync`, `flexo_reqif_sync`); the dbt manifest → `SysGraph` builder lives upstream in `ufo-types` and is not built |
