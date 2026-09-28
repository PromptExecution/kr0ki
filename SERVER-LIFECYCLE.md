# kr0ki Server Lifecycle Management

This document describes the `just start`, `just stop`, and related recipes for managing the kr0ki server lifecycle.

## Quick Start

```bash
# Start the server (with full validation)
just start

# Check status
just status

# Stop the server
just stop
```

## Recipes

### Primary Commands

#### `just start [port] [backend_port]`
Start the kr0ki server in the background with full validation.

**What it does:**
1. Checks if server is already running (fails if yes)
2. Checks if Kroki backend is available (starts it if not)
3. Starts kr0ki-server in background
4. Waits for server to become ready (30s timeout)
5. Validates health endpoint and contract headers
6. Reports success with URLs and PID

**Parameters:**
- `port` (default: 8787) - Port for kr0ki server
- `backend_port` (default: 8010) - Port for Kroki backend

**Example:**
```bash
just start                          # Use defaults
just start 9000                     # Custom server port
just start 9000 8020                # Custom server and backend ports
```

**Output:**
```
✓ Kroki backend available on port 8010
Starting kr0ki server on port 8787...
✓ Server started with PID 12345
Waiting for kr0ki server to become ready (timeout: 30s)...
✓ kr0ki server is ready after 2s
Validating kr0ki server...
✓ Health endpoint responding
✓ Contract header present
✓ Request ID header present
✓ All validations passed

✓ kr0ki server is running and validated
  Docs: http://127.0.0.1:8787/docs
  Health: http://127.0.0.1:8787/health
  Logs: .kr0ki-run/server.log
  PID: 12345
```

#### `just stop [port]`
Stop the kr0ki server gracefully.

**What it does:**
1. Checks if server is running (fails if not)
2. Reads PID from `.kr0ki-run/server.pid`
3. Sends SIGTERM for graceful shutdown
4. Waits up to 10s for process to exit
5. Falls back to SIGKILL if needed
6. Verifies server stopped
7. Cleans up PID file

**Parameters:**
- `port` (default: 8787) - Port where server is running

**Example:**
```bash
just stop                           # Stop on default port
just stop 9000                      # Stop on custom port
```

**Output:**
```
Stopping kr0ki server...
  Sending SIGTERM to PID 12345...
✓ Server stopped gracefully
✓ kr0ki server stopped successfully
```

#### `just status [port]`
Show current server status.

**What it does:**
- Checks if server is running
- Shows PID if available
- Displays log location and URLs

**Example:**
```bash
just status
```

**Output (running):**
```
✓ kr0ki server is running on port 8787
  PID: 12345
  Logs: .kr0ki-run/server.log
  Docs: http://127.0.0.1:8787/docs
```

**Output (stopped):**
```
✗ kr0ki server is not running on port 8787
```

### Helper Recipes (Composable Building Blocks)

These recipes are used by `start` and `stop` but can also be used independently.

#### `just check-server-running [port]`
Check if server is currently running.

**Returns:** 0 if running, 1 if not

**Example:**
```bash
just check-server-running && echo "Server is up"
```

#### `just check-backend-available [port]`
Check if Kroki backend is available.

**Returns:** 0 if available, 1 if not

**Example:**
```bash
just check-backend-available || just dev-kroki-up
```

#### `just wait-for-server [port] [timeout]`
Wait for server to become ready.

**Parameters:**
- `port` (default: 8787)
- `timeout` (default: 30) - Seconds to wait

**Example:**
```bash
just wait-for-server 8787 60        # Wait up to 60 seconds
```

#### `just validate-server [port]`
Validate server is working correctly.

**What it checks:**
- Health endpoint responds with valid JSON
- `X-Kr0ki-Contract` header is present (issue #57)
- `X-Kr0ki-Request-Id` header is present (issue #57)

**Example:**
```bash
just validate-server
```

## Design Principles

### Composition
Each recipe does one thing well. Complex workflows are built by composing simple recipes:
- `start` = check + start + wait + validate
- `stop` = check + kill + verify

### Validation
Every state-changing operation validates the result:
- `start` validates the server is actually working (not just listening)
- `stop` validates the server actually stopped
- Contract headers are checked (issue #57 implementation)

### Graceful Degradation
- Tries graceful shutdown (SIGTERM) before force kill (SIGKILL)
- Falls back to port-based process lookup if PID file is missing
- Cleans up stale PID files automatically

### Transparency
- Logs go to `.kr0ki-run/server.log` (inspectable)
- PID stored in `.kr0ki-run/server.pid` (machine-readable)
- Clear success/failure messages with context

## File Layout

```
.kr0ki-run/
├── server.pid    # PID of running server (created by `just start`)
└── server.log    # Server stdout/stderr (created by `just start`)
```

## Troubleshooting

### Server won't start
```bash
# Check if port is in use
just check-server-running

# Check backend
just check-backend-available

# View logs
tail -f .kr0ki-run/server.log
```

### Server won't stop
```bash
# Manual stop
kill $(cat .kr0ki-run/server.pid)

# Force kill
kill -9 $(cat .kr0ki-run/server.pid)

# Find process by port
lsof -i :8787
```

### Stale PID file
If the server crashed but PID file remains:
```bash
rm .kr0ki-run/server.pid
just start
```

## Integration with Existing Recipes

These new recipes complement (don't replace) existing recipes:

- `just dev` - Interactive foreground mode (Ctrl-C to stop)
- `just run` - Simple foreground run with custom params
- `just start` - Background mode with validation (NEW)
- `just stop` - Graceful shutdown (NEW)
- `just dev-kroki-up/down` - Backend container lifecycle

Use `just start/stop` when you want:
- Background operation
- Automatic validation
- PID tracking
- Contract header verification

Use `just dev/run` when you want:
- Foreground operation
- Interactive debugging
- Direct log output to terminal
