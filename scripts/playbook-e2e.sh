#!/usr/bin/env bash
# Exercise the live playb00k through the deployed kr0ki service.
# Usage: ./scripts/playbook-e2e.sh [kr0ki_url]

set -euo pipefail

KR0KI_URL="${1:-http://192.168.1.137:8787}"
FLOW_SOURCE="templates/kr0ki-render-flow.d2"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

[[ -f "$FLOW_SOURCE" ]] || {
  echo "FAIL: missing $FLOW_SOURCE (run from the repository root)" >&2
  exit 1
}

curl -fsS "$KR0KI_URL/health" >"$WORK_DIR/health.json"
rg -q '"status":"ok"' "$WORK_DIR/health.json"

curl -fsS "$KR0KI_URL/docs" >"$WORK_DIR/docs.html"
rg -q '/docs/examples/kr0ki-render-flow.svg' "$WORK_DIR/docs.html"

# This endpoint renders the D2 source through RenderService, then serves the
# cached artifact. It is the live visual contract for the Box-5 Rust flow.
curl -fsS -D "$WORK_DIR/flow.headers" \
  "$KR0KI_URL/docs/examples/kr0ki-render-flow.svg" \
  -o "$WORK_DIR/flow.svg"
rg -qi '^content-type: image/svg\+xml' "$WORK_DIR/flow.headers"
rg -q '<svg' "$WORK_DIR/flow.svg"
rg -q 'data-d2-version' "$WORK_DIR/flow.svg"

# Render the same fixture twice through the public API. The second request must
# use the same content-addressed artifact and be a cache hit.
curl -fsS -D "$WORK_DIR/first.headers" \
  -X POST "$KR0KI_URL/render/d2?output=svg" \
  --data-binary "@$FLOW_SOURCE" -o "$WORK_DIR/first.svg"
curl -fsS -D "$WORK_DIR/second.headers" \
  -X POST "$KR0KI_URL/render/d2?output=svg" \
  --data-binary "@$FLOW_SOURCE" -o "$WORK_DIR/second.svg"

cmp "$WORK_DIR/first.svg" "$WORK_DIR/second.svg"
FIRST_KEY="$(awk 'BEGIN{IGNORECASE=1} /^x-kr0ki-key:/ {print $2}' "$WORK_DIR/first.headers" | tr -d '\r')"
SECOND_KEY="$(awk 'BEGIN{IGNORECASE=1} /^x-kr0ki-key:/ {print $2}' "$WORK_DIR/second.headers" | tr -d '\r')"
[[ "$FIRST_KEY" =~ ^[[:xdigit:]]{64}$ ]]
[[ "$FIRST_KEY" == "$SECOND_KEY" ]]
rg -qi '^x-kr0ki-cache: hit' "$WORK_DIR/second.headers"

echo "PASS: playb00k live flow rendered and cache-verified at $KR0KI_URL"
