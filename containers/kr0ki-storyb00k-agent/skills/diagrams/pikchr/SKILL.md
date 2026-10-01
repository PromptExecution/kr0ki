---
name: kr0ki-pikchr
description: Use before writing or fixing pikchr diagrams for render_diagram (format pikchr): statement separators, label naming, string and colour syntax, error meanings, worked examples, identifier rule.
---
# pikchr (render_diagram format `pikchr`)

## Rules (each checked against the renderer)
- Statements are separated by `;` or a newline. `box "A" arrow box "B"` fails: `ERROR: syntax error` pointing at `arrow`.
- Text must be in double quotes: `box 'A'` -> `ERROR: unrecognized token`; `box A` -> `ERROR: no such object`; an unterminated `"A` -> `unrecognized token`. A literal backslash-n typed between statements is also `unrecognized token`; use a real newline or `;`.
- Object labels must start with an uppercase letter: `a: box "A"` and `a1: box "x"` are syntax errors; `A:`, `N0002:`, `Sys_Engine:` work. `public.orders:` and `Id-1:` are syntax errors.
- Referencing a label that does not exist: `ERROR: no such object`. `same` with no earlier object of that type: `no prior objects of the same type`. A misspelt shape (`boxx`) is `syntax error`.
- Colours: `fill red`, `fill lightblue`, `fill 0xff0000` work; `fill "red"` and `color #ff0000` are syntax errors.
- Double-headed arrows need a direction: `arrow <-> right 1` works, `arrow <-> 1` is a syntax error. `for` loops are a syntax error.
- Do not wrap the source in markup: ```` ```pikchr ```` or `<pikchr>` fail. Comments `#`, `//` and `/* */` are accepted.

## Identifiers
Name objects with the identifier as the capitalised label and the display name as the string: `N0002: box "engine"` then `arrow from N0002.e to N0003.w`. Labels must start with an uppercase letter and may not contain `.` or `-` (use `_`). Never invent identifiers.

## Verified examples (all render)
### flow of boxes
```pikchr
box "A"; arrow; box "B"; arrow; box "C"
```
### named objects and labelled arrow
```pikchr
Src: box "Source"
Dst: box "Sink" at 2 right of Src
arrow from Src.e to Dst.w "data" above
```
### shapes and fill colours
```pikchr
oval "Start" fill lightblue
arrow
box "Work" rad 10px fill 0xffeeaa
arrow
cylinder "DB"
arrow
diamond "Done?"
```
### direction change and bidirectional arrow
```pikchr
down
A1: box "Top"
arrow <-> down 1
A2: box "Bottom" dashed
```
### group in brackets
```pikchr
G1: [ box "in"; arrow; box "out" ]
arrow from G1.e right 0.5
circle "end"
```
