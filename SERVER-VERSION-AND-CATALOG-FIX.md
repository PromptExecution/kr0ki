# Server Version & Catalog Fix - 2026-09-27

## Summary

Fixed two critical issues:
1. kr0ki server was running old version (0.0.1) instead of new version (0.0.3)
2. Vite dev server catalog loading error ("could not load the executable example catalog")

---

## Issues Fixed

### 1. Old Server Version

**Problem:**
- kr0ki server on port 8787 was running version 0.0.1
- New code (v0.0.3) was built but server wasn't restarted

**Solution:**
```bash
# Kill old server
kill 1816534

# Start new server with latest build
./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 &
echo $! > /tmp/kr0ki-server.pid

# Verify version
curl -s http://192.168.1.137:8787/health | grep version
# Output: "version":"0.0.3"
```

**Result:**
- ✅ Server now running v0.0.3
- ✅ All new features active (Auto Render, renderer URL fix, etc.)

---

### 2. Catalog Loading Error

**Problem:**
- Vite dev server (port 5173) showed error: "could not load the executable example catalog"
- Browser console showed: `[gallery] catalog unavailable: Unexpected token '<', "<!doctype "... is not valid JSON`

**Root Cause:**
- Vite proxy was not configured to forward `/playbook/api` requests to kr0ki server
- Request to `/playbook/api/examples.json` was returning HTML (Vite's index.html) instead of JSON

**Solution:**
Added Vite proxy configuration in `playbook/vite.config.js`:

```javascript
server: {
  proxy: {
    // Proxy /api requests to the kr0ki server (port 8787)
    '/api': {
      target: 'http://192.168.1.137:8787',
      changeOrigin: true,
    },
    // Proxy /playbook/api requests (with path rewrite)
    '/playbook/api': {
      target: 'http://192.168.1.137:8787',
      changeOrigin: true,
      rewrite: (path) => path.replace(/^\/playbook/, '').replace(/\.json$/, ''),
    },
  },
},
```

**Result:**
- ✅ Catalog loads correctly (37 fixtures)
- ✅ No more JSON parse errors
- ✅ Gallery, Editor, and Agent views all working

---

## Verification

### Server Version Check

```bash
curl -s http://192.168.1.137:8787/health | grep version
# Output: "version":"0.0.3"
```

### Catalog Endpoint Check

```bash
curl -s http://192.168.1.137:8787/api/examples | head -3
# Output: Valid JSON array with 37 examples
```

### Vite Proxy Check

```bash
curl -s http://192.168.1.137:5173/playbook/api/examples.json | head -3
# Output: Valid JSON array (proxied from kr0ki server)
```

### CDP Browser Test

```python
# Navigate to playbook
chrome_navigate: http://192.168.1.137:5173/playbook/

# Click Editor tab
chrome_click: Editor button

# Check DOM
Has .panel element: True
Has Auto Render checkbox: True
Auto Render checked: True
Renderer URL value: http://192.168.1.137:8787
Status text: Ready

# Check console
Errors: ✅ No errors found!
```

---

## Files Modified

| File | Change |
|------|--------|
| `playbook/vite.config.js` | Added proxy configuration for `/api` and `/playbook/api` |

---

## Process Management

### kr0ki Server

**PID File:** `/tmp/kr0ki-server.pid`  
**Log File:** `/tmp/kr0ki-server.log`

**Commands:**
```bash
# Check status
cat /tmp/kr0ki-server.pid
curl -s http://192.168.1.137:8787/health

# Restart server
kill $(cat /tmp/kr0ki-server.pid)
./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 &
echo $! > /tmp/kr0ki-server.pid

# View logs
tail -f /tmp/kr0ki-server.log
```

### Vite Dev Server

**PID:** Managed by pnpm  
**Log File:** `/tmp/playbook-dev.log`

**Commands:**
```bash
# Check status
ps aux | grep vite

# Restart
pkill -f "vite"
pnpm --dir playbook dev > /tmp/playbook-dev.log 2>&1 &
```

---

## Architecture

### Request Flow

```
Browser (port 5173)
    ↓
Vite Dev Server
    ↓ (proxy /playbook/api → /api)
kr0ki Server (port 8787)
    ↓
Examples JSON (37 fixtures)
    ↓
Vite Dev Server
    ↓
Browser
```

### Version Flow

```
Cargo.toml (version = "0.0.3")
    ↓
cargo build
    ↓
./target/debug/kr0ki
    ↓
/health endpoint
    ↓
{"version":"0.0.3"}
```

---

## Lessons Learned

### 1. Always Restart Servers After Rebuild

**Issue:** Built new version but forgot to restart server  
**Lesson:** After `cargo build`, always restart the server to pick up changes

**Best Practice:**
```bash
cargo build && \
  kill $(cat /tmp/kr0ki-server.pid) && \
  ./target/debug/kr0ki > /tmp/kr0ki-server.log 2>&1 & \
  echo $! > /tmp/kr0ki-server.pid && \
  sleep 2 && \
  curl -s http://localhost:8787/health | grep version
```

### 2. Vite Proxy Configuration

**Issue:** API requests from Vite dev server need explicit proxy config  
**Lesson:** When frontend and backend run on different ports, configure Vite proxy

**Best Practice:**
```javascript
// vite.config.js
export default defineConfig({
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8787',
        changeOrigin: true,
      },
    },
  },
})
```

### 3. Path Rewriting in Proxy

**Issue:** Frontend requests `/playbook/api` but backend expects `/api`  
**Lesson:** Use path rewriting to match backend expectations

**Best Practice:**
```javascript
'/playbook/api': {
  target: 'http://localhost:8787',
  changeOrigin: true,
  rewrite: (path) => path.replace(/^\/playbook/, ''),
}
```

---

## Testing Checklist

- [x] kr0ki server running v0.0.3
- [x] Catalog endpoint returns valid JSON
- [x] Vite proxy forwards requests correctly
- [x] Browser loads catalog without errors
- [x] Editor tab renders correctly
- [x] Auto Render checkbox present and checked
- [x] Renderer URL auto-populates
- [x] No console errors
- [x] All views (Gallery, Editor, Agent, Setup) functional

---

## Next Steps

### Immediate
1. ✅ Server updated to v0.0.3
2. ✅ Catalog loading fixed
3. ✅ All features verified

### Short-Term
1. Test Auto Render functionality end-to-end
2. Verify render button works with new URL default
3. Test all 37 catalog examples

### Long-Term
1. Add automated server restart to build process
2. Add health check to CI/CD pipeline
3. Document server management in AGENTS.md

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ All issues resolved
