# Skill: Packet structure (`packet`, format `packetdiag`)

## Choose it when
- The reader wants the exact layout of a header, frame or register: which bits mean what.
- An implementer must parse or build a binary format and needs field widths and offsets.
- A key, tag or flag byte layout needs documenting (cache key, status register).

## Not when
- You need the order of messages between parties: use `sequence`.
- You need physical devices and cabling: use `rack` or `wiring`.
- You need a data model of records and relations: use `erd` or `dbml`.

## Anatomy
- Each field is a labelled range of bit positions, `first-last: Label;`, or a single bit `0: Flag;`.
- Fields lay out left to right in rows of fixed width (`colwidth`, bits per row, often 32).
- Positions are the identity: a field has an offset and a width, no separate key.
- Gaps between ranges are legal and mean unspecified or reserved bits.
- Colour highlights a group such as flags or reserved bits.

## What makes it good
- Set `colwidth` to the natural word size of the format (8, 16 or 32) so rows match how people read the spec.
- Every bit is accounted for: fields are contiguous, with explicit `Reserved` or `Padding` fields rather than silent gaps, unless the gap is deliberate.
- Label fields with the name used in the spec, and put the width or meaning in the label when it is not obvious.
- Mark reserved or unused bits in a muted colour and flags in another, and state the bit-numbering convention (bit 0 on the left or right) when it matters.
- One structure per diagram; show a nested sub-structure (flags byte) as its own diagram.

## What makes it bad
- Overlapping ranges, which the renderer rejects.
- Field widths that do not add up to the stated total length.
- Unlabelled or vague fields ("Data", "Misc") with no meaning given.
- Mixing byte offsets and bit offsets in the same diagram.
- No hint of which end is the most significant bit, so readers misread values.

## Questions to ask
- Which structure is it (protocol header, register, key format) and how wide is it in bits?
- What are the fields, in order, with their widths?
- Which bits are reserved or flags that the reader must not miss?

## Contrast
Bad:
```packetdiag
packetdiag {
  0-7: Header;
  8-31: Stuff;
}
```
Good:
```packetdiag
packetdiag {
  colwidth = 16;
  0-3: Version;
  4-7: Type;
  8-14: Reserved [color = "#dddddd"];
  15: Last [color = "#ffcccc"];
  16-31: Length;
  32-47: Checksum;
}
```
The good version matches the row width to a 16-bit word, names each field with its meaning, marks reserved and flag bits, and leaves no unexplained span.
