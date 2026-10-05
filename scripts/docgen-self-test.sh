#!/usr/bin/env bash
# Container checks run serially against the same immutable tracked sources.
set -euo pipefail
work=$(mktemp -d /tmp/docgen-test.XXXXXXXX)
trap 'rm -rf "$work"' EXIT
# Cargo opens cache lock files even in offline mode; give the read-only runner
# its own writable copy, while toolchains remain immutable in RUSTUP_HOME.
cp -a "${CARGO_HOME:-/usr/local/cargo}" "$work/cargo"
export CARGO_HOME="$work/cargo"
python3 /opt/docgen/docgen-bundle-test.py
CARGO_TARGET_DIR="$work/stable-target" CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true \
    cargo +1.98.0 test --offline --locked -p kr0ki-behavior -- --test-threads=1
CARGO_TARGET_DIR="$work/stable-target" CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true \
    cargo +1.98.0 run --quiet --offline --locked \
    --manifest-path /src/crates/kr0ki-behavior/tests/fixtures/state_machine/Cargo.toml > "$work/trace.json"
python3 - "$work/trace.json" /src/crates/kr0ki-behavior/tests/fixtures/ooda-trace.json <<'PY'
import json, pathlib, sys
actual, expected = [json.loads(pathlib.Path(p).read_text()) for p in sys.argv[1:]]
if actual != expected:
    raise SystemExit('shared state machine runtime trace differs from declared transition contract')
PY
/opt/docgen/docgen-build.sh "$work/first"
/opt/docgen/docgen-build.sh "$work/second"
cmp "$work/first/bundle.tar.zst" "$work/second/bundle.tar.zst"
cmp "$work/first/manifest.json" "$work/second/manifest.json"
echo 'Extraction, model validation, and deterministic bundle checks passed.'
echo 'Offline phase complete; the wrapper runs the separate renderer gate when configured.'
cat "$work/first/bundle.sha256"
