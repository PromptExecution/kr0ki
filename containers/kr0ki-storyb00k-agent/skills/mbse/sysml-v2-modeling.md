---
name: sysml-v2-modeling
description: Write textual SysML v2 for requirements, parts and the links between them, and validate it with the real parser.
confidence: recalled (SysML v2 textual notation; always confirm with /sysml/validate)
---
# Skill: SysML v2 textual modeling

## Core idioms
- `package P { ... }` groups elements. `part def` is a type; `part x : T;` is a usage.
- `requirement def` / `requirement <'ID'> name { doc /* ... */ }`.
- Links: `satisfy <req> by <element>;`, `verify <req>;` inside a verification case, `allocate <a> to <b>;`.
- Names are identifiers; use `<'SHORT'>` for the stable short id and a readable name after it.

## Always
1. Draft the smallest model that answers the question.
2. Run `/sysml/validate` (the Validate button in Projects). Fix every syntax error before presenting; semantic notes such as missing documentation are advice.
3. Say what the validator cannot tell you: the sidecar does not surface `satisfy` relations or short names, and does not parse pure KerML.

## Reserved words are not names
SysML v2 keywords cannot be used as element names. Verified against the validator (2026-10-07): `part render : T;` and `attribute subject : String;` fail with
`no viable alternative at input 'partrender'` / `'attributesubject'`. The message joins the two tokens without a space (that is how the parser prints it): it
does NOT mean whitespace was lost. Read it as "the name after this keyword is reserved". Rename (`renderer`, `subj`) or quote the name: `'render'`.
Also verified as rejected names: `view`, `viewpoint`, `verify`, `state`, `action`, `item`, `port`, `flow`. Quoting (`'render'`) is accepted.

## Do not
- Invent library types. If you are unsure a type exists, model it as a local `part def`.
- Present unvalidated text as correct.
