# Handoff: agent harness mode, diagram skills, and a clearer agent UX

_Written 2026-10-01 for whoever picks this up (human or agent). **Updated the same day with the owner's decisions: see [`DESIGN-NOTE-skills-identity-and-svg-enhancement.md`](DESIGN-NOTE-skills-identity-and-svg-enhancement.md), which supersedes §3 on runtime choice, brand, the skill type and the SVG overlay.** Everything marked **FACT** was verified in this
session; **PROPOSAL** and **UNVERIFIED** are labelled. Companion: [`REPORT-mbse-sysmlv2-assistant.md`](REPORT-mbse-sysmlv2-assistant.md)._

## 1. What the user wants
The gallery planner (v0.0.6) and its UX are "much better" but the **drawing agent still bumbles and guesses, and it is
unclear what it is about to do**. The user wants to use the tool. Ask, in their words:
1. An **agent harness mode built on the pi plug-in system**.
2. **Skills** the agent loads per diagram: (a) syntax examples incl. non-discoverable advanced/escaping variants
   (for retrieval-augmented context), (b) what makes each diagram type good or bad (tips/hints), (c) common brand
   iconography, (d) application-level iconography and colour palettes.
3. A **skills agent that guarantees** the right skill is loaded for the selected diagram before the agent acts.
4. Skills part of the **b00t ecosystem**, modelled with **UFO types**, so skill usage can be understood.
5. An **MBSE / SysML v2 assistant report** (done: `REPORT-mbse-sysmlv2-assistant.md`) with skills for it.
Interpretation to confirm with the user: "start in the GitHub Auto loaded" was read as *skills live in the repo and
are auto-discovered from a checkout* (pi scans `.agents/skills/` and `.pi/skills/`). If they meant GitHub-hosted
content or Copilot-style instructions, WP2's packaging changes, nothing else.

## 2. Why it bumbles (FACT, from 34 real runs in `/tmp/kr0ki-storyb00k-debug/*.jsonl`; small sample, older runs predate some guards)
| # | Cause | Evidence |
|---|---|---|
| 1 | **No syntax grounding.** Per-type skills exist for 4 of 24 catalog types (PlantUML activity/component/erd/sequence). In discover mode (the default) only `magicgrid.md` + `requirements_quality.md` (~550 B) load. | `containers/kr0ki-storyb00k-agent/server.py` ~791-804 |
| 2 | **Uninformative tool errors.** 75 of 121 renders failed; 70 surfaced as bare `HTTP Error 400: Bad Request`. The server does return a useful body (e.g. `Syntax Error? (Assumed diagram type: sequence)`); the agent's `http_call` drops it. One thread burned 64 rounds retrying format `svg`. | logs; `manifest_dispatch.http_call` |
| 3 | **Weak loop control.** One thread made 14 byte-identical renders then hit the 15-round cap; the guard only covers `svg/png` and 3 consecutive failures. | thread `d4bc5ad5` |
| 4 | **Prompt pushes render-and-inspect exploration** with ~19 tools and a quantised 27B local model. | avg 5.4 rounds on real runs, max 64 |
| 5 | **Opaque UI.** Tool steps show only a name and "running/failed": no arguments, error text, plan, mode, round counter or budget. | `playbook/src/components/StoryB00k.vue` ~174-180, 670-675 |

**Pilot (FACT, this session; `tools/skill-pilot/`).** The local model (via pi, no skill, no examples) was asked for six
advanced-syntax examples per format and each was rendered by kr0ki: **graphviz 6/6, d2 3/6, plantuml 1/6, nwdiag 1/6
(11/24, 46%)**. Failures were exactly the guessing the user sees: PlantUML sources without `@startuml`, invented nwdiag
shapes/attributes (`cylinder`, `router`, `stack`), D2 escaping/substitution mistakes. Two consequences: skills have
real value, **and the renderer is a free, automatic judge of every example** (the failing run took 9-19 s per format).

## 3. Target design (PROPOSAL unless stated; **runtime choice, brand and skill typing revised by the design note**)
```
 playbook UI (Vue)  <--SSE ui events-->  kr0ki-server (:8787, MCP tool manifest, render, catalog)
        |  /run (AG-UI)                          ^
        v                                        | MCP (stdio bridge or HTTP binding)
 harness: pi  --mode rpc  (sidecar)  --- pi-mcp-adapter (directTools) ---+
   package "kr0ki-diagram-harness":  skills/  extensions/  prompts/
   local model: llama.cpp :8002 (vision, tool calling)
```
- **Skills as the unit of knowledge.** One pi skill per diagram *language* (syntax, escaping, advanced variants, worked
  examples that are render-verified) and one per diagram *type* (when to use it, what makes it good/bad, anti-patterns,
  hints), plus `brand-iconography` and `app-palette` skills. Layout: `skills/diagrams/<lang>/SKILL.md` + `types/<id>.md`.
  Keep each skill small and trigger-rich (pi advertises only name+description; the body loads on demand).
- **Gate extension (the "skills agent").** FACT: pi skill loading is model-judged and can be skipped ("a model might fail
  to load a relevant skill"). So: a TypeScript extension using `pi.on("tool_call")` blocks any render call whose format has
  no recorded skill read and tells the model which skill to read; `tool_result` records reads; `before_agent_start`/`input`
  can pre-inject the skill for the type the user selected in the gallery; `agent_before_settle` can force one more turn.
  Works in print/json/rpc (no TUI needed). **UNVERIFIED in practice**: build and test it first (WP3).
- **Plan-before-act and legible errors**, not just more knowledge (see §5): the extension should also require a one-line
  plan, dedupe identical calls, and return the renderer's real error body.
- **b00t integration.** Register each skill as a b00t datum (`~/.b00t/_b00t_/diagram-<lang>.skill.toml`, copy
  `ab-experiment-dispatch.skill.toml`; fields `[b00t]` name/type="skill"/type_tags/keywords, `[b00t.learn].content`;
  tomllm variant adds `depends_on`/`unlocks`). `b00t skill search diagram` returns nothing today (FACT). Short tips via
  `b00t lfmf`; retrieval/RAG via `b00t skill activate` and `b00t grok`. **Source of truth = the kr0ki repo**, synced to
  datums and to the pi package, never hand-edited in two places.
- **UFO typing (PROPOSAL; nothing named Skill/Usage exists in ufo-types — FACT).** Reuse what is there
  (`capability.rs`: `CapabilityDomain`, `Task`, `Attempt`/`ActionRecord`, `ReviewVerdict`, `History`): DiagramLanguage = Kind;
  DiagramSkill = Kind in a `CapabilityDomain` (Act authors, Verify reviews); BrandStyle = Mode/Role; skill usage =
  Event (like `ActionRecord`); the render outcome judged by a `ReviewVerdict` (Relator). Edges from existing `UfoRelation`
  only (`Specializes`, `Selects`/`Binds`, `Instantiates`, `Precedes`, `GovernedBy`, `TracesTo`). Per `AGENTS.md` §4 the typed
  layer lives in `ufo-types` (upstream) — propose the type there; kr0ki only emits usage events. Vocabulary: OMG terms verbatim,
  "projection" banned (`VOCABULARY.md`).
- **Brand assets (FACT: little exists).** Only `~/.b00t/b00t-vscode/icons/*.svg|png`, `templates/*.d2`, and UI tokens in
  `playbook/src/style.css`. No palette file found. The palettes and icon sets are a **design decision for the user**, not
  something to invent (see §6).

## 4. Work packages (in order; each has an acceptance test)
| WP | Task | Who | Acceptance |
|---|---|---|---|
| **0** | **Quick win, ~1 hour:** make tool errors carry the HTTP body (`manifest_dispatch.http_call` / `server.py` catch of `HTTPError`); dedupe identical consecutive render calls; show the real error in the UI step | Claude agent | A bad PlantUML source yields `Syntax Error? (Assumed diagram type: sequence)` to the model; an identical re-render is refused with a hint; unit tests |
| 1 | **(first slice DONE: d2, graphviz, plantuml-class, nwdiag; 13 syntaxes + 6 PlantUML types remain)** **Skill corpus pipeline:** per language, generate candidate examples with the local model through pi (`tools/skill-pilot/pilot.py` is the seed), **keep only those that render**, store source + the gotcha sentence. Start with the 13 syntaxes that have no skill and PlantUML's 6 missing types | Local model via pi for drafting; Claude agent reviews | Each skill has >=8 render-verified examples; script re-verifies all examples in CI against a Kroki backend |
| 2 | **Skills content** (syntax, type quality, brand): human-reviewed; trigger-rich descriptions; size budget (<4 KB per skill body, longer material in `references/`) | Claude agent + user review of brand/palette | Review checklist per skill; spot-check by rendering |
| 3 | **(gate DONE in the Python agent, see §7 of the design note; the pi extension is NOT built)** **pi package + gate extension** (`kr0ki-diagram-harness`): `skills/`, `extensions/gate.ts`, `prompts/`; kr0ki MCP tools via `pi-mcp-adapter` with `directTools: true` | Claude agent | Run 20 canned prompts per language against :8002: with the gate, **0 renders happen before the matching skill is read**; render-success rate vs the no-skill baseline (§2) |
| 4 | **Harness mode in kr0ki:** run pi as a sidecar (`--mode rpc`), adapt the AG-UI `/run` endpoint (or a feature flag next to the Python agent) so the UI is unchanged; planner threads keep their own tool set | Claude agent | Same UI works against either backend; existing 50 agent tests + playbook tests pass |
| 5 | **UX legibility** in `StoryB00k.vue`: a **plan card** before acting (mode, chosen type, skills loaded, round x/15, question budget); tool steps expandable with arguments, format and the real error; "stopped because ..." as a structured message with Retry/Edit | Claude agent | Component tests; a screenshot walk-through against a real run |
| 6 | **b00t + UFO registration:** datums for every skill; `b00t lfmf` tips; usage events (skill loaded-for type, outcome verdict) written as a `SysGraph` snapshot or `b00t influence` entry; propose the Skill type upstream in `ufo-types` | Claude agent; user decides upstream | `b00t skill search diagram` lists them; a usage report answers "which skills were loaded, and did the render pass?" |
| 6b | **Identifier rule** (skills require identifier-as-key, name-as-label). **SysML display names: DONE in v0.0.7** (`to_d2_named`, cache token `-v2`); WP0 also DONE (real error bodies, identical-render refusal) | Claude agent | Remaining: write the rule into the skills; rendered SysML already shows names and carries the `@id` |
| 6c | **SVG enhancement layer** (identifier stamping, CSS-selected brand rules, packages, later live state): design note §8 order of work | Claude agent | Spike tests ported to the real implementation; browser check of selectors/icons/colours |
| 7 | **MBSE skills** (14 in the report) as pi skills on top of the same harness, read-only first | Later; needs a live SysML v2 server | See the report's 30/60/90 plan |

## 5. Operating notes (FACT unless marked)
- **One GPU slot.** The vision model on `:8002` (container `b00t-heretic`) serves the planner chat too. Run bulk errands
  while nobody is using the UI, and keep them small. `~20.6 GB` VRAM; cold start 1.5-2.5 min.
- **pi pitfalls.** Always close stdin (`</dev/null`) in print/json runs or they hang (also true of `pi --help`).
  Isolate config with `PI_CODING_AGENT_DIR=<dir>` plus `PI_OFFLINE=1 PI_SKIP_VERSION_CHECK=1 PI_TELEMETRY=0`; `~/.pi` was
  untouched in the smoke test. `--no-tools --no-extensions --no-skills --no-context-files` gives a clean text run.
  `pi-mcp-adapter` also reads `~/.config/mcp/mcp.json`, `~/.agents/mcp.json` and `.mcp.json` regardless of the agent dir.
- **models.json** shape that works: provider `local`, `baseUrl http://127.0.0.1:8002/v1`, `api openai-completions`,
  `input ["text","image"]`; model id = the gguf path from `GET /v1/models`.
- **Verified in the smoke test:** plain prompt 7.7 s; a skill was auto-loaded on demand through a `read` call (6.5 s).
  **Not tested:** extensions, vision through pi, `pi-mcp-adapter` with kr0ki's bridge, rpc mode, tool-name prefixes.
- **Repo/agent facts:** agent in `containers/kr0ki-storyb00k-agent/` (Python; `PLANNER_*` constants; skills in `skills/`);
  catalog in `crates/kr0ki-core/src/catalog.rs` (24 types / 17 syntaxes; 26 `DiagramFormat` slugs); MCP manifest
  `GET /mcp/tools` (19 tools). `.env` `OPENAI_API_URL` must be `:8002`, not the dead `:8001`.
- **Budget discipline** (user instruction): delegate research and bulk drafting; keep orchestration context small; read
  reports not transcripts; cap subagent output (<=700 words) and have them write files.

## 6. Decisions only the user can make
1. Brand: **configurable** (decided). Still needed from the owner: the canonical logo/icon sets and palettes (b00t, PromptExecution, per application) to ship as the first real packages. None is on disk beyond the b00t VS Code icons; `brand.example.json` is a placeholder.
2. ~~Harness~~ **Decided direction (design note §7):** skills as portable `SKILL.md`; Python agent stays the UX runtime with deterministic injection + gate; pi is the second harness (MBSE, errands, bulk generation).
3. Where skills are hosted/auto-loaded ("GitHub auto-loaded", see §1).
4. `Skill` **is a UFO stereotype** (decided). Still to approve: the upstream `ufo-types` change (a Kind in a `CapabilityDomain` with an identifier and an icon reference); see design note §6.
5. MBSE: whether to proceed to a read-only live demo against the organisation's SysML v2 server (needs a URL/token).

## 7. Status update (end of 2026-10-01) and how to add a language skill
**Built and live (v0.0.7 + the gate):** `skills/diagrams/{d2,graphviz,plantuml,nwdiag}/SKILL.md` (portable `SKILL.md`; hand-written rules
**each checked against the renderer** + examples **that all render**, 27/27); a **deterministic gate** in the Python agent
(`language_skill`, `SkillRequiredError` in `server.py`): the first `render_diagram` in a language that has a skill is held back
and the model receives the guide (the user sees one line: "Loaded the d2 syntax guide"); the retry runs. Gating does not
count as a failure. Verified live: an nwdiag request was gated, then rendered. Mutation-checked.

**Measured (single sample, local 27B model, new drawing tasks, renderer as judge; `tools/skill-pilot/results-ab-2026-10-01.json`):**
without the skill 12/24 rendered, with it 21/24 (nwdiag 1->6, d2 2->4, graphviz 5->6, plantuml 4->5). The model could not
learn nwdiag or much D2 from error messages alone (repair loop: plantuml 6->9 of 9, d2 stuck at 5/8, nwdiag 0/8), so those
rules had to be hand-written from verified behaviour. Indicative, not statistical.

**Add a language skill (process):**
1. Pick the format slug (`GET /formats`). Start the stack (kr0ki :8787 + Kroki :8010) and the local model (:8002).
2. `PI_SMOKE_DIR=<isolated pi dir> python3 tools/skill-pilot/generate.py out.json <lang>`: the model drafts examples, the renderer judges,
   failures are repaired with the real error. If first-try success is poor (nwdiag was 0/8), write examples by hand.
3. Probe each rule you intend to write with a one-line render (`/render/<fmt>`) and record both the passing and the failing form; write
   only what the renderer confirmed into `tools/skill-pilot/skill-src/<lang>.md`. Add the language to `build_skills.py` (IDENT, DESC, example source).
4. `python3 tools/skill-pilot/build_skills.py out.json containers/kr0ki-storyb00k-agent/skills/diagrams` (re-renders every example; skips failures; <=5800 chars).
5. `python3 tools/skill-pilot/verify_skills.py containers/kr0ki-storyb00k-agent/skills/diagrams` must print `N/N examples render`.
6. `AB_ONLY=<lang> PI_SMOKE_DIR=... python3 tools/skill-pilot/ab.py <skills dir> ab.json` to measure with vs without; keep the result file.
7. Agent tests: `cd containers/kr0ki-storyb00k-agent && .venv/bin/python -m unittest discover -p "test_*.py"` (static shape check covers the new skill); restart the agent (`just stop-agent && just start-agent`).
Pitfall: the SysML/graphviz `D2` cache token and skills are independent; but if you change what a renderer route emits, bump its cache version token.
