#!/usr/bin/env bash
# Delegate an errand to the local Qwen3.8 NEO-CODER (vision LLM on :8002) through the pi harness.
# Pattern: docs/PATTERN-pi-subagents.md. The model is a first-pass worker, never an authority: triage what it returns.
#
#   scripts/delegate-qwen38.sh [-f PROMPT_FILE] [-C DIR] [--tools] [-t SECONDS] [-o OUT] [PROMPT...]
#
#   (default)  read-only: pi runs with --no-tools, so it can only answer from the prompt. Put the material in the prompt.
#   --tools    pi may read/edit/run inside DIR. Use ONLY in a throwaway git worktree, then review `git diff`.
#   -C DIR     working directory for the errand (default: current directory)
#   -f FILE    prompt from a file ("-" = stdin); remaining args are appended
#   -t N       hard timeout in seconds (default 300)
#   -o OUT     write the answer to OUT (default: stdout)
#
# Never put secrets in a prompt. Exit codes: 0 ok, 2 usage, 3 model not ready, 4 pi provider missing, 124 timeout.
set -euo pipefail

PROVIDER="${KR0KI_DELEGATE_PROVIDER:-neo-coder}"
MODEL="${KR0KI_DELEGATE_MODEL:-neo-coder}"
ENDPOINT="${KR0KI_DELEGATE_ENDPOINT:-http://127.0.0.1:8002}"   # 127.0.0.1, not localhost (podman pasta IPv6 empty replies)
PI_MODELS="${PI_MODELS_JSON:-$HOME/.pi/agent/models.json}"

tools=0; dir="$PWD"; secs=300; out=""; file=""; args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --tools) tools=1 ;;
    -C) dir="$2"; shift ;;
    -f) file="$2"; shift ;;
    -t) secs="$2"; shift ;;
    -o) out="$2"; shift ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) args+=("$1") ;;
  esac
  shift
done

prompt=""
if [ -n "$file" ]; then prompt="$([ "$file" = "-" ] && cat || cat "$file")"; fi
[ ${#args[@]} -gt 0 ] && prompt="${prompt:+$prompt$'\n\n'}${args[*]}"
[ -n "$prompt" ] || { echo "no prompt (give text, -f FILE, or -f -)" >&2; exit 2; }

# Preflight: fail fast and loudly if the model is not serving (cold start after boot takes minutes).
if ! curl -fsS -m 5 "$ENDPOINT/health" >/dev/null 2>&1; then
  echo "Qwen3.8 is not answering at $ENDPOINT/health (cold start takes ~2-5 min after boot: systemctl --user status b00t-hive-inference-heretic-neo-coder)" >&2
  exit 3
fi
if ! grep -q "\"$PROVIDER\"" "$PI_MODELS" 2>/dev/null; then
  echo "pi has no '$PROVIDER' provider in $PI_MODELS; add: {\"providers\":{\"$PROVIDER\":{\"baseUrl\":\"$ENDPOINT/v1\",\"api\":\"openai-completions\",\"apiKey\":\"local-b00t\",\"models\":[{\"id\":\"$MODEL\"}]}}}" >&2
  exit 4
fi

flags=(-p --provider "$PROVIDER" --model "$MODEL" --no-session)
[ "$tools" = 1 ] || flags+=(--no-tools)

# `< /dev/null` is load-bearing: off a TTY pi otherwise waits on stdin forever (observed: a 7 minute hang, model answered in 3 s).
run() { cd "$dir" && timeout "$secs" pi "${flags[@]}" "$prompt" < /dev/null 2> >(grep -v -e "No models match pattern" -e "MCP: Project servers blocked" >&2 || true); }
if [ -n "$out" ]; then run > "$out"; else run; fi
