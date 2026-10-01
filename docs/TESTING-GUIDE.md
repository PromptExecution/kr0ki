# Testing guide (v0.0.9)

What is built, how to try each piece, what you should see, and what is known to be incomplete. Every step below was run
against the live system on 2026-10-01. Host: `http://192.168.1.137:8787` (kr0ki) and `:8789` (agent). Replace the host if you
run elsewhere. The agent's model is the local vision model on `:8002`; `OPENAI_API_URL` in `.env` must point at it.

## 1. Playbook (browser): `http://192.168.1.137:8787/playbook/`
| Try | Expect |
|---|---|
| Footer / sidebar | `v0.0.9` |
| **Gallery -> planner** (right-hand panel, "● page linked") | The Renderer URL field is gone (it lives in Setup). Type *"I need to show how our database tables relate, with foreign keys"*. Within ~10 s the gallery filters to **data model**, three cards get a **★ suggested** badge with a banner, one card is selected, and the planner answers with a pick and an alternative. |
| Planner session id (under the chat) | `planner-…`. Any MCP client can steer your page with it (see §3). |
| **Code Editor -> Render -> Send to Agent** | The Agent view opens with a **Starting point** card above the chat: the rendered image and the source you sent. It can be collapsed, not dismissed, and survives sending messages. |
| **Gallery card -> Agent** (locks the type) | The agent loads that type's **quality skill** (good/bad practice) and asks scoping questions about *what to convey*. |
| **Agent: ask for a diagram in any of the 26 formats** (e.g. *"draw an nwdiag network with a DMZ and an internal LAN"*) | The first render shows one line: **"Loaded the nwdiag syntax guide (N chars); retrying with it."**, then the retry renders. Failed renders now show the renderer's real message (not just "HTTP 400") and an identical repeat render is refused. |

## 2. SysML v2 with names and brand (API)
No real SysML server is wired in. Use the fixture to see the whole path:
```bash
python3 tools/svg-enhance-spike/sysml_fixture_server.py 18081 &
KR0KI_BIND=127.0.0.1:8788 KR0KI_BACKEND_URL=http://127.0.0.1:8010 KR0KI_SYSMLV2_BASE_URL=http://127.0.0.1:18081 ./target/debug/kr0ki &
curl -s localhost:8788/brand                                                   # {"brands":["example"]}
curl -s -D- -X POST 'localhost:8788/render/sysmlv2/projects/p1/commits/c1?output=svg&brand=example' -o branded.svg
```
Expect headers `x-kr0ki-brand: example`, `x-kr0ki-enhance: indexed=4;unindexed=1` (the requirement is not drawn). Open `branded.svg` in a browser:
nodes read **Vehicle / engine / transmission / battery** (not UUIDs), each carries an icon and a blue/purple stroke, and `document.querySelectorAll('[data-kr0ki-type="PartUsage"]')`
returns 3. `brand=nope` -> 404; `brand=../x` -> 400. **For your real server:** set `KR0KI_SYSMLV2_BASE_URL` (+ `KR0KI_SYSMLV2_TOKEN`) and use your project/commit ids.

## 3. MCP tools and UI steering
```bash
curl -s localhost:8787/mcp/tools | python3 -c "import sys,json;print([t['name'] for t in json.load(sys.stdin)][-3:])"   # list/suggest/navigate
curl -s -X POST localhost:8787/api/catalog/suggest --data-binary 'show a deployment schedule with milestones'     # ranked, with reasons
curl -s -X POST 'localhost:8787/ui/<session id from the planner>/navigate?use_case=timeline&suggest=gantt&note=hello'   # moves your open page
```
An unknown type or use case is rejected with 422; a session nobody is listening on returns `delivered: 0`.

## 4. Automated checks
`just check` (fmt + clippy), `just test` (workspace + playbook), agent tests: `cd containers/kr0ki-storyb00k-agent && .venv/bin/python -m unittest discover -p "test_*.py"` (61).
Skills against a live renderer: `python3 tools/skill-pilot/verify_skills.py containers/kr0ki-storyb00k-agent/skills/diagrams` (133/133) and
`python3 tools/skill-pilot/verify_type_skills.py containers/kr0ki-storyb00k-agent/skills/types` (24/24).

## 5. Known limits (so you do not mistake them for regressions)
- **Skills effect is a small, single-sample measurement** on the local 27B model (12/24 -> 22/24 drawing tasks rendered); expect improvement, not perfection. D2 is the weakest (4/6).
- **Silent failures:** wavedrom (unknown key), `vega` given a Vega-Lite spec, and umlet (unknown element id) return HTTP 200 with an empty diagram; the agent cannot detect that yet.
- **Brand overlay is SysML-only** (`?brand=` on the SysML render route). Graphviz/PlantUML overlays, a brand picker in the UI, and animation/live state are **not built**. `brand/example` is a placeholder, not your brand.
- **Not yet verified against a real SysML v2 server.** All SysML testing used the fixture.
- The planner and agent share one GPU slot: a second concurrent chat will queue behind the first.
- With `KR0KI_AUTH_TOKEN` set, the browser's live page-link (SSE) is refused (EventSource cannot send the bearer header).

## 6. What would unblock the rest
Canonical logos/palettes (-> real brand packages and brand/palette skills), approval to add `Skill` to upstream `ufo-types`, and a SysML v2 server URL/token.
See `HANDOFF-agent-harness-mode.md` and `DESIGN-NOTE-skills-identity-and-svg-enhancement.md` for the rest of the plan.
