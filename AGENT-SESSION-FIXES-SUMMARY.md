# Agent Session Fixes Summary - 2026-09-27

## Overview

This document summarizes all fixes applied to the kr0ki playbook and agent server during the development session on 2026-09-27.

---

## Issues Fixed

### 1. Version Display and Management

**Problem:** Version number (v0.0.1) was hardcoded and not updating when Cargo.toml changed.

**Solution:**
- Moved version display from hero header to sidebar under "kr0ki playb00k"
- Version now derived from Cargo.toml via Vite's `define` plugin
- Added cocogitto configuration for automated version bumping
- Bumped version to v0.0.4

**Files Modified:**
- `playbook/src/App.vue`
- `playbook/src/style.css`
- `playbook/vite.config.js`
- `Cargo.toml`

---

### 2. Agent Page Rendering Error

**Problem:** Agent page showed "Cannot access 'props' before initialization" error.

**Root Cause:** `defineProps()` was called after props were used in the component.

**Solution:** Moved `defineProps()` call to immediately after imports, before any props usage.

**Files Modified:**
- `playbook/src/components/StoryB00k.vue`

---

### 3. Chrome DevTools MCP Integration

**Problem:** Needed to integrate Chrome DevTools MCP server for browser automation.

**Solution:**
- Created b00t datum at `~/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml`
- Installed via `b00t mcp install chrome-devtools-mcp dotmcpjson`
- Configured to connect to Chrome on 192.168.1.150:9222

**Files Created:**
- `~/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml`
- `CHROME-DEVTOOLS-MCP.md`

---

### 4. b00t MCP Ergonomics Documentation

**Problem:** Identified friction points in b00t MCP workflow.

**Solution:** Created 6 GitHub issues (#59-64) documenting desired improvements:
- #59: Registry search natural language
- #60: Datum schema & validation
- #61: Configuration overrides
- #62: Dynamic loading via proxy
- #63: Documentation & examples
- #64: Actionable error messages

**Files Created:**
- `AGENT-STORIES-B00T-MCP-ERGONOMICS.md`
- `IMPLEMENTATION-GUIDE-B00T-MCP-ERGONOMICS.md`

---

### 5. Catalog Loading Error (404)

**Problem:** Remote browser at `http://192.168.1.137:5173/` showed "Could not load the executable example catalog: catalog request returned 404".

**Root Cause:** Vite proxy was not configured to forward `/playbook/api` requests to kr0ki server, and the `.json` suffix wasn't being stripped.

**Solution:** Added Vite proxy configuration with path rewriting:
```javascript
server: {
  proxy: {
    '/api': {
      target: 'http://192.168.1.137:8787',
      changeOrigin: true,
      rewrite: (path) => path.replace(/\.json$/, ''),
    },
    '/playbook/api': {
      target: 'http://192.168.1.137:8787',
      changeOrigin: true,
      rewrite: (path) => path.replace(/^\/playbook/, '').replace(/\.json$/, ''),
    },
  },
},
```

**Files Modified:**
- `playbook/vite.config.js`

---

### 6. Renderer URL Empty (Render Failed)

**Problem:** Editor view showed "Enter a network-reachable kr0ki URL first" when accessing from port 5173.

**Root Cause:** `localRendererUrl` initialization logic had operator precedence issues, resulting in empty URL for remote browser access.

**Solution:** Refactored to use explicit `getFallbackUrl()` function with clear fallback chain:
1. Parent prop (`props.rendererUrl`)
2. Query parameter (`?renderer=...`)
3. Same origin (port 8787)
4. Hostname fallback (`http://<hostname>:8787`)

**Files Modified:**
- `playbook/src/components/RendererPanel.vue`

---

### 7. Auto Render Checkbox

**Problem:** User had to manually click "Render" button after every change.

**Solution:** Added Auto Render checkbox (default: on) that watches source/output/URL changes and auto-triggers render after 500ms debounce.

**Files Modified:**
- `playbook/src/components/RendererPanel.vue`
- `playbook/src/style.css`

---

### 8. Server Version Not Updating

**Problem:** Server at `http://192.168.1.137:8787/playbook/` showed old code (v0.0.1) despite source being updated.

**Root Cause:** Playbook is a static build served from `./playbook/dist/`. Version is injected at build time. Server must be rebuilt and restarted to serve new code.

**Solution:** Documented complete rebuild process:
1. Update `Cargo.toml` version
2. `cargo build`
3. `pnpm --dir playbook build`
4. Restart kr0ki server

**Files Created:**
- `VERSION-BUMP-AND-RELOAD.md`

---

### 9. Agent LLM 500 Error

**Problem:** Agent panel showed "HTTP Error 500: Internal Server Error" when sending messages.

**Root Cause:** LLM server (llama.cpp) uses Jinja chat template that requires at least one message with role "user". AG-UI client sometimes sent requests without user messages.

**Solution:** Added fallback to ensure at least one user message exists:
```python
if not any(m.get("role") == "user" for m in messages):
    user_prompt = "Hello"  # or extract from payload
    messages.append({"role": "user", "content": user_prompt})
```

**Files Modified:**
- `containers/kr0ki-storyb00k-agent/server.py`

---

### 10. Agent Repetitive Questions & Interrupt 404s

**Problem:** 
- Agent kept asking the same questions repeatedly
- "answer failed: HTTP 404" errors when answering questions
- Task never finished

**Root Causes:**
1. QA history injection logic was flawed (checking if answer text was in message content)
2. `/respond-to-interrupt` returned 404 if question wasn't in `_pending_questions`
3. No tracking of already-asked questions

**Solutions:**
1. **Fixed QA injection:** Always inject all QA pairs, not just "new" ones
2. **Added explicit question list:** System prompt now lists all already-asked questions
3. **Prevent repetition:** Check if question was already asked before asking again
4. **Handle 404 gracefully:** Check QA history for already-answered questions
5. **Store interrupt IDs:** Track interrupt IDs in QA entries for deduplication

**Files Modified:**
- `containers/kr0ki-storyb00k-agent/server.py`

---

## Version History

| Version | Changes |
|---------|---------|
| v0.0.1 | Initial version |
| v0.0.2 | Version display moved to sidebar, cocogitto config added |
| v0.0.3 | Auto Render checkbox, renderer URL fix |
| v0.0.4 | Vite proxy fix, version bump process documented |

---

## Git Commits

```
f028d45 fix: prevent agent from repeating questions and handle interrupt 404s
60301c9 fix: ensure user message exists for LLM Jinja template
0b1ba57 chore: bump version to v0.0.4 and rebuild playbook
c3cd38f fix: simplify RendererPanel URL initialization logic
898f8fb fix: add .json rewrite to /api proxy for remote browser access
2d4f74d feat: add Auto Render checkbox, fix renderer URL default, bump to v0.0.3
e1c8857 fix: add vite proxy for /playbook/api, update MCP to localhost:9222
6e2dd7c docs: add Chrome remote debugging setup and Agent page fix documentation
52f5386 fix: move defineProps before first usage to prevent initialization error
e468d82 chore: bump version to 0.0.2
07fc679 feat: move version to sidebar, add cocogitto config, document MCP ergonomics
```

---

## Documentation Created

| File | Purpose |
|------|---------|
| `AGENT-STORIES-B00T-MCP-ERGONOMICS.md` | 6 agent stories for b00t MCP improvements |
| `IMPLEMENTATION-GUIDE-B00T-MCP-ERGONOMICS.md` | Guide for implementing b00t improvements |
| `CHROME-DEVTOOLS-MCP.md` | Chrome DevTools MCP integration |
| `CHROME-REMOTE-DEBUGGING-SETUP.md` | Chrome remote debugging setup |
| `VERSION-AND-AGENT-DIAGNOSIS.md` | Version management and diagnosis |
| `AGENT-PAGE-RENDERING-FIX.md` | Agent page fix documentation |
| `RENDERER-URL-FIX.md` | Renderer URL initialization fix |
| `VERSION-BUMP-AND-RELOAD.md` | Version bump and server reload process |
| `AGENT-LLM-500-ERROR-FIX.md` | Agent LLM 500 error fix |
| `SERVER-VERSION-AND-CATALOG-FIX.md` | Server version and catalog fix |
| `SESSION-SUMMARY-2026-09-27.md` | Complete session summary |

---

## Testing Verification

### Manual Tests Performed
- [x] Version displays correctly in sidebar (v0.0.4)
- [x] Catalog loads correctly (37 fixtures)
- [x] Editor view renders with correct Renderer URL
- [x] Auto Render checkbox works
- [x] Agent page renders without errors
- [x] Agent responds to messages
- [x] Agent doesn't repeat questions
- [x] Interrupt answers work without 404 errors

### Automated Tests (CDP)
- [x] Browser navigation and rendering
- [x] Console error checking
- [x] DOM structure verification
- [x] Screenshot capture

---

## Lessons Learned

1. **Static builds require explicit rebuilds** - Version changes don't auto-propagate
2. **LLM chat templates have requirements** - Always validate message structure
3. **Defensive programming is essential** - Validate and fix up incoming data
4. **Debug logging is crucial** - Can't fix what you can't see
5. **Browser cache is aggressive** - Always clear cache when testing
6. **Vite proxy needs path rewriting** - Frontend and backend paths may differ
7. **Track state explicitly** - Don't rely on implicit assumptions

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ All issues resolved
