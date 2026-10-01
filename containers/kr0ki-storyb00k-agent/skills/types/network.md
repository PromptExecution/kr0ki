# Skill: Network diagram (`network`, format `nwdiag`)

## Choose it when
- The reader wants to know which machines sit on which network segments and at which addresses.
- Someone must see where the boundary is (DMZ versus internal) and which host bridges it.
- Firewall or routing reviews need the segment layout, not the application logic.

## Not when
- You need workloads and services inside a Kubernetes cluster: use `k8s-topology`.
- You need what software artifact runs on which node: use `deployment`.
- You need physical units in a cabinet: use `rack`.

## Anatomy
- A `network` block is one segment (a subnet or VLAN), drawn as a horizontal bus.
- A node on the bus is a host or device attached to that segment.
- A node listed in two networks is drawn on both: it is a gateway, router or dual-homed host.
- A node `address` is its IP on that segment; a node can carry several.
- A cloud shape conventionally stands for the internet or an external network.

## What makes it good
- One segment per subnet, with its CIDR in `address` and a short `label` ("DMZ", "Internal").
- Order segments outside to inside (internet, DMZ, internal, data) so trust reads top to bottom.
- Show gateways by listing the same node in both segments, so the crossing point is explicit.
- Keep to about 4-6 segments and under 12 nodes per segment; split by site otherwise.
- Use real hostnames as node names and role in a label; when an identifier exists it is the node name, the display name the label.

## What makes it bad
- Everything on one flat segment, hiding the real boundaries.
- Nodes with no addresses when the question is about addressing.
- Overlapping or inconsistent CIDRs drawn side by side with no remark.
- Mixing logical services, physical cabling and cloud regions in one picture.
- A router or firewall drawn as just another host, with no sign it connects segments.

## Questions to ask
- Which segments exist, and what are their address ranges?
- Which hosts or devices cross segments (routers, firewalls, dual-homed servers)?
- Is the point addressing, trust boundaries, or just who is reachable from the internet?

## Contrast
Bad:
```nwdiag
nwdiag {
  network all {
    web01;
    web02;
    db01;
    router;
  }
}
```
Good:
```nwdiag
nwdiag {
  inet [shape = "cloud"];
  inet -- fw01;
  network dmz {
    label = "DMZ";
    address = "10.0.1.0/24";
    fw01 [address = "10.0.1.1"];
    web01 [address = "10.0.1.10"];
  }
  network internal {
    label = "Internal";
    address = "10.0.2.0/24";
    fw01 [address = "10.0.2.1"];
    db01 [address = "10.0.2.20"];
  }
}
```
The good version separates DMZ from internal, shows the firewall as the node on both segments, and gives each segment and node an address.
