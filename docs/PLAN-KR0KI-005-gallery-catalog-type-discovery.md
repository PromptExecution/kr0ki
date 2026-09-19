# PLAN-KR0KI-005 — Gallery catalog & type-discovery agent

Status: proposed (2026-09-19)
Depends on: PR #35 (revision graphs, projects, planning agent), Plan 004 Phase 0
Supersedes: the "catalog/discovery" line item in the Plan 004 roadmap

## 0. Outcome

A user who has something to convey — but does not know (or care) which diagram
syntax fits — can discover the right type two ways, and both paths converge:

1. **Gallery**: browse by *intent* ("process flow", "who talks to whom"),
   not by syntax. Every diagram type has a distinct, addressible name.
2. **Agent**: say what they want to convey in plain language; the agent runs a
   **type-discovery question series** (different from requirement refinement)
   and recommends a type with a rendered sample before locking anything in.

Core ergonomics assumption (user directive 2026-09-19): **users do not know
the graph type they need.** They know the idea they want to convey. Both the
gallery's information architecture and the agent's question series must work
from intent → type, never assuming type → intent.

## 1. Gallery: intent-first catalog

### 1.1 Catalog data (single source of truth)

New Rust module `crates/kr0ki-core/src/catalog.rs` (exported to the playbook
via `/api/catalog`; hardcoded taxonomy lives in Rust per the architectural
rule, never in JS):

```rust
pub struct DiagramType {
    pub id: &'static str,          // addressible name, e.g. "sequence"
    pub syntax: &'static str,      // kr0ki format: "plantuml", "d2", "graphviz"
    pub name: &'static str,        // "Sequence diagram"
    pub use_cases: &'static [&'static str],  // intent tags, e.g. ["process flow", "interaction"]
    pub blurb: &'static str,       // one sentence: when to choose this
    pub example_id: Option<&'static str>,    // fixture to render in the card
}
```

Initial taxonomy (top-level types; PlantUML expanded beyond today's single
example):

- **Structure**: component, class, object, deployment, ERD (`plantuml`,
  `erd`, `dbml`), package, state machine
- **Interaction**: sequence (`plantuml` sequence), communication, timing,
  network (`nwdiag`), rack (`rackdiag`), packet (`packetdiag`)
- **Flow**: activity/process flow (`plantuml` activity), flowchart (`d2`,
  `graphviz`), use case, state transition
- **Data**: ERD, JSON/YAML, Vega-Lite charts (`vegalite`, `vega`)
- **Layout/sketch**: block diagram (`blockdiag`, `ditaa`, `svgbob`, `goat`),
  bytefield, wireviz (wiring), pikchr, nomnoml, umlet, structurizr (C4),
  c4plantuml, tikz, wavedrom, symbolator, k8s-topology, kubediagram

Each `use_case` tag is drawn from a small controlled vocabulary (also in
Rust): `process flow`, `interaction`, `data model`, `network`, `timeline`,
`sketch`, `chart`, `architecture`, `state machine`, `hardware`.

### 1.2 UX

- **Filter bar** above the grid: `All | process flow | interaction | data
  model | network | …` — **defaults to "All"** and auto-selects "All" whenever
  a search/filter is cleared. Multi-tag OR matching.
- Each card: rendered thumbnail (the fixture, cached), the type name, its
  syntax badge, blurb, and **two buttons**: `Test` (existing behavior) and
  `Agent` (new).
- Card is deep-linkable: `/playbook/?type=sequence` (distinct addressible
  name in the URL fragment/query).

### 1.3 The `Agent` button (gallery → agent handoff)

Adjacent to `Test`. Clicking it:

1. Switches to Agent view.
2. Pre-populates the composer with a sample prompt that **explicitly names the
   type**, e.g. for `sequence`:
   > "Draw a **sequence diagram** for: <one-line blurb>. Start by asking me
   > about the participants and the flow."
3. Keeps focus in the composer — the user edits before sending. Nothing is
   auto-sent.
4. Carries `syntax` + `typeId` as `forwardedProps` on the run so the agent
   knows the gallery context without the user restating it.

## 2. Agent: type-discovery question series

### 2.1 Two distinct question modes

Today's `ask_user` serves requirement refinement. Add a parallel mode:

| Mode | When | Question series |
|---|---|---|
| `refine` (existing) | type already known/locked | participants, scope, level of detail |
| `discover` (new) | **type unknown** — the default when the user hasn't named a syntax | audience → intent → shape → fidelity |

The system prompt decides the mode: if the user has not named a syntax AND
PROJECT MEMORY holds no locked type, `discover` mode is mandatory before any
render.

### 2.2 The discover series (new `recommend_diagram_type` tool)

New agent tool `recommend_diagram_type` — like `ask_user` but structured for
type discovery; server-side it can ALSO drive the gallery (see 2.4). Question
series the prompt enforces, one `ask_user` round each, max 3 before the agent
must commit to a recommendation:

1. **Audience/purpose**: "Who is this for and what should they walk away
   with?" (options: developers / managers / mixed audience; document / live
   discussion / onboarding)
2. **Intent shape**: "Which is closest to what you want to convey?" (options
   drawn from the controlled vocabulary: *how a process flows*, *who talks to
   whom and in what order*, *how things are structured/related*, *how data is
   organized*, *what the network looks like*, *how it changes state over time*)
3. **Fidelity** (only if still ambiguous): "Sketch-quality or
   production/documentation-quality?"

Then the agent **recommends** 1 primary + 1 alternative type with a one-line
rationale each ("A sequence diagram shows the request→approval→provisioning
handoffs in time order; an activity diagram would suit if you care more about
the decision branches than the participants"), and asks a single confirm
question ("Go with sequence?" [sequence / activity / show me both]).

"Show me both" renders BOTH as sample panels — visual comparison beats prose.

### 2.3 Type lock-in and memory

- The chosen type is recorded in the project: `typeId` + `syntax` join
  `requirements` in the lock-in (server-side capture, same `Locked in:`
  marker, extended to parse `Type: <id>`).
- PROJECT MEMORY gains: `Type chosen: sequence (plantuml) — do not switch
  without asking.`
- Gallery handoff (`forwardedProps`) skips discovery round 2 (the shape is
  already implied) but still confirms.

### 2.4 Server support

- `recommend_diagram_type` tool schema served from the manifest-adjacent tool
  list; arguments carry `mode: "discover"|"refine"`, `question`, `options`,
  `typeOptions` (id+name+blurb triples when options are diagram types).
- Interrupts extend `responseSchema.properties.options.default` as today —
  type options ride the same channel the UI already renders.
- `/api/catalog` (GET, kr0ki server): the taxonomy above; the agent's skills
  file is generated from it (single source of truth, no drift).

## 3. Per-syntax agent skills (foundation, not full rollout)

Directory layout lands now; content per type lands incrementally:

```
containers/kr0ki-storyb00k-agent/skills/types/<type-id>.md
```

- Loaded into the system prompt ONLY when the type is locked (keeps context
  small vs. today's all-skills dump).
- Each file: what the type is good/bad at, syntax gotchas, a minimal correct
  sample, and the render route/format pair.
- Ship with `sequence.md`, `flow.md`, `erd.md`, `component.md`; the catalog
  generator flags types missing a skill file (checklist in `just check`-adjacent
  gate, warning only initially).

## 4. Delivery plan

1. **Catalog core** — `catalog.rs` taxonomy + `/api/catalog` + fixture
   linkage; Rust unit tests for the controlled vocabulary (every example's
   format resolves; every typeId unique).
2. **Gallery UX** — filter bar (default All, auto-reset), card grid from
   `/api/catalog`, deep links, `Test` + `Agent` buttons; `Agent` prefill via
   forwardedProps. Vitest: filter selection, prefill content, deep-link parse.
3. **Discover mode** — `recommend_diagram_type` tool + prompt-mode rules +
   type lock-in parsing. Python E2E: discover series → recommend → confirm →
   type captured in project; re-run does not re-discover.
4. **Skills split** — skills/types/<id>.md loading by locked type; four seed
   files; missing-skill warning.
5. **Live acceptance** — the user test: "I want to show how our release
   process works" with NO syntax named → agent discovers (≤3 questions) →
   recommends → confirm → renders; and from the gallery: click Agent on a
   card → composer pre-filled → send → renders that type.

## 5. Non-goals

- No new render backends (Kroki serves all syntaxes already).
- No user-defined taxonomies (Rust-owned vocabulary, PR-reviewed).
- No auto-sent agent runs from gallery clicks (prefill only — the user is
  always the one who sends).
- Full per-type skill content is incremental; only 4 seed files in this plan.
