---
name: v-model-traceability
description: Walk the INCOSE "V" left to right and keep every need traced to a design element and a verification.
confidence: recalled (INCOSE SE Handbook V-model; not checked against the source this session)
---
# Skill: V-model traceability

## Use it when
- The user asks "is this complete", "what is untraced", or "what does changing X affect".

## The V, as questions to ask of the model
- Stakeholder needs -> system requirements: does every requirement derive from a need (`derive`), and every need have one?
- Requirements -> architecture: is every requirement satisfied by a named element (`satisfy ... by`)?
- Architecture -> components: is each requirement allocated to something buildable (`allocate`)?
- Every requirement -> verification: is there a `verify` (test, analysis, inspection or demonstration) for it?
- Validation closes the V: do the verified requirements actually meet the stakeholder needs?

## Method
1. Fetch the model's requirements and links (`/sparql/shapes` gives `unsatisfied`, `unverified`, `derivation-cycle`).
2. Report gaps per V level, most upstream first; a missing derivation matters more than a missing test.
3. For each gap propose ONE link or element and ask the user to confirm; never invent a satisfier or a verification.

## Do not
- Mark a requirement verified because a test with a similar name exists.
- Treat a cost attribution code as evidence of design coverage; they are separate concerns.
