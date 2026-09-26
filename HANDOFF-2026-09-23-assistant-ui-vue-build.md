# HANDOFF — 2026-09-23 — Assistant-UI Vue Build Integration

**Context:** Fixed `just test` failures caused by vendored `@assistant-ui/vue` not being built. Implemented Option C: Minimal build with automated wiring.

---

## 1. What Was Done

### Problem
- `just test` failed on the Playbook Vue test suite with: `Failed to resolve entry for package "@assistant-ui/vue"`
- The `@assistant-ui/vue` package is **not published to npm** — it's a preview package in the assistant-ui monorepo
- kr0ki had vendored the entire assistant-ui monorepo as a git submodule but wasn't building the Vue package
- The Playbook test only validates the API surface exports (`AuiProvider`, `AuiConfig`, `useAui`, etc.) — actual StoryB00k still uses `@synoped/ag-ui-vue`

### Solution Implemented
- **Option C: Minimal build** — Build the Vue package from the submodule, not vendoring compiled output
- Added `build-assistant-ui-vue` justfile recipe that builds the package in ~3-4 seconds
- Wired this recipe into `build` and `test` so it runs automatically
- Updated AGENTS.md with documentation and a "sharp corner" note

---

## 2. Architecture & Wiring

### Git Submodule Structure
```
vendor/assistant-ui/                  # Git submodule (elasticdotventures/assistant-ui@b00t-vue-package)
├── packages/vue/
│   ├── src/                          # Vue source (TypeScript)
│   ├── package.json                  # "private": true, exports point to ./dist
│   └── dist/                         # Built output (not committed)
└── .gitignore                        # Includes "dist"

playbook/
├── package.json
│   └── dependencies
│       └── "@assistant-ui/vue": "file:../vendor/assistant-ui/packages/vue"
└── node_modules/.pnpm/
    └── @assistant-ui+vue@file+.../node_modules/@assistant-ui/vue/  # Symlink to vendor
```

### Build Flow
```
just build-assistant-ui-vue
  └─> cd vendor/assistant-ui
      ├─> pnpm install --frozen-lockfile      # Install monorepo deps (~1.5m, first time only)
      └─> pnpm --filter @assistant-ui/vue build  # Build Vue package to dist/ (~3-4s)

just test
  ├─> cargo test --workspace                 # Rust tests
  ├─> Python tests (kr0ki-mcp, storyb00k-agent)
  ├─> just build-assistant-ui-vue            # <--- New: Build Vue package
  ├─> pnpm --dir playbook install            # Re-link dist/ into pnpm store
  └─> pnpm --dir playbook test               # Playbook Vue tests (15 tests)
```

---

## 3. Files Modified

### `justfile`
```diff
 # Build everything (Rust workspace + vendored @assistant-ui/vue).
 build:
     cargo build --workspace
+    just build-assistant-ui-vue

+# Build the vendored @assistant-ui/vue package from the assistant-ui submodule.
+build-assistant-ui-vue:
+    cd vendor/assistant-ui && pnpm install --frozen-lockfile && pnpm --filter @assistant-ui/vue build

 test:
     cargo test --workspace
     cd containers/kr0ki-mcp && python3 test_bridge.py && python3 test_http_worker.py
     cd containers/kr0ki-storyb00k-agent && python3 -m venv .venv && .venv/bin/pip install -q -r requirements.txt && .venv/bin/python3 -m unittest discover -p 'test_*.py'
+    just build-assistant-ui-vue
     pnpm --dir playbook install --frozen-lockfile
     pnpm --dir playbook test
```

### `AGENTS.md`
```diff
 just build-assistant-ui-vue  # build vendored @assistant-ui/vue from the submodule
 just test                     # unit + in-process HTTP (all pass, builds assistant-ui first)
```

Added sharp corner entry:
```diff
+🤓 **@assistant-ui/vue is a preview (not on npm).** The package is vendored as a
+git submodule at `vendor/assistant-ui/` (branch `b00t-vue-package`). It must be built
+before playbook tests can run: `just build-assistant-ui-vue` (or it runs as part of
+`just test` / `just build`). The built `dist/` is not committed — it lives in the
+submodule's `.gitignore`. If `just test` fails on playbook import, run
+`just build-assistant-ui-vue` then `pnpm --dir playbook install`.
```

---

## 4. Status & Verification

### All Tests Passing
```
just test
  └─> 163 Rust tests passed
  └─> 7 kr0ki-mcp Python tests passed
  └─> 43 kr0ki-storyb00k-agent Python tests passed
  └─> 15 Playbook Vue tests passed
  └─> Total: 228 tests, 0 failed, 0 ignored (live tests require env vars)
```

### Build Outputs
- `vendor/assistant-ui/packages/vue/dist/` — 154 files, 253.71 kB built
- Includes JS, sourcemaps, and TypeScript declaration files (.d.ts)
- Exports validated by test: `AuiProvider`, `AuiConfig`, `useAui`, `useAuiState`, `useAuiEvent`

### Dependencies
- pnpm workspace build works within the submodule
- No changes to kr0ki's root `package.json` or `pnpm-lock.yaml`
- Playbook `package.json` unchanged — still uses file: dependency

---

## 5. Context for Future Work

### Plan 004 Phase 0 Status
**Current Phase:** Proposed — no UI migration authorized yet.

The Plan 004 documents say:
- Phase 0.2 requires: "Pin and verify the compatible `@assistant-ui/vue` release. Build a throwaway Vue adapter spike"
- **The spike has NOT been written yet** — we only built the package, not the adapter
- Current StoryB00k still uses `@synoped/ag-ui-vue` (per `StoryB00k.vue` line 3)
- The ADR explicitly notes there's no Vue AG-UI adapter upstream — a custom adapter must be written

### When to Migrate
Per the plan:
1. **Phase 0 exit gate:** Record a fixture trace through connect → proposal → approval → reconnect
2. **Phase 2.1:** Remove `@synoped/ag-ui-vue` from Playbook after the adapter spike passes
3. **Phase 2:** Split `StoryB00k.vue` into workspace shell components using assistant-ui Vue

### Tracking
- ADR-KR0KI-0001 documents the Vue adoption decision and notes the preview status
- PLAN-KR0KI-004 has the full roadmap for the revisioned procedural workspace
- The submodule branch is `b00t-vue-package` — track changes there

---

## 6. Common Commands

```bash
# Build everything
just build

# Build just the Vue package
just build-assistant-ui-vue

# Run full test suite (includes Vue build)
just test

# Run only Rust tests (faster, no Vue build)
cargo test --workspace

# Run only Playbook tests (requires Vue build first)
just build-assistant-ui-vue && pnpm --dir playbook test

# Update the submodule to latest from our fork
git submodule update --remote vendor/assistant-ui
cd vendor/assistant-ui && git checkout b00t-vue-package

# Rebuild after submodule changes
cd vendor/assistant-ui && pnpm --filter @assistant-ui/vue build
pnpm --dir playbook install
```

---

## 7. Known Issues & Sharp Corners

### Build Time Impact
- First-time build: ~1.5 minutes (pnpm install in the monorepo, one-time only)
- Subsequent builds: ~3-4 seconds (pnpm build, uses cache)
- `just test` now includes this step — added ~3-4 seconds to total time

### Submodule Synchronization
- If `vendor/assistant-ui` submodule commits change, run `just build-assistant-ui-vue` again
- The `dist/` directory is **not committed** — must rebuild after any source change
- CI/CD must include the build step before playbook tests

### Pnpm Store Caching
- Playbook's pnpm store hardlinks from the vendor source during install
- If dist didn't exist at install time, the link won't include it
- Solution: Always `just build-assistant-ui-vue` → `pnpm --dir playbook install` in sequence

### Preview Package Status
- `@assistant-ui/vue` is not on npm — this is expected per upstream docs
- The package is marked `"private": true` and `"version": "0.0.0"` in the vendor
- Upstream docs say "Vue binding is in preview" — APIs may change
- We're tracking a custom fork branch (`b00t-vue-package`) for our work

---

## 8. Orientation for Next Agent

### What This Is
- This is **build integration work**, not product migration
- We made the preview Vue package buildable and testable in kr0ki
- No UI code changes — StoryB00k still uses `@synoped/ag-ui-vue`
- The actual assistant-ui migration is blocked on Plan 004 Phase 0 exit gate

### What This Isn't
- This is **not** the Phase 0 adapter spike — that's separate work
- This is **not** removing `@synoped/ag-ui-vue` — that's Phase 2.1
- This is **not** committing compiled output — dist/ stays in .gitignore
- This is **not** changing the UI framework — Vue remains our standard

### Where to Start Next
1. Read ADR-KR0KI-0001 for the Vue adoption context
2. Read PLAN-KR0KI-004 for the full roadmap
3. Decide if you're working on Phase 0 (adapter spike) or Phase 2 (UI migration)
4. If working on the adapter: Implement `ExternalStoreRuntimeCore` adapter for AG-UI → assistant-ui Vue
5. If working on UI migration: Wait for Phase 0 exit gate first

### Key Files
- `vendor/assistant-ui/packages/vue/` — Vue package source (submodule)
- `playbook/src/components/__tests__/assistant-ui-vendor.test.js` — API validation test
- `playbook/src/components/StoryB00k.vue` — Current UI using `@synoped/ag-ui-vue`
- `docs/ADR-KR0KI-0001-assistant-ui-agui.md` — Adoption decision
- `docs/PLAN-KR0KI-004-revisioned-procedural-workspace.md` — Full roadmap

---

## 9. Verification Checklist

Use this to verify the build is working correctly after any changes:

```bash
# 1. Clean build
rm -rf vendor/assistant-ui/node_modules vendor/assistant-ui/packages/vue/dist
just build-assistant-ui-vue

# 2. Verify dist exists
ls vendor/assistant-ui/packages/vue/dist/index.js

# 3. Reinstall playbook
pnpm --dir playbook install

# 4. Run playbook tests
pnpm --dir playbook test  # Should pass with 15 tests

# 5. Full test suite
just test  # Should pass with 228 tests
```

All commands should complete successfully.

---

## 10. Questions for Decision Makers

1. **When should Phase 0 adapter spike start?** The build is ready, but the spike is a separate work item.
2. **Should we track upstream for Vue publication?** If/when `@assistant-ui/vue` publishes to npm, we can switch from file: dependency.
3. **CI/CD integration:** Should the build step be added to CI, or is local-only sufficient for now?
4. **Submodule maintenance:** Who owns the `b00t-vue-package` branch upstream? When do we sync back to assistant-ui/assistant-ui main?

---

**End of handoff.** For questions about the assistant-ui integration, reference ADR-KR0KI-0001 and PLAN-KR0KI-004. For build issues, run the verification checklist above.