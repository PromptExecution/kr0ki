---
name: kr0ki-wireviz
description: Use before writing or fixing WireViz harness YAML for render_diagram (format wireviz): rules checked against the renderer, worked examples, identifier rule.
---
# wireviz (render_diagram format `wireviz`)

## Rules (each checked against the renderer)
- Input is YAML with top-level `connectors:`, `cables:` and `connections:`. Not YAML-mapping input (e.g. `digraph { a -> b }`) fails with a traceback ending `AttributeError: 'str' object has no attribute 'get'`.
- Each entry in `connections:` is a list of three-part chain written as a nested list: `-` then `- X1: [1, 2]`, `- W1: [1, 2]`, `- X2: [1, 2]`. Flattening it (`- X1: ...` / `- W1: ...` directly under `connections:`) fails: `KeyError: 0`.
- A connector must have `pincount: N` or `pinlabels: [...]`. Connect connector-to-connector with no cable in between fails: `Exception: X2 is not in cables`; always put a cable (or a ferrule) in the middle.
- Referencing an undefined designator fails: `Exception: X3 is not in connectors`; a pin outside the pin count fails: `Exception: X2:5 not found.`.
- Pin lists must be equal length across the chain, else `All lists and dict lists must be the same length!`; ranges `[1-4]` are accepted.
- Pins can be addressed by label: `pinlabels: [GND, VCC]` then `- X1: [GND, VCC]`. Unknown connector keys fail: `Connector.__init__() got an unexpected keyword argument 'colour'` (it is `color`).
- Useful attributes that render: connectors `type`, `subtype`, `color`, `notes`, `show_name: false`; cables `gauge`, `length`, `color_code: DIN`, `colors: [BK, RD]`, `shield: true`, `notes`. Shield wire is addressed as `s`.

## Identifiers
Connector and cable keys are the designators shown in the drawing, so use the identifier as the key and put the display name in `type:` (connectors) or `notes:` (cables): `n0002: {pincount: 2, type: engine}`. Never invent identifiers.

## Verified examples (all render)
### basic harness
```wireviz
connectors:
  X1:
    pinlabels: [GND, VCC]
  X2:
    pinlabels: [GND, VCC]
cables:
  W1:
    wirecount: 2
    colors: [BK, RD]
connections:
  -
    - X1: [GND, VCC]
    - W1: [1, 2]
    - X2: [GND, VCC]
```
### attributes and colour code
```wireviz
connectors:
  X1:
    pincount: 4
    type: Molex
    subtype: female
    notes: control side
  X2:
    pincount: 4
cables:
  W1:
    wirecount: 4
    gauge: 0.25 mm2
    length: 1
    color_code: DIN
    notes: main loom
connections:
  -
    - X1: [1-4]
    - W1: [1-4]
    - X2: [1-4]
```
### shield
```wireviz
connectors:
  X1:
    pincount: 2
  X2:
    pincount: 2
cables:
  W1:
    wirecount: 2
    shield: true
connections:
  -
    - X1: [1, 2]
    - W1: [1, 2]
    - X2: [1, 2]
  -
    - X1: [1]
    - W1: s
```
### id as key, name as type
```wireviz
connectors:
  n0002:
    pincount: 2
    type: engine
  n0003:
    pincount: 2
    type: wheel
cables:
  w0001:
    wirecount: 2
connections:
  -
    - n0002: [1, 2]
    - w0001: [1, 2]
    - n0003: [2, 1]
```
