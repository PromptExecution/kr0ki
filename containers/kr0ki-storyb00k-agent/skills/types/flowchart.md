# Skill: Flowchart (`flowchart`, format `d2`)

## Choose it when
- The reader wants to know "what happens, in what order, and where does it branch?" for one process.
- You are sketching a procedure quickly and swimlanes or formal notation are not needed.
- A decision path (yes/no outcomes) must be followed by someone new to the process.

## Not when
- Several roles hand work to each other and ownership matters: use `activity` (swimlanes).
- The subject is one object's lifecycle driven by events: use `state`.
- The point is message order between systems: use `sequence`.

## Anatomy
- Boxes are steps; each says one action, verb first.
- Diamonds are decisions; every outgoing arrow carries a label (yes/no or the condition).
- Arrows show the order of control, not data movement.
- Rounded or oval terminals mark the single start and the end(s).
- Containers (grouping boxes) mark a phase or a subprocess, if used sparingly.

## What makes it good
- One clear start and explicit ends; every path reaches an end.
- Flow runs one direction (top-down or left-to-right); set `direction` in D2 and keep it.
- Every decision has labelled exits, and each exit leads somewhere different.
- Keep it to roughly 15 nodes; collapse detail into a named subprocess and draw it separately.
- Short labels, verb-first ("Validate order"); when an identifier exists, keep it as the node key and the display name as the label.

## What makes it bad
- Unlabelled branches that leave the reader guessing which arrow means what.
- Arrows looping back in several places with no stated retry condition.
- Dead ends: steps with no outgoing arrow and no terminal.
- Sentences as labels, or mixed levels (a click and a quarterly audit in one chart).
- Crossing lines and decoration (colour per step) that carry no meaning.

## Questions to ask
- Where does the process start, and what are all the ways it can end?
- Which decisions change what happens next, and what are their possible answers?
- Who is the reader, and how much detail do they need?

## Contrast
Bad:
```d2
a -> b
b -> c
b -> d
c -> e
d -> e
e -> a
x
```
Good:
```d2
direction: down
start: Order received { shape: oval }
check: In stock? { shape: diamond }
ship: Ship order
backorder: Notify customer of delay
done: Done { shape: oval }
start -> check
check -> ship: yes
check -> backorder: no
backorder -> done
ship -> done
```
The good version names every step, labels both exits of the decision, and gives the flow a start, an end, and one direction.
