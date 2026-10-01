# Skill: Wiring diagram (`wiring`, format `wireviz`)

## Choose it when
- The reader wants to know which pin of one connector goes to which pin of another, through which cable.
- A harness must be built or checked: wire colours, gauge, length, shields.
- A technician needs a pin-to-pin reference rather than a layout of parts.

## Not when
- You need parts stacked in a cabinet: use `rack`.
- You need how machines connect over IP: use `network`.
- You need a bit layout of a message: use `packet`.

## Anatomy
- A connector (`X1`) is a plug or socket, with a pin count or named pins.
- A cable (`W1`) is a bundle of wires with a count, colours, gauge, length and optional shield.
- A connection chain runs connector pins, then cable wires, then connector pins, so every link goes through a cable.
- Pin labels (`GND`, `VCC`) name signals; the order of pins in the lists defines which wire joins which pin.
- A shield is addressed separately from the signal wires.

## What makes it good
- Designators follow the convention (X for connectors, W for cables) and match the physical labels on the harness.
- Every pin that is used has a signal name (`pinlabels`), so the diagram reads as a signal list.
- Cables give wire count, colours and gauge, so the build needs no guesswork.
- Pin lists on both ends of a chain match in length; crossed wires (pin 1 to pin 2) are drawn deliberately and noted.
- One harness per diagram. When an identifier exists (part number or asset id) it is the designator key, the display name goes in `type` or `notes`.

## What makes it bad
- Pins unlabelled, so the reader must guess what each wire carries.
- No cable details (colour, gauge, length), leaving the harness unbuildable.
- Connectors given a pin count but only some pins connected, with no note whether the rest are unused.
- Several unrelated harnesses crammed into one drawing.
- Treating it as a layout picture: this type shows connectivity, not physical routing or dimensions.

## Questions to ask
- Which connectors are at each end, and how many pins does each have?
- What does each pin carry (signal or power), and which wire colour goes to which pin?
- What cable details matter: gauge, length, shielding?

## Contrast
Bad:
```wireviz
connectors:
  X1:
    pincount: 2
  X2:
    pincount: 2
cables:
  W1:
    wirecount: 2
connections:
  -
    - X1: [1, 2]
    - W1: [1, 2]
    - X2: [1, 2]
```
Good:
```wireviz
connectors:
  X1:
    pinlabels: [GND, VCC]
    type: Molex
    notes: sensor side
  X2:
    pinlabels: [GND, VCC]
    notes: controller side
cables:
  W1:
    wirecount: 2
    colors: [BK, RD]
    gauge: 0.25 mm2
    length: 1
connections:
  -
    - X1: [GND, VCC]
    - W1: [1, 2]
    - X2: [GND, VCC]
```
The good version names each pin by signal, gives wire colours, gauge and length, and says which end each connector is on.
