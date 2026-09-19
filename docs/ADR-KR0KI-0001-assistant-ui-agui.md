# ADR-KR0KI-0001 — assistant-ui Vue + AG-UI for StoryB00k; CopilotKit rejected

**Status:** Accepted (2026-09-18) — implements Plan 004 Phase 0 item 1.
**Parent:** [`PLAN-KR0KI-004-revisioned-procedural-workspace.md`](PLAN-KR0KI-004-revisioned-procedural-workspace.md) §0, §8.
**Supersedes:** the prototype's `@synoped/ag-ui-vue` binding (to be removed in Phase 2.1, not now).

---

## Context

Plan 004 converts StoryB00k from a chat dashboard into a revisioned procedural
workspace. The chat/tool presentation library, the agent event protocol, and the
absence of an orchestration framework are contract decisions that must be frozen
before any product migration. The shipped prototype (`playbook/src/components/StoryB00k.vue`,
`containers/kr0ki-storyb00k-agent/server.py`) already proves AG-UI streaming via
`@ag-ui/client` + `@synoped/ag-ui-vue`, but its `DraftGraph` state model is not a
workspace and must not constrain the contracts.

## Decision

1. **Chat/tool UI library: `@assistant-ui/vue` (assistant-ui Vue), exclusively.**
   All transcript rendering, composer, tool-call cards, and the pending-approval
   card use assistant-ui's Vue package and its Vue tool UI APIs. We do not build a
   second bespoke chat component system beside it.

2. **Agent event protocol: AG-UI.** The sidecar emits AG-UI events
   (`RUN_*`, message, tool, `STATE_SNAPSHOT`, `STATE_DELTA`). On join/reconnect the
   server sends a full `STATE_SNAPSHOT`; subsequent state changes are ordered
   `STATE_DELTA` operations. If the installed Vue package lacks a direct AG-UI
   runtime adapter, we implement a small local assistant-ui custom-runtime
   transport adapter inside `playbook/` — we do not fork assistant-ui.

3. **CopilotKit is rejected — no dependency, adapter, wrapper, or migration path.**
   Reasons: (a) it is React-first; kr0ki's browser is Vue and Plan 004 forbids a
   React island; (b) its `useCopilotAction` model encourages client-side mutation
   execution, while Plan 004 invariant 5 requires every agent write to be a
   revision-scoped, server-validated proposal the workspace service owns; (c) it
   couples the UI to an orchestration layer, which §8 defers until contracts are
   accepted. This rejection is final for Plan 004's lifetime; revisiting it
   requires a new ADR that answers (a)–(c).

4. **No orchestration framework.** LangGraph, LangChain, Vercel AI SDK workflow
   layers, and equivalents stay out until Plan 004 Phase 0's exit gate (fixture
   trace from connect → proposal → approval → reconnect) is recorded and accepted.
   The existing simple sidecar loop is adequate while contracts are proved.

5. **Server remains the sole durable writer.** assistant-ui and AG-UI are
   presentation/transport only. They never write revisions client-side; the
   Vue bridge applies server-authorized state deltas to local view state.

## Consequences

- Phase 0 item 2 must pin a verified `@assistant-ui/vue` release and build the
  throwaway AG-UI↔assistant-ui adapter spike before `playbook/package.json`
  changes (`@synoped/ag-ui-vue` removal is a Phase 2.1 action, gated on the spike).
- No second chat library, no React, no extra state manager may enter
  `playbook/package.json`.
- If assistant-ui's Vue support proves insufficient at the spike, the correct
  escalation is a new ADR, not a silent CopilotKit side-door.
- Compliance with Plan 004 §8 non-goals: no DOM mutation, no `eval`, no framework
  defaults hiding the identity-provider or merge decisions.

## Phase 0 research addendum (2026-09-18, verified against npm + upstream repo)

**`@assistant-ui/vue` is NOT published to npm.** Verified 2026-09-18:
- `npm view @assistant-ui/vue` → 404 (checked latest + canary dist-tags).
- Upstream repo `assistant-ui/assistant-ui` `packages/vue/package.json`:
  `"private": true`, `"version": "0.0.0"`, README: *"Not published yet. This
  package is the in-repo preview of the Vue bridge and stays private until the
  Vue integration is complete."*
- The package is real and actively developed (source commits the day of this
  note): `AuiProvider`, `useAui`, `useAuiState`, `useAuiEvent`, thread/message/
  composer/tool primitives. Its runtime layer runs on `@assistant-ui/tap` with a
  `react` → `@assistant-ui/tap/standalone-shim` Vite alias (no React renderer needed).
- **The official AG-UI adapter (`@assistant-ui/react-ag-ui` 0.0.59) is React-only**
  (peerDeps `react ^18||^19`). There is no Vue AG-UI adapter upstream. This plan's
  "implement a small local assistant-ui custom-runtime/transport adapter" clause
  (§6 Phase 0.2) is therefore mandatory, not a fallback: an AG-UI → external-store
  (`ExternalStoreRuntimeCore` + `RuntimeAdapter`, both exported from
  `@assistant-ui/core/internal` / `@assistant-ui/core/store`) adapter must be
  written locally.
- Note: `@ag-ui/client` is now at 1.0.0; the prototype pins `^0.0.59`. The spike
  must verify protocol compatibility between the sidecar's emitted events and the
  client version finally pinned.

### Pinning options for Phase 0 item 2 (decision needed before the spike)

1. **Git dependency**: `"@assistant-ui/vue": "github:assistant-ui/assistant-ui#<sha>,semver:packages/vue"` style install from the monorepo (works with pnpm/npm workspaces; fragile — private package, no API stability guarantee).
2. **Vendor the source**: copy `packages/vue/src` into `playbook/` temporarily for the throwaway spike (no supply-chain surprise, but drifts from upstream).
3. **Wait for publication** and run Phase 0 item 3 (JSON Schema + Rust types, no UI dependency) first — schemas and the workspace store are UI-independent and are on the critical path anyway.

Operator decision required; option 3 is recommended because it is unblocked work
and Phase 0's exit gate is the fixture trace, which can be produced schema-first
with the existing sidecar events.

## Verification

| Claim | How to verify |
|---|---|
| Only approved UI deps | `jq .dependencies playbook/package.json` after Phase 2.1 shows no `@synoped/*`, no copilotkit |
| Adapter spike works | Phase 0 exit-gate fixture trace consumes real `RUN_*`/`STATE_SNAPSHOT`/`STATE_DELTA` events through assistant-ui |
| No orchestration SDK | no langgraph/langchain/ai-sdk entries anywhere in the workspace manifests |

<!-- b00t:map v1
summary: ADR — assistant-ui Vue is the sole chat/tool UI, AG-UI the transport; CopilotKit and orchestration frameworks rejected
tags: kr0ki, adr, storyb00k, assistant-ui, vue, ag-ui, copilotkit, plan-004
tier: frontier
complexity: 6
-->
