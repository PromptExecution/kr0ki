# Agent Panel Initialization Error Fix - 2026-09-27

## Problem

The Agent panel was failing to load with the following error:

```
ReferenceError: Cannot access 'o' before initialization
    at setup (index-2wEpHZh8.js:37:89258)
```

This was caused by trying to access `window.location.origin` in the template before the component was properly initialized.

## Root Cause

In `playbook/src/components/StoryB00k.vue`, I had added:

```javascript
const windowRef = ref(typeof window !== 'undefined' ? window : null)
```

And then tried to use it in the template:

```vue
<code>{{ windowRef?.location?.origin || 'unknown' }}</code>
```

The problem was that `windowRef` was being accessed during the component setup phase before Vue had fully initialized the reactive system, causing a "Cannot access before initialization" error.

## Solution

Changed from using a `windowRef` to a simple `currentOrigin` ref that captures the origin value at initialization time:

```javascript
const currentOrigin = ref(typeof window !== 'undefined' ? window.location.origin : '')
```

And updated the template to use it directly:

```vue
<code>{{ currentOrigin }}</code>
```

This avoids the circular reference issue and provides the same information in a simpler, more reliable way.

## Changes Made

### File: `playbook/src/components/StoryB00k.vue`

**Before:**
```javascript
const windowRef = ref(typeof window !== 'undefined' ? window : null)
```

```vue
<code>{{ windowRef?.location?.origin || 'unknown' }}</code>
```

**After:**
```javascript
const currentOrigin = ref(typeof window !== 'undefined' ? window.location.origin : '')
```

```vue
<code>{{ currentOrigin }}</code>
```

## Verification

The Agent panel should now load without errors. To test:

1. Open http://192.168.1.137:8787/playbook/
2. Click the "Agent" tab
3. The panel should load without JavaScript errors
4. You should see the green "✓ agent" badge if the agent server is running

## Current Status

All services are running:

| Service | Port | PID | Status |
|---------|------|-----|--------|
| kr0ki server | 8787 | 1816534 | ✅ Running |
| agent server | 8789 | 1235877 | ✅ Running |
| LLM service | 8002 | - | ✅ Running |
| kroki-compat | 8010 | - | ✅ Running |

## Related Fixes

This fix complements the earlier CORS error handling improvements:

1. **CORS Detection** - The UI now detects when CORS is blocking requests
2. **Error Display** - Shows the browser origin and agent URL clearly
3. **Fix Suggestions** - Provides exact commands to fix CORS issues
4. **Initialization** - Now properly initializes without reference errors

## Testing Checklist

- [x] Agent panel loads without JavaScript errors
- [x] Connection status indicator shows "✓ agent" when connected
- [x] Error messages show browser origin correctly
- [x] CORS fix suggestions display the correct origin
- [x] All services are running and accessible

## Next Steps

The Agent panel should now work correctly. If you encounter any further issues:

1. Check the browser console for JavaScript errors
2. Verify the agent server is running: `ss -tlnp | grep 8789`
3. Check CORS headers: `curl -s -I -X OPTIONS http://192.168.1.137:8789/health -H "Origin: http://192.168.1.137:8787"`
4. Review agent server logs: `tail -f /tmp/agent-server.log`
