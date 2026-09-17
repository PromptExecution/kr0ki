# kr0ki — thin command surface (b00t convention: recipes stay thin, logic lives in code).

default:
    @just --list

# Build everything.
build:
    cargo build --workspace

# Full test suite (unit + in-process HTTP). Live render tests stay ignored.
test:
    cargo test --workspace
    cd containers/kr0ki-mcp && python3 test_bridge.py && python3 test_http_worker.py

# Live render test against a real Kroki (needs a backend URL).
test-live backend="https://kroki.io":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_render -- --ignored --nocapture

# Live PNG render test against a real Kroki.
test-live-png backend="https://kroki.io":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_png -- --ignored --nocapture

# Exercise every test-backed playb00k example through the deployed HTTP service.
test-playbook kr0ki_url="http://192.168.1.137:8787":
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
# Uses the public Kroki endpoint by default; pass a local kr0ki-server URL if running.
render-template kroki="https://kroki.io":
    ./scripts/render-template.sh {{kroki}} b00t-stack-orchestration.svg

# Run the server. Point KR0KI_BACKEND_URL at a SECURE-mode Kroki.
# Set KR0KI_AUTH_TOKEN to enable bearer-token auth (FR7 minimal).
# Default bind is 0.0.0.0:8787 so the docs endpoint is reachable from the network.
run bind="0.0.0.0:8787" backend="https://kroki.io":
    KR0KI_BIND={{bind}} KR0KI_BACKEND_URL={{backend}} cargo run -p kr0ki-server

# Open LAN-reachable docs in the default browser (server must be running).
docs-open docs_url="http://192.168.1.137:8787/docs":
    xdg-open {{docs_url}} || open {{docs_url}} || echo "open {{docs_url}}"

# Generate the static GitHub Pages-compatible mdb00k/playb00k bundle.
static-docs output="site":
    cargo run -p kr0ki-core --bin mdb00k -- {{output}}

# Live end-to-end playb00k proof against the LAN-reachable k0s service.
playbook-e2e kr0ki_url="http://192.168.1.137:8787":
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
dev bind="127.0.0.1:8788" playbook_dir="playbook/dist":
    curl -fsS http://127.0.0.1:{{dev_kroki_port}}/health >/dev/null 2>&1 || just dev-kroki-up
    KR0KI_BIND={{bind}} KR0KI_BACKEND_URL=http://127.0.0.1:{{dev_kroki_port}} KR0KI_PLAYBOOK_DIR={{playbook_dir}} cargo run -p kr0ki-server

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

k0s-load image:
    podman save {{image}} | sudo k0s ctr images import -

pod-up: pod-build
    just k0s-load localhost/kr0ki-server:dev
    just k0s-load localhost/kr0ki-mcp:dev
    just k0s-load localhost/kr0ki-kroki-compat:dev
    just k0s-load localhost/kr0ki-storyb00k-agent:dev
    kubectl --context Default apply -f deploy/namespace.yaml
    # This is a standalone Pod, not a Deployment: apply alone preserves old
    # containers when the tag is unchanged. Recreate after import for hot reload.
    kubectl --context Default -n kr0ki delete pod --ignore-not-found kr0ki-local
    kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml

pod-down:
    kubectl --context Default delete --ignore-not-found -f deploy/kr0ki-local.pod.yaml
