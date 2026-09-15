# kr0ki — thin command surface (b00t convention: recipes stay thin, logic lives in code).

default:
    @just --list

# Build everything.
build:
    cargo build --workspace

# Full test suite (unit + in-process HTTP). Live render tests stay ignored.
test:
    cargo test --workspace

# Live render test against a real Kroki (needs a backend URL).
test-live backend="https://kroki.io":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_render -- --ignored --nocapture

# Live PNG render test against a real Kroki.
test-live-png backend="https://kroki.io":
    KR0KI_TEST_BACKEND={{backend}} cargo test -p kr0ki-core --test live_png -- --ignored --nocapture

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

# Open the generated docs in the default browser (server must be running).
docs-open:
    xdg-open http://localhost:8787/docs || open http://localhost:8787/docs || echo "open http://localhost:8787/docs"

# Generate the static GitHub Pages-compatible mdb00k/playb00k bundle.
static-docs output="site":
    cargo run -p kr0ki-core --bin mdb00k -- {{output}}

# Bring up a local SECURE-mode Kroki to render against.
kroki-up:
    podman run -d --name kr0ki-kroki -p 8000:8000 -e KROKI_SAFE_MODE=secure docker.io/yuzutech/kroki

kroki-down:
    podman rm -f kr0ki-kroki

# Container-only local lifecycle. Podman builds OCI images; the local k0s cluster
# imports and runs them, and kubectl is the sole workload lifecycle interface.
pod-build:
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-server:dev -f containers/kr0ki-server/Containerfile .
    podman build --memory=16g --memory-swap=16g -t localhost/kr0ki-mcp:dev -f containers/kr0ki-mcp/Containerfile .

k0s-load image:
    podman save {{image}} | sudo k0s ctr images import -

pod-up: pod-build
    just k0s-load localhost/kr0ki-server:dev
    just k0s-load localhost/kr0ki-mcp:dev
    kubectl --context Default apply -f deploy/namespace.yaml
    kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml

pod-down:
    kubectl --context Default delete --ignore-not-found -f deploy/kr0ki-local.pod.yaml
