#!/usr/bin/env bash
# Exercise the live playb00k through the deployed kr0ki service.
# Usage: ./scripts/playbook-e2e.sh [kr0ki_url]

set -euo pipefail

KR0KI_URL="${1:-http://127.0.0.1:8787}"
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

# The Vue/Vite playb00k consumes exactly the same catalog as Rust tests and
# mdb00k's static export. Check both the live API and the UI-relative alias.
curl -fsS "$KR0KI_URL/playbook/" >"$WORK_DIR/playbook.html"
rg -q 'id="app"' "$WORK_DIR/playbook.html"
curl -fsS "$KR0KI_URL/api/examples" >"$WORK_DIR/examples.json"
curl -fsS "$KR0KI_URL/playbook/api/examples.json" >"$WORK_DIR/playbook-examples.json"
cmp "$WORK_DIR/examples.json" "$WORK_DIR/playbook-examples.json"
rg -q '"format":"d2"' "$WORK_DIR/examples.json"
rg -q '"format":"graphviz"' "$WORK_DIR/examples.json"

# The panel must let a caller test raw, hand-authored diagram source too, not
# just the fixture catalog (PLAN-KR0KI-003.md §6) — check the built bundle
# still ships the custom-diagram controls (reset/clear/upload).
PLAYBOOK_JS_PATH="$(rg -o 'assets/[^"]*\.js' "$WORK_DIR/playbook.html" | head -1)"
curl -fsS "$KR0KI_URL/playbook/$PLAYBOOK_JS_PATH" >"$WORK_DIR/playbook.js"
rg -q 'Start blank' "$WORK_DIR/playbook.js"
rg -q 'Reset to example' "$WORK_DIR/playbook.js"
rg -q 'Upload file' "$WORK_DIR/playbook.js"

# A custom-route example (POST /render/k8s-topology) isn't reachable by
# templating format into /render/{format} -- Gallery.vue/RendererPanel.vue's
# endpoint construction must fall back to example.route when it's set.
rg -q 'example\.route\s*\|\|' "$WORK_DIR/playbook.js"

# The catalog itself must carry that one custom-route entry, distinct from
# every DiagramFormat-routed example (kr0ki_core::examples::PlaybookExample::route).
rg -q '"id":"k8s-topology-web-service".*"route":"/render/k8s-topology"' "$WORK_DIR/examples.json"

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

# The k8s-topology recognizer -> lift -> D2 pipeline (kr0ki#30), driven with
# its own catalog fixture (pulled from the live catalog, not a duplicated
# local copy, so this can't silently drift from what the recognizer test in
# kr0ki-core's own examples.rs already proved produces real edges).
jq -r '.[] | select(.id == "k8s-topology-web-service") | .source' \
  "$WORK_DIR/examples.json" >"$WORK_DIR/k8s-topology.yaml"
[[ -s "$WORK_DIR/k8s-topology.yaml" ]]
curl -fsS -D "$WORK_DIR/k8s-first.headers" \
  -X POST "$KR0KI_URL/render/k8s-topology?output=svg" \
  --data-binary "@$WORK_DIR/k8s-topology.yaml" -o "$WORK_DIR/k8s-first.svg"
curl -fsS -D "$WORK_DIR/k8s-second.headers" \
  -X POST "$KR0KI_URL/render/k8s-topology?output=svg" \
  --data-binary "@$WORK_DIR/k8s-topology.yaml" -o "$WORK_DIR/k8s-second.svg"
cmp "$WORK_DIR/k8s-first.svg" "$WORK_DIR/k8s-second.svg"
rg -qi '^x-kr0ki-cache: hit' "$WORK_DIR/k8s-second.headers"
rg -q '<svg' "$WORK_DIR/k8s-first.svg"
# Real recognized relationships, not a bare unconnected node list -- the
# exact assertion kr0ki-core's own
# k8s_topology_example_actually_exercises_the_recognizer_pipeline test makes,
# now proven against the real deployed Kroki backend instead of just to_d2().
rg -q 'selects|dependency' "$WORK_DIR/k8s-first.svg"

echo "PASS: playb00k live flow rendered and cache-verified at $KR0KI_URL"
