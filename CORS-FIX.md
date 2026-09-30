# CORS Error Handling Fix - 2026-09-27

## Problem

When accessing the playbook UI from a remote host (e.g., `http://192.168.1.137:8787`), the Agent tab would fail with a generic "Failed to fetch" error. This was caused by CORS (Cross-Origin Resource Sharing) restrictions - the agent server was only configured to allow requests from `http://127.0.0.1:8787`.

## Root Cause

The storyb00k agent server has a CORS whitelist controlled by the `KR0KI_STORYB00K_ALLOWED_ORIGINS` environment variable. When this variable doesn't include the browser's origin, the browser blocks the response and the UI shows a generic network error.

## Solution

### 1. Enhanced Error Detection in UI

Modified `playbook/src/components/StoryB00k.vue` to detect CORS failures explicitly:

```javascript
// Try a no-cors request to see if the server is reachable
const testResponse = await fetch(`${agentUrl}/health`, {
  method: 'GET',
  mode: 'no-cors',
})

// If we got here with no-cors, the server is reachable but CORS is blocking
agentConnectionError.value = `CORS error: Agent server at ${agentUrl} is reachable, 
  but does not allow requests from ${currentOrigin}. 
  Update KR0KI_STORYB00K_ALLOWED_ORIGINS to include ${currentOrigin}`
```

### 2. Improved Error Display

The error message now shows:
- Agent server URL being accessed
- Browser origin (where the request is coming from)
- Specific CORS error message
- **Fix suggestion** with the exact command to run

Example error display:
```
Error: Failed to fetch
Agent server: http://192.168.1.137:8789
Browser origin: http://192.168.1.137:8787
CORS error: Agent server at http://192.168.1.137:8789 is reachable, but does not 
allow requests from http://192.168.1.137:8787. Update KR0KI_STORYB00K_ALLOWED_ORIGINS 
to include http://192.168.1.137:8787

Fix: Restart the agent server with:
export KR0KI_STORYB00K_ALLOWED_ORIGINS="http://192.168.1.137:8787,http://127.0.0.1:8787"
```

### 3. Updated start-agent.sh

The startup script now automatically detects the hostname and IP address and includes them in the CORS whitelist:

```bash
# Detect hostname and IP for CORS configuration
HOSTNAME=$(hostname)
IP_ADDR=$(hostname -I | awk '{print $1}')

# CORS configuration - allow access from multiple origins
export KR0KI_STORYB00K_ALLOWED_ORIGINS="http://127.0.0.1:8787,http://localhost:8787,http://${HOSTNAME}:8787,http://${IP_ADDR}:8787"
```

## Current Configuration

The agent server is now running with these allowed origins:
- `http://127.0.0.1:8787` (localhost)
- `http://localhost:8787` (localhost)
- `http://sm3llsl1k3s0ld3r:8787` (hostname)
- `http://192.168.1.137:8787` (IP address)

## Testing

### Verify CORS Headers

```bash
curl -s -I -X OPTIONS http://192.168.1.137:8789/health \
  -H "Origin: http://192.168.1.137:8787" \
  -H "Access-Control-Request-Method: GET" | grep -i "access-control"
```

Expected output:
```
Access-Control-Allow-Origin: http://192.168.1.137:8787
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: Content-Type
```

### Test in Browser

1. Open http://192.168.1.137:8787/playbook/
2. Click the "Agent" tab
3. You should see a green "✓ agent" badge
4. Try sending a message

## How to Fix CORS Issues in the Future

If you see a CORS error in the UI:

1. **Read the error message** - it will tell you exactly which origin needs to be allowed
2. **Copy the suggested command** from the error display
3. **Restart the agent server**:
   ```bash
   pkill -f "python3 server.py"
   cd /home/brianh/promptexecution/kr0ki
   ./start-agent.sh
   ```

## Files Modified

- `playbook/src/components/StoryB00k.vue` - Enhanced CORS error detection and display
- `start-agent.sh` - Auto-detect hostname/IP for CORS configuration

## Verification

All services are now running and accessible from remote hosts:

| Service | Port | Status | CORS |
|---------|------|--------|------|
| kr0ki server | 8787 | ✅ Running | N/A |
| kroki-compat backend | 8010 | ✅ Running | N/A |
| LLM service | 8002 | ✅ Running | N/A |
| storyb00k agent | 8789 | ✅ Running | ✅ Configured |

## Next Steps

The Agent tab should now work correctly from any of the allowed origins. If you need to access from a new host, the UI will tell you exactly what to do.
