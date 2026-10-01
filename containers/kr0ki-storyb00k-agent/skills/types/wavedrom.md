# Skill: Waveform (WaveDrom) (`wavedrom`, format `wavedrom`)
## Choose it when
- The reader needs digital signals against a clock: edges, setup and hold, handshakes, bus values.
- You are documenting a protocol or hardware interface cycle by cycle.
- You want a register bitfield alongside timing in the same notation.
## Not when
- Actors have named states rather than digital levels: use `timing`.
- Messages between parties matter more than signal levels: use `sequence`.
- The subject is a byte layout, not a time axis: use `bytefield`.
## Anatomy
- Each row is a signal; its `name` is on the left and `wave` is a string of one character per clock cycle.
- `p` and `n` are clocks, `0` and `1` levels, `.` repeats the previous state, `x` is unknown, digits like `3` open a bus value taken from `data`.
- Groups nest signals (`["bus", ...]`); `{}` is a blank spacer row.
- `node` and `edge` draw labelled arrows between points.
- `reg` draws a register bitfield instead of waves.
## What makes it good
- A clock row comes first, so every other row is read against it.
- Rows are ordered by the direction of causality: requester, then responder.
- Related signals are grouped; spacer rows separate unrelated groups.
- Bus rows carry `data` labels that match the spec's names.
- Edges mark the one or two causal links that matter, such as `req` leading to `ack`.
## What makes it bad
- No clock reference, so cycle counts are unknowable.
- More than about eight rows with no grouping.
- Unknown wave characters or unknown top-level keys, which this renderer accepts silently and draws wrongly or not at all.
- Bus values with no labels.
- Arrows on every transition, so none stands out.
## Questions to ask
- Which signals are involved, and which is the clock?
- How many cycles, and what happens at each edge?
- Which cause and effect should the reader notice?
## Contrast
Bad:
```wavedrom
{ signal: [ { name: "a", wave: "0101" }, { name: "b", wave: "1010" } ] }
```
Good:
```wavedrom
{ signal: [ { name: "clk", wave: "p......" }, { name: "req", wave: "01..0..", node: ".a....." }, { name: "ack", wave: "0..1..0", node: "...b..." }, { name: "dat", wave: "x..34x.", data: ["hdr","body"] } ], edge: ["a~>b latency"] }
```
The good version adds a clock, names the handshake, labels the bus values, and marks the one delay worth noticing.
