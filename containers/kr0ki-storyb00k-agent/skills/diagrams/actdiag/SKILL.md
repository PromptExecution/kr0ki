---
name: kr0ki-actdiag
description: Use before writing or fixing actdiag activity diagrams for render_diagram (format actdiag): rules checked against the renderer, worked examples, identifier rule.
---
# actdiag (render_diagram format `actdiag`)

## Rules (each checked against the renderer)
- Wrapper `actdiag { ... }`. Swimlanes are `lane name { ... }` holding node names; edges go outside lanes (or inside) and use spaced arrows: `A->B` fails (`Got unexpected token at line 1 column 13`), `A -> B` works.
- **A node may belong to only one lane.** Listing it in two lanes fails with an unhelpful dump: `<DiagramNode 'A' ...> <NodeGroup 'x' ...> <NodeGroup 'z' ...>`.
- `group { ... }` is not allowed in actdiag (`got unexpected token: '{', expected: '}'`); use `lane`.
- Lane options: `label = "Web";` and `color = "#eee";` inside the lane block. Quote lane names with spaces: `lane "my lane" { ... }`. An empty lane is accepted.
- Node shapes: `flowchart.database`, `flowchart.condition`, `actor` render; `cylinder` fails (`unknown node shape: cylinder`). Unknown attributes fail (`Unknown attribute: Diagram.lane_height`); `span_height` and `node_height` work.
- Edge options `[label="go"]` and `[style=dashed]` work. Nodes declared only in edges still render (outside any lane).

## Identifiers
Node names are the key and `label` is the display text: `n0002 [label = "engine"];` then use `n0002` in edges. Node attributes go inside the lane: `lane srv { n0002 [label = "check"]; }`. Never invent identifiers.

## Verified examples (all render)
### two lanes
```actdiag
actdiag {
  write -> read -> render;
  lane user {
    label = "User";
    write [label = "Write"];
  }
  lane server {
    label = "Server";
    read; render;
  }
}
```
### decision shape and skip edge
```actdiag
actdiag {
  A -> B -> C;
  A -> C [label = "skip", style = dashed];
  lane x { A; }
  lane y { B [shape = flowchart.condition, label = "ok?"]; }
  lane z { C [shape = flowchart.database]; }
}
```
### lane colour and quoted lane
```actdiag
actdiag {
  span_height = 60;
  lane "front end" {
    color = "#DDEEFF";
    form; submit;
  }
  lane api {
    handler [label = "handle\nrequest"];
  }
  form -> submit -> handler;
}
```
### id as key, name as label
```actdiag
actdiag {
  lane user { label = "User"; n0001 [label = "login"]; }
  lane srv { label = "Server"; n0002 [label = "check"]; n0003 [label = "reply"]; }
  n0001 -> n0002 -> n0003;
}
```
