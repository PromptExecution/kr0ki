# b00t Stack Orchestration — Functional Template

This directory contains a **reusable pattern template** for b00tyverse services that consume kr0ki as their rendering cut-node. It is an *integration* artifact — it does not invent new renderers, model types, or auth layers; it wires what already exists.

## What is here

| File | Purpose |
|---|---|
| `b00t-stack-orchestration.d2` | **Executable diagram** — valid D2 source describing the b00t orchestration pattern. Render it via kr0ki P0 (`POST /render/d2`) to produce the canonical architecture SVG. |
| `kr0ki-render-flow.d2` | **Executable Rust-flow diagram** — Box-5 HTTP → route → cache/backend → artifact flow, rendered on live `/docs`. |
| `kr0ki-render-flow.kerml` / `.sysml` | **Inspectable model fixtures** — KerML and SysML v2 structural representations of the same implemented Rust flow. |
| `datum.template.toml` | **b00t registration datum** — copy, fill in `name`, `repo`, `upstream.*`, and submit to `elasticdotventures/_b00t_` (FR8). |
| `service-integration.template.rs` | **Rust integration snippet** — how a sibling service calls kr0ki-core's `RenderService` programmatically (type-entangled, not duplicated). |

## The pattern in one paragraph

> Every b00t service that needs diagrams crosses the model→render boundary at **one node**: kr0ki. Upstream owns the model (`ufo-types`, `systhread-core`, `ledgrrr`); downstream owns the consumer (docs, comic engine, explorer). kr0ki owns only the cut: caching, rendering, and artifact reference resolution. N services × 1 cut-node = a DAG, not a mesh.

## Rendering the template diagram

### Via kr0ki-server (P0)

```bash
# 1. Start or refresh the local k0s Pod, then use its LAN address.
just pod-up

# 2. In another shell, render the template to SVG
curl -X POST http://192.168.1.137:8787/render/d2 \
  --data-binary @templates/b00t-stack-orchestration.d2 \
  --output b00t-stack-orchestration.svg

# 3. Prove the live playb00k's diagram and cache contract.
just playbook-e2e
```

The local page is `http://192.168.1.137:8787/docs`. Its Rust-flow image is a
real `RenderService` D2 request (`/docs/examples/kr0ki-render-flow.svg`), not a
mock or static screenshot. The same source has KerML/SysML v2 fixtures on the
page for model-level inspection.

### Via kr0ki-core (programmatic)

See `service-integration.template.rs` — a `RenderService<HttpKrokiBackend>` wired to the public Kroki instance, caching in `/tmp`.

## 2D / 3D rendering library as a service

The template shows two backend paths behind `RenderBackend`:

1. **2D** — `HttpKrokiBackend` → Kroki-family (`DiagramFormat`: PlantUML, GraphViz, D2, C4, Vega-Lite, Ditaa, Nomnoml, WaveDrom). P0 already ships this.
2. **3D / isometric** — `systhread-core`'s `layout.rs` / `render.rs` → SVG via Cassowary. This is *not yet wired* in kr0ki; the template documents the intended integration point (`FR3` in PRD-KR0KI-001). kr0ki will call `systhread-core` by value; it will not port or re-solve the layout.

Both paths share the same `cache_key` + `FsCache` origin store; a future CDN tier (D5) sits in front without changing cache identity.

## Applying this template to a new b00t service

1. Copy `datum.template.toml` → `{service}.repo.toml` in `_b00t_`.
2. Decide which side of the cut your service lives on:
   - **Model side** → depend on `ufo-types`, emit `iso_ir` or KerML, do *not* call Kroki directly.
   - **Render side** → call kr0ki's service surface (or embed `kr0ki-core` if you are a leaf that needs local rendering).
3. If you need a custom diagram format, add it to `DiagramFormat` *only* if Kroki already supports it (NFR3: no new grammar).
4. Open a PR against `PromptExecution/kr0ki` if your service needs a new `RenderBackend` adapter — do not vendor a parallel renderer.

## Scope guardrails (read before extending)

- **Do not add model authoring** — kr0ki renders validated input; it never mutates (PRD §2.3).
- **Do not add new diagram grammars** — if Kroki can't render it, it doesn't belong here (NFR3).
- **Do not add a SysML parser** — `ufo_types::sysml::validate_sysml_v2` is the gate; anything deeper stays in `ledgrrr`.
- **Do not add auth** — FR7 is tracked in TODO; P0 is localhost/trusted-proxy only.
- **Do not add CDN logic** — D5 is infrastructure-owned; `FsCache` is the origin store.

## References

- PRD: [`docs/PRD-KR0KI-001-foundational.md`](../docs/PRD-KR0KI-001-foundational.md)
- Plan: [`docs/PLAN-KR0KI-002.md`](../docs/PLAN-KR0KI-002.md)
- Typed layer design note: [`docs/DESIGN-NOTE-typed-model-layer.md`](../docs/DESIGN-NOTE-typed-model-layer.md)
- b00t SysML v2 spine (closed epic): `elasticdotventures/_b00t_#1177`
