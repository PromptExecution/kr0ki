# Agent Connection Fix - 2026-09-26

## Problem

The StoryB00k (Agent) module was failing to connect to the LLM server with a generic "Failed to fetch" error. The root cause was that the agent server (port 8789) was not running, and the error handling was insufficient to diagnose the issue.

## Root Cause Analysis

1. **Missing Agent Server**: The StoryB00k component connects to an agent server on port 8789, not directly to the LLM. This agent server orchestrates the diagram generation process and manages LLM interactions.

2. **Poor Error Handling**: The original error handling only showed "Failed to fetch" without indicating:
   - Which URL was being accessed
   - What the specific error was
   - Whether the agent server or LLM server was the problem

3. **No Connection Status**: There was no visual indicator of the agent connection status in the UI.

## Changes Made

### 1. Enhanced Error Handling in StoryB00k.vue

**Added connection status tracking:**
```javascript
const agentConnectionStatus = ref('unknown') // 'unknown', 'checking', 'connected', 'failed'
const agentConnectionError = ref('')
```

**Added connection test function:**
```javascript
async function testAgentConnection() {
  // Tests agent server health endpoint
  // Provides detailed error messages including:
  // - Timeout errors (5s limit)
  // - Network errors with the specific URL
  // - HTTP status codes
}
```

**Enhanced error display:**
```vue
<p v-if="error" class="storyb00k__error">
  <strong>Error:</strong> {{ error.message }}
  <span v-if="agentConnectionStatus === 'failed'" class="storyb00k__error-detail">
    <br>Agent server: <code>{{ agentUrl }}</code>
    <br v-if="agentConnectionError">{{ agentConnectionError }}
  </span>
  <span v-else class="storyb00k__error-detail">
    <br>Agent: <code>{{ agentUrl }}</code>
    <br>LLM: <code>{{ llmUrl || 'not configured' }}</code>
    <br>Model: {{ llmModel || 'not configured' }}
  </span>
</p>
```

### 2. Visual Connection Status Indicator

**Added status badge in header:**
```vue
<span class="storyb00k__agent-status" :data-status="agentConnectionStatus" 
      :title="agentConnectionError || `Agent: ${agentUrl}`">
  {{ agentConnectionStatus === 'connected' ? '✓ agent' : 
     agentConnectionStatus === 'checking' ? '… agent' : '✗ agent' }}
</span>
```

**Added retry button:**
```vue
<button v-if="agentConnectionStatus === 'failed'" 
        class="storyb00k__retry-connection" 
        @click="testAgentConnection" 
        title="Retry agent connection">
  ↻
</button>
```

**Status colors:**
- Green (connected): `#10b981`
- Yellow (checking): `#fbbf24`
- Red (failed): `#ef4444`

### 3. Agent URL Configuration in Setup Panel

**Added Agent section to Setup.vue:**
```vue
<section class="setup__section">
  <h3>Agent</h3>
  <p class="setup__section-desc">The storyb00k agent server that orchestrates diagram generation.</p>
  
  <label class="setup__field">
    <span class="setup__label">Agent URL</span>
    <input v-model="localAgentUrl" type="url" 
           placeholder="http://127.0.0.1:8789" />
    <span class="setup__hint">The storyb00k agent server. Defaults to the current host on port 8789.</span>
  </label>
  
  <div class="setup__actions">
    <button @click="testAgent" :disabled="agentTestStatus === 'testing'">
      {{ agentTestStatus === 'testing' ? 'Testing...' : 'Test Connection' }}
    </button>
    <span v-if="agentTestMessage" :class="['setup__test-result', agentTestStatus]">
      {{ agentTestMessage }}
    </span>
  </div>
</section>
```

**Added test function:**
```javascript
async function testAgent() {
  // Tests agent server health endpoint
  // Provides detailed error messages
  // Shows success/failure status
}
```

### 4. Reactive Agent URL in App.vue

**Changed from constant to ref:**
```javascript
const agentUrl = ref(
  localStorage.getItem('kr0ki:agentUrl') ||
    (typeof window !== 'undefined' && window.location.hostname
      ? `${window.location.protocol}//${window.location.hostname}:8789`
      : 'http://127.0.0.1:8789')
)

function updateAgentUrl(url) {
  agentUrl.value = url
  localStorage.setItem('kr0ki:agentUrl', url)
}
```

**Updated component props:**
```vue
<StoryB00k :agent-url="agentUrl" ... />
<Setup :agent-url="agentUrl" @update:agent-url="updateAgentUrl" ... />
```

### 5. Agent URL Prop in StoryB00k.vue

**Added prop:**
```javascript
const props = defineProps({
  // ... other props
  agentUrl: { type: String, default: '' },
})
```

**Updated agentUrl initialization:**
```javascript
const agentUrl = props.agentUrl || 
                 import.meta.env.VITE_STORYB00K_AGENT_URL || 
                 `${window.location.protocol}//${window.location.hostname}:8789`
```

## How to Get the Agent Working

### Step 1: Verify Agent Server is Running

The agent server should be running on port 8789. Check if it's running:

```bash
curl http://127.0.0.1:8789/health
```

If you get a response like `{"status":"ok"}`, the agent is running.

### Step 2: Start the Agent Server (if not running)

The agent server is a separate service from kr0ki. You need to start it separately. Check the agent server documentation or deployment scripts.

### Step 3: Configure Agent URL in Setup Panel

1. Navigate to the Setup panel in the playbook UI
2. Under the "Agent" section, verify the Agent URL is correct
3. Click "Test Connection" to verify connectivity
4. If the test fails, check:
   - Is the agent server running?
   - Is the port correct (default: 8789)?
   - Are there any firewall rules blocking access?
   - Is the hostname correct for remote access?

### Step 4: Configure LLM Settings

1. In the Setup panel, under "LLM Configuration":
   - **LLM API URL**: Should point to your LLM server (e.g., `http://sm3lly:8002/v1`)
   - **API Key**: Enter your API key if required
   - **Model**: Select or enter the model name (e.g., `qwen3-coder-27b`)
2. Click "Test Connection" to verify LLM connectivity
3. The available models will be loaded from the LLM server

### Step 5: Test the Agent

1. Navigate to the Agent tab
2. Check the connection status indicator in the header:
   - Green "✓ agent" = connected
   - Yellow "… agent" = checking
   - Red "✗ agent" = failed (hover for details)
3. If failed, click the retry button (↻) to test again
4. Try sending a message to the agent

## Error Messages and Troubleshooting

### "Cannot reach agent at http://hostname:8789 - is it running?"

**Cause**: Agent server is not running or not accessible.

**Solution**:
1. Start the agent server
2. Verify the URL in Setup panel
3. Check firewall rules
4. For remote access, ensure the hostname is correct (not localhost)

### "Connection timeout (5s) - agent server not responding"

**Cause**: Agent server is running but not responding within 5 seconds.

**Solution**:
1. Check agent server logs
2. Verify the agent server is healthy
3. Check network connectivity
4. Increase timeout if server is slow

### "HTTP 503: Service Unavailable"

**Cause**: Agent server is running but unhealthy.

**Solution**:
1. Check agent server logs
2. Verify LLM server is accessible from agent
3. Check agent server dependencies

### "Failed to fetch" (generic)

**Cause**: Network error or CORS issue.

**Solution**:
1. Check browser console for detailed error
2. Verify CORS configuration on agent server
3. Check network connectivity
4. Verify the URL is correct

## Architecture

```
┌─────────────────┐
│   Playbook UI   │
│  (port 8787)    │
└────────┬────────┘
         │
         ├──────────────────────┐
         │                      │
         ▼                      ▼
┌─────────────────┐    ┌─────────────────┐
│  kr0ki Server   │    │  Agent Server   │
│  (port 8787)    │    │  (port 8789)    │
│                 │    │                 │
│ - Render API    │    │ - AG-UI API     │
│ - Health        │    │ - Project Mgmt  │
│ - Cache         │    │ - LLM Orchestr. │
└─────────────────┘    └────────┬────────┘
                                │
                                ▼
                       ┌─────────────────┐
                       │   LLM Server    │
                       │  (port 8002)    │
                       │                 │
                       │ - OpenAI API    │
                       │ - Models        │
                       └─────────────────┘
```

## Next Steps

1. **LLM Profiles**: Implement LLM profile management to support multiple LLM configurations
2. **Agent Server Deployment**: Document agent server deployment and startup procedures
3. **Health Checks**: Add more comprehensive health checks for all services
4. **Connection Pooling**: Implement connection pooling for better performance
5. **Retry Logic**: Add automatic retry with exponential backoff for transient failures

## Files Modified

- `playbook/src/App.vue` - Added agentUrl ref and update function
- `playbook/src/components/StoryB00k.vue` - Enhanced error handling and connection status
- `playbook/src/components/Setup.vue` - Added Agent configuration section
- `playbook/src/style.css` - Added version badge styling (from previous change)

## Testing

1. Start kr0ki server: `just start`
2. Navigate to http://127.0.0.1:8787/playbook/
3. Go to Setup panel
4. Test Agent connection
5. Test LLM connection
6. Go to Agent tab
7. Verify connection status indicator
8. Try sending a message

## Conclusion

The enhanced error handling now provides clear diagnostics when the agent connection fails, making it much easier to identify and resolve connectivity issues. The visual connection status indicator gives immediate feedback on the agent's availability, and the Setup panel allows easy configuration of the agent URL and LLM settings.
