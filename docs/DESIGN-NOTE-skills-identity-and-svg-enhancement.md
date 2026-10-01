# Design note: skills as typed things, identifier-driven SVG enhancement, brand packages, live state

_2026-10-01. Follows [`HANDOFF-agent-harness-mode.md`](HANDOFF-agent-harness-mode.md). **FACT** = verified in this session
(commands in §8); **PROPOSAL** = recommended, not built; **UNVERIFIED** = not checked._

## 1. Decisions taken (from the owner) and my recommendations
| Topic | Direction | Status |
|---|---|---|
| Brand and iconography | **Configurable**, not hard-coded; swappable packages; canonical brand packs plug in later | Owner decision; format proposed in §4 |
| Diagram code | Only a **version-controlled base layer**. Everything visual beyond it (icons, palettes, animation) is an overlay that can be replayed or replaced | Owner decision |
| Overlay mechanism | Run over the rendered SVG, find well-known identifiers, rewrite the DOM with **CSS selectors** | Owner decision; spike in §3 |
| A skill | **A UFO stereotype**, with a well-known identifier and its own iconography | Owner decision; upstream mechanics UNVERIFIED (§6) |
| Identifiers in diagrams | When an identifier exists (SysML v2 `@id`, schema relationships), the diagram must carry it | Owner rule; §2 |
| Animation / live state | Browser layer for live state; manim for exported clips; SysML v2 "simulated" by reading OTel | §5 |
| **Python agent or pi?** | **Keep the Python agent as the UX runtime for now; share the skills with pi as the second harness** (§7) | My recommendation |

## 2. Identity contract (the part worth keeping)
Every element a skill or recogniser can name gets a **well-known identifier** and the diagram must keep it as the
*stable key*, with the human name as the *label*. That is what lets an overlay find it later.

| Domain | Identifier (PROPOSAL: scheme-prefixed) | Source |
|---|---|---|
| SysML v2 | `sysml:<element @id>` (UUID); `qualifiedName` as secondary | OMG API element |
| Database schema | `db:<schema>.<table>[.<column>]`; FK = `db:<schema>.<table>.<fk_name>` | catalog/DDL |
| Kubernetes | `k8s:<namespace>/<kind>/<name>` | manifest |
| Skill | `skill:diagram/<language>` , `skill:diagram-type/<id>`, `skill:mbse/<name>` | skill registry |

Stamped onto the SVG as `data-kr0ki-id`, `data-kr0ki-type`, `data-kr0ki-name` (and later `data-kr0ki-ns`). **Skill rule:**
*"When an identifier is available, use it as the node key and put the display name in the label; never invent identifiers."*

## 3. What exists, and what the spike proved
**The "SVG enhancement layer" is not built (FACT).** `crates/kr0ki-svg` has an SVG parser and the layout solver. Its
`Hook`/`Animation`/`EnrichmentConfig` types are skeletons: `parse_svg` always returns empty `hooks`/`animations`;
nothing selects by CSS, replaces identifiers or applies a brand. The spike (`tools/svg-enhance-spike/`) tested the idea
against a **real SysML v2 render** (fixture OMG-API server -> kr0ki `/render/sysmlv2/...` -> D2 SVG):

| Finding | Detail |
|---|---|
| **Identifiers survive (FACT)** | D2 puts each element key on its `<g>` as an **unpadded base64 class name** (decodes to the exact `@id`). Opaque but deterministic: since the model's ids are known, compute `base64(id)` and select `g.<that>`. |
| **Labels were raw UUIDs (FACT; FIXED in v0.0.7)** | `crates/kr0ki-core/src/sysml_render.rs` labels every node with its id ("a richer label needs an id -> display-name lookup this module doesn't have"). A person sees `00000000-0000-4000-...0004` instead of `battery`. Names are in the snapshot. Fixed: `sysml_render::to_d2_named` keeps the id as the D2 key and uses the name as the label; the route's cache version token was bumped to `sysmlv2-ufo-graph-v2` (**any change to the emitted D2 for the same model must bump it, or stale SVGs are served**). Verified live: labels `Vehicle/engine/transmission/battery`, 4 elements still stampable by id. |
| Not every element is drawn | The fixture's `RequirementUsage` and `SatisfyRequirementUsage` did not appear (indexed 4 of 5). UNVERIFIED whether my fixture shape matches what `sysml_lift` expects; the next agent should check before claiming a renderer gap. |
| **Two-pass design works (FACT)** | Pass 1 *index*: stamp `data-kr0ki-*`. Pass 2 *rules*: plain CSS selectors over the stamped DOM add classes, inject `<use>` icons from `<symbol>`s and restore display names. Verified in headless Chromium: `querySelectorAll('[data-kr0ki-type="PartUsage"]')` returned 3; labels became `Vehicle/engine/transmission/battery`; 0 UUID labels left; 4 icons rendered. |
| **D2 CSS outranks brand CSS (FACT)** | D2 ships `.d2-<hash> .stroke-B1 {...}` (specificity 0,2,0). A plain `.k-part rect` rule had **no effect** in the browser; `!important` (or >=3-class specificity) fixed it. Brand rules must account for this. |
| Idempotent (FACT, after a bug) | A first version added the icon twice on a second run; caught by a unit test (`test_idempotent`). Rules must be safe to re-run because the overlay is regenerated. |
Graphviz exposes ids differently (`<title>` inside `<g class="node">`); PlantUML differently again: the *index* pass is the
only renderer-specific code, per language (UNVERIFIED for graphviz/plantuml; not tested).

## 4. Brand packages (configurable) — PROPOSAL, prototyped as `brand.example.json`
```
brand/<package>/brand.json   { version, icons{name:{viewBox,svg}}, rules[{select,class,icon,label}], css, palette{} }
brand/<package>/icons/*.svg   (referenced, inlined as <symbol> at apply time)
```
- **Resolution order** (later wins): base theme -> organisation brand -> application palette -> per-diagram overrides.
- **Select with CSS** over `data-kr0ki-*`; the same JSON runs in the browser (`querySelectorAll`) or server-side
  (Rust, e.g. `scraper`/`lol_html`, or in `kr0ki-svg` via WASM) — pick when building (UNVERIFIED which).
- **Never invent a brand.** Shipped packages are placeholders (`brand.example.json` uses flat shapes). Canonical packs
  (b00t, PromptExecution, product icons) replace the files later; nothing in diagram source changes. The only icons on disk today
  are `~/.b00t/b00t-vscode/icons/*` (FACT); no palette file exists (owner to supply).
- Skills reference brand **by name**, never by colour value, so a pack swap re-themes every skill's examples.

## 5. Animation and live state (research: scratchpad `E-manim-otel.md`, summarised)
- **Live state = the browser DOM layer.** Inline SVG + CSS classes/variables for state colour, Web Animations API for
  transitions; d3 only if a data join over many elements is needed. Selectable by the same identifiers. Manim cannot be
  CSS-selected and whether ids survive its SVG import is UNVERIFIED.
- **Exported clips = manim** fed by a recorded sequence of states (or headless capture of the same DOM). Keep one
  state -> style mapping shared by both. Existing b00t plan: `~/.b00t/_b00t_/datums/MONTY-MANIM-WASM.tomllmd` (proposed, unbuilt).
- **OTel -> SysML v2 current state (PROPOSAL; no existing convention found, not exhaustive):** resource attribute
  `sysml.element.id` (+ `sysml.element.qualified_name`); on spans/events `sysml.state`, `sysml.model.commit`. A
  `StateUsage` shows the last `sysml.state` per element; a transition not in the model is flagged as a conformance
  finding. **"Simulation" here means observed-state replay onto the model** (deterministic from stored telemetry), not
  executing the model; execution belongs to an upstream engine (OpenSysML claims one, UNVERIFIED), consistent with the
  scope rule that kr0ki renders and does not model. Existing collector config: `~/.b00t/containers/otel-collector/otelcol-config*.yaml`.

## 6. A skill is a UFO stereotype
`ufo-types` (rev `ee87348`) has no `Skill` type (FACT). Direction: model a skill as a **Kind** within a
`CapabilityDomain` (Act = authoring skills, Verify = review skills), with a `skill:` identifier (§2), an iconography
reference (a brand rule selecting `[data-kr0ki-type="Skill"]`), `Specializes` for skill families (language -> type), usage as an `Event`
(like `ActionRecord`) and the render verdict as a `ReviewVerdict` (Relator). **UNVERIFIED:** whether `OntologicalNode` can carry the icon/identifier
payload or needs a new field; `UfoStereotype` is a closed 14-variant enum so this is an *instance* of Kind, not a new variant.
Per `AGENTS.md` §4 the type belongs upstream in `ufo-types`; kr0ki only emits usage events. Needs an upstream issue/PR (owner to approve).

## 7. Python agent or pi? (recommendation, reasoned from what we verified)
| | Python agent (today) | pi |
|---|---|---|
| Owns UI protocol | **Yes**: AG-UI SSE, interrupts (`ask_user`), project memory, planner tools, 50 tests | Not verified; would need an AG-UI adapter (WP4) |
| Skill loading | Can be **deterministic** (inject the skill for the selected type; refuse to render without it) | Model-judged by spec ("a model might fail to load a relevant skill"); needs a gate extension |
| Guarantee "skill loaded before acting" | ~50 lines in the tool dispatcher it already controls | TS extension (`tool_call` block) + rpc sidecar |
| Portability of skills | Plain files | `SKILL.md` is a shared convention (pi, Claude Code and others discover it) |
| Strengths | Tight to the product loop | Extensions/packages, local-model errands (verified 7.7 s), file-heavy long tasks (MBSE) |
**Recommendation:** write skills once as `SKILL.md` directories (portable). Use them from the **Python agent now** with
deterministic injection + a gate (fastest path to "stops guessing"), and from **pi** for the MBSE assistant, dev errands and
bulk generation, which are file/tool-heavy and not tied to the AG-UI loop. Swap the diagram UX runtime to pi only if the
pi AG-UI adapter proves out. This revises the earlier "run both behind a flag" suggestion because deterministic injection
beats model-judged loading and the Python side already owns the protocol.

## 8. Reproduce, and how to continue (process for the next agent)
```bash
# 0. services: kr0ki on :8787 (just run), Kroki backend on :8010. Never use `pkill -f` (kills your own shell); kill by PID.
# 1. fixture SysML v2 server + a second kr0ki pointed at it
python3 tools/svg-enhance-spike/sysml_fixture_server.py 18081 &
KR0KI_BIND=127.0.0.1:8788 KR0KI_BACKEND_URL=http://127.0.0.1:8010 KR0KI_SYSMLV2_BASE_URL=http://127.0.0.1:18081 ./target/debug/kr0ki &
curl -s -X POST 'http://127.0.0.1:8788/render/sysmlv2/projects/p1/commits/c1?output=svg' -o sysml.svg
curl -s http://127.0.0.1:18081/projects/p1/commits/c1/elements > snapshot.json
# 2. enhance + unit tests
cd tools/svg-enhance-spike && python3 -m unittest test_enhance && python3 enhance.py ../../sysml.svg ../../snapshot.json brand.example.json enhanced.svg
# 3. verify in a browser (serve enhanced.svg, evaluate querySelectorAll / getComputedStyle); headless Chromium on CDP :9222
```
Order of work: **(1)** ~~`sysml_render` display-name lookup~~ done in v0.0.7; **(2)** identifier stamping at render time
(server returns an id map, or `kr0ki-svg` indexes) for D2 first, then graphviz/plantuml; **(3)** `kr0ki-svg` rules engine
(brand JSON, CSS select, idempotent) + wasm export; **(4)** brand package loader + Setup UI; **(5)** live-state layer in the
playbook (class toggling from an SSE/OTLP-derived stream) reusing the `/ui` SSE pattern; **(6)** skills with the identifier rule
(`HANDOFF` WP1-3, 6); **(7)** upstream `Skill` stereotype proposal. Keep each step independently testable; do not claim a renderer
gap without checking the fixture shape (§3).
Pitfalls: close stdin for `pi` runs; the vision model is one GPU slot shared with the planner chat; check `OPENAI_API_URL` is `:8002`.

## 9. Status update: the enhancement layer is built (v0.0.8)
`crates/kr0ki-svg/src/enhance.rs` (+ WASM `enhance_svg_json`) implements §3-§4 for real: identifier **index/stamp** (D2 base64 classes), **brand rules**
(CSS-subset selectors: tag, `.class`, `[attr]`, `[attr="v"]`, no combinators, unsupported = error), restored display names, `<symbol>` icons, brand CSS,
**idempotent byte-range splicing** (the renderer's output is preserved exactly), and validation of everything injected (no scripts/handlers/foreignObject/external refs/@import;
names XML-escaped). 34 tests incl. a captured real D2 SysML render; mutation-checked (13 mutants; 2 survived and the tests were strengthened).
Server: `POST /render/sysmlv2/projects/{p}/commits/{c}?output=svg&brand=<name>` loads `KR0KI_BRAND_DIR/<name>/brand.json` (default `./brand`), enhances the render (post-cache, so one cached render serves
every brand) and reports `x-kr0ki-brand` / `x-kr0ki-enhance: indexed=N;unindexed=M`. `GET /brand` lists packages. `brand/example/` is a **placeholder** package, not canonical.
Verified end to end: fixture SysML server -> kr0ki -> D2 -> `?brand=example`; in headless Chromium the selectors, names, 4 icons and the brand stroke applied.
**Still not built:** applying brand to non-SysML routes/Graphviz/PlantUML (index step differs per renderer), a browser-side use of the WASM export in the playbook, brand UI in Setup, animation/live state, canonical packs.
