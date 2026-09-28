# Agent Server - Quick Start Guide

## ✅ Agent Server is Now Running

The storyb00k agent server is now running on port 8789 and connected to the LLM on port 8002.

### Service Status

| Service | Port | Status |
|---------|------|--------|
| kr0ki server | 8787 | ✅ Running |
| kroki-compat backend | 8010 | ✅ Running |
| LLM service (b00t-heretic) | 8002 | ✅ Running |
| storyb00k agent | 8789 | ✅ Running |

### Test the Agent Server

```bash
curl http://127.0.0.1:8789/health
```

Expected response:
```json
{
    "status": "ok",
    "service": "kr0ki-storyb00k-agent",
    "llm_configured": true,
    "active_threads": 0,
    "max_model_tool_rounds": 64,
    "max_clarifying_questions": 6
}
```

## Using the Agent Tab

1. Open your browser to: http://127.0.0.1:8787/playbook/
2. Click the **Agent** tab
3. You should see a green "✓ agent" badge in the header
4. Try sending a message like:
   - "summarize the physical architecture"
   - "render the deployment as a diagram"
   - "what components are in this system?"

## Architecture

```
Browser (Playbook UI on port 8787)
    ↓ AG-UI protocol
storyb00k agent (port 8789) ← YOU ARE HERE
    ↓ OpenAI-compatible API
LLM service (port 8002) - Qwen3.8-27B
    ↓ tool calls
kr0ki server (port 8787) - diagram rendering
```

## Managing the Agent Server

### Check if running
```bash
ss -tlnp | grep 8789
```

### View logs
```bash
tail -f /tmp/agent-server.log
```

### Stop the server
```bash
pkill -f "python3 server.py"
```

### Start the server
```bash
cd /home/brianh/promptexecution/kr0ki
./start-agent.sh
```

Or manually:
```bash
cd /home/brianh/promptexecution/kr0ki/containers/kr0ki-storyb00k-agent
export KR0KI_URL="http://127.0.0.1:8787"
export KR0KI_STORYB00K_PORT="8789"
export OPENAI_API_URL="http://127.0.0.1:8002/v1"
export OPENAI_API_KEY="dummy-key"
export ALLOWED_ORIGINS="http://127.0.0.1:8787"
.venv/bin/python3 server.py
```

## LLM Configuration

The agent is configured to use:
- **URL**: `http://127.0.0.1:8002/v1`
- **Model**: Qwen3.8-27B-TurboFCFusion (auto-discovered)
- **API Key**: `dummy-key` (llama.cpp doesn't require a real key)

### Test LLM Connection
```bash
curl http://127.0.0.1:8002/v1/models
```

## Troubleshooting

### Agent tab shows "Failed to fetch"
- Check if agent server is running: `ss -tlnp | grep 8789`
- Check logs: `tail /tmp/agent-server.log`
- Restart: `./start-agent.sh`

### Agent connects but no responses
- Check LLM is running: `curl http://127.0.0.1:8002/health`
- Check agent logs: `tail -f /tmp/agent-server.log`
- Check kr0ki server: `curl http://127.0.0.1:8787/health`

### CORS errors in browser
- Ensure `ALLOWED_ORIGINS` includes your playbook URL
- Default: `http://127.0.0.1:8787,http://localhost:8787`
- For LAN access: Add `http://your-hostname:8787`

## What Changed

### Before
- Agent server was not running
- StoryB00k component tried to connect to port 8789 but got "Failed to fetch"
- No clear error messages about what was wrong

### After
- Agent server running on port 8789
- Connected to LLM on port 8002
- Enhanced error handling shows connection status
- Setup panel allows configuring Agent URL
- Clear diagnostics when connection fails

## Files Created/Modified

### Created
- `/home/brianh/promptexecution/kr0ki/start-agent.sh` - Script to start the agent server
- `/home/brianh/promptexecution/kr0ki/AGENT-SERVER-STATUS.md` - Detailed status report
- `/home/brianh/promptexecution/kr0ki/AGENT-QUICKSTART.md` - This file

### Modified (from previous session)
- `playbook/src/components/StoryB00k.vue` - Added connection status tracking and error handling
- `playbook/src/components/Setup.vue` - Added Agent URL configuration
- `playbook/src/App.vue` - Made agentUrl reactive and configurable

## Next Steps

1. ✅ Agent server is running
2. ✅ LLM is accessible on port 8002
3. ✅ Error handling is improved
4. Test the Agent tab in the browser
5. Try sending messages and generating diagrams

## Notes

- Port 8789 is **correct** - it's defined in the code and deployment configs
- The agent server acts as an orchestrator between the UI, LLM, and kr0ki
- The LLM on port 8002 is working correctly with Qwen3.8-27B model
- All services are now running and connected
