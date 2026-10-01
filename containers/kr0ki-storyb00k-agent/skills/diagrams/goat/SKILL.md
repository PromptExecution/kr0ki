---
name: kr0ki-goat
description: Use before writing or fixing goat ASCII-art diagrams for render_diagram (format goat): supported characters, arrowhead spacing, circles, what is drawn as plain text, worked examples, identifier rule.
---
# goat (render_diagram format `goat`)

## Rules (each checked against the renderer)
- Source is plain ASCII art; goat never raises syntax errors (only an empty source gives 400). Mistakes appear as stray characters in the picture.
- Only ASCII is interpreted: Unicode box-drawing characters (`┌─┐│└`) are drawn as plain text glyphs, not lines.
- Arrowheads (`>`, `<`, `^`, `v`) attach to a line only when adjacent to it: `A<---B` leaves `<` as literal text; write `A <--- B` (verified: 1 arrowhead, no stray text).
- `o` at a line end draws a hollow circle and `*` a filled circle (`class="hollow"` / `"filled"`).
- `/` and `\` are diagonal lines, not box corners: `/---\ | A | \---/` renders stray `|` characters. Build boxes with `+`.
- Quotes are not special (`"x y"` is drawn with the quote characters) and `~~>`, `===` are plain text; use `-` lines.

## Identifiers
This language cannot separate a key from a label: the text you draw is the only name. Draw the display name, put the identifier in the text only when the reader needs it (for example `n0002 engine`). Never invent identifiers.

## Verified examples (all render)
### boxes with a down arrow
```goat
+---+
| A |
+---+
  |
  v
+---+
| B |
+---+
```
### left-to-right arrows between labels
```goat
A ---> B ---> C
```
### hollow and filled circle markers
```goat
o------>*
|       |
|       v
o       o
```
### crossing lines and corners
```goat
|
----+----
    |
    v
```
### two-way arrow
```goat
<---->
```
