# Skill: ASCII sketch (ditaa) (`sketch`, format `ditaa`)
## Choose it when
- The user already has, or wants, a quick box-and-arrow picture they can paste into code comments, docs or a README.
- The diagram should stay editable as plain text in a repo.
- A rough, informal layout is enough to settle a discussion.
## Not when
- You need automatic layout and many nodes: use `block` or `flowchart`.
- You need free-form ASCII art with curves or unusual characters: use `svgbob`.
- You need precise semantics (UML, SysML): use the matching typed diagram such as `component`.
## Anatomy
- A box is a rectangle of `+`, `-` and `|` characters; its text is the label.
- A line of `-`, `|` or `:` (dashed) joins boxes; `v`, `^`, `<`, `>` at the end make an arrowhead.
- Colour tags (`cBLU`) and shape tags (`{s}` storage, `{d}` document) inside a box change its look.
- Layout is exactly what you type: the drawing has no layout engine.
## What makes it good
- Every box is a clean rectangle: top, bottom and text rows have equal width.
- Boxes are aligned on a grid with equal spacing, so the picture reads at a glance.
- At most about eight boxes; ditaa does not scale to dense graphs.
- Colour marks a category (for example data stores are one colour), used consistently and explained in the text nearby.
- Flow runs top to bottom or left to right with arrowheads on every connector.
## What makes it bad
- Misaligned edges, so the renderer silently draws loose lines instead of a box.
- Long connectors that wander across other boxes.
- Text crammed to the border with no padding.
- Colour tags applied at random with no meaning.
- Trying to show a complex system that needs a real layout engine.
## Questions to ask
- What are the boxes, and what connects to what?
- Does the picture need to live as text in a repo?
- Is there a category (storage, external, document) worth a colour?
## Contrast
Bad:
```ditaa
+---+
| Web
+--+
 |
+---+
| DB |
+----+
```
Good:
```ditaa
+-------+
|cBLU   |
| Web   |
+---+---+
    |
    v
+---+---+
|{s}    |
| DB    |
+-------+
```
The good version has aligned rectangles, an arrowhead showing direction, and a storage shape for the database.
