---
name: kr0ki-seqdiag
description: Use before writing or fixing seqdiag sequence diagrams for render_diagram (format seqdiag): rules checked against the renderer, worked examples, identifier rule.
---
# seqdiag (render_diagram format `seqdiag`)

## Rules (each checked against the renderer)
- Wrapper `seqdiag { ... }`. Participants appear in first-mention order. `A -> B;` is a call, `B --> A;` is a dashed reply, `<-`, `->>`, `<<-`, `-->>` and `=>` all render. `A -x B` fails (`Got unexpected token`).
- **Put spaces around arrows**: `A->B` fails (`Got unexpected token at line 1 column 13`).
- Edge text goes in brackets, quoted: `A -> B [label = "req"];`. `[label=two words]` fails (`got unexpected token: 'words', expected: ']'`). Unknown attributes fail: `Unknown attribute: DiagramEdge.foo`, same for nodes; `color=nope` fails (`not defined as a named color in css3`).
- Working edge options: `label`, `note`, `leftnote`, `rightnote`, `color`, `style = dotted|dashed`, `thick`, `failed`, `diagonal`, `folded`, `return = "done"`. Self-calls `A -> A;` work.
- Nested calls use a brace body: `A -> B { B -> C; }` (activation boxes). Separators `=== text ===;` and `... delay ...;` work between messages.
- Rename participants with `A [label = "Alice", color = "#fcc"];` before the messages. Diagram options such as `autonumber = True;` and `edge_length = 300;` work.
- Mermaid syntax (`sequenceDiagram`) is rejected with `Got unexpected token`.

## Identifiers
Participant names are the key and `label` is the display text: `n0002 [label = "engine"];` then use `n0002` in edges. Declare participants first: `n0002 [label = "engine"];`. Never invent identifiers.

## Verified examples (all render)
### request/response with labels
```seqdiag
seqdiag {
  browser -> server [label = "GET /"];
  server --> browser [label = "200 OK", color = "#2a7"];
  browser -> server [label = "POST /x", failed];
}
```
### nested calls (activation)
```seqdiag
seqdiag {
  client -> api [label = "order"] {
    api -> db [label = "insert"];
    api -> mail [label = "notify"];
  }
  api --> client [label = "id"];
}
```
### notes, separators, delay
```seqdiag
seqdiag {
  autonumber = True;
  A -> B [label = "hello", note = "first"];
  === phase two ===;
  B -> C [leftnote = "left", rightnote = "right"];
  ... wait ...;
  C -> A [label = "done", style = dotted];
}
```
### id as key, name as label
```seqdiag
seqdiag {
  n0002 [label = "engine", color = "#fcc"];
  n0003 [label = "gearbox"];
  n0002 -> n0003 [label = "torque"];
  n0003 --> n0002 [label = "ack"];
  n0002 -> n0002 [label = "self"];
}
```
### diagonal edge and arrow forms
```seqdiag
seqdiag {
  A -> B [label = "slow", diagonal];
  B ->> C [label = "async"];
  C <<- B;
  A <- C [label = "back"];
}
```
