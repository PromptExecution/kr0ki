# lsp-bridge

Optional bridge that lets the playbook's code editor use a **Language Server** ("when available").
`just lsp-bridge` -> `ws://127.0.0.1:8791/json` and `/yaml`. Then in the playbook: **Setup -> Language servers**, paste the URL for the language.

| Language | Server | Notes |
|---|---|---|
| vega, vegalite | `vscode-json-language-server` (`/json`) | verified end to end: a trailing comma shows as an error mark |
| wireviz | `yaml-language-server` (`/yaml`) | installed; not exercised end to end |
| everything else | none we can run | the editor works without one (d2, graphviz, plantuml, ... have no usable server; wavedrom is JSON5-ish so a strict JSON server would mislead) |

## Security (this process spawns local binaries for a web page)
Binds `127.0.0.1` by default. The browser picks only a URL path; the argv for each path is a **fixed allowlist** (`COMMANDS` in `bridge.mjs`), never taken from the page.
The WebSocket **Origin must be allowed** (`LSP_BRIDGE_ORIGINS`, comma separated; localhost :8787/:5173 and `KR0KI_PUBLIC_URL` are allowed by default); a missing Origin is refused.
One process per socket, killed (and its temp dir removed) when the socket closes; max 4 sessions; 1 MiB message/frame cap; idle sessions closed after 10 min; the server runs in an empty temp dir with only `PATH` and `HOME`.
`LSP_BRIDGE_TRACE=1` logs JSON-RPC method names and document metadata (never contents).
Do not expose this beyond localhost. If you need remote use, put it behind kr0ki-server's bearer auth rather than opening the port.

## Compatibility note
`@codemirror/lsp-client` 6.3.0 advertises pull-diagnostics support but never pulls, so servers like the JSON one would never push. The editor's transport (`playbook/src/lib/lsp.js`, `patchOutgoing`) removes that capability from `initialize`.
