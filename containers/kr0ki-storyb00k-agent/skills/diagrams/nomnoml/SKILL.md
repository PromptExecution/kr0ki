---
name: kr0ki-nomnoml
description: Use before writing or fixing nomnoml UML diagrams for render_diagram (format nomnoml): node and association syntax, label placement, compartments, type tags, error meanings, worked examples, identifier rule.
---
# nomnoml (render_diagram format `nomnoml`)

## Rules (each checked against the renderer)
- Nodes are `[Name]`; an association is `[A]->[B]` (also `--`, `-->`, `<:--` inheritance, `+->` composition, `o->` aggregation, `-:>` dependency: all render). Reusing a name reuses the node.
- A relation label goes between the arrow parts, before the target: `[A] uses -> [B]` or `[A]uses->[B]`. `[A]->[B]: label` fails: `Parse error ... expected "--" or "-/-" or "-" but got end of file`. Multiplicities go on each end: `[A] 1 -> 0..* [B]`.
- Two nodes on one line with no association fail: `Parse error at line 1 column 9, expected "label" but got "["`. Put each association on its own line (or separate with a newline).
- An unclosed bracket fails: `Parse error at line 1 column 7, expected "]" but got end of file`.
- Without brackets the text is NOT a diagram: `A->B` renders as the literal text `A->B`, no error.
- Compartments are separated by `|`, members by `;` or newline: `[Foo|+a;+b|+run()]`. Type tags: `<abstract>`, `<actor>`, `<usecase>`, `<database>`, `<frame>`, `<note>`, `<start>`, `<end>`, `<table>` all render. Nodes can be nested: `[<frame>F|[Inner]]`.
- `#direction: right` changes layout (verified wider than tall); unknown directives such as `#bogus: 1` are silently ignored.

## Identifiers
Use the identifier as the node name and put the display name in a compartment, e.g. `[n0002|engine]` ; namespace-style ids such as `public.orders` and `ns/kind/name` are accepted inside brackets. Never invent identifiers.

## Verified examples (all render)
### class with compartments and abstract tag
```nomnoml
[<abstract>Foo|+name: String;+age: int|+run();+stop()]
```
### association label and multiplicities
```nomnoml
[Customer] 1 -> 0..* [Order]
[Order] contains -> [Item]
```
### actor and use case
```nomnoml
[<actor>User]->[<usecase>Login]
[<usecase>Login]-:>[<usecase>Audit]
```
### direction and database node
```nomnoml
#direction: right
[<frame>App|[Api]->[Worker]]
[Worker]->[<database>DB]
```
### inheritance, aggregation and note
```nomnoml
[Animal]<:--[Dog]
[Dog]+->[Leg]
[Dog]--[<note>barks]
```
