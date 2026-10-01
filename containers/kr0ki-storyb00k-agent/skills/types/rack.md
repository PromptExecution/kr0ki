# Skill: Rack diagram (`rack`, format `rackdiag`)

## Choose it when
- The reader wants to know what equipment sits in which rack unit, top to bottom.
- A data-centre or lab change needs free space and ordering checked (where does the new 2U server go).
- Power and cooling planning needs the physical stack, not the logical network.

## Not when
- You need IP segments and addressing: use `network`.
- You need which software runs on which machine: use `deployment`.
- You need cable pin-outs: use `wiring`.

## Anatomy
- A `rack` block is one cabinet, with slots numbered by rack unit (U).
- A slot `unit: Name;` places a device at that unit; a height such as `[2U]` makes it span several units.
- Several `rack` blocks sit side by side as separate cabinets.
- A rack height statement (`16U;`) sets how many units the cabinet has.
- Colour marks a category (storage, compute, network), and a label gives the display name.

## What makes it good
- Set the rack height to match the real cabinet, so empty space is visible as empty.
- Give every multi-unit device its height, and check numbering: nothing overlaps, nothing is double-booked.
- Order follows physical convention for the site (state whether unit 1 is at the bottom), and keep it the same in every rack.
- Name devices by their asset tag or hostname; when an identifier exists it is the slot name, the model or role is the label.
- One or two racks per diagram; use consistent colours by device class and say what they mean.

## What makes it bad
- No heights, so a 4U server looks the same as a 1U switch.
- Overlapping or duplicate unit numbers (the renderer will not warn you).
- Rack height omitted or wrong, hiding how much room is left.
- Network links, power feeds and software drawn into what is only a stack diagram.
- Anonymous devices ("Server", "Server") with no hostname or role.

## Questions to ask
- How tall is the rack, and which unit is number 1 (top or bottom)?
- Which devices are in it, with their heights in U and their hostnames or roles?
- Is the point free space, power load, or just documenting what is installed?

## Contrast
Bad:
```rackdiag
rackdiag {
  1: Server;
  2: Server;
  3: Server;
  4: Switch;
}
```
Good:
```rackdiag
rackdiag {
  12U;
  rack {
    1: UPS [2U, color = "#fcc"];
    3: sw-core01 [label = "Core switch"];
    4: db01 [2U, color = "#DDEEFF", label = "Database"];
    6: app01 [label = "App server"];
    7: app02 [label = "App server"];
  }
}
```
The good version sets the rack height, gives multi-unit devices their size, names each device, and leaves free units visible.
