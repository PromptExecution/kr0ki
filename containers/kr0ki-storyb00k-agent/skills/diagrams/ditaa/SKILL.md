---
name: kr0ki-ditaa
description: Use before writing or fixing ditaa ASCII-art diagrams for render_diagram (format ditaa): column alignment, colour and shape tags, rounded corners, worked examples, identifier rule.
---
# ditaa (render_diagram format `ditaa`)

## Rules (each checked against the renderer)
- The source is plain ASCII art; it never raises syntax errors (only an empty source returns 400 `empty_source`). Mistakes show up as a wrong drawing, so check box geometry carefully.
- Box edges must line up column-for-column: `+---+ / | A | / +--+` (bottom shorter than top) renders only an open outline, no filled box. Make every row of a box the same width.
- Text rows inside a box must keep the same width as the border; a 4-character interior (`|cGRE|`) inside a 3-dash `+---+` box also breaks the box.
- Colour tag: first text in the box, `c` + 3 hex digits or a name: `cRED` fills `#ee3322`, `cBLU` `#5555bb`, `cGRE` `#99dd99`, `cPNK` `#ffaaaa`, `c1AB` `#11aabb`, `cF80` `#ff8800`. The tag is removed from the visible label.
- Shape tags are also removed from the label: `{d}` draws a document (wavy bottom), `{s}` a storage cylinder; `{io}`, `{c}`, `{mo}`, `{tr}` are accepted too.
- `/---\` over `\---/` makes rounded corners; `+` corners make square ones. `:` makes a dashed vertical line; `v`, `^` under a `|` draw arrowheads.

## Identifiers
This language cannot separate a key from a label: the text you draw is the only name. Draw the display name, put the identifier in the text only when the reader needs it (for example `n0002 engine`). Never invent identifiers.

## Verified examples (all render)
### two boxes joined by a vertical arrow
```ditaa
+-----+
| A   |
+--+--+
   |
   v
+--+--+
| B   |
+-----+
```
### colour tags on boxes
```ditaa
+-------+
|cBLU   |
| Web   |
+---+---+
    |
    v
+---+---+
|cRED   |
| DB    |
+-------+
```
### rounded boxes
```ditaa
/-------\
| Rounded |
\-------/
```
### document and storage shapes
```ditaa
+-------+  +-------+
|{d}    |  |{s}    |
| Report|  | Store |
|       |  |       |
+-------+  +-------+
```
### dashed vertical connector
```ditaa
+-----+
| A   |
+--+--+
   :
   :
   v
+--+--+
| B   |
+-----+
```
