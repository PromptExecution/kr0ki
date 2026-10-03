# Skills

A short index. Each portable skill is a directory with a `SKILL.md` (metadata + instructions)
and optional `scripts/` and `references/`, per the [Agent Skills
specification](https://agentskills.io/specification). Skill text **guides**; it never grants
permission. The tool gateway and the sandbox enforce what a caller may do.

| skill | what it is for |
|---|---|
| [`skills/requirements-authoring`](skills/requirements-authoring/SKILL.md) | Author, review and trace requirements through the assurance thread; keep satisfaction assertions apart from revision-bound verification results. |

The storyb00k agent's own prompt-sized skills (diagram types and MBSE guidance) live in
[`containers/kr0ki-storyb00k-agent/skills/`](containers/kr0ki-storyb00k-agent/skills) and are loaded
with its `load_mbse_skill` tool. They are a different, flatter format and are not Agent Skills packages.
