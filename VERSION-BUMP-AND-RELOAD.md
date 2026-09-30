# Version Bump and Server Reload Process - 2026-09-27

## Summary

Documented the complete process for bumping the version and ensuring the kr0ki server on port 8787 serves the latest code. This addresses the issue where the :8787 server was serving old code (v0.0.1) despite the source being updated to v0.0.3.

---

## Problem

The kr0ki server at `http://192.168.1.137:8787/playbook/` was serving old code (v0.0.1) even though:
- Source code was updated to v0.0.3
- Cargo.toml showed version 0.0.3
- The Vite dev server on port 5173 showed the correct version

**Root Cause:** The kr0ki server serves the playbook from `./playbook/dist/`, which contains a static build. When source code changes, the dist directory must be rebuilt and the server must be restarted.

---

## Solution: Complete Version Bump Process

### Step 1: Update Version in Cargo.toml

```bash
# Edit Cargo.toml
version = "0.0.4"  # Bump from 0.0.3
```

### Step 2: Rebuild Rust Server

```bash
cd /home/brianh/promptexecution/kr0ki
cargo build
```

This compiles the server with the new version embedded in the binary.

### Step 3: Rebuild Playbook

```bash
pnpm --dir playbook build
```

This rebuilds the Vue application and updates `./playbook/dist/` with:
- New JavaScript bundle (with version embedded via Vite's `define` plugin)
- New CSS bundle
- Updated `index.html`

**Important:** The version is injected at build time via `vite.config.js`:

```javascript
const cargoToml = readFileSync(resolve(__dirname, '../Cargo.toml'), 'utf-8')
const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)
const appVersion = versionMatch ? versionMatch[1] : '0.0.0'

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
})
```

### Step 4: Restart kr0ki Server

```bash
# Kill old server
ps aux | grep "./target/debug/kr0ki" | grep -v grep | awk '{print $2}' | xargs kill

# Wait for graceful shutdown
sleep 2

# Start new server
cd /home/brianh/promptexecution/kr0ki
nohup ./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 &
echo $! > /tmp/kr0ki-server.pid

# Wait for startup
sleep 3

# Verify version
curl -s http://192.168.1.137:8787/health | grep -o '"version":"[^"]*"'
# Expected: "version":"0.0.4"
```

### Step 5: Verify in Browser

```bash
# Clear browser cache and navigate
python3 /tmp/cdp-hard-refresh.py

# Expected output:
# Version tag text: v0.0.4
```

---

## Automated Script

Create a script to automate the entire process:

```bash
#!/bin/bash
# scripts/bump-and-reload.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Step 1: Update version ==="
CURRENT_VERSION=$(grep "^version" Cargo.toml | cut -d'"' -f2)
echo "Current version: $CURRENT_VERSION"

# Increment patch version
MAJOR=$(echo $CURRENT_VERSION | cut -d. -f1)
MINOR=$(echo $CURRENT_VERSION | cut -d. -f2)
PATCH=$(echo $CURRENT_VERSION | cut -d. -f3)
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"

echo "New version: $NEW_VERSION"
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml

echo "=== Step 2: Rebuild Rust server ==="
cargo build

echo "=== Step 3: Rebuild playbook ==="
pnpm --dir playbook build

echo "=== Step 4: Restart server ==="
if [ -f /tmp/kr0ki-server.pid ]; then
    kill $(cat /tmp/kr0ki-server.pid) 2>/dev/null || true
    sleep 2
fi

nohup ./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 &
echo $! > /tmp/kr0ki-server.pid
sleep 3

echo "=== Step 5: Verify ==="
VERSION=$(curl -s http://192.168.1.137:8787/health | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
echo "Server version: $VERSION"

if [ "$VERSION" = "$NEW_VERSION" ]; then
    echo "✅ Version bump successful: $CURRENT_VERSION → $NEW_VERSION"
else
    echo "❌ Version mismatch: expected $NEW_VERSION, got $VERSION"
    exit 1
fi
```

Make it executable:

```bash
chmod +x scripts/bump-and-reload.sh
```

Usage:

```bash
./scripts/bump-and-reload.sh
```

---

## Why This Happens

### Architecture

```
Source Code (Cargo.toml, Vue components)
    ↓
Build Process (cargo build, pnpm build)
    ↓
Artifacts (./target/debug/kr0ki, ./playbook/dist/)
    ↓
Server (./target/debug/kr0ki reads from ./playbook/dist/)
    ↓
Browser (serves static files from dist/)
```

### Key Points

1. **Static Build**: The playbook is a static build, not served dynamically
2. **Version Injection**: Version is embedded at build time via Vite's `define` plugin
3. **File Reading**: Server reads files from disk on each request (no caching)
4. **Browser Cache**: Browsers cache JavaScript files aggressively

### Common Pitfalls

1. **Forgetting to rebuild playbook**: Changing Vue components without rebuilding dist/
2. **Forgetting to restart server**: Server reads from dist/ but browser may cache old files
3. **Browser cache**: Even after server restart, browser may serve cached JS files
4. **Version mismatch**: Cargo.toml updated but dist/ not rebuilt

---

## Verification Checklist

After version bump, verify:

- [ ] `Cargo.toml` shows new version
- [ ] `cargo build` completes successfully
- [ ] `pnpm --dir playbook build` completes successfully
- [ ] `./playbook/dist/assets/index-*.js` contains new version string
- [ ] Server process restarted with new binary
- [ ] `curl http://192.168.1.137:8787/health` shows new version
- [ ] Browser shows new version (after cache clear)
- [ ] All features work correctly (Gallery, Editor, Agent, Setup)

---

## Git Workflow

```bash
# After version bump and rebuild
git add -A
git commit -m "chore: bump version to v0.0.4 and rebuild playbook"
git tag -a v0.0.4 -m "v0.0.4"
git push && git push --tags
```

---

## Related Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust workspace version |
| `playbook/vite.config.js` | Version injection via `__APP_VERSION__` |
| `playbook/dist/` | Built playbook (served by kr0ki server) |
| `./target/debug/kr0ki` | Built server binary |
| `/tmp/kr0ki-server.pid` | Server PID file |
| `/tmp/kr0ki-server.log` | Server log file |

---

## Lessons Learned

### 1. Always Rebuild After Version Bump

**Issue:** Updated Cargo.toml but forgot to rebuild  
**Lesson:** Version bumps require full rebuild of both server and playbook

### 2. Browser Cache is Aggressive

**Issue:** Server serving new files but browser shows old version  
**Lesson:** Always clear browser cache or use cache-busting techniques

### 3. Static Builds Don't Auto-Reload

**Issue:** Expected server to pick up changes automatically  
**Lesson:** Static builds (dist/) must be explicitly rebuilt

### 4. Version is Build-Time Constant

**Issue:** Expected `__APP_VERSION__` to be available at runtime  
**Lesson:** Vite's `define` plugin inlines values at build time

---

## Future Improvements

### 1. Automated Version Bumping

Use `cocogitto` for automated version management:

```bash
cog bump --auto  # Auto-bump based on conventional commits
```

### 2. CI/CD Pipeline

Add automated build and deploy:

```yaml
# .github/workflows/release.yml
on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo build
      - run: pnpm --dir playbook build
      - run: ./scripts/deploy.sh
```

### 3. Health Check Endpoint

Add version to health check:

```rust
// Already implemented in kr0ki-server/src/health.rs
// Returns: {"version":"0.0.4", ...}
```

### 4. Cache Busting

Add cache-busting headers to playbook assets:

```rust
// In playbook_file() handler
headers.insert("Cache-Control", "no-cache, no-store, must-revalidate");
```

---

## Quick Reference

### Full Rebuild Command

```bash
cd /home/brianh/promptexecution/kr0ki && \
  cargo build && \
  pnpm --dir playbook build && \
  pkill -f "./target/debug/kr0ki" && \
  sleep 2 && \
  nohup ./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 & \
  echo $! > /tmp/kr0ki-server.pid && \
  sleep 3 && \
  curl -s http://192.168.1.137:8787/health | grep version
```

### Verify Version

```bash
# Server version
curl -s http://192.168.1.137:8787/health | grep -o '"version":"[^"]*"'

# Browser version (via CDP)
python3 /tmp/cdp-hard-refresh.py
```

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ Process documented and verified
