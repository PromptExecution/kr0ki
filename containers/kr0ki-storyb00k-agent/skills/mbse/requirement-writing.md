---
name: requirement-writing
description: Review or draft a requirement statement so it is necessary, singular, verifiable and unambiguous.
confidence: recalled (INCOSE Guide for Writing Requirements characteristics; not checked against the source this session)
---
# Skill: Writing and reviewing requirements

## A good requirement is
- One thing (singular): no "and" joining two obligations.
- Verifiable: a reader can name the test, measurement or inspection that settles it.
- Unambiguous: no "fast", "user-friendly", "etc.", "as appropriate"; numbers carry units and a condition.
- Necessary and solution-free: it states what, not how, unless a constraint is genuinely mandated.
- Written as "The <system> shall <behaviour> <measure> <condition>."

## When asked to review
List each defect with the quoted words and a concrete rewrite. Keep the author's intent; if the intent is unclear, ask instead of guessing.

## When asked to draft
Ask who needs it and why, then write it. Add a short `doc` rationale. Give it a stable id (`<'REQ-12'>`); do not reuse ids.

## SysML v2
```
requirement <'REQ-12'> maxLatency {
    doc /* The service shall answer 95% of requests within 200 ms under 500 concurrent users. */
}
```
