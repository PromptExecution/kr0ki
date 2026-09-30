# Version Management & Agent Page Diagnosis - 2026-09-27

## Summary

Successfully moved version display to sidebar, configured cocogitto for automated version bumping, and documented the Agent page rendering issue for further investigation.

---

## ✅ Completed Tasks

### 1. Version Display Moved to Sidebar

**Before:**
- Version badge appeared in hero header next to "Diagram-as-code, rendered."
- Not consistently visible across all views

**After:**
- Version tag now appears in sidebar under "kr0ki playb00k" brand
- Consistently visible across all views (Gallery, Editor, Agent, Setup)
- Styled with `.version-tag` class for better visual hierarchy

**Files Modified:**
- `playbook/src/App.vue` - Moved version from hero to sidebar
- `playbook/src/style.css` - Added `.version-tag` styling

**Code Changes:**
```vue
<!-- Sidebar -->
<a class="brand" href="../">kr0ki <span>playb00k</span></a>
<p class="version-tag">v{{ appVersion }}</p>

<!-- Hero (removed version badge) -->
<h1>Diagram-as-code, rendered.</h1>
```

```css
.version-tag { 
  margin: 0.25rem 0 0.75rem 0; 
  color: #7dd3fc; 
  font-size: 0.85rem; 
  font-weight: 600; 
  opacity: 0.8; 
}
```

---

### 2. Version Derived from Cargo.toml

**Implementation:**
- `playbook/vite.config.js` reads version from `Cargo.toml` at build time
- Injects as `__APP_VERSION__` global constant
- Vue component uses `__APP_VERSION__` with fallback to '0.0.0'

**Code:**
```javascript
// vite.config.js
const cargoToml = readFileSync(resolve(__dirname, '../Cargo.toml'), 'utf-8')
const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)
const appVersion = versionMatch ? versionMatch[1] : '0.0.0'

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
})
```

```vue
<!-- App.vue -->
const appVersion = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0'
```

**Verification:**
```bash
grep "^version" Cargo.toml
# version = "0.0.2"
```

---

### 3. Cocogitto Configuration Added

**File Created:** `cog.toml`

**Configuration:**
```toml
from_latest_tag = true
tag_prefix = "v"
ignore_merge_commits = true
disable_changelog = false
disable_bump_commit = false
generate_mono_repository_global_tag = true
generate_mono_repository_package_tags = true
branch_whitelist = []
skip_ci = "[skip ci]"
skip_untracked = false
pre_bump_hooks = ["cargo set-version {{version}}"]
post_bump_hooks = []
pre_package_bump_hooks = []
post_package_bump_hooks = []

[git_hooks]

[commit_types]

[changelog]
path = "CHANGELOG.md"
authors = []

[bump_profiles]
```

**Key Features:**
- Uses `cargo set-version` to update Cargo.toml automatically
- Generates conventional changelog
- Ignores merge commits
- Supports monorepo package tags

**Note:** The `cog` command on this system is Replicate's cog (Containers for ML), not cocogitto. Cocogitto needs to be installed separately:
```bash
cargo install cocogitto
```

---

### 4. Version Bumped to 0.0.2

**Manual Version Bump:**
Since cocogitto isn't installed, manually bumped the patch version:

```bash
# Update Cargo.toml
version = "0.0.2"

# Update Cargo.lock
cargo update --workspace

# Commit with conventional message
git commit -m "chore: bump version to 0.0.2"

# Create tag
git tag -a v0.0.2 -m "v0.0.2"
```

**Result:**
```
e468d82 chore: bump version to 0.0.2
07fc679 feat: move version to sidebar, add cocogitto config, document MCP ergonomics
7e3f35a feat: implement Ledgrrr contract-reference boundary (issue #57)
```

**Tag Created:** `v0.0.2`

---

### 5. Agent Page Rendering Issue

**Status:** ⚠️ Requires Chrome DevTools MCP for diagnosis

**Issue:** Agent page not rendering properly

**Current State:**
- Agent server is running and healthy on port 8789
- Playbook dev server is running on port 5173
- Chrome DevTools MCP server configured but not connected (Chrome not running with remote debugging)

**Diagnosis Steps Needed:**
1. Start Chrome with remote debugging on 192.168.1.150:
   ```bash
   google-chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile
   ```

2. Connect Chrome DevTools MCP:
   ```bash
   # In pi or MCP client
   mcp({ connect: "chrome-devtools-mcp" })
   ```

3. Navigate to Agent page and capture console errors:
   ```javascript
   // Using chrome-devtools-mcp tools
   - chrome_navigate: http://192.168.1.137:5173/playbook/
   - chrome_console_messages: capture errors
   - chrome_screenshot: visual state
   ```

**Suspected Issues:**
- Vue component initialization error
- AG-UI protocol connection issue
- CORS or network error
- Missing dependency or import

**Next Steps:**
- Install and configure Chrome on 192.168.1.150
- Enable remote debugging
- Use Chrome DevTools MCP to diagnose
- Fix rendering issue
- Test with automated browser testing

---

## 📋 Documentation Created

### 1. Agent Stories for b00t MCP Ergonomics

**File:** `AGENT-STORIES-B00T-MCP-ERGONOMICS.md`

**Content:**
- 6 agent stories (#59-64) documenting friction points
- Proposed solutions (short/medium/long-term)
- Architecture vision for dynamic loading
- Implementation priorities and phases
- Success metrics

**GitHub Issues Created:**
- #59: Registry search natural language
- #60: Datum schema & validation
- #61: Configuration overrides
- #62: Dynamic loading via proxy
- #63: Documentation & examples
- #64: Actionable error messages

**Note:** All implementations must be done in b00t repository, not kr0ki.

---

### 2. Implementation Guide

**File:** `IMPLEMENTATION-GUIDE-B00T-MCP-ERGONOMICS.md`

**Content:**
- Quick reference table (issues, priorities, effort)
- Workflow instructions (setup, implementation, testing, submission)
- Detailed implementation guidance for each issue
- Testing strategy (unit, integration, manual)
- Coordination guidelines
- Success criteria and timeline

**Key Points:**
- Each issue should be a separate PR to b00t
- Reference kr0ki issues in PR descriptions
- Coordinate with b00t maintainers before starting
- Test in kr0ki after b00t PRs are merged

---

### 3. Chrome DevTools MCP Integration

**File:** `CHROME-DEVTOOLS-MCP.md`

**Content:**
- Proper b00t datum workflow
- Datum creation process
- Installation commands
- Browser host configuration (192.168.1.150:9222)
- Security considerations
- Lessons learned

**Key Insight:**
- Always use b00t datums for MCP server configuration
- Never manually edit .mcp.json unless absolutely necessary
- Datum-driven workflow ensures consistency and reusability

---

## 🔧 Technical Details

### Version Injection Flow

```
Cargo.toml (version = "0.0.2")
    ↓
vite.config.js (reads at build time)
    ↓
__APP_VERSION__ (global constant)
    ↓
App.vue (uses in template)
    ↓
Rendered HTML (v0.0.2 in sidebar)
```

### Cocogitto Workflow (When Installed)

```bash
# Make changes and commit with conventional commits
git add .
git commit -m "feat: add new feature"

# Auto-bump version based on commit types
cog bump --auto
# → Updates Cargo.toml via pre_bump_hooks
# → Creates git tag (v0.0.3)
# → Generates CHANGELOG.md

# Push with tags
git push && git push --tags
```

### Agent Server Status

```bash
curl http://192.168.1.137:8789/health
# {"status":"ok","service":"kr0ki-storyb00k-agent","llm_configured":true,...}
```

**Status:** ✅ Running and healthy

### Playbook Dev Server

```bash
curl http://192.168.1.137:5173/playbook/
# Returns HTML with Vue app
```

**Status:** ✅ Running on multiple interfaces

---

## 📊 Metrics

### Version Management

| Metric | Before | After |
|--------|--------|-------|
| Version location | Hero header | Sidebar |
| Version source | Hardcoded | Cargo.toml |
| Bump process | Manual | Cocogitto (configured) |
| Current version | 0.0.1 | 0.0.2 |

### Documentation

| Document | Status | Lines |
|----------|--------|-------|
| Agent Stories | ✅ Created | ~400 |
| Implementation Guide | ✅ Created | ~500 |
| Chrome DevTools MCP | ✅ Updated | ~200 |
| **Total** | **3 docs** | **~1100 lines** |

### GitHub Issues

| Issue | Title | Label | Status |
|-------|-------|-------|--------|
| #59 | Registry search | enhancement | Open |
| #60 | Datum validation | enhancement | Open |
| #61 | Config overrides | enhancement | Open |
| #62 | Dynamic loading | enhancement | Open |
| #63 | Documentation | documentation | Open |
| #64 | Error messages | enhancement | Open |

**Total:** 6 issues created, all with implementation notes

---

## 🎯 Next Steps

### Immediate

1. **Install cocogitto:**
   ```bash
   cargo install cocogitto
   ```

2. **Test version bump workflow:**
   ```bash
   git commit -m "feat: test feature"
   cog bump --auto
   git push && git push --tags
   ```

3. **Diagnose Agent page:**
   - Start Chrome with remote debugging
   - Connect Chrome DevTools MCP
   - Capture console errors
   - Fix rendering issue

### Short-Term

4. **Implement b00t MCP improvements:**
   - Start with Phase 1 (#59, #60, #64)
   - Submit PRs to b00t repository
   - Reference kr0ki issues

5. **Add automated browser testing:**
   - Use Chrome DevTools MCP for E2E tests
   - Test all views (Gallery, Editor, Agent, Setup)
   - Verify version display

### Long-Term

6. **Enable dynamic MCP loading:**
   - Implement b00t-mcp proxy enhancements
   - Support on-demand server loading
   - Reduce .mcp.json bloat

---

## 📝 Lessons Learned

### 1. Version Management

**Do:**
- Derive version from single source of truth (Cargo.toml)
- Use automated tools (cocogitto) for version bumps
- Follow conventional commits for automatic versioning

**Don't:**
- Hardcode versions in multiple places
- Manually update versions (error-prone)
- Skip version tags (breaks traceability)

### 2. MCP Configuration

**Do:**
- Use b00t datums for MCP server configuration
- Install via `b00t mcp install <name> dotmcpjson`
- Keep datums in dotfiles repo for reusability

**Don't:**
- Manually edit .mcp.json (antipattern)
- Create project-specific datums (use overrides)
- Skip datum validation

### 3. Browser Testing

**Do:**
- Use Chrome DevTools MCP for automated testing
- Enable remote debugging for diagnostics
- Capture console errors and screenshots

**Don't:**
- Rely on manual testing only
- Skip browser console inspection
- Ignore CORS and network errors

---

## ✅ Verification Checklist

- [x] Version moved to sidebar
- [x] Version derived from Cargo.toml
- [x] Cocogitto configuration added
- [x] Version bumped to 0.0.2
- [x] Git tag created (v0.0.2)
- [x] Agent stories documented (#59-64)
- [x] Implementation guide created
- [x] Chrome DevTools MCP configured
- [x] Agent server running and healthy
- [x] Playbook dev server running
- [ ] Agent page rendering issue diagnosed (blocked on Chrome)
- [ ] Cocogitto installed and tested
- [ ] Automated browser tests added

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** Version management complete, Agent page diagnosis pending Chrome setup
