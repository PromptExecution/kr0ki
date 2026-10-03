# Pattern: pi sub-agents on the local Qwen3.8 (always delegate)

**Rule (owner, 2026-10-02): errands are always delegated to Qwen3.8 through pi.** Reviews, first drafts, test
scaffolding, summaries, log triage, "is this claim right?" checks: send them to the local NEO-CODER (vision LLM,
`127.0.0.1:8002`), then triage the answer. The expensive model plans, decides and verifies; the local model does the
legwork. The model is a first-pass worker, never an authority.

## The one command
```bash
scripts/delegate-qwen38.sh "Review this for factual errors: ..."          # read-only (pi --no-tools)
scripts/delegate-qwen38.sh -f errand.txt -o answer.txt                    # prompt from a file, answer to a file
scripts/delegate-qwen38.sh --tools -C ../wt-errand -t 600 "Add tests for ..."   # may edit: throwaway worktree ONLY
just delegate "one-line prompt"
```
Exit codes: 0 ok, 2 usage, 3 model not serving, 4 pi provider missing, 124 timeout.

## Lifecycle of an errand
1. **Scope it small and closed.** One question or one file-sized change; put every fact the model needs in the prompt
   (it has no memory of this repo). Say exactly what output shape you want ("list only concrete errors, quote the words").
2. **Pick the mode.** Default is read-only: the model can only answer from the prompt. Use `--tools` only inside a
   throwaway `git worktree`, never the main checkout, and read `git diff` afterwards.
3. **Run it.** ~35 s for a 3k-token prompt; the model is warm. A cold start after boot takes 2-5 minutes (exit 3 until
   `/health` answers).
4. **Triage, always.** Treat the output as a lead, not a result. In the first real errand (reviewing the MBSE skills),
   5 findings produced ~1 valid nit; the rest were wrong or contradicted something already verified live.
   Verify each claim against the source, run the tests, and only then apply. Record what you rejected and why.
5. **Never delegate the verification.** The model may draft a test; a human or the main agent runs it and checks it
   fails when the code is broken (mutation check).

## Don't
- Put secrets, tokens or private keys in a prompt (it is logged by pi and the server).
- Let it decide architecture, security posture or what ships.
- Run `--tools` in a checkout you care about, or without `-t`.
- Use `localhost` for the endpoint: podman's pasta forward answers IPv6 `localhost` with empty replies. Use `127.0.0.1`.

## Plumbing facts (verified 2026-10-02)
- pi 0.87.1 provider `neo-coder` in `~/.pi/agent/models.json` -> `http://127.0.0.1:8002/v1`. The script prints the exact
  JSON to add if it is missing. The old `llama-cpp/ch0nky` (:8001) entry is dead; pi only warns about it.
- **`< /dev/null` is load-bearing.** Off a TTY, pi waits on stdin forever (a 7-minute hang; the model had answered in 3 s).
- Boot default LLM: `b00t-hive-inference-heretic-neo-coder.service` (`deploy/systemd/`), Conflicts= every other GPU model.
- `b00t agent invoke neo-coder "task"` works from the kr0ki repo root: `./_b00t_/neo-coder.agent.toml` (tracked here) is a
  pi-driven, read-only executor against :8002. `invoke` resolves `./_b00t_/<agent>.agent.toml` from the CURRENT directory and
  never consults `~/.b00t/_b00t_/neo-coder.agent.toml` (global, still names the retired `sm3lly:8001` and has no executor),
  so the project-local datum overrides it with no change to b00t. A datum needs `[b00t.agent.ipc]` and `[b00t.agent.crew]`
  even for a one-shot agent, or it fails to parse. Verified 2026-10-02.

## For b00t
To make this a first-class b00t pattern: a `pi-subagent` datum whose `ExecStart` is `scripts/delegate-qwen38.sh`
semantics (preflight `/health`, `--no-tools` default, `</dev/null`, hard timeout, triage-required result flag). The
script is the reference implementation; port its preflight and guard rails, not its flags.
