---
name: kr0ki-nwdiag
description: Use before writing or fixing nwdiag network diagrams for render_diagram (format nwdiag): network/node syntax, valid attributes and shapes, worked examples, identifier rule.
---
# nwdiag (render_diagram format `nwdiag`)

## Rules (each checked against the renderer, 2026-10-01)
- Wrapper: `nwdiag { ... }`. Declare `network name { ... }` blocks; a node listed in two networks is drawn on both (a gateway). Node attributes go in brackets: `web01 [address = "10.0.0.1"];`.
- A node that only appears outside any `network` fails: `DiagramNode X does not belong to any networks`. Peer links (`inet -- router;`) need the other end inside a network.
- Network attributes: `address = "10.0.0.0/24";`, `color = "#CCFFCC";`, `label = "Office LAN";`. **`description` is not valid** (`Unknown attribute: Network.description`).
- Node shapes must be ones nwdiag knows: `cloud`, `actor`, `note`, `box` and the flowchart family such as `flowchart.database` work; **`database`, `cylinder`, `router`, `stack` are rejected** (`unknown node shape`).
- Quote names with spaces: `network "front end" { "web server 1" [address = "10.0.0.1"]; }`. Several addresses: `address = "10.0.1.1, 10.0.1.2"`.
- Do not put `group` blocks around nodes that sit in several networks: a verified failure (`'<' not supported between instances of 'Network'`).

## Identifiers
When an identifier exists, use it as the node name and the display name as `label`: `n0002 [label = "engine"];`. Never invent identifiers.

## Verified examples (all render)
### two networks sharing nodes
```nwdiag
nwdiag {
  network dmz {
    address = "210.x.x.x/24"
    web01 [address = "210.x.x.1"];
    web02 [address = "210.x.x.2"];
  }
  network internal {
    address = "172.x.x.x/24";
    web01 [address = "172.x.x.1"];
    web02 [address = "172.x.x.2"];
    db01;
    app01 [address = "172.x.x.100, 172.x.x.101"];
  }
}
```
### shapes and multi-line labels
```nwdiag
nwdiag {
  network office {
    router01 [shape = "cloud", label = "Edge\nrouter"];
    pc01 [shape = "actor"];
    printer [shape = "note", color = "#DDEEFF"];
  }
}
```
### network colour and label
```nwdiag
nwdiag {
  network lan {
    address = "192.168.0.0/24";
    color = "#CCFFCC";
    label = "Office LAN";
    nas [address = "192.168.0.10"];
    laptop [address = "192.168.0.20"];
  }
}
```
### peer link to the internet
```nwdiag
nwdiag {
  inet [shape = "cloud"];
  inet -- router;
  network lan {
    router;
    host1;
  }
}
```
### quoted names with spaces
```nwdiag
nwdiag {
  network "front end" {
    "web server 1" [address = "10.0.0.1"];
  }
}
```
