---
name: kr0ki-d2
description: Use before writing or fixing D2 source for render_diagram (format d2): verified syntax rules, quoting/escaping gotchas, worked examples, identifier rule.
---
# d2 (render_diagram format `d2`)

## Rules (each checked against the renderer, 2026-10-01)
- Layout: one statement per line. `a -> b: label` is a connection; `a: Label` sets a label; `a; b` and `a -> b; b -> c` put several on one line.
- **Quote labels that contain `$`**: an unquoted `$` starts a variable substitution and fails (`substitutions must begin on {`). Use single quotes: `a: 'costs $5'`. `${name}` must refer to a variable defined under `vars`, otherwise: `could not resolve variable`.
- A `.` in a key means nesting (`a.b: hi` creates `b` inside `a`). To get a literal dot in a key, quote it: `"a.b": hi`.
- Containers: `cloud: { api; db; api -> db }`. Reach inside from outside with dotted paths: `x.a -> x.b`.
- Shapes: `db: DB { shape: cylinder }`, `u: User { shape: person }`, `t: { shape: sql_table; id: int {constraint: primary_key} }`, `s: { shape: sequence_diagram; a -> b: hi }`.
- **Valid shapes** (verified): `rectangle square circle oval diamond hexagon cylinder queue package page document step callout stored_data parallelogram person cloud text code`. Anything else is rejected (`unknown shape "browser"`). `shape: image` needs an `icon:` field.
- **Fill and stroke live under `style.`**: `style.fill: red` works; a bare `fill: red` fails (`fill must be style.fill`).
- Styling: `style.fill`, `style.stroke` (quote hex colours: `'#ffeeee'`); reuse with `classes: { box: { style.fill: lightblue } }` then `a.class: box`.
- Multi-line or markdown labels use a block: `n: |md ... |`.
- If a render fails, read the line:column in the error; D2 reports `unexpected text after double quoted string` / `missing value after colon` for broken quoting or a stray colon in a style value.

## Identifiers
When an identifier exists (a SysML v2 element id, `schema.table`, `namespace/kind/name`), use it as the node **key** and put the display name in the **label**: `"00000000-0000-4000-8000-000000000002": engine`. Never invent identifiers.

## Verified examples (all render)
### nested_containers
```d2
parent {
  child1: "First"
  child2: "Second"
}

parent.child1 -> parent.child2
```
### styling_shapes
```d2
db {
  shape: cylinder
  label: "Database"
}

user {
  shape: person
  label: "Alice"
}

user -> db
```
### connection_labels
```d2
A -> B: "request"
B -> A: "response"
```
### escaping_quotes
```d2
"O'Brien" -> "It's done"

"She said \"hello\"" -> "Response"
```
### multiline_labels
```d2
box {
  label: "Line 1\nLine 2\nLine 3"
}
```
