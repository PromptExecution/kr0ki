# Operating kr0ki

How to run, configure, verify and troubleshoot the services. Everything here was checked against the
`justfile`, the server/agent source, and a browser run (see [§7](#7-verifying-with-a-real-browser)).
Machine-specific addresses never live in tracked files — they come from the gitignored `.env`.

## 1. Services and ports

| Service | Default | What it is | Started by |
|---|---|---|---|
| `kr0ki` server | `0.0.0.0:8787` | axum HTTP service: render loop, `/model/*`, `/docs`, playbook host | `just start` / `just run` / `just dev` / pod |
| Kroki-compatible backend | `127.0.0.1:8010` | private renderer (`KROKI_SAFE_MODE=secure`); never point at public kroki.io | `just dev-kroki-up` / pod |
| StoryB00k agent | `0.0.0.0:8789` | AG-UI SSE sidecar; talks to an OpenAI-compatible LLM and to kr0ki tools | `just start-agent` / pod |
| Playbook dev server | `0.0.0.0:5173` | Vite dev server (`pnpm --dir playbook dev`); proxies `/api` to kr0ki | manual |
| SysML v2 model server | unset | optional; enables the `/model/*` routes | external |

## 2. Configuration

`just` loads `.env` automatically (`set dotenv-load`). Copy `.env.example` to `.env` and fill it in; it is
gitignored. The full variable table for the server is in [`AGENTS.md` §3](../AGENTS.md). The ones that matter
day to day:

| Variable | Used by | Notes |
|---|---|---|
| `OPENAI_API_URL`, `OPENAI_API_KEY` | agent, `/health` LLM probe | `just start-agent` refuses to start without `OPENAI_API_URL` |
| `KR0KI_PUBLIC_URL`, `KR0KI_AGENT_PUBLIC_URL` | Vite dev proxy, `tests/functional`, agent CORS | where a browser/test reaches the services; loopback if unset |
| `KR0KI_STORYB00K_ALLOWED_ORIGINS` | agent CORS allowlist | `just start-agent` defaults to loopback + `KR0KI_PUBLIC_URL` |
| `KR0KI_STORYB00K_PORT` | agent bind port | `just start-agent <port>` sets it for you |
| `KR0KI_STORYB00K_MAX_MODEL_TOOL_ROUNDS` | agent | default **15** |
| `KR0KI_STORYB00K_MAX_CONSECUTIVE_FAILURES` | agent | default 3: repeated identical failures stop the loop |
| `KR0KI_STORYB00K_MAX_CLARIFYING_QUESTIONS` | agent | default 6 (user-facing question budget, separate from tool rounds) |
| `KR0KI_STORYB00K_MAX_OUTPUT_CHARS` / `_THREAD_TTL_SECS` | agent | defaults 20000 / 6 h |
| `KR0KI_SYSMLV2_BASE_URL`, `KR0KI_SYSMLV2_TOKEN` | server | turn on `/model/*` (incl. `POST /model/projects/{id}/sync`) |
| `KR0KI_PLAYBOOK_DIR` | server | built playbook to serve (default `playbook/dist`) |
| `KR0KI_CONTRACT_URI` | server | value of the `X-Kr0ki-Contract` header; an invalid value falls back to the default with a warning |
| `KR0KI_AUTH_TOKEN` | server | bearer auth on every route **except `/health`**, which is always exempt (readiness probes, `just validate-server`) |

## 3. Running the server

```bash
cargo build                 # `just start` / `just run` execute ./target/debug/kr0ki
just start                  # background, starts the backend if needed, waits for ready, validates
just status                 # running/stopped, PID, log location  (state in .kr0ki-run/)
just validate-server        # /health responds and X-Kr0ki-Contract / X-Kr0ki-Request-Id are present
just stop
```

Other modes:

| Recipe | Use |
|---|---|
| `just run` | foreground, `KR0KI_BIND=0.0.0.0:8787`, backend `http://127.0.0.1:8010` |
| `just dev` | foreground against our own pinned kroki-compat image via plain podman (`just dev-kroki-up/down`). For iteration only — re-verify with `pod-up` before calling fixture work done |
| `just pod-build` / `pod-up` / `pod-down` | podman builds, import into the local k0s cluster, `kubectl` owns the lifecycle. `pod-env` turns `.env` into the `kr0ki-local-env` Secret without printing values |
| `just static-docs` | static GitHub-Pages-compatible mdb00k/playb00k bundle into `site/` |

## 4. Running the agent

```bash
just start-agent            # default port 8789; creates containers/kr0ki-storyb00k-agent/.venv on first run
just status-agent
just stop-agent
just start-agent 18789      # another port: the recipe passes it to the server as KR0KI_STORYB00K_PORT
```

It needs `OPENAI_API_URL` (from `.env` or the environment) and stops with a console error if it is missing.
The agent's `/health` is standalone (it reports `llm_configured`, `active_threads` and the round/question
limits). The playbook calls it before chatting, and kr0ki's own `/health` probes the agent only when
`KR0KI_STORYB00K_AGENT_URL` is set on the server. State for the recipes lives in
`.kr0ki-run/agent.{pid,log}` (gitignored).

## 5. The playbook

- **Version** is read from `Cargo.toml` at build time (Vite `define` → `__APP_VERSION__`) and shown in the
  sidebar. `cog.toml` (cocogitto) bumps it with `cargo set-version`; `scripts/check-version-sync.sh` checks the
  pieces line up.
- **The server serves a static build**, not the Vite dev server. After changing playbook source run
  `pnpm --dir playbook build` (output `playbook/dist`) and restart the server, or the page keeps serving the old
  bundle. The dev server on `:5173` always shows current source.
- **Renderer URL** defaults to the page's host on port **8787**, or `?renderer=<url>`, or the value saved by the
  Setup tab (`localStorage` key `kr0ki:rendererUrl`). On any other port (e.g. a server started on 18787) set it in
  Setup first, or *Test all* does nothing.
- **Vite proxy** (`playbook/vite.config.js`) forwards `/api` and `/playbook/api` to `KR0KI_PUBLIC_URL`
  (from `.env` or the environment), else `http://127.0.0.1:8787`.
- **Setup tab** stores renderer URL, LLM URL/key/model, agent URL and output format in `localStorage`
  (`kr0ki:*`). *Test Connection* checks the renderer's `/health` (accepts `ok` or `degraded`) and the LLM's
  `/models`; the model field is a free-text input with a `datalist` filled from that response. The key is held in
  plaintext in browser storage — see [§8](#8-known-gaps).
- **Code Editor ↔ Agent**: a gallery card's *Agent* button opens the agent with a type-specific prompt;
  a rendered chart's *EDIT* button opens the editor with that diagram's source and format. Revisions are a
  prompt/edit/fork DAG (`playbook/src/lib/revisionGraph.js`, drawn with `@vue-flow/core`), persisted server-side
  by the agent through `jj` (`chart_store.py`; plain files if `jj` is absent).

## 6. Tests

| Command | Covers |
|---|---|
| `just check` | `cargo fmt --check` + `clippy -D warnings` (the CI gate) |
| `just test` | Rust workspace, the two MCP bridge suites, the agent's unittest suite (in its `.venv`), playbook vitest |
| `just test-live` / `test-live-png` | live render against the backend (`KR0KI_TEST_BACKEND`) |
| `just test-playbook [url]` | every catalog fixture through a running server, twice (bytes + cache hit) |
| `just playbook-e2e [url]` | health + the genuine D2 Rust-flow SVG + cache hit |
| `tests/functional/run-tests.sh` | API/agent smoke; reads `BASE_URL` / `AGENT_URL`, then `.env`, then loopback |
| ignored live tests | need `KR0KI_SYSMLV2_BASE_URL` (+ `KR0KI_SYSMLV2_TEST_PROJECT_ID`) — see `kr0ki-sysmlv2-client/tests/live.rs`, `kr0ki-core/tests/flexo_reqif_sync.rs` |

Run the agent tests with the project's interpreter, not the system one: `containers/kr0ki-storyb00k-agent/.venv/bin/python -m unittest discover -p "test_*.py"`.

## 7. Verifying with a real browser

The Chrome DevTools MCP server (`chrome-devtools-mcp`) attaches to a Chrome on `localhost:9222`. A disposable
headless one:

```bash
chrome --headless=new --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile --no-sandbox about:blank &
curl -s http://127.0.0.1:9222/json/version     # confirms CDP is up
```

(Playwright's bundled Chromium works and avoids snap confinement.) Then drive the playbook through the MCP
tools. What a healthy run looks like, from the last full check:

- Gallery shows **37 fixtures** and the version; *Test all* ends **37/37 passed**.
- Setup, Agent, Code Editor, Gallery tabs all render; the Agent tab says **Ready ✓ agent** when the agent's
  origin allowlist contains the page's origin.
- The only console noise is the missing favicon (`404`).
- Save large `take_snapshot` output with `filePath`; the gallery snapshot alone is tens of thousands of tokens.
- A headless `take_screenshot` can hang; prefer DOM/network/console evidence.

## 8. Known gaps

- **`status: degraded` is normal** when no LLM is configured; the server still renders.
- **Cold backend:** the first PlantUML/Symbolator render after the backend starts can return `502
  backend_unavailable`, and *Test all* can show 2 failures out of 37 on a cold start; a second run passes.
- **The agent's accepted input formats** (`VALID_INPUT_FORMATS` in `server.py`) include names such as `mermaid`,
  `bpmn`, `excalidraw` that kr0ki does not advertise (`DiagramFormat::ALL` is the 26 companion-free Kroki
  formats; Mermaid, BPMN and Excalidraw need a headless-browser companion). The server rejects those at render time.
- **LLM key in `localStorage`** (Setup tab) is readable by any script on the page.
- **`just test-playbook` / `playbook-e2e`** need a built `playbook/dist` and a running server.
