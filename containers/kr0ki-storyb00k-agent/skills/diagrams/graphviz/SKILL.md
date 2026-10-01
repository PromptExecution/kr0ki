---
name: kr0ki-graphviz
description: Use before writing or fixing Graphviz DOT source for render_diagram (format graphviz): verified syntax rules, record/port escaping, clusters, worked examples, identifier rule.
---
# graphviz (render_diagram format `graphviz`)

## Rules (each checked against the renderer)
- Source is a `digraph name { ... }` (directed, `->`) or `graph name { ... }` (undirected, `--`). Statements end with `;` (optional but safest).
- Quote any identifier or label containing spaces or punctuation: `"Auth Service" -> "User DB";`.
- `record` shapes: fields are separated by `|`, nested fields use `{ }`, ports use `<port>`; **escape literal `{ } | < >` with a backslash** inside a record label. Edges can target a port: `a:p1 -> b:p2`.
- Group nodes with `subgraph cluster_x { label="..."; ... }` (the name must start with `cluster`). Force a row with `{ rank=same; a; b; }`.
- Defaults: `node [shape=box, style=filled, fillcolor="#eef"]; edge [color=gray];` before the nodes they apply to.
- HTML-like labels use `<...>` instead of quotes: `a [label=<<b>bold</b><br/>line two>];`.

## Identifiers
When an identifier exists, use it as the node id and the display name as the label: `"00000000-...02" [label="engine"];`. Never invent identifiers.

## Verified examples (all render)
### record-ports
```graphviz
digraph { A [shape=record, label="{top|f|bot}"]; B [shape=record, label="{left|f|right}"]; A:f -> B:f; }
```
### html-labels
```graphviz
digraph { A [label="<b>Bold</b><br/>New line"]; B [label="<table border=1><tr><td>Cell</td></tr></table>"]; A -> B; }
```
### clusters
```graphviz
digraph { subgraph cluster_0 { label="Group"; A; B; } C; A -> C; B -> C; }
```
### rank-same
```graphviz
digraph { { rank=same; A; B; } C; A -> C; B -> C; }
```
### edge-ports
```graphviz
digraph { A [shape=box]; B [shape=box]; A:s -> B:n; }
```
### quoted-ids
```graphviz
digraph { "Node A" -> "Node B"; "A B" [shape=ellipse]; }
```
### default-attrs
```graphviz
digraph { node [shape=box, style=filled, color=lightblue]; edge [color=red]; A -> B -> C; }
```
### escape-chars
```graphviz
digraph { A [shape=record, label="{Header|Item1|Item2}"]; B [label="A < B > C"]; A -> B; }
```
