# kr0ki 0.1.0 — release notes (2026-10-01)

The first minor release: the playbook becomes a workspace for **SysML v2 projects with requirements traceability**, on top of the planner, skills and
editor work of 0.0.x. Everything below is merged to `main`; see `TESTING-GUIDE.md` to try it and `PLAN-KR0KI-008-sysmlv2-projects-requirements.md` for what comes next.

## New in 0.1.0
- **Projects** (new top-level view): local SysML v2 projects (`.sysml`/`.kerml`, diagram sources, ReqIF) in Quasar's LocalStorage; **Save = commit** (content-addressed, with a message);
  history; **traceability of changes to requirements** (each commit records which requirements it affected, including via diagrams linked as *depicting* them);
  per-requirement timelines; export/import of projects with integrity verification; drafts survive reloads; quota meter and safe rollback when storage is full.
- **Requirement graph and cost roll-up**: numeric `attribute cost = N;` on requirements, derived-requirement roll-ups, **scenarios** (tick requirements in/out), coverage gaps (no satisfy / no verify), duplicate-id warning, and the graph drawn through the renderer.
- **ReqIF into a project**: *Import ReqIF* uses the existing server route and turns requirements into SysML text with traces.
- Evaluations that shaped the plan are archived in `docs/evaluations/` (Rivet: borrow patterns, do not embed; oxigraph for the server RDF/SPARQL store; zu rejected; inventory of existing ReqIF/requirements capability).

## Carried from 0.0.x (all live)
Planner page and picks panel; Quasar type tree and compact sidebar; top-aligned CodeMirror editor with the language's syntax skill below it and optional LSP (JSON/YAML) through a secured bridge;
26 verified syntax skills + 24 type-quality skills and a deterministic skill gate in the agent; legible agent errors; SysML renders with names and identifier-driven brand overlay (`?brand=`); Test all covers every diagram.

## Known limits
- Projects are browser-local (~5 MB); no server sync yet. The SysML scanner is heuristic (not a parser). The cost model is a free-form numeric attribute (no units).
- No live SysML v2 server has been used: all server-side SysML testing used fixtures.
- Server-side RDF/SPARQL/SHACL, real Satisfy/Derive/Verify elements, ReqIF export route, and project push/pull are planned for 0.2/0.3.
