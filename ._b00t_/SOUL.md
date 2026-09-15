# Soul — kr0ki

Workspace soul initialized 2026-09-15.
Distilled memories from sessions will be appended here.

## Project patterns

- Podman kube owns the local `kr0ki` and isolated `kubediagram` MCP pods; b00t installs
  the Codex entries. Do not add host language packages for this integration.
- Validate the GitHub workflow with `wrkflw`, then run `just test` and `just check`
  through b00t.
- The KubeDiagrams bridge accepts manifest text only. It has no network, a read-only
  filesystem except for its temporary volume, and never exposes `-c` because that
  KubeDiagrams option can execute Python.
- `vendor/kubediagrams` follows PromptExecution/KubeDiagrams branch
  `b00t/kr0ki-extensions`; preserve upstream command compatibility and make b00t
  behavior opt-in.
