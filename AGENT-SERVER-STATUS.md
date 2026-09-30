# Agent Server Status and Fix - 2026-09-27

## Current Status

### ✅ Working Services
- **kr0ki server**: Running on port 8787 (PID 1122575)
- **kroki-compat backend**: Running on port 8010
- **LLM service (b00t-heretic)**: Running on port 8002
  - Model: Qwen3.8-27B-TurboFCFusion
  - Endpoint: `http://127.0.0.1:8002/v1`
  - Status: ✅ Healthy and responding

### ❌ Not Running
- **storyb00k agent server**: Should be on port 8789
  - This is the AG-UI protocol server that orchestrates LLM calls and tool dispatch
  - Located at: `containers/kr0ki-storyb00k-agent/server.py`
  - **This is why the Agent tab shows "Failed to fetch"**

## Architecture Clarification

The StoryB00k (Agent) tab does NOT connect directly to the LLM. It uses a three-tier architecture:

```
Browser (Playbook UI)
    ↓ AG-UI protocol
storyb00k agent server (port 8789)
    ↓ OpenAI-compatible API
LLM service (port 8002)
    ↓ tool calls
kr0ki server (port 8787)
```

The agent server:
1. Receives chat messages from the UI via AG-UI protocol
2. Calls the LLM (port 8002) for completions
3. Dispatches tool calls to kr0ki's MCP tools (port 8787)
4. Returns results to the UI

## Port 8789 - Not a Hallucination

Port 8789 is **correctly defined** in the code:
- `containers/kr0ki-storyb00k-agent/server.py` line: `ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("KR0KI_STORYB00K_PORT", "8789"))), Handler)`
- `deploy/kr0ki-local.pod.yaml`: `containerPort: 8789, hostPort: 8789`
- `playbook/src/components/StoryB00k.vue`: defaults to port 8789

The agent server is part of the full pod deployment but needs to be started separately for local development.

## How to Start the Agent Server

### Option 1: Use the start script (recommended)

```bash
cd /home/brianh/promptexecution/kr0ki
./start-agent.sh
```

This will:
- Create a Python virtual environment if needed
- Set the correct environment variables
- Start the agent server on port 8789
- Connect to the LLM on port 8002

### Option 2: Manual start

```bash
cd /home/brianh/promptexecution/kr0ki/containers/kr0ki-storyb00k-agent

# Create venv if needed
python3 -m venv .venv
.venv/bin/pip install -q -r requirements.txt

# Set environment variables
export KR0KI_URL="http://127.0.0.1:8787"
export KR0KI_STORYB00K_PORT="8789"
export OPENAI_API_URL="http://127.0.0.1:8002/v1"
export OPENAI_API_KEY="dummy-key"  # llama.cpp doesn't require a real key
export ALLOWED_ORIGINS="http://127.0.0.1:8787,http://localhost:8787"

# Start the server
.venv/bin/python3 server.py
```

### Option 3: Run in pod (production)

If you're using the full pod deployment:

```bash
# Check if the pod is running
podman pod ps | grep kr0ki-local

# If not running, deploy it
just deploy-local
```

## Testing the Agent Server

Once started, test the agent server:

```bash
# Test health endpoint
curl http://127.0.0.1:8789/health

# Expected response:
# {"status":"ok","version":"0.1.0","llm_configured":true}
```

Then in the browser:
1. Navigate to http://127.0.0.1:8787/playbook/
2. Click the "Agent" tab
3. You should see a green "✓ agent" badge in the header
4. Try sending a message like "summarize the physical architecture"

## LLM Configuration

The LLM service on port 8002 is working correctly:

```bash
# Test LLM health
curl http://127.0.0.1:8002/health
# Response: {"status":"ok"}

# List available models
curl http://127.0.0.1:8002/v1/models
# Response: Shows Qwen3.8-27B model
```

The model name is very long:
```
/models/snapshots/0b4bc07a8c549be631ed8f5fd78b68fb0e80eced/Qwen3.8-27B-TurboFCFusion-735-882-Here-Uncen-NEO-CODER-MAX-MTP-IQ4_NL.gguf
```

The agent server will automatically discover and use this model via the `/models` endpoint.

## Troubleshooting

### "Failed to fetch" in Agent tab
- **Cause**: Agent server not running on port 8789
- **Fix**: Start the agent server using `./start-agent.sh`

### Agent server starts but LLM calls fail
- **Check**: `OPENAI_API_URL` is set to `http://127.0.0.1:8002/v1`
- **Check**: LLM service is running: `podman ps | grep b00t-heretic`
- **Check**: LLM is accessible: `curl http://127.0.0.1:8002/v1/models`

### CORS errors in browser console
- **Check**: `ALLOWED_ORIGINS` includes the playbook URL
- **Default**: `http://127.0.0.1:8787,http://localhost:8787`
- **For LAN access**: Add your hostname/IP: `http://hostname:8787`

### Agent server crashes on startup
- **Check**: Python dependencies are installed: `.venv/bin/pip install -r requirements.txt`
- **Check**: Port 8789 is not already in use: `ss -tlnp | grep 8789`
- **Check**: kr0ki server is running on port 8787

## Summary

- ✅ Port 8002 (LLM) is working
- ❌ Port 8789 (agent server) is not running
- ✅ Port 8787 (kr0ki server) is working
- ✅ Playbook UI is accessible on port 8787

**To fix the Agent tab**: Start the agent server with `./start-agent.sh`

The architecture is correct - the agent server is the missing piece that connects the UI to the LLM and orchestrates tool calls.
