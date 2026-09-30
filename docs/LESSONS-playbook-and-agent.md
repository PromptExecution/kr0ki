# Lessons from debugging the playbook, the agent and the tooling

One row per real incident: what it looked like, the actual root cause, and what now guards against it.
This replaces about a dozen session notes that used to sit at the repo root; the originals are still in git
at `8a5a5b7` (`git show 8a5a5b7:CORS-FIX.md`). Operational how-to lives in [`OPERATIONS.md`](OPERATIONS.md).

## Agent / chat path

| Symptom | Root cause | Guard now |
|---|---|---|
| Agent tab: generic "Failed to fetch" | The agent sidecar on `:8789` was not running (the UI talks to the agent, never straight to the LLM) and the UI hid which URL failed | UI reports the URL and error kind; `just start-agent` / `status-agent`; agent `/health` |
| Same "Failed to fetch" from a remote browser | Browser origin missing from the agent's CORS allowlist (`KR0KI_STORYB00K_ALLOWED_ORIGINS`) | UI detects and names a CORS failure; `start-agent` builds the allowlist from loopback + `KR0KI_PUBLIC_URL`; verified: allowed origin gets `Access-Control-Allow-Origin`, a foreign one does not |
| LLM returned 500 `No user query found in messages` | llama.cpp's Jinja chat template needs at least one `user` message; the AG-UI client sometimes sent none | `server.py` appends a user turn when the messages contain none |
| Agent looped 60+ rounds re-sending SVG as a diagram *source* | SVG/PNG are output formats; no format validation, tool-round ceiling of 64, no failure detection, unhelpful errors | `VALID_INPUT_FORMATS`, `MAX_MODEL_TOOL_ROUNDS` = 15, `MAX_CONSECUTIVE_FAILURES` = 3, errors fed back to the model |
| `just start-agent` reported failure / "address already in use" | A previous agent instance still held the port | `status-agent` / `stop-agent`; see also the recipe bugs below |
| Chat rejected every event | Events did not match `@ag-ui/core`'s zod schemas (missing `threadId`, interrupt placed outside `outcome`, tool results on `TOOL_CALL_END`, usage as an object) | agent emits schema-exact events (checked against `@ag-ui/core`'s `EventSchemas` in an earlier session, recorded in `.b00t/tasks.json` task 27; no test in this repo re-checks it) |
| `crypto.randomUUID is not a function` on a LAN/HTTP page | `randomUUID` exists only in secure contexts | a `getRandomValues`-based v4 fallback installed on `Crypto.prototype` (assigning `window.crypto` throws: it is getter-only) |
| Agent page blank, `Cannot access 'props' before initialization` | `props` was read before `defineProps()` ran in `StoryB00k.vue` (and a `ref(window)` in the same spirit) | `defineProps()` is the first statement in `<script setup>` (`StoryB00k.vue`) |
| "Answer" / tool-call flow errors (`No active tool call found`, pending interrupts not addressed) | interrupts were emitted without a full `TOOL_CALL` lifecycle; resume used the wrong call | full lifecycle before each interrupt; `chat.resume()` |

## Playbook / renderer

| Symptom | Root cause | Guard now |
|---|---|---|
| Editor's Renderer URL empty, render refuses | the default was only set when the page itself was on port 8787 | fallback is `?renderer=`, else the page host on `:8787`; **still wrong on any other port**, so set it in Setup (documented in `OPERATIONS.md`). Hit again when verifying a server on 18787: *Test all* silently did nothing |
| Page shows v0.0.1 after a bump | the server serves the static `playbook/dist`; nothing rebuilt it or restarted the server | version comes from `Cargo.toml` via Vite `define`; rebuild `playbook/dist` and restart; `scripts/check-version-sync.sh` |
| "Could not load the executable example catalog" on the Vite dev server | `/playbook/api/examples.json` fell through to Vite's `index.html` (HTML where JSON was expected) | Vite proxy for `/api` and `/playbook/api`, target from `KR0KI_PUBLIC_URL` |
| 2 of 37 fixtures fail in *Test all* | cold backend: the first PlantUML/Symbolator render after start returns 502/500; the same sources return 200 individually | re-run; not a code defect (confirmed: backend, old server and new server all return 200 for both) |

## Server and tooling (found while reviewing and verifying the recent PRs)

| Finding | Why it mattered | Fix |
|---|---|---|
| Contract middleware did `.parse().expect()` on an env-configurable URI per request | a bad `KR0KI_CONTRACT_URI` would panic every response | validated once at construction; invalid value falls back with a warning |
| Auth layer ran outside the contract layer | `401` responses had no `X-Kr0ki-Contract` / request-id headers | contract layer is outermost; regression test fails without it |
| `require_bearer` also guarded `/health` | the pod's readiness probes and `just validate-server` call it unauthenticated; docs always said "except `/health`" | `/health` exempt by exact path; test added first |
| `just start-agent <port>` ignored `<port>` | the recipe never set `KR0KI_STORYB00K_PORT`, so a second agent collided with 8789 | port passed through |
| `start-agent` / `status-agent` said "failed" for a healthy agent | they grepped `"status":"ok"`; the agent emits `"status": "ok"` | tolerant pattern |
| `OPENAI_API_URL:?` guard hidden in a backgrounded command | its message went to a log file | checked up front on the console |
| Running `just start-agent` from the main checkout overwrote `agent.pid` / truncated `agent.log` of a live agent | the recipe trusts a single pid file | restore the pid file by hand; verify in a throwaway worktree |
| `git commit -am` staged an unrelated, deliberately-excluded modified file (`.b00t/tasks.json`) | it was squash-merged into `main` with the PR | stage by explicit path (`git add <files>`); check `git status` and `git show --stat` before pushing. Left in place (benign task bookkeeping) and reported |
| A `sed` over the `justfile` rewrote every `cat .kr0ki-run/agent.pid` | it changed recipes where `PID_FILE` is undefined | repaired and re-verified by running `status-agent` and the already-running guard |
| `pkill -f <pattern>` killed its own shell | the command line contains the pattern | kill by PID taken from `ss -ltnp` |

## Patterns worth keeping

- **Test the claim, not the parse.** `just --summary` succeeding says nothing about a recipe; run it. Same for
  "the test fails without the fix": revert only the fix and watch it fail.
- **Run the real thing against a faithful stub.** A stateful SysML v2 stub that enforces full-replace payloads,
  returns 409 on a stale `previousCommit`, and lists commits oldest-first exposed ordering and race behaviour
  that unit mocks cannot.
- **Security-relevant defaults belong in code, not in prose.** The `/health` exemption existed only in docs
  for weeks.
- **Status notes rot.** "✅ Running on port 8789 (PID …)" is true for an afternoon. Record causes, guards and
  commands; leave live state to `just status`.
- **Keep machine addresses out of tracked files.** Loopback defaults plus `.env`.
