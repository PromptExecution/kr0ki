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

# Run the P0 server. Point KR0KI_BACKEND_URL at a SECURE-mode Kroki.
run bind="127.0.0.1:8787" backend="https://kroki.io":
    KR0KI_BIND={{bind}} KR0KI_BACKEND_URL={{backend}} cargo run -p kr0ki-server

# Bring up a local SECURE-mode Kroki to render against.
kroki-up:
    podman run -d --name kr0ki-kroki -p 8000:8000 -e KROKI_SAFE_MODE=secure docker.io/yuzutech/kroki

kroki-down:
    podman rm -f kr0ki-kroki
