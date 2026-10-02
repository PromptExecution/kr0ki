---
name: cost-attribution-and-compliance
description: Attach accounting attribution codes and AI-governance tags to requirements without turning them into dollars or legal claims.
confidence: recalled for framework details (see docs/evaluations/AI-GOVERNANCE-crosswalk-2026-10-01.md for each framework's confidence)
---
# Skill: Cost attribution codes and compliance tags

## Attribution
- A requirement is charged to an accounting code, not an amount: `@CostAttribution { code = 'CC-4410/WBS-2.3'; share = 0.6; }`.
- Codes are hierarchical (`/`); shares on one requirement should sum to at most 1. `/sparql/shapes` reports `over-allocated`; `/sparql/rollup` sums by code and prefix.
- How a code becomes a budget is deliberately not modelled yet. Never convert a share to money.

## Compliance tags
- `@ComplianceTag { tag = 'au-ai6:P5'; basis = 'judgement'; }` links a requirement to a control in the crosswalk. `basis` says how sure we are: say "judgement" unless an assessor confirmed it.
- Order of relevance for this organisation: EU AI Act, Australian Guidance for AI Adoption, ISO/IEC 42001, NIST AI RMF, AESCSF/CIRMP.
- A tag shows intent to trace, not compliance. Do not tell a user a system "complies".

## When unsure
Quote the crosswalk's confidence note (confirmed vs recalled) and recommend checking the primary source before relying on a control reference.
