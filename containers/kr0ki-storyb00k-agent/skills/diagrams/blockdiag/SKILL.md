---
name: kr0ki-blockdiag
description: Use before writing or fixing blockdiag block diagrams for render_diagram (format blockdiag): rules checked against the renderer, worked examples, identifier rule.
---
# blockdiag (render_diagram format `blockdiag`)

## Rules (each checked against the renderer)
- Wrapper is required: `blockdiag { ... }`. Bare `A -> B` fails: `got unexpected token: 'A', expected: '{'`.
- **Put spaces around arrows**: `A->B` fails (`Got unexpected token at line 1 column 15`); `A -> B` renders. Edges chain (`A -> B -> C`), fan out (`A -> B, C`), and `<->`, `--` work; `=>` fails.
- Attribute lists need commas and quotes around text with spaces: `[label=Hello World]` fails (`got unexpected token: 'World', expected: ']'`), `[label="x" color=red]` fails, a trailing comma fails. Use `[label = "Hello World", color = "#ffcccc"]`.
- Unknown attributes are rejected: `Unknown attribute: DiagramNode.foo`. Bad colours too: `"nocolor" is not defined as a named color in css3`.
- Shapes: `box`, `roundedbox`, `ellipse`, `diamond`, `square`, `note`, `mail`, `cloud`, `actor`, `beginpoint`, `minidiamond`, `flowchart.database`, `flowchart.condition`, `flowchart.input`, `flowchart.terminator` render. **`database`, `cylinder` fail (`unknown node shape`), `flowchart.document` fails too.**
- `group name { label = "Core"; color = "#DDEEFF"; A; B; }` boxes nodes together. `\n` in a label makes a line break. Edge options that work: `label`, `color`, `style = dashed`, `dir = both`, `thick`, `folded`.
- Names with spaces need double quotes: `"my node" -> "other node"`. Comments `//`, `#`, `/* */` are fine.

## Identifiers
Node names are the key and `label` is the display text: `n0002 [label = "engine"];` then use `n0002` in edges. Never invent identifiers.

## Verified examples (all render)
### flowchart shapes and edge labels
```blockdiag
blockdiag {
  start [shape = beginpoint];
  check [shape = flowchart.condition, label = "ok?"];
  store [shape = flowchart.database, label = "DB"];
  done [shape = flowchart.terminator];
  start -> check;
  check -> store [label = "yes", color = red];
  check -> done [label = "no", style = dashed];
  store -> done;
}
```
### group with colour
```blockdiag
blockdiag {
  A -> B -> C;
  A -> D;
  group g1 {
    label = "Core";
    color = "#DDEEFF";
    A; B;
  }
}
```
### id as key, name as label
```blockdiag
blockdiag {
  n0002 [label = "engine"];
  n0003 [label = "wheel", color = "#ffcccc"];
  n0002 -> n0003 [label = "drives"];
}
```
### fan-out and bidirectional
```blockdiag
blockdiag {
  orientation = portrait;
  api -> web, mobile;
  web <-> cache;
  mobile -> cache [dir = both, style = dotted];
}
```
### stacked and numbered nodes
```blockdiag
blockdiag {
  A [stacked, label = "workers"];
  B [numbered = 1, shape = roundedbox];
  "my node" [shape = actor];
  A -> B -> "my node";
}
```
