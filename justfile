# kr0ki — thin command surface (b00t convention: recipes stay thin, logic lives in code).

# Machine-local settings (LLM endpoint, public URLs) come from the gitignored `.env`
# (copy `.env.example`); nothing machine-specific is hard-coded in this file.
set dotenv-load := true

default:
    @just --list

# Build everything (Rust workspace + vendored @assistant-ui/vue).
build:
    cargo build --workspace
    just build-assistant-ui-vue

# Build the vendored @assistant-ui/vue package from the assistant-ui submodule.
build-assistant-ui-vue:
    cd vendor/assistant-ui && pnpm install --frozen-lockfile && pnpm --filter @assistant-ui/vue build

# Full test suite (unit + in-process HTTP). Live render tests stay ignored.
test:
    cargo test --workspace
    cd containers/kr0ki-mcp && python3 test_bridge.py && python3 test_http_worker.py
    cd containers/kr0ki-storyb00k-agent && python3 -m venv .venv && .venv/bin/pip install -q -r requirements.txt && .venv/bin/python3 -m unittest discover -p 'test_*.py'
    just build-assistant-ui-vue
    pnpm --dir playbook install --frozen-lockfile
    pnpm --dir playbook test

# Live render test against the private Kroki-compatible backend (needs it running).
test-live backend="http://127.0.0.1:8010":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_render -- --ignored --nocapture

# Live PNG render test against the private Kroki-compatible backend.
test-live-png backend="http://127.0.0.1:8010":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_png -- --ignored --nocapture

# Exercise every test-backed playb00k example through the deployed HTTP service.
test-playbook kr0ki_url="http://127.0.0.1:8787":
    KR0KI_PLAYBOOK_URL={{kr0ki_url}} cargo test -p kr0ki-server --test playbook_live -- --ignored --nocapture

# SysML-v2-Release conformance harness (phase 1): fetch the pinned corpus, then
# gate kr0ki's SysML-v2 handling via the `sysml-v2-parser` crate. See docs/CONFORMANCE.md.
conformance:
    bash scripts/fetch-sysml-v2-release.sh
    cargo test -p kr0ki-core --test conformance -- --ignored --nocapture

# Lint + format gate (matches CI).
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

# Render the b00t stack orchestration template diagram to SVG.
# Uses the local kr0ki-server by default; its backend is private kroki-compat.
render-template kroki="http://127.0.0.1:8787":
    ./scripts/render-template.sh {{kroki}} b00t-stack-orchestration.svg

# Run the server against the private SECURE-mode Kroki-compatible backend.
# Set KR0KI_AUTH_TOKEN to enable bearer-token auth (FR7 minimal).
# Default bind is 0.0.0.0:8787 so the docs endpoint is reachable from the network.
run bind="0.0.0.0:8787" backend="http://127.0.0.1:8010":
    KR0KI_BIND={{bind}} KR0KI_BACKEND_URL={{backend}} ./target/debug/kr0ki

# Print a LAN-reachable service URL. The host is explicit because automatic
# interface selection is ambiguous on multihomed hosts.
lan-url host port="8787":
    printf 'http://%s:%s\n' "{{host}}" "{{port}}"

# Open docs in the default browser. `docs_url` may be a LAN URL from `just lan-url`.
docs-open docs_url="http://127.0.0.1:8787/docs":
    xdg-open {{docs_url}} || open {{docs_url}} || echo "open {{docs_url}}"

# Generate the static GitHub Pages-compatible mdb00k/playb00k bundle.
static-docs output="site":
    cargo run -p kr0ki-core --bin mdb00k -- {{output}}

# Live end-to-end playb00k proof against the LAN-reachable k0s service.
playbook-e2e kr0ki_url="http://127.0.0.1:8787":
    bash scripts/playbook-e2e.sh {{kr0ki_url}}

# Fast local dev loop (no k0s): our own pinned kroki-compat image via plain podman,
# kr0ki-server via cargo run against it. For iteration only — version/config can
# drift from the real k0s deployment, so always re-verify with `just pod-up` +
# `just playbook-e2e`/`just test-playbook` before calling format or fixture work done.
dev_kroki_port := "8010"

# Build (if needed) and (re)start the local kroki-compat container in the background.
# --memory/--cpus are required: b00t's OCI limits hook rejects any `podman run`
# without an explicit resource budget (matches the pod manifest's own 2Gi/1 CPU).
dev-kroki-up:
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-kroki-compat:dev -f containers/kroki-compat/Containerfile .
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-storyb00k-agent:dev -f containers/kr0ki-storyb00k-agent/Containerfile .
    podman rm -f kr0ki-dev-kroki >/dev/null 2>&1 || true
    podman run -d --name kr0ki-dev-kroki --memory=2g --memory-swap=2g --cpus=1 -p {{dev_kroki_port}}:8000 -e KROKI_SAFE_MODE=secure localhost/kr0ki-kroki-compat:dev
    @for i in $(seq 1 30); do curl -fsS http://127.0.0.1:{{dev_kroki_port}}/health >/dev/null 2>&1 && exit 0; sleep 1; done; echo "kroki-compat did not become ready" >&2; exit 1

dev-kroki-down:
    podman rm -f kr0ki-dev-kroki >/dev/null 2>&1 || true

# Run kr0ki-server locally against our own kroki-compat image (started if not
# already running) — no k0s, no image import, no pod recreate. Ctrl-C stops the
# server; kroki-compat keeps running for the next `just dev` (stop it with
# `just dev-kroki-down`). Defaults to a fresh `playbook/dist`; pass `npm run build`
# output elsewhere if needed.
dev bind="0.0.0.0:8787" playbook_dir="playbook/dist":
    curl -fsS http://127.0.0.1:{{dev_kroki_port}}/health >/dev/null 2>&1 || just dev-kroki-up
    KR0KI_BIND={{bind}} KR0KI_BACKEND_URL=http://127.0.0.1:{{dev_kroki_port}} KR0KI_PLAYBOOK_DIR={{playbook_dir}} ./target/debug/kr0ki

# Discover which of a Kroki backend's registered converters are companion-free
# and not yet in DiagramFormat::ALL (kr0ki#18). Defaults to our own local
# kroki-compat image (started if not already running) — no k0s round-trip
# needed for discovery. Probes with the vendored kroki.io example catalogue
# (kr0ki#19), never a guessed source; hits the backend directly, never through
# kr0ki-server's own cache.
probe-formats backend="":
    #!/usr/bin/env bash
    set -euo pipefail
    backend="{{backend}}"
    if [ -z "$backend" ]; then
      curl -fsS http://127.0.0.1:{{dev_kroki_port}}/health >/dev/null 2>&1 || just dev-kroki-up
      backend="http://127.0.0.1:{{dev_kroki_port}}"
    fi
    cargo run -p kr0ki-core --bin probe_formats -- "$backend"

# Container-only local lifecycle. Podman builds OCI images; the local k0s cluster
# imports and runs them, and kubectl is the sole workload lifecycle interface.
pod-build:
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-server:dev -f containers/kr0ki-server/Containerfile .
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-mcp:dev -f containers/kr0ki-mcp/Containerfile .
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-kroki-compat:dev -f containers/kroki-compat/Containerfile .
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-storyb00k-agent:dev -f containers/kr0ki-storyb00k-agent/Containerfile .

k0s-load image:
    podman save {{image}} | sudo k0s ctr images import -

# Load deployment-only settings (LLM credentials, LAN origins, optional SysML
# endpoint) from the local, gitignored .env into the local k0s Secret. Values
# are passed directly to kubectl and are never printed by this recipe.
pod-env:
    test -f .env || { echo "missing .env; copy .env.example and fill in local settings" >&2; exit 1; }
    kubectl --context Default -n kr0ki create secret generic kr0ki-local-env --from-env-file=.env --dry-run=client -o yaml | kubectl --context Default -n kr0ki apply -f -

pod-up: pod-build
    just k0s-load localhost/kr0ki-server:dev
    just k0s-load localhost/kr0ki-mcp:dev
    just k0s-load localhost/kr0ki-kroki-compat:dev
    just k0s-load localhost/kr0ki-storyb00k-agent:dev
    kubectl --context Default apply -f deploy/namespace.yaml
    just pod-env
    # This is a standalone Pod, not a Deployment: apply alone preserves old
    # containers when the tag is unchanged. Recreate after import for hot reload.
    kubectl --context Default -n kr0ki delete pod --ignore-not-found kr0ki-local
    kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml

pod-down:
    kubectl --context Default delete --ignore-not-found -f deploy/kr0ki-local.pod.yaml

# ============================================================================
# Local server lifecycle (start/stop with validation)
# ============================================================================

# Check if kr0ki server is currently running on the specified port.
# Returns 0 if running, 1 if not. Used by other recipes for conditional logic.
check-server-running port="8787":
    #!/usr/bin/env bash
    set -euo pipefail
    if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✓ kr0ki server is running on port {{port}}"
        exit 0
    else
        echo "✗ kr0ki server is not running on port {{port}}"
        exit 1
    fi

# Check if the Kroki backend is available.
# Returns 0 if available, 1 if not.
check-backend-available port="8010":
    #!/usr/bin/env bash
    set -euo pipefail
    if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✓ Kroki backend is available on port {{port}}"
        exit 0
    else
        echo "✗ Kroki backend is not available on port {{port}}"
        exit 1
    fi

# Wait for the kr0ki server to become ready (with timeout).
# Polls the health endpoint until it responds or timeout is reached.
wait-for-server port="8787" timeout="30":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Waiting for kr0ki server to become ready (timeout: {{timeout}}s)..."
    for i in $(seq 1 {{timeout}}); do
        if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
            echo "✓ kr0ki server is ready after ${i}s"
            exit 0
        fi
        sleep 1
    done
    echo "✗ kr0ki server did not become ready within {{timeout}}s" >&2
    exit 1

# Validate that the server is working correctly.
# Checks health endpoint and contract headers (issue #57).
validate-server port="8787":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Validating kr0ki server..."
    
    # Check health endpoint
    health=$(curl -fsS http://127.0.0.1:{{port}}/health)
    if echo "$health" | grep -q '"status"'; then
        echo "✓ Health endpoint responding"
    else
        echo "✗ Health endpoint not responding correctly" >&2
        exit 1
    fi
    
    # Check contract headers (issue #57)
    headers=$(curl -fsSI http://127.0.0.1:{{port}}/health)
    if echo "$headers" | grep -qi "x-kr0ki-contract"; then
        echo "✓ Contract header present"
    else
        echo "✗ Contract header missing" >&2
        exit 1
    fi
    
    if echo "$headers" | grep -qi "x-kr0ki-request-id"; then
        echo "✓ Request ID header present"
    else
        echo "✗ Request ID header missing" >&2
        exit 1
    fi
    
    echo "✓ All validations passed"

# Start the kr0ki server in the background with full validation.
# Checks prerequisites, starts the server, waits for readiness, and validates.
# Usage: just start [port] [backend_port]
start port="8787" backend_port="8010":
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Check if already running
    if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✗ kr0ki server is already running on port {{port}}"
        echo "  Use 'just stop {{port}}' to stop it first"
        exit 1
    fi
    
    # Check backend availability
    if ! curl -fsS http://127.0.0.1:{{backend_port}}/health >/dev/null 2>&1; then
        echo "⚠ Kroki backend not available on port {{backend_port}}"
        echo "  Starting backend container..."
        just dev-kroki-up
    else
        echo "✓ Kroki backend available on port {{backend_port}}"
    fi
    
    # Create PID file directory
    mkdir -p .kr0ki-run
    
    # Start server in background
    echo "Starting kr0ki server on port {{port}}..."
    KR0KI_BIND=0.0.0.0:{{port}} \
    KR0KI_BACKEND_URL=http://127.0.0.1:{{backend_port}} \
    ./target/debug/kr0ki > .kr0ki-run/server.log 2>&1 &
    SERVER_PID=$!
    echo $SERVER_PID > .kr0ki-run/server.pid
    echo "✓ Server started with PID $SERVER_PID"
    
    # Wait for server to be ready
    just wait-for-server {{port}} 30
    
    # Validate server
    just validate-server {{port}}
    
    echo ""
    echo "✓ kr0ki server is running and validated"
    echo "  Docs: http://127.0.0.1:{{port}}/docs"
    echo "  Health: http://127.0.0.1:{{port}}/health"
    echo "  Logs: .kr0ki-run/server.log"
    echo "  PID: $SERVER_PID"

# Stop the kr0ki server gracefully.
# Checks if running, stops the process, and verifies it stopped.
# Usage: just stop [port]
stop port="8787":
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Check if running
    if ! curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✗ kr0ki server is not running on port {{port}}"
        # Clean up stale PID file if it exists
        rm -f .kr0ki-run/server.pid
        exit 1
    fi
    
    echo "Stopping kr0ki server..."
    
    # Try to stop using PID file first
    if [ -f .kr0ki-run/server.pid ]; then
        PID=$(cat .kr0ki-run/server.pid)
        if kill -0 $PID 2>/dev/null; then
            echo "  Sending SIGTERM to PID $PID..."
            kill $PID
            
            # Wait for graceful shutdown
            for i in $(seq 1 10); do
                if ! kill -0 $PID 2>/dev/null; then
                    echo "✓ Server stopped gracefully"
                    rm -f .kr0ki-run/server.pid
                    exit 0
                fi
                sleep 1
            done
            
            # Force kill if still running
            echo "  Server did not stop gracefully, sending SIGKILL..."
            kill -9 $PID 2>/dev/null || true
            sleep 1
        fi
        rm -f .kr0ki-run/server.pid
    else
        # Fallback: find process by port (less reliable)
        echo "  No PID file found, attempting to find process by port..."
        PIDS=$(lsof -ti :{{port}} 2>/dev/null || true)
        if [ -n "$PIDS" ]; then
            for PID in $PIDS; do
                echo "  Killing PID $PID..."
                kill $PID 2>/dev/null || true
            done
            sleep 2
        fi
    fi
    
    # Verify it stopped
    if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✗ Failed to stop kr0ki server" >&2
        exit 1
    else
        echo "✓ kr0ki server stopped successfully"
    fi

# Show kr0ki server status (running/stopped, PID, logs location).
status port="8787":
    #!/usr/bin/env bash
    set -euo pipefail
    
    if curl -fsS http://127.0.0.1:{{port}}/health >/dev/null 2>&1; then
        echo "✓ kr0ki server is running on port {{port}}"
        if [ -f .kr0ki-run/server.pid ]; then
            PID=$(cat .kr0ki-run/server.pid)
            if kill -0 $PID 2>/dev/null; then
                echo "  PID: $PID"
            else
                echo "  PID file exists but process is not running (stale PID file)"
            fi
        fi
        echo "  Logs: .kr0ki-run/server.log"
        echo "  Docs: http://127.0.0.1:{{port}}/docs"
    else
        echo "✗ kr0ki server is not running on port {{port}}"
    fi

# Agent server lifecycle
start-agent port="8789":
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .kr0ki-run
    if [ -f .kr0ki-run/agent.pid ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "✗ Agent server is already running on port {{port}}"
        echo "  Use 'just stop-agent {{port}}' to stop it first"
        exit 1
    fi
    if lsof -ti:{{port}} >/dev/null 2>&1; then
        echo "✗ Port {{port}} is already in use"
        echo "  Use 'lsof -ti:{{port}} | xargs kill -9' to clear it"
        exit 1
    fi
    # Fail on the console, not silently inside the backgrounded process below.
    : "${OPENAI_API_URL:?set OPENAI_API_URL in .env (see .env.example)}"
    echo "Starting agent server on port {{port}}..."
    KR0KI_ROOT="$(pwd)"
    AGENT_DIR="$KR0KI_ROOT/containers/kr0ki-storyb00k-agent"
    LOG_FILE="$KR0KI_ROOT/.kr0ki-run/agent.log"
    PID_FILE="$KR0KI_ROOT/.kr0ki-run/agent.pid"
    cd "$AGENT_DIR"
    if [ ! -d .venv ]; then
        echo "Creating virtual environment..."
        python3 -m venv .venv
        .venv/bin/pip install -q -r requirements.txt
    fi
    nohup env \
        KR0KI_STORYB00K_PORT="{{port}}" \
        OPENAI_API_URL="${OPENAI_API_URL:?set OPENAI_API_URL in .env (see .env.example)}" \
        OPENAI_API_KEY="${OPENAI_API_KEY:-not-needed}" \
        KR0KI_STORYB00K_ALLOWED_ORIGINS="${KR0KI_STORYB00K_ALLOWED_ORIGINS:-http://localhost:8787,http://127.0.0.1:8787,http://localhost:5173,http://127.0.0.1:5173${KR0KI_PUBLIC_URL:+,$KR0KI_PUBLIC_URL}}" \
        .venv/bin/python server.py > "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"
    sleep 2
    if curl -s http://127.0.0.1:{{port}}/health | grep -Eq '"status": ?"ok"'; then
        echo "✓ Agent server started on port {{port}}"
        echo "  PID: $(cat "$PID_FILE")"
        echo "  Logs: .kr0ki-run/agent.log"
    else
        echo "✗ Agent server failed to start"
        echo "  Check logs: .kr0ki-run/agent.log"
        exit 1
    fi

stop-agent port="8789":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f .kr0ki-run/agent.pid ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 $PID 2>/dev/null; then
            echo "Stopping agent server (PID: $PID)..."
            kill $PID
            sleep 2
            if kill -0 $PID 2>/dev/null; then
                echo "  Force killing..."
                kill -9 $PID
            fi
            rm -f .kr0ki-run/agent.pid
            echo "✓ Agent server stopped"
        else
            echo "✗ Agent server is not running (stale PID file)"
            rm -f .kr0ki-run/agent.pid
        fi
    else
        if lsof -ti:{{port}} >/dev/null 2>&1; then
            echo "Stopping agent server on port {{port}}..."
            lsof -ti:{{port}} | xargs kill -9
            echo "✓ Agent server stopped"
        else
            echo "✗ Agent server is not running on port {{port}}"
        fi
    fi

status-agent port="8789":
    #!/usr/bin/env bash
    set -euo pipefail
    if curl -s http://127.0.0.1:{{port}}/health | grep -Eq '"status": ?"ok"'; then
        echo "✓ Agent server is running on port {{port}}"
        if [ -f .kr0ki-run/agent.pid ]; then
            PID=$(cat "$PID_FILE")
            if kill -0 $PID 2>/dev/null; then
                echo "  PID: $PID"
            else
                echo "  PID file exists but process is not running (stale PID file)"
            fi
        fi
        echo "  Logs: .kr0ki-run/agent.log"
    else
        echo "✗ Agent server is not running on port {{port}}"
    fi
