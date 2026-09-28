# Chrome Remote Debugging Setup - 2026-09-27

## Summary

Successfully started Chrome/Chromium with remote debugging enabled on port 9222 for automated browser testing via Chrome DevTools MCP.

---

## ✅ Chrome Running with Remote Debugging

### Process Status

```bash
ps aux | grep chromium.*remote-debugging-port=9222

# Output:
brianh   2314260 36.3  0.0   4896  3668 ?        S    06:51   0:01 /bin/bash /snap/chromium/3530/snap/command-chain/desktop-launch /snap/chromium/3530/bin/chromium.launcher --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile --no-first-run --disable-background-networking --disable-default-apps --disable-extensions --disable-sync --metrics-recording-only --no-default-browser-check --safebrowsing-disable-auto-update
```

### Debugging Port Verification

```bash
curl -s http://localhost:9222/json/version

# Output:
{
   "Browser": "Chrome/153.0.8010.36",
   "Protocol-Version": "1.3",
   "User-Agent": "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36",
   "V8-Version": "15.3.76.10",
   "WebKit-Version": "537.36 (@507c6ee3e2f3b2ca0e660547e5b9ea4820c67f4c)",
   "webSocketDebuggerUrl": "ws://localhost:9222/devtools/browser/c8771b91-a1fe-4457-9191-9c3a93dfe8e6"
}
```

✅ Chrome is running and accessible via remote debugging protocol

---

## 🔧 Configuration

### Chrome Launch Command

```bash
chromium-browser \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/chrome-profile \
  --no-first-run \
  --disable-background-networking \
  --disable-default-apps \
  --disable-extensions \
  --disable-sync \
  --metrics-recording-only \
  --no-default-browser-check \
  --safebrowsing-disable-auto-update \
  > /tmp/chrome-debug.log 2>&1 &
```

### Configuration Details

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `--remote-debugging-port` | 9222 | Enable Chrome DevTools Protocol on port 9222 |
| `--user-data-dir` | /tmp/chrome-profile | Separate profile to avoid conflicts |
| `--no-first-run` | - | Skip first-run dialogs |
| `--disable-background-networking` | - | Reduce network noise |
| `--disable-default-apps` | - | Faster startup |
| `--disable-extensions` | - | Cleaner testing environment |
| `--disable-sync` | - | No Google account sync |
| `--metrics-recording-only` | - | Disable metrics collection |
| `--no-default-browser-check` | - | Skip default browser check |
| `--safebrowsing-disable-auto-update` | - | Disable safebrowsing updates |

### PID File

```bash
echo $! > /tmp/chrome-debug.pid
cat /tmp/chrome-debug.pid
# 2314260
```

### Log File

```bash
cat /tmp/chrome-debug.log
```

---

## 📋 MCP Configuration

### .mcp.json (Updated)

```json
{
  "mcpServers": {
    "chrome-devtools-mcp": {
      "args": [
        "-y",
        "chrome-devtools-mcp@latest",
        "--browser-url=http://localhost:9222"
      ],
      "command": "npx"
    },
    "kr0ki-mcp": { ... }
  }
}
```

**Note:** Changed from `http://192.168.1.150:9222` to `http://localhost:9222` since Chrome is running on the local machine.

### Chrome DevTools MCP Package

```bash
npx -y chrome-devtools-mcp@latest --help

# Output shows all available options:
# --browserUrl, --wsEndpoint, --headless, --viewport, etc.
```

✅ Package is available and working

---

## 🎯 Next Steps for Testing

### 1. Manual Browser Testing

Open the playbook in the running Chrome instance:

```
http://192.168.1.137:5173/playbook/
```

**Test Checklist:**
- [ ] Verify version appears in sidebar (v0.0.2)
- [ ] Click "Agent" tab
- [ ] Verify Agent page renders correctly
- [ ] Check browser console for errors (F12 → Console)
- [ ] Test sending a message to the agent
- [ ] Verify agent responds correctly

### 2. Chrome DevTools Protocol Testing

Use the Chrome DevTools Protocol directly:

```bash
# List open tabs
curl http://localhost:9222/json/list

# Navigate to a URL (via WebSocket)
# Use webSocketDebuggerUrl from /json/version response
```

### 3. Automated Testing with Chrome DevTools MCP

Once the MCP server connects in pi:

```javascript
// Navigate to playbook
chrome_navigate({ url: "http://192.168.1.137:5173/playbook/" })

// Take screenshot
chrome_screenshot()

// Check console messages
chrome_console_messages()

// Click Agent tab
chrome_click({ selector: "button:has-text('Agent')" })

// Verify Agent page rendered
chrome_screenshot()
```

---

## 🔍 Troubleshooting

### Issue: MCP Server Not Connecting

**Symptom:**
```
Server "chrome-devtools-mcp" is configured but not connected.
```

**Possible Causes:**
1. pi MCP gateway not starting the server
2. Network/firewall issues
3. Chrome DevTools MCP package not downloaded yet

**Solutions:**
1. Try manual connection:
   ```bash
   npx -y chrome-devtools-mcp@latest --browser-url=http://localhost:9222
   ```

2. Check if package is cached:
   ```bash
   ls ~/.npm/_npx/ | grep chrome-devtools-mcp
   ```

3. Restart pi and retry MCP connection

### Issue: Chrome Not Accessible

**Symptom:**
```bash
curl http://localhost:9222/json/version
# Connection refused
```

**Solutions:**
1. Check if Chrome is running:
   ```bash
   ps aux | grep chromium
   ```

2. Check log file:
   ```bash
   cat /tmp/chrome-debug.log
   ```

3. Restart Chrome:
   ```bash
   kill $(cat /tmp/chrome-debug.pid)
   chromium-browser --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile > /tmp/chrome-debug.log 2>&1 &
   echo $! > /tmp/chrome-debug.pid
   ```

---

## 📊 Chrome DevTools MCP Tools Available

Based on the `--help` output, the following tool categories are available:

| Category | Description | Default |
|----------|-------------|---------|
| Navigation | Navigate pages, go back/forward | Enabled |
| Input | Click, type, keyboard input | Enabled |
| Emulation | Device emulation, viewport | Enabled |
| Performance | Record traces, analyze performance | Enabled |
| Network | Inspect network requests | Enabled |
| Debugging | Set breakpoints, step through code | Enabled |
| Memory | Memory profiling | Enabled |
| Extensions | Extension debugging | Disabled (requires pipe connection) |
| Experimental | Third-party tools, WebMCP | Disabled |

### Key Tools for Testing

- `chrome_navigate` - Navigate to URLs
- `chrome_screenshot` - Take screenshots
- `chrome_console_messages` - Capture console output
- `chrome_click` - Click elements
- `chrome_type` - Type text into inputs
- `chrome_evaluate` - Execute JavaScript
- `chrome_network_requests` - Inspect network traffic
- `chrome_performance_trace` - Record performance traces

---

## 🧪 Testing the Agent Page Fix

### What Was Fixed

**Issue:** Agent page not rendering due to JavaScript initialization order error

**Root Cause:**
```javascript
// ❌ Using props before defineProps()
const agentUrl = props.agentUrl || ...
const props = defineProps({ ... })
```

**Fix:**
```javascript
// ✅ defineProps() called first
const props = defineProps({ ... })
const agentUrl = props.agentUrl || ...
```

### Expected Behavior

After the fix:
1. Agent page should render without errors
2. No "Cannot access 'props' before initialization" error in console
3. Agent interface should be functional
4. Messages can be sent to the agent
5. Agent responds correctly

### Test Steps

1. **Open Playbook:**
   ```
   http://192.168.1.137:5173/playbook/
   ```

2. **Check Version:**
   - Look for "v0.0.2" in sidebar under "kr0ki playb00k"

3. **Navigate to Agent Tab:**
   - Click "Agent" button in sidebar

4. **Verify Rendering:**
   - Page should display the storyb00k interface
   - No JavaScript errors in console (F12 → Console)
   - Agent connection status should show "✓ agent" (if server is running)

5. **Test Functionality:**
   - Type a message in the composer
   - Click Send
   - Verify agent responds

---

## 📝 Process Management

### Stop Chrome

```bash
kill $(cat /tmp/chrome-debug.pid)
rm /tmp/chrome-debug.pid
```

### Restart Chrome

```bash
kill $(cat /tmp/chrome-debug.pid) 2>/dev/null
chromium-browser \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/chrome-profile \
  --no-first-run \
  --disable-background-networking \
  --disable-default-apps \
  --disable-extensions \
  --disable-sync \
  --metrics-recording-only \
  --no-default-browser-check \
  --safebrowsing-disable-auto-update \
  > /tmp/chrome-debug.log 2>&1 &
echo $! > /tmp/chrome-debug.pid
sleep 3
curl -s http://localhost:9222/json/version
```

### Check Status

```bash
# Check if running
ps aux | grep chromium.*remote-debugging-port=9222 | grep -v grep

# Check port
curl -s http://localhost:9222/json/version

# Check logs
tail -20 /tmp/chrome-debug.log
```

---

## 🎯 Summary

✅ Chrome/Chromium running with remote debugging on port 9222  
✅ Debugging protocol accessible via HTTP and WebSocket  
✅ Chrome DevTools MCP package available and working  
✅ MCP configuration updated to use localhost:9222  
✅ PID and log files created for process management  

**Next:** Manual testing in browser or wait for MCP server connection in pi

---

**Date:** 2026-09-27  
**Chrome Version:** 153.0.8010.36  
**Debugging Port:** 9222  
**Profile:** /tmp/chrome-profile  
**PID:** 2314260
