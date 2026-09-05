# kr0ki

**The b00tyverse cut-node between [Kroki](https://kroki.io) and b00t/systhread.**

kr0ki is the rendering + CDN-cached-artifact service layer for SysML/KerML diagrams in
the b00t ecosystem. It is deliberately *one node in the graph* — the boundary where an
abstract, validated model (owned upstream by `systhread` / `ufo-types`) becomes a
concrete, cached, referenceable picture (Mermaid / PlantUML / GraphViz / D2 / SVG,
served from `kr0ki.b00t.promptexecution.com`).

It does **not** own the model. It does **not** re-implement diagram grammars. It
wraps an existing multi-format renderer ([`kroki-mcp`](vendor/kroki-mcp), vendored)
and an existing isometric renderer (`systhread-core`'s `layout.rs`/`render.rs`),
and adds the one thing neither has: a caching, cross-referencing service surface.

> Status: **foundational** — requirements only. No implementation yet.
> Read [`docs/PRD-KR0KI-001-foundational.md`](docs/PRD-KR0KI-001-foundational.md) first.

## Orientation for agents

| Thing | Where | Why it matters here |
|---|---|---|
| b00t SysML v2 spine epic (closed) | [`elasticdotventures/_b00t_#1177`](https://github.com/elasticdotventures/_b00t_/issues/1177) | The upstream model/validation layer kr0ki renders *from*. P0–P3 shipped. |
| `ufo-types` v0.11.0 | [`PromptExecution/ufo-types`](https://github.com/PromptExecution/ufo-types) | `iso_ir::{Node,Edge}`, `stereotype::UfoStereotype`, `sysml::validate_sysml_v2`, `mbse`. kr0ki's input vocabulary. |
| Live P1 prototype | `elasticdotventures/_b00t_` → `b00t-cli/src/dispatch_sysml.rs` | Working `Rust type → iso_ir → SysML v2 / Mermaid / Rhai`. The pattern kr0ki's adapter follows. |
| `systhread-core` | `fungible-farm/nem-poweragent-lab` → `rust/systhread-core` | Owns `iso_ir`/`layout`/`render`/`sysml_gen`. Its isometric `render.rs` becomes a kr0ki input format. |
| systhread v2 SysML/KerML viz scope | [`nem-poweragent-lab#53`](https://github.com/fungible-farm/nem-poweragent-lab/pull/53) (merged) | Defines the typed-model → views contract kr0ki renders. Names **cim-gridy** as first consumer. |
| kroki-b00t (comic engine) | [`PromptExecution/infrastructure#217`](https://github.com/PromptExecution/infrastructure/issues/217) | A **downstream consumer**, disconnected as its own leaf — the comic team renders kr0ki SVGs to make jokes about b00t. Not part of kr0ki's core. |
| Dependency-posture decisions (OPEN) | [`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202), [`ledgrrr#203`](https://github.com/PromptExecution/ledgrrr/issues/203) | extend-vs-wrap-vs-re-export `sysml-derive`; whether `holon-viz` becomes a real dep. **kr0ki defers to these — does not pre-empt them.** |
| Generic Kroki datum | `elasticdotventures/_b00t_` → `_b00t_/kroki.mcp.toml` | The existing b00t MCP datum wrapping public/self-hosted Kroki. kr0ki supersedes it as the *service*, keeps it as the *client*. |
| KerML anchor | `elasticdotventures/_b00t_` → `_b00t_/types/b00tyverse.kerm` | The canonical KerML the whole thread serializes/reasons over. |
| Vendored renderer | [`vendor/kroki-mcp`](vendor/kroki-mcp) → [`PromptExecution/kroki-mcp`](https://github.com/PromptExecution/kroki-mcp) | b00tyverse fork of [`utain/kroki-mcp`](https://github.com/utain/kroki-mcp) (MIT, Go), pinned at `08765f64`. |
| SysML-v2 tooling survey | `PromptExecution/ledgrrr` → `docs/sysml-v2-tooling-survey.md` | Decided infra (`holon-viz`, `ufo-types`, wrap-vs-build for LSP/MCP). Read before proposing new tooling. |

## Layout

```
kr0ki/
├── README.md
├── docs/
│   └── PRD-KR0KI-001-foundational.md   ← the requirements document
└── vendor/
    └── kroki-mcp/                      ← submodule, PromptExecution/kroki-mcp @ 08765f64
```
