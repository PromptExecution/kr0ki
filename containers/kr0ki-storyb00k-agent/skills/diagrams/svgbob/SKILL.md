---
name: kr0ki-svgbob
description: Use before writing or fixing svgbob ASCII-art diagrams for render_diagram (format svgbob): box alignment, rounded corners, quoted text, what styling is not supported, worked examples, identifier rule.
---
# svgbob (render_diagram format `svgbob`)

## Rules (each checked against the renderer)
- The source is plain ASCII art; it never raises syntax errors (only an empty source gives 400 `empty_source`). Mistakes show up as a wrong drawing.
- A box is only recognised when every edge lines up: `+---+ / | A | / +--+` produces loose lines and no rectangle. Keep top and bottom the same width.
- `+---+` corners give a square `<rect>`; `.---.` on top with `'---'` below gives a rounded rect (`rx="4"`). Unicode box-drawing characters also render as a box.
- Wrap text in double quotes to keep it verbatim as one label (`"A | B"` stays one text with the quotes removed). Square brackets such as `[Hello]` stay literal text.
- `*` corners/line ends render as filled circle markers; a lone `v` or `^` under/over a `|` or after `-` becomes an arrowhead.
- Style footnotes such as `[a]: {fill: red}` are NOT applied by this backend: they are drawn as plain text.

## Identifiers
This language cannot separate a key from a label: the text you draw is the only name. Draw the display name, put the identifier in the text only when the reader needs it (for example `n0002 engine`). Never invent identifiers.

## Verified examples (all render)
### boxes with a down arrow
```svgbob
+-----+
| App |
+--+--+
   |
   v
+--+--+
| DB  |
+-----+
```
### rounded corners
```svgbob
.-----.
  | foo |
  '-----'
     |
     v
  .-----.
  | bar |
  '-----'
```
### horizontal flow with arrow
```svgbob
+---+     +---+
| A |---->| B |
+---+     +---+
```
### quoted text kept as one label
```svgbob
"A | B"

+-------+
| "x y" |
+-------+
```
### star corners and circles
```svgbob
*-----*
| dot |
*-----*
    |
    o
```
