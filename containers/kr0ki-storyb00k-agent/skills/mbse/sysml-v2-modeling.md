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

## Do not
- Invent library types. If you are unsure a type exists, model it as a local `part def`.
- Present unvalidated text as correct.
