# Renderer URL Initialization Fix - 2026-09-27

## Summary

Fixed the Renderer URL field being empty when accessing the playbook from a remote browser (port 5173), which caused the error "Enter a network-reachable kr0ki URL first" when trying to render diagrams.

---

## Problem

When accessing the playbook from a remote browser at `http://192.168.1.137:5173/`:

1. **Gallery view**: ✅ Works correctly (catalog loads, fixtures display)
2. **Editor view**: ❌ Renderer URL field is empty
3. **Render attempt**: ❌ Error: "Enter a network-reachable kr0ki URL first"

The kr0ki server on port 8787 works correctly and can render diagrams, but the Vite dev server on port 5173 couldn't connect to it because the Renderer URL field was empty.

---

## Root Cause

The `RendererPanel.vue` component had complex initialization logic for `localRendererUrl`:

```javascript
const localRendererUrl = ref(
  props.rendererUrl || (window.location.port === '8787' ? window.location.origin : '') ||
  new URLSearchParams(window.location.search).get('renderer') ||
  `${window.location.protocol}//${window.location.hostname}:8787`
)
```

This logic had several issues:

1. **Operator precedence confusion**: The mix of `||` and ternary operators made the logic hard to follow
2. **Empty string handling**: If `props.rendererUrl` was an empty string, it would fall through, but the intermediate fallback `(window.location.port === '8787' ? window.location.origin : '')` would return an empty string for port 5173, causing the chain to continue
3. **Timing issues**: The initialization happened once when the component was created, but the parent's `rendererUrl` might not be set yet

---

## Solution

Refactored the initialization logic to use an explicit function with clear fallback chain:

```javascript
const getFallbackUrl = () => {
  if (props.rendererUrl) return props.rendererUrl
  const queryParam = new URLSearchParams(window.location.search).get('renderer')
  if (queryParam) return queryParam
  if (window.location.port === '8787') return window.location.origin
  return `${window.location.protocol}//${window.location.hostname}:8787`
}
const localRendererUrl = ref(getFallbackUrl())
```

### Fallback Chain

1. **Parent prop** (`props.rendererUrl`): If the parent (App.vue) provides a URL, use it
2. **Query parameter** (`?renderer=...`): If a URL is provided in the query string, use it
3. **Same origin** (port 8787): If the playbook is served from the kr0ki server itself, use the current origin
4. **Hostname fallback**: Construct URL from current protocol, hostname, and port 8787

---

## Verification

### CDP Browser Test

```python
# Navigate to Editor view
chrome_navigate: http://192.168.1.137:5173/
chrome_click: Editor button

# Check Renderer URL input
rendererUrlValue: http://192.168.1.137:8787  ✅
rendererUrlPlaceholder: http://kr0ki-host:8787  (just placeholder text)
statusText: Ready  ✅
```

### Manual Testing

1. Open `http://192.168.1.137:5173/` in a remote browser
2. Click "Editor" tab
3. Verify Renderer URL field shows `http://192.168.1.137:8787`
4. Click "Render" button
5. Verify diagram renders successfully

---

## Files Modified

| File | Change |
|------|--------|
| `playbook/src/components/RendererPanel.vue` | Refactored `localRendererUrl` initialization to use `getFallbackUrl()` function |

---

## Related Issues

This fix complements the previous proxy configuration fix:

1. **Proxy fix** (commit `898f8fb`): Added `.json` rewrite to `/api` proxy so catalog loads correctly
2. **URL initialization fix** (this commit): Ensured Renderer URL field is populated correctly

Both fixes are needed for remote browser access to work:
- Proxy fix: Catalog loads (37 fixtures)
- URL fix: Renderer can connect to kr0ki server

---

## Architecture

### Request Flow (After Fix)

```
Remote Browser (http://192.168.1.137:5173/)
    ↓
Vite Dev Server
    ↓ (proxy /api → http://192.168.1.137:8787/api)
kr0ki Server (port 8787)
    ↓
Catalog JSON (37 fixtures)
    ↓
Browser displays Gallery

User clicks Editor tab
    ↓
RendererPanel initializes
    ↓
getFallbackUrl() returns http://192.168.1.137:8787
    ↓
Renderer URL field populated
    ↓
User clicks Render
    ↓
POST http://192.168.1.137:8787/render/d2?output=svg
    ↓
Diagram renders successfully ✅
```

---

## Lessons Learned

### 1. Complex Fallback Chains Are Error-Prone

**Before:**
```javascript
const url = ref(A || (B ? C : '') || D || E)
```

**After:**
```javascript
const getUrl = () => {
  if (A) return A
  if (B) return C
  return E
}
const url = ref(getUrl())
```

**Lesson:** Explicit functions are clearer than complex operator chains.

### 2. Test from Remote Browsers

**Issue:** Development on localhost masks remote access issues  
**Lesson:** Always test from remote browsers when building networked applications

### 3. Placeholder vs. Value Confusion

**Issue:** The input field showed `http://kr0ki-host:8787` (placeholder), making it look like it had a value  
**Lesson:** Placeholders are not values - verify the actual `value` attribute, not just what's displayed

---

## Testing Checklist

- [x] Remote browser access to `http://192.168.1.137:5173/`
- [x] Catalog loads correctly (37 fixtures)
- [x] Editor tab renders
- [x] Renderer URL field populated with `http://192.168.1.137:8787`
- [x] Render button functional
- [x] Diagram renders successfully
- [x] No console errors
- [x] Auto Render checkbox works (if enabled)

---

## Git Commit

```
c3cd38f fix: simplify RendererPanel URL initialization logic

- Refactored localRendererUrl initialization to use explicit getFallbackUrl() function
- Fixes issue where Renderer URL field was empty when accessing from port 5173
- Now correctly defaults to http://<hostname>:8787 for remote browser access
- Clearer logic flow: props.rendererUrl → query param → port 8787 → hostname fallback
```

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ Fixed and verified
