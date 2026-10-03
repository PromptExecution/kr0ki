#!/usr/bin/env bash
# Networked phase consumes validated evidence only; it never invokes Cargo/rustc.
set -euo pipefail
: "${KR0KI_DOCGEN_BACKEND:?an explicit private renderer is required}"
work=$(mktemp -d /tmp/docgen-render.XXXXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir "$work/input"
zstd -dq --stdout /artifacts/bundle.tar.zst | tar -xf - -C "$work/input"
kr0ki-docgen bundle --input "$work/input/ir.json" --output "$work/model" \
    --backend "$KR0KI_DOCGEN_BACKEND"
test -s "$work/model/diagram.svg"
test -s "$work/model/diagram.png"
python3 /opt/docgen/docgen-bundle.py --input "$work/model" --output /export \
    --metadata /opt/docgen/build-inputs.json
echo 'Private SVG/PNG renderer checks passed; rendered files are included in the bundle.'
