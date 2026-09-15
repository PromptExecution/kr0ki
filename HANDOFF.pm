# HANDOFF — kr0ki P0+ (post-docgen, post-auth, post-png)
**Date**: 2026-09-15 | **Branch**: `main` | **Node**: sm3llsl1k3s0ld3r
**Last verified by**: pi session | **Next review**: before SysML-model path work resumes

---

## TL;DR for the next engineer

kr0ki's **P0 render loop is complete and tested** (FR2 + FR5 cache). Three
post-P0 features landed today: **caller auth** (FR7 minimal), **PNG output**
(`?output=png`), and **docgen/mdb00k** (self-documenting `/docs` endpoints that
harvest kr0ki's own Rust source). The server binds `0.0.0.0:8787` by default and
logs its DNS hostname on startup.

The **SysML-model ingestion path (FR1/FR3/FR4)** remains fully blocked on upstream
dependencies: `ufo-types` graph container (Box 2), Kubernetes recognizer (Box 3),
and SysML v2 view definitions as data (Box 4). Do not attempt to unblock these
from within kr0ki — they live in other crates.

**Single immediate task** (§2 below): wire `ModelSnapshot.content_hash` into the
render cache key so that the SysML-v2 client path can participate in cache
identity once Box 2 exists. This is a 15-minute plumbing change with a test.

---

## 1. What works right now

### P0 render loop (unchanged, still passing)
- `kr0ki-core::RenderService` — cache + `HttpKrokiBackend`
- `kr0ki-server` — axum routes `/health`, `/formats`, `/render/{format}`, `/cache/{key}`
- Content-addressed `FsCache` — SHA-256 key, atomic temp-file writes
- Live-verified against `https://kroki.io` — SVG miss → byte-identical hit

### Post-P0 features (new today)

| Feature | Evidence | File |
|---|---|---|
| **PNG output** | `POST /render/graphviz?output=png` → 200 image/png; cache hit byte-identical | `crates/kr0ki-core/src/cache.rs` `OutputKind::Png` |
| **Caller auth (FR7 minimal)** | `KR0KI_AUTH_TOKEN=testsecret ./kr0ki` → no auth = 401, bad token = 401, good token = 200 | `crates/kr0ki-server/src/app.rs` `require_bearer` |
| **Docgen / mdb00k** | `GET /docs` → HTML (22KB); `/docs/api.json` → 50 symbols; `/docs/api.tomllm` → b00t format; `/docs/api.rustdoc` → rustdoc blocks | `crates/kr0ki-core/src/docgen/`, `crates/kr0ki-server/src/docs.rs` |
| **0.0.0.0 bind + hostname** | `just run` binds 0.0.0.0:8787; logs `listening on 0.0.0.0:8787 — docs at http://<hostname>:8787/docs` | `crates/kr0ki-server/src/main.rs` |
| **Template CI** | `just test-live` renders `templates/b00t-stack-orchestration.d2` against live Kroki; cache hit verified | `crates/kr0ki-core/tests/template_render.rs` |
| **PNG live CI** | `just test-live-png` renders GraphViz PNG against live Kroki; cache hit byte-identical | `crates/kr0ki-core/tests/live_png.rs` |

### Test summary
```
cargo test --workspace        → all pass (22 tests across 3 crates)
cargo clippy --workspace --all-targets -- -D warnings → clean
```

---

## 2. FIRST TASK: wire ModelSnapshot.content_hash into cache key (~15 min)

**Why**: When the SysML-v2 client path (Box 1) eventually feeds Box 2 → Box 4 →
Box 5, the render cache key must include the model's content hash so that a new
commit invalidates derived diagrams. The plumbing exists; it needs a function
signature change.

**What to change**:
1. Extend `kr0ki_core::cache::cache_key` to accept an optional `model_hash: Option<&str>`
2. If present, prepend it to the hashed payload
3. Update `RenderService::render()` to pass `None` for now (P0 text path)
4. Add unit test: same source + different model_hash → different keys

**Where**: `crates/kr0ki-core/src/cache.rs` `cache_key()` and `RenderService::render()`.

**Blocked by**: nothing. This is prep work for the SysML-model path.

---

## 3. Remaining work (in priority order)

| # | Task | Box | State | Blocked on |
|---|---|---|---|---|
| 1 | Wire `ModelSnapshot.content_hash` into cache key | 5 | §2 above | nothing |
| 2 | **CDN tier (FR5 / D5)** | 5 | Design only | Infra (Cloudflare R2 + Workers) |
| 3 | **Artifact reference resolver (FR6)** | 5 | Not started | `ledgrrr` graph query integration |
| 4 | **`iso_ir → Mermaid / D2` adapter (FR1)** | 5 | Not started | Boxes 2+3+4 |
| 5 | **KubeDiagrams backend (sandboxed)** | 5 | Spec in `EVAL-kubediagrams.md` §2a/§4 | Box 3 recognizer |
| 6 | **per-`ViewDefinition` rendering (FR4)** | 5 | Not started | Boxes 2+3+4 |
| 7 | **systhread-core isometric backend (FR3)** | 5 | Not started | `systhread-core` API stabilizes |
| 8 | **ModelSnapshot → UFO graph builder** | 2 | Not started | `ufo-types` graph container type |
| 9 | **Kubernetes recognizer** | 3 | Design in `PATTERNS-kubernetes.md` | Box 2 + `KubeDiagrams` rule port |
| 10 | **ViewDefinition / ViewpointDefinition as data** | 4 | Not started | Box 2 + Box 3 |
| 11 | **b00t MCP surface** | cross | Not started | `kr0ki.mcp.toml` datum or vendored `kroki-mcp` |
| 12 | **KubeDiagrams oracle CI job** | QA | Not started | Box 3 + examples corpus |
| 13 | **Phase 2 conformance** | QA | Not started | FR1 adapter + upstream fixture set |

---

## 4. Architecture decisions already made (do not re-litigate)

- **D3** (typed layer lives in `ufo-types`, kr0ki consumes it) — ✅ 2026-09-05
- **D6** (vocabulary: OMG terms verbatim, "projection" banned, "digital thread" qualified) — ✅ 2026-09-05, `VOCABULARY.md`
- **PNG is `OutputKind::Png`** — Kroki does not support PNG for all formats (D2 → 400). The server correctly forwards the 400 as 422 `bad_source`. This is correct behavior.
- **Mermaid excluded** — `DiagramFormat::ALL` omits Mermaid because Kroki's Mermaid renderer requires a companion (puppeteer). Only standalone formats are advertised.
- **Docgen uses `syn`, not `codebase-memory-mcp`** — b00t's `docgen.rs` pattern is the format convention; the implementation is Rust-native. This avoids coupling kr0ki to b00t's CBM binary availability.
- **Auth is env-var bearer token** — FR7 minimal. No OAuth/JWT. Suitable for localhost/trusted-proxy only until D4/D5 land.

---

## 5. Gaps discovered during this session

These were NOT in TODO.md at session start. They should be tracked.

| Gap | Severity | Where | Mitigation |
|---|---|---|---|
| Docgen workspace root detection uses string-search `[workspace]` in Cargo.toml | Low — works for kr0ki's layout | `kr0ki-core/src/docgen/mod.rs` | Could use `cargo metadata --format-version=1` if robustness needed |
| Docgen does not harvest nested `impl` blocks or associated items | Low — public API surface only | `kr0ki-core/src/docgen/harvest.rs` | `syn::visit` can be extended; currently we harvest `pub` items only |
| `/docs` HTML references `templates/b00t-stack-orchestration.d2` by filesystem path — breaks if server CWD changes | Low | `kr0ki-server/src/docs.rs` | Embed template as `include_str!` or resolve relative to executable path |
| No CI job for docgen endpoints | Low | `.github/workflows/` (doesn't exist) | Add to CI once GH Actions set up |
| D2 template edge labels with `{slug}` syntax break D2 parser — replaced with `slash slug slash` | Fixed | `templates/b00t-stack-orchestration.d2` | Verify against Kroki D2 renderer after any template edits |
| b00t MCP bridge down (`bad handshake: expected ident at line 1 column 2`) | External | b00t MCP transport | Use direct HTTP or `b00t-cli` instead |

---

## 6. Files you should read before touching code

| File | Why |
|---|---|
| `docs/PRD-KR0KI-001-foundational.md` | Requirements — what kr0ki must and must not do |
| `docs/PLAN-KR0KI-002.md` | The five-box pipeline — where your change fits |
| `docs/DESIGN-NOTE-typed-model-layer.md` | Why the typed layer design was rejected and what replaced it |
| `docs/VOCABULARY.md` | Approved terms (D6) — use "viewpoint" not "projection" |
| `docs/PATTERNS-kubernetes.md` | Kubernetes recognizer design (Box 3) |
| `AGENTS.md` | This file's active counterpart — agent orientation |
| `templates/b00t-stack-orchestration.d2` | The canonical diagram template — test any D2 change against this |

---

## 7. Operator-only actions

- [ ] Provision `kr0ki.b00t.promptexecution.com` DNS (D4)
- [ ] Set up Cloudflare R2 + Workers CDN in front of kr0ki (D5)
- [ ] Review `templates/datum.template.toml` for b00t registration accuracy
- [ ] Decide if kr0ki gets its own `_b00t_/kr0ki.hive.toml` profile (resource planning)

---

<!-- b00t:map v1
summary: kr0ki handoff — P0+ state, post-docgen/auth/png, next tasks, gaps discovered
tags: kr0ki, handoff, P0, docgen, auth, png, sysml, ufo-types, b00t
tier: frontier
cmds: just test, just check, just run, just test-live, just test-live-png
complexity: 8
-->
