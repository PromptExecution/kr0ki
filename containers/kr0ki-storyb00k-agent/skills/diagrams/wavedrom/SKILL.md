---
name: kr0ki-wavedrom
description: Use before writing or fixing WaveDrom timing diagrams for render_diagram (format wavedrom): WaveJSON syntax, silent failure modes, groups, edges, register bitfields, worked examples, identifier rule.
---
# wavedrom (render_diagram format `wavedrom`)

## Rules (each checked against the renderer)
- Source is WaveJSON (JSON5): unquoted keys, single quotes and trailing commas are all accepted, as is strict JSON. Values must still be quoted strings: `name: clk` fails `JSON5: invalid character 'c'`.
- The whole source must be one object: `signal: [...]` without braces fails `invalid character 's' at 1:1`; trailing text after `}` fails `invalid character 'e'`.
- Unknown top-level keys are SILENT: `{ foo: 1 }` returns HTTP 200 with just `<div class="WaveDrom"/>` (no SVG). Use `signal` (timing) or `reg` (register bitfield).
- Unknown wave characters are silent too: `wave: "q...."` renders the undefined (`x`) symbol instead of failing. Check wave strings against the WaveDrom character set; `.` repeats the previous state (used in all examples below).
- `{ signal: [] }` and `{ signal: "x" }` render an empty chart without error. Groups are nested arrays starting with the title (`["bus", {...}, {...}]`); `{}` is a blank spacer row.
- `{ reg: [ { bits: 8, name: "data" }, ... ] }` renders a register bitfield (bit numbers 0, 7, 8, 15 appear in the SVG).

## Identifiers
This language cannot separate a key from a label: the text you draw is the only name. Draw the display name, put the identifier in the signal `name` only when the reader needs it (for example `{ name: "n0002 engine", wave: "01.0" }`). Never invent identifiers.

## Verified examples (all render)
### clock and data bus
```wavedrom
{ signal: [ { name: "clk", wave: "p......" }, { name: "dat", wave: "x.345x.", data: ["head","body","tail"] }, { name: "req", wave: "01.0..." } ] }
```
### signal group with spacer and phase
```wavedrom
{ signal: [ ["bus", { name: "a", wave: "01.0" }, { name: "b", wave: "10.1" }], {}, { name: "c", wave: "0..1", phase: 0.5 } ] }
```
### head, foot and horizontal scale
```wavedrom
{ signal: [ { name: "a", wave: "01.0" } ], head: { text: "Timing", tock: 0 }, foot: { text: "end" }, config: { hscale: 2 } }
```
### edges between nodes
```wavedrom
{ signal: [ { name: "a", wave: "01..0", node: ".a..b" } ], edge: ["a~>b delay"] }
```
### register bitfield
```wavedrom
{ reg: [ { bits: 8, name: "data" }, { bits: 8, name: "addr" } ] }
```
