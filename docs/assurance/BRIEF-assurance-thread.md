# Brief: versioned assurance thread (owner brief, 2026-10-02)

This is the source obligation `OBL-OWNER-BRIEF` in `kr0ki.assurance.toml`. Section anchors
`#KR-A01` … `#KR-A08` are the locators the baseline's `derives` edges point at. It records
the brief as given; it is not a verification result. The repository README's claims
(ReqIF import/export, requirements views, OMG model API client and sync path, MCP tools, Vue
Playb00k) are repository declarations, not independently verified runtime results, and live
write-path validation against a real model server remains outstanding.

## Goal

Build a **versioned assurance thread**:

```text
source obligation → requirement → system element → enforcing control → verification case → evidence
```

Use kr0ki's own requirements and agent workflow as the first working example. Extend the
existing surfaces. kr0ki consumes the upstream model; it renders and exposes views.

Benefits sought from the requirements agent (reqiffy): reusable requirements authoring,
exchange, visible relationships and implementation gaps, reproducible assurance, and smaller
agent context through targeted model queries.

## Model contract

A minimal application profile over ReqIF and SysML v2. Standard elements are kept distinct
from application metadata.

| Concept | ReqIF representation | SysML v2 oriented representation |
|---|---|---|
| Source obligation | Typed SpecObject, source URI/version | Referenced source metadata |
| Engineering requirement | SpecObject with stable identifier and typed attributes | Requirement definition/usage |
| Architecture | External element reference | Parts, ports, connections, constraints |
| Traceability | Typed SpecRelation | Derivation, satisfaction and verification relationships |
| Acceptance test | Verification reference attributes | Verification case |
| Evidence | URI, digest, result and revision attributes | Application evidence record linked to verification |

**Required requirement fields:** `id`, `statement`, `source`, `source_kind`, `owner`,
`rationale`, `verification_id`, `acceptance`, `status`. Guidance, organisational policy and
binding obligations stay distinguishable (`source_kind`).

**Evidence fields:** `requirement_id`, `verification_id`, `model_revision`,
`implementation_revision`, `configuration_digest`, `result`, `artifact_uri`,
`artifact_digest`.

A **satisfaction relationship is an architectural assertion**; a **passing result is
revision-specific evidence**.

### Evidence is revision-bound: re-verify on a minor release, not a patch

Evidence records carry `model_revision` and `implementation_revision`, so a pass proves
only the revision it ran against; any new commit makes it stale for display.

Release policy: run `just assurance-verify` (then `just assurance-view`) **when the
minor version bumps** (0.1.x -> 0.2.0) and the evidence is promoted for that release. A
patch bump (0.1.1 -> 0.1.2) does not trigger a new assurance run; it keeps the evidence
of the minor it belongs to. Re-run earlier only when a change touches a verified
requirement's code, model, or configuration digest.

## Runtime parts

`AgentWorkload`, `WorkloadIdentity`, `ToolGateway`, `ExecutionSandbox`, `ModelService`,
`VerificationRunner`, `EvidenceStore`. Each requirement is assigned to its responsible part and
verification case. Sandbox and identity enforcement stay in their deployment components;
kr0ki displays their relationships and evidence.

(The model, `kr0ki-assurance.sysml`, adds one sub-part, `ToolGateway::AuditRecorder`, because
KR-A08 names an "audit service" that is not among the seven parts. That is a modelling choice
to confirm with the owner.)

## Requirements and progressive dogfooding

Each row becomes one requirement object and one acceptance case. Implement in order; run every
completed stage against this brief's own requirement baseline.

### KR-A01

The requirement adapter shall preserve supported requirement identifiers, typed attributes and
asserted relations across ReqIF export and reimport.

*Acceptance:* round-trip this baseline; compare semantic content. Report unsupported content
explicitly.

### KR-A02

The traceability adapter shall resolve requirement links against a specified model revision.

*Acceptance:* link KR-A01 to its adapter and test; report a deliberately dangling reference.

### KR-A03

The requirements view shall distinguish satisfaction assertions from verification results.

*Acceptance:* show one satisfied but untested requirement and one verified requirement.

### KR-A04

The verification runner shall bind each result to the model, implementation and configuration
revisions used.

*Acceptance:* run KR-A01's case; retrieve its evidence. Change a revision and display the
previous result as stale.

### KR-A05

The model commit service shall reject a proposed mutation whose expected base revision differs
from the current revision.

*Acceptance:* submit two edits from one baseline; reject the second stale edit without losing
the first.

### KR-A06

The tool gateway shall deny operations outside the caller identity's capability grant.

*Acceptance:* permit model reads; deny a commit from the same read-only identity through MCP
and direct HTTP.

### KR-A07

The workload sandbox shall deny writes outside its declared writable mounts.

*Acceptance:* permit temporary output; deny a write to the mounted requirement baseline.

### KR-A08

The audit service shall record each tool invocation with caller, operation, decision, model
revision and correlation identifier.

*Acceptance:* retrieve records for both the permitted and denied KR-A06 calls.

### Per-increment procedure

For each increment: add the fixture and acceptance case, implement, exercise it through
Playb00k/MCP, attach evidence, then regenerate the requirements view. Use the existing
`just check` and `just test` gates; run relevant live checks before recording a live
integration as verified.

## MCP surfaces

MCP separates resources, tools and prompts: prompts are user-selected workflows, resources
supply context, tools execute operations. The names below are proposed contracts, not claims
about existing tool names; reuse existing tools where equivalent. Return structured results
with identifiers and revision references. Enforce authorisation in the service/gateway for
every transport; tool visibility, descriptions and read-only annotations are insufficient
enforcement.

| Surface | Proposed minimum interface | Purpose |
|---|---|---|
| Resources | Revision-qualified requirements, model elements, evidence and skill text | Retrieve only the needed context |
| Read tools | `get_requirement`, `trace_requirement`, `render_view` | Inspect obligations, implementation and gaps |
| Draft tools | `propose_change`, `validate_change` | Return a diff and diagnostics without committing |
| Commit tool | `commit_change(change_id, expected_revision)` | Apply an authorised, revision-checked change |
| Verification tool | `run_verification(case_id, revision)` | Execute a declared case and return evidence references |
| Prompts | `review_requirement`, `explain_gap`, `implement_next_slice` | Guide repeatable developer workflows |

## Skills

`SKILLS.md` is a short repository index; the portable executable instruction package is a
singular `SKILL.md` with metadata, instructions and optional scripts/references. The
requirements-authoring skill's workflow:

1. Retrieve the source and existing requirements before proposing additions.
2. Write one observable obligation per requirement: responsible component + "shall" +
   behaviour + condition.
3. Separate rationale and implementation choices from the statement.
4. Assign a stable ID, source, owner and measurable acceptance case.
5. Check duplicates, contradictions, missing links and undefined terms.
6. Produce a proposed diff; validate before committing.
7. After implementation, run the linked case and attach revision-bound evidence.

Use deterministic checks for schema, identifiers and links; use the agent to flag ambiguity and
suggest improvements. Skill instructions guide authoring; the gateway and sandbox enforce
permissions.

## References

- OMG ReqIF 1.2, formal/16-07-01 — <https://www.omg.org/spec/ReqIF/1.2>
- OMG Systems Modeling API and Services 1.0, formal/26-03-04 —
  <https://www.omg.org/spec/SystemsModelingAPI/1.0>
- Model Context Protocol specification 2026-07-28 —
  <https://modelcontextprotocol.io/specification/2026-07-28>
- Agent Skills specification — <https://agentskills.io/specification>
- NASA Systems Engineering Handbook, Appendix C "How to Write a Good Requirement" —
  <https://www.nasa.gov/reference/appendix-c-how-to-write-a-good-requirement/>
  (C.1: "shall" for requirements, "will" for facts, "should" for goals; C.3: rationale; C.4:
  clarity, completeness, verifiability.)
