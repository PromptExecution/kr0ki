---
name: kr0ki-tikz
description: Use before writing or fixing TikZ/LaTeX diagrams for render_diagram (format tikz): full-document wrapper, required libraries, LaTeX escaping, node-name restrictions, error meanings, worked examples, identifier rule.
---
# tikz (render_diagram format `tikz`)

## Rules (each checked against the renderer)
- Send a complete document: `\documentclass[tikz]{standalone}` + `\begin{document}` + `\begin{tikzpicture}...\end{tikzpicture}` + `\end{document}`. A bare `tikzpicture` fails: `! LaTeX Error: Missing \begin{document}.`
- Every path statement ends with `;`. Missing one fails: `Package tikz Error: Giving up on this path. Did you forget a semicolon?`
- Layout keys need their library in the preamble: `right=of a` without `\usetikzlibrary{positioning}` fails `Package PGF Math Error: Unknown function `of'`. Verified working: `positioning`, `fit`, `calc`, `arrows.meta`, `shapes.geometric`, `automata`, `mindmap`; packages `amsmath`, `pgfplots`, `tikz-cd`.
- An unknown library fails `I did not find the tikz library 'nonexistentlib'`; an unknown option fails `I do not know the key '/tikz/foo'`; an undefined colour fails `Undefined color `nocolor'`; drawing to an undeclared node fails `No shape named `nope' is known`.
- Escape LaTeX specials in node text: `A_b` fails `Missing $ inserted`, `A & B` fails `Misplaced alignment tab character &`. Write `A\_b`, `A \& B`, `50\%`. Accented letters (`é ü`) are fine.
- Node names cannot contain a dot: `\node (public.orders)` then `of public.orders` fails `No shape named `public' is known`. `n0002` and `n_1` work.

## Identifiers
Use the identifier as the node name and the display name as the text: `\node[draw] (n0002) {engine};`. Node names may use letters, digits and `_` but NOT `.` (`public.orders` fails), so convert dots/slashes to `_` in the name and keep the real id in the label text. Never invent identifiers.

## Verified examples (all render)
### two nodes with positioning and an arrow
```tikz
\documentclass[tikz]{standalone}
\usetikzlibrary{positioning}
\begin{document}
\begin{tikzpicture}
\node[draw] (n0002) {engine};
\node[draw,right=of n0002] (n0003) {wheel};
\draw[->] (n0002) -- node[above]{drives} (n0003);
\end{tikzpicture}
\end{document}
```
### foreach loop of circles
```tikz
\documentclass[tikz]{standalone}
\begin{document}
\begin{tikzpicture}
\foreach \i in {0,1,2} \node[draw,circle] at (\i*1.5,0) {\i};
\end{tikzpicture}
\end{document}
```
### modern arrow tips
```tikz
\documentclass[tikz]{standalone}
\usetikzlibrary{arrows.meta}
\begin{document}
\begin{tikzpicture}
\draw[-{Stealth[length=3mm]},thick,blue] (0,0) -- (3,0);
\fill[red!30] (0,0.3) rectangle (2,1);
\end{tikzpicture}
\end{document}
```
### finite automaton
```tikz
\documentclass[tikz]{standalone}
\usetikzlibrary{automata}
\begin{document}
\begin{tikzpicture}
\node[state,initial] (q0) {$q_0$};
\node[state,accepting,right of=q0] (q1) {$q_1$};
\path[->] (q0) edge node[above]{a} (q1);
\end{tikzpicture}
\end{document}
```
### grouping box with fit
```tikz
\documentclass[tikz]{standalone}
\usetikzlibrary{positioning,fit}
\begin{document}
\begin{tikzpicture}
\node[draw] (a) {A};
\node[draw,right=of a] (b) {B};
\node[draw,dashed,fit=(a)(b),inner sep=6pt] {};
\end{tikzpicture}
\end{document}
```
