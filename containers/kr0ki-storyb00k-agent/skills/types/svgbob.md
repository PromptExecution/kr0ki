# Skill: ASCII diagram (svgbob) (`svgbob`, format `svgbob`)
## Choose it when
- The user has ASCII art, or wants it, and the text version must stay the source of truth.
- A diagram sits in a README or code comment and should also render crisply as SVG.
- You need freeform shapes (circles, rounded corners, arrows at odd angles) that a layout engine would not give.
## Not when
- Automatic layout of many nodes is needed: use `block` or `flowchart`.
- A plain rectangular box layout with colours is enough: use `sketch`.
- The diagram has formal semantics (UML, SysML): use the matching typed diagram such as `sequence`.
## Anatomy
- Lines are drawn with `-`, `|`, `/`, `\` and joined at `+`; `.` and `'` give rounded corners.
- Arrowheads are `v`, `^`, `>`, `<` at a line end; `*` and `o` are filled and open circles.
- Text in double quotes is kept verbatim as one label.
- Layout is exactly what you type; the tool only redraws it.
## What makes it good
- Every box closes: edges line up column for column.
- Whitespace is generous; boxes are separated by at least two characters.
- Eight to ten shapes at most; more should be split or moved to a layout-based type.
- Labels are short and quoted if they contain characters that could be misread as lines.
- The ASCII stays readable as plain text, because that is how it will be maintained.
## What makes it bad
- Almost-closed boxes that render as loose lines.
- Tabs or proportional-font alignment, so columns drift between editors.
- Relying on style footnotes such as `[a]: {fill: red}`: this renderer draws them as plain text.
- Dense diagrams with crossing lines.
- Ambiguous `+` and `.` joins where two lines touch by accident.
## Questions to ask
- Can you paste the ASCII art you already have?
- What are the shapes, and which direction does flow go?
- Will this stay as text in a repo, or can it move to a layout-based type?
## Contrast
Bad:
```svgbob
+---+
| App
+--+
  |
 | DB |
```
Good:
```svgbob
.-------.
|  App  |
'---+---'
    |
    v
.---+---.
|  DB   |
'-------'
```
The good version closes every box, uses rounded corners consistently, and ends the connector in an arrowhead.
