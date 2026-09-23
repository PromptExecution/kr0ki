#!/usr/bin/env bash
# Render the b00t stack orchestration template diagram through kr0ki.
# Usage: ./scripts/render-template.sh [kr0ki_url] [output.svg]
#
# Requires a running kr0ki-server (just run) backed by the private
# kroki-compat service.

set -euo pipefail

KR0KI_URL="${1:-http://127.0.0.1:8787}"
OUTPUT="${2:-b00t-stack-orchestration.svg}"
TEMPLATE="templates/b00t-stack-orchestration.d2"

if [[ ! -f "$TEMPLATE" ]]; then
  echo "error: $TEMPLATE not found (run from repo root)" >&2
  exit 1
fi

echo "Rendering $TEMPLATE via $KR0KI_URL/render/d2 → $OUTPUT"

curl -fsS -X POST "$KR0KI_URL/render/d2" \
  -H "Content-Type: text/plain" \
  --data-binary "@$TEMPLATE" \
  -D - \
  --output "$OUTPUT"

echo "Done. Check $OUTPUT"
