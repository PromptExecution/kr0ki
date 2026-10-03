---
name: requirements-authoring
description: Author, review and trace kr0ki requirements with the assurance thread. Use when adding or changing a requirement, when asked why a requirement is not verified, or when linking a requirement to its source, system element, enforcing control, verification case and evidence. Drives the kr0ki MCP tools (list_requirements, get_requirement, trace_requirement, propose_change, commit_change, run_verification) and keeps satisfaction assertions separate from revision-bound verification results.
license: MIT
compatibility: Needs the kr0ki MCP tools (or the same HTTP routes under /assurance) and an identity whose grants cover what you try to do. The gateway enforces grants; this text does not.
metadata:
  author: PromptExecution
  version: "0.1"
---

# Requirements authoring

A requirement here is one node in a **thread**:

```
source obligation → requirement → system element → enforcing control → verification case → evidence
```

You write the requirement. The tools tell you the rest. Do not fill in a link you have not
looked up, and never report a requirement as *verified* on the strength of a design assertion.

## Two claims, never one

- **Satisfied** is an *architectural assertion*: some system element is said to satisfy the
  requirement. It says nothing about whether anything was run.
- **Verified** is *revision-specific evidence*: a verification case passed at one model,
  implementation and configuration revision. Change any of the three and it is **stale**.

Report both axes. "Satisfied, untested", "verified", "failing" and "stale" are four different
states; saying "done" collapses them.

## Workflow

1. **Retrieve before proposing.** `list_requirements` (filter by `status`, `owner`, `state`,
   `gap`) to see what exists; `get_requirement` for the ones you touch. Read the source and the
   existing neighbours first. Ask for one requirement, not the whole baseline.
2. **One observable obligation per requirement:**
   *responsible component* + **shall** + *behaviour* + *condition*.
   "The tool gateway shall deny operations outside the caller identity's capability grant."
   One `shall`. A component before it. Something a test can observe.
3. **Keep rationale and implementation choices out of the statement.** Put *why* in
   `rationale`. Put *how* in the control, not the requirement.
4. **Give it an identity and an owner.** Stable `id` (never reuse one), `source` (a locator
   into the source obligation), `source_kind` (`binding_obligation`, `organisational_policy` or
   `guidance`: keep them apart), `owner`, `verification_id`, and a measurable `acceptance`.
5. **Check, deterministically first.** `propose_change` validates the nine profile fields and
   lints the statement (missing or repeated `shall`, no responsible component, vague terms such
   as "TBD", "and/or", "as appropriate"). `trace_requirement` resolves the links at the current
   model revision and names every dangling one. Fix what these report before anything else.
6. **Then use judgment** for what a scan cannot know: duplicates, contradictions with
   neighbouring requirements, undefined terms, ambiguity, whether the behaviour is really
   observable. Quote the words you object to and offer a rewrite.
7. **Propose a diff; validate before committing.** `propose_change` returns the diff and the
   **base revision** it was computed against, and writes nothing. `validate_change` says whether
   that base is still current. `commit_change(change_id, expected_revision)` applies it only if
   the model head still *is* that revision; if it is not you get `stale_base`, nothing is
   written, and you must re-read and propose again. Never retry a stale commit unchanged.
8. **After implementing, run the case.** `run_verification(case_id, revision)` stores evidence
   bound to the three revisions. Then `get_requirement` again and report the new state.

## Rules that keep you honest

- **Permissions are enforced by the gateway and the sandbox, not by this skill.** If a call is
  refused (`403`), that is the answer: do not look for another route to the same effect, and do
  not describe a refused change as made.
- **A passing result is only evidence for the revisions it ran at.** If `revisions` in a reply
  differ from the ones on the evidence, say the result is stale.
- **Zero tests run is not a pass.** `run_verification` reports it as an error. Believe it.
- **Do not invent data.** An unknown `source`, `owner` or acceptance is a question for the
  requirement's owner, not a field to fill with a plausible value.
- **An empty `implementation` on a control is a real gap**, not a formatting problem.

## References

- `references/profile.md`: the nine requirement fields, the evidence fields, edge directions.
- `references/statement-patterns.md`: good and bad statements, and what the lint can and cannot see.
- `scripts/check.sh`: runs the same deterministic gate from a shell (`just assurance-lint`).
