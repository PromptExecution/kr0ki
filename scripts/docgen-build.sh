#!/usr/bin/env bash
# Runs inside the pinned generator image, never on untrusted host source.
set -euo pipefail
output=${1:-/artifacts}
work=$(mktemp -d /tmp/docgen.XXXXXXXX)
trap 'rm -rf "$work"' EXIT
export CARGO_TARGET_DIR="$work/target"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true
export SOURCE_DATE_EPOCH=0 TZ=UTC LC_ALL=C
export KR0KI_SOURCE_REVISION=${KR0KI_SOURCE_REVISION:?missing source revision}
export KR0KI_SOURCE_TREE=${KR0KI_SOURCE_TREE:?missing source tree}
manifest=${KR0KI_DOCGEN_MANIFEST:-tools/rust-behavior-extractor/tests/fixtures/workspace/Cargo.toml}
rust-behavior-extractor --manifest-path "/src/$manifest" --output "$work/ir.json" \
    --revision "$KR0KI_SOURCE_REVISION" --tree-digest "$KR0KI_SOURCE_TREE" --workspace-root /src
bundle_args=(bundle --input "$work/ir.json" --output "$work/model")
if [[ "$manifest" == tools/rust-behavior-extractor/tests/fixtures/workspace/Cargo.toml ]]; then
    bundle_args+=(--state-machine /src/crates/kr0ki-behavior/tests/fixtures/ooda.json)
fi
kr0ki-docgen "${bundle_args[@]}"
python3 /opt/docgen/docgen-bundle.py --input "$work/model" --output "$output" \
    --metadata /opt/docgen/build-inputs.json
