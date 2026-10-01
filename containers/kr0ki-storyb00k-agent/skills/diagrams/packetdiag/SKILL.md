---
name: kr0ki-packetdiag
description: Use before writing or fixing packetdiag packet/bit-field diagrams for render_diagram (format packetdiag): rules checked against the renderer, worked examples, identifier rule.
---
# packetdiag (render_diagram format `packetdiag`)

## Rules (each checked against the renderer)
- Wrapper `packetdiag { ... }`. Each field is `first-last: Label;` using bit numbers (`0-15: Source Port;`) or a single bit `0: Flag;`. A missing colon fails (`got unexpected token: 'A', expected: ':'`).
- Overlapping ranges fail: `Field 'B' is conflicted to other field`. Gaps between ranges are allowed.
- Text with spaces needs no quotes (`0-15: Source Port;`); quotes also work. A bad attribute fails: `Unknown attribute: FieldItem.colspan`. `shape=cylinder` fails (`unknown node shape`).
- Field attributes go AFTER the label in brackets: `106-111: Flags [color = "#ffcccc"];`, `[rotate = 270]`, `[height = 80]`, `[linecolor = "#f00"]`. Putting them before the label fails (`expected: '='`).
- Diagram options go first: `colwidth = 32;` (bits per row), `node_height = 72;`, `scale_direction = rtl;`, `scale_interval = 4;`.
- There are no edges or nodes; a field has a position, not a key.

## Identifiers
packetdiag fields are positional (`0-15: Label;`) and have no key, so there is no place for an identifier; the label text is the only name, so put the human-readable name there (optionally prefixed: `0-15: n0002 engine;`). Never invent identifiers.

## Verified examples (all render)
### TCP header
```packetdiag
packetdiag {
  colwidth = 32;
  node_height = 40;
  0-15: Source Port;
  16-31: Destination Port;
  32-63: Sequence Number;
  64-95: Acknowledgment Number;
  96-99: Offset [rotate = 270];
  100-105: Reserved;
  106-111: Flags [color = "#ffcccc"];
  112-127: Window;
}
```
### single-bit flags
```packetdiag
packetdiag {
  colwidth = 8;
  0: URG;
  1: ACK;
  2: PSH;
  3: RST;
  4: SYN;
  5: FIN;
  6-7: Spare [color = "#dddddd"];
}
```
### 16-bit rows with a gap
```packetdiag
packetdiag {
  colwidth = 16;
  0-7: Type;
  8-15: Code;
  16-31: Checksum [height = 60];
  48-63: Payload;
}
```
### right-to-left scale
```packetdiag
packetdiag {
  scale_direction = rtl;
  0-3: Version;
  4-7: IHL;
  8-15: ToS;
  16-31: "Total Length";
}
```
