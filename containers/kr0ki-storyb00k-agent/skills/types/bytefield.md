# Skill: Bytefield (`bytefield`, format `bytefield`)
## Choose it when
- The reader needs to know where each field sits in a header, packet, record or memory word, and how wide it is.
- Someone must implement or parse a binary layout and needs exact bit/byte boundaries.
- You are documenting a wire format or file header, not a flow of messages.
## Not when
- The question is about message order between parties: use `sequence`.
- The question is how signals change over clock cycles: use `wavedrom` or `timing`.
- The question is how tables relate in a database: use `erd`.
## Anatomy
- One box is one field; its width (`:span`) is proportional to its size in bits or bytes.
- A row is one machine word (often 16 or 32 columns); fields that cross rows wrap.
- Column headers give the bit or byte offsets, so a reader can count positions without arithmetic.
- A gap (`draw-gap`) marks variable-length data such as a payload; `draw-bottom` closes the layout.
- Emphasis (bold text, a fill) marks fields that carry meaning, such as flags or the length field.
## What makes it good
- Spans add up exactly to the row width: every row is full, and the total matches the spec.
- One layout per diagram; a second encapsulated layer (header inside payload) gets its own diagram.
- Field names match the spec's names, with units or widths stated once (`Length (bytes)`).
- Fixed-size fields sit before the variable-length one, shown with a gap, in on-the-wire order.
- Fill is used sparingly to group related fields (flags, reserved), and reserved bits are labelled, not left blank.
## What makes it bad
- Spans that do not match the real field sizes, so the picture misleads about alignment.
- Cramming a whole protocol stack into one row-after-row wall of boxes.
- Unlabelled or abbreviated fields that only the author can decode.
- Mixing bit-level and byte-level units without saying which.
- Decoration colours that carry no meaning.
## Questions to ask
- What is the layout (field names and sizes), and is the unit bits or bytes?
- How wide is a row in the spec (16, 32, 64 bits)?
- Which fields matter most, and is there a variable-length part?
## Contrast
Bad:
```bytefield
(draw-box "a" {:span 4})
(draw-box "b" {:span 4})
(draw-box "c" {:span 8})
```
Good:
```bytefield
(def boxes-per-row 32)
(draw-box "Source Port" {:span 16})
(draw-box "Destination Port" {:span 16})
(draw-box "Length" {:span 16})
(draw-box "Checksum" {:span 16})
(draw-gap "Payload")
(draw-bottom)
```
The good version uses spec field names, fills the 32-bit row exactly, and shows the variable-length payload as a gap.
