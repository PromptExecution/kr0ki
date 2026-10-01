---
name: kr0ki-rackdiag
description: Use before writing or fixing rackdiag server-rack diagrams for render_diagram (format rackdiag): rules checked against the renderer, worked examples, identifier rule.
---
# rackdiag (render_diagram format `rackdiag`)

## Rules (each checked against the renderer)
- Wrapper `rackdiag { ... }`. Slots are `unit: Name;` (`1: UPS;`); put them inside `rack { ... }` for a proper rack (bare slots also render). Optional first statement `16U;` sets rack height (`24U; rack { 24: top; }` renders).
- Item attributes in brackets: a height such as `[2U]`, `[height = 3]`, `[color = "#ccc"]`, `[label = "Disk Array"]`, `[description = "battery"]`, `[width = 2]`, `[rotate = 270]`; combine with commas: `[3U, color = "#fcc"]`. Ranges `1-2: X;` fail (`got unexpected token: '-', expected: ':'`).
- Invalid attributes are rejected: `Unknown attribute: RackItem.attr`, `Unknown attribute: RackItem.unit`, `unknown node shape: cylinder`, and `weight = "4kg"` fails (`must be real number, not str`).
- Several `rack { }` blocks sit side by side. `ascending;` is accepted as a bare statement; `ascending = true;` fails (`got unexpected token: '=', expected: '}'`).
- Duplicate or overlapping slots (`1: UPS; 1: R;`, or `1: A [3U]; 2: B;`) raise no error, so check unit numbers yourself.
- Quote names with spaces: `1: "Big Server";`.

## Identifiers
Slot is `unit: name [label = "display"]`: `1: n0002 [label = "engine"];` keeps the id as the name and shows the label. Never invent identifiers.

## Verified examples (all render)
### one rack
```rackdiag
rackdiag {
  16U;
  rack {
    1: UPS [2U];
    3: Router;
    4: Server [4U, color = "#DDEEFF"];
    8: "Disk Array" [description = "storage"];
  }
}
```
### two racks
```rackdiag
rackdiag {
  rack {
    1: UPS;
    2: PDU;
  }
  rack {
    1: Switch;
    2: Server [width = 2];
  }
}
```
### id as key, name as label
```rackdiag
rackdiag {
  description = "rack A";
  rack {
    1: n0002 [label = "engine"];
    2: n0003 [label = "gearbox", 2U];
  }
}
```
### ascending numbering
```rackdiag
rackdiag {
  ascending;
  8U;
  rack {
    1: A [2U];
    3: B [color = "#fcc", rotate = 270];
  }
}
```
