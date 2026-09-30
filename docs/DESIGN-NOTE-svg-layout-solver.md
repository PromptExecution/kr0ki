# DESIGN NOTE — `kr0ki-svg` layout solver: scope and contract

**Status:** implemented, replaces the root-level `PHASE-2-COMPLETE.md` status report.
**Owner:** PromptExecution (@elasticdotventures). **Reviewed:** `/code-review high`, findings F1–F10 closed.

## 1. Scope (the decision)

`kr0ki-svg` is the **browser/WASM-side enrichment layer**: it parses rendered SVG and
lays out / re-lays out a *semantic graph* so the browser can move nodes and keep edges and
constraints attached. It consumes diagrams kr0ki already rendered; it does not render, model,
or validate models.

| In scope | Out of scope (lives elsewhere) |
|---|---|
| SVG parse/enrich (`usvg`) | Isometric layout/render → `systhread-core` (call it, don't port it) |
| 2D force-directed layout + constraints, runs in WASM | Model validation → `ufo-types` |
| Incremental re-layout of an edited graph | Raster → diagram-as-code ingestion → its own crate + plan (next) |

The solver sits here, not in `kr0ki-core`, because it must run client-side without a server
round trip and `kr0ki-core` is the server render loop. Revisit if `systhread-explorer` ships a
reusable layout crate: then this becomes a thin adapter over it.

## 2. Libraries, not bespoke math

| Concern | Library | Replaces |
|---|---|---|
| Force-directed physics | `fdg-sim` (Fruchterman-Reingold) | hand-rolled repulsion/attraction/annealing loop, duplicated in `solve` and `solve_incremental` |
| Graph container for the simulation | `petgraph` `StableGraph` (via `fdg-sim`) | `HashMap<String, _>` force tables rebuilt every iteration |
| Vector math, rotation, clamping | `glam` (via `fdg-sim`) | `(f64, f64)` tuple arithmetic |
| Errors | `thiserror` typed enums | `unwrap()` / index on untrusted JSON |

Only the constraint corrections, validation, deterministic start, and incremental seeding are
kr0ki code, because `fdg-sim`'s `Force` is a bare `fn` pointer and cannot carry per-request constraints.

## 3. Contract

- **No panics on input.** `Graph::validate`, `Constraint::validate` and finite-position checks run
  inside `GraphSolver::solve*`; bad input is a `SolveError`, which the WASM exports surface as a JS exception.
- **Deterministic.** Nodes and edges are processed in sorted-id order and co-located nodes are spread on
  a golden-angle spiral, so the output is a pure function of the input (required by the content-addressed cache).
- **Incremental keeps the layout.** Missing nodes start at the centroid of placed neighbours, stale
  entries are dropped, and the step is scaled by `incremental_step_factor` so existing nodes move little.
- **Constraints** are position corrections scaled by `strength` (0..=1). Arity per kind: Distance,
  NonOverlap, RelativePosition = 2; Angle = 3 (vertex is the middle node); Boundary = 1; Align* = 2+.
- **WASM API.** `solve_graph_layout(graph, constraints)` and
  `solve_graph_layout_incremental(graph, constraints, positions)` return JSON
  `{positions, iterations, residual, converged}`. **Breaking vs. the unreleased first draft**, which
  returned the bare positions map; there were no callers in the repo.

## 4. Known limits

- O(N²) repulsion (`fdg-sim`); fine for diagrams, not for thousands of nodes.
- Constraints are soft and solved by relaxation, not exactly. If hard constraints are needed, swap the
  correction pass for a Cassowary solver (`kiwi`/`cassowary`, as `systhread-core` already does).
- `Boundary` constrains node centres, not node extents.
