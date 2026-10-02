# The assurance profile

## Requirement fields (all required)

| field | meaning |
|---|---|
| `id` | stable identifier, e.g. `KR-A01`; never reused |
| `statement` | one observable obligation: component + shall + behaviour + condition |
| `source` | locator into the source obligation (a URI, `file#anchor`, or clause) |
| `source_kind` | `binding_obligation` \| `organisational_policy` \| `guidance` |
| `owner` | who answers questions about it |
| `rationale` | why it exists (not part of the statement) |
| `verification_id` | the verification case that accepts it, e.g. `VC-A01` |
| `acceptance` | the measurable acceptance criterion |
| `status` | `draft` → `review` → `validated` → `gated` → `implemented` |

## Evidence fields

`requirement_id`, `verification_id`, `model_revision`, `implementation_revision`,
`configuration_digest`, `result` (`pass` / `fail` / `error`), `artifact_uri`, `artifact_digest`.

`configuration_digest` digests what the case checks: its command, its acceptance, the statement
and acceptance of every requirement it verifies, and the toolchain. Edit a verified
requirement's statement and its earlier evidence goes stale.

## Edge directions (read `source —kind→ target`)

| edge | meaning |
|---|---|
| `source_obligation —derives→ requirement` | the requirement is derived from the obligation |
| `system_element —satisfies→ requirement` | **assertion** that the design satisfies it |
| `control —implements→ requirement` | the control enforces it |
| `control —allocated_to→ system_element` | where the control lives |
| `verification_case —verifies→ requirement` | the requirement's acceptance case |

Only *asserted* edges count. Inferred and proposed edges are review-only.

## Assurance states

`unsatisfied` (nothing asserts it) · `satisfied_untested` (asserted, never run) · `verified`
(a fresh pass, no fresh failure) · `failing` (a fresh failure or error) · `stale` (results exist
only for other revisions).

## Gaps the tools report

`no_source`, `unsatisfied`, `no_enforcing_control`, `control_not_implemented`,
`no_verification_case`, `unknown_verification_case`, `satisfied_untested`, `failing`, `stale`,
`dangling_element`.
