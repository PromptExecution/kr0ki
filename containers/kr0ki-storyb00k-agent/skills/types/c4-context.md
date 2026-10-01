# Skill: C4 context diagram (`c4-context`, format `c4plantuml`)

## Choose it when
- The reader wants to know what the system is, who uses it, and what it depends on.
- The audience includes non-technical stakeholders or new team members.
- You need the first, highest-level picture before drawing containers.

## Not when
- The point is the inside of the system (apps, databases): use `component`.
- The point is message order: use `sequence`.
- The point is network or deployment layout: use a deployment-oriented type.

## Anatomy
- The system in scope is one box in the centre, with a short description.
- People are the users or roles that interact with it.
- External systems are neighbours it depends on or serves, drawn as external (`System_Ext`).
- Relationships are labelled arrows saying what is exchanged (and the technology if useful).
- There are no internals at this level; technology detail belongs in container diagrams.

## What makes it good
- Exactly one system in scope, with a name and a one-line purpose.
- Every person and external system has a role description, not only a name.
- Every relationship carries a verb-style label ("Sends invoices to") and a direction that matches the label.
- About 5-10 elements in total; more means the scope is too broad.
- Aliases use letters, digits and underscores; identifiers stay as the alias, display name as the label.

## What makes it bad
- Showing databases, services, or libraries inside the system box.
- Unlabelled lines, or labels like "uses" on every relationship.
- Several systems in scope, so there is no centre.
- Mixing in network or deployment details.
- Leaving out external dependencies that would break the system if they failed.

## Questions to ask
- What system are we drawing, and what is it for in one sentence?
- Who uses it, and which outside systems does it rely on or feed?
- What flows over each link (data, requests, files)?

## Contrast
Bad:
```c4plantuml
@startuml
!include <C4/C4_Context>
System(a, "A")
System(b, "B")
System(c, "C")
Rel(a, b, "uses")
Rel(b, c, "uses")
@enduml
```
Good:
```c4plantuml
@startuml
!include <C4/C4_Context>
Person(cust, "Customer", "Buys goods online")
System(shop, "Web Shop", "Lets customers browse and order")
System_Ext(pay, "Payment Provider", "Charges cards")
Rel(cust, shop, "Places orders with", "HTTPS")
Rel_R(shop, pay, "Requests payment from")
@enduml
```
The good version centres one system, distinguishes the person and the external dependency, and labels each link.
