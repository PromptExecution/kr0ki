---
name: kr0ki-c4plantuml
description: Use before writing or fixing C4-PlantUML source for render_diagram (format c4plantuml): the required stdlib !include, which macros each C4 level offers, alias and quoting rules, boundaries, relationships, worked examples, identifier rule.
---
# c4plantuml (render_diagram format `c4plantuml`)

## Rules (each checked against the renderer)
- Start with the C4 stdlib include, then use macros: `@startuml` / `!include <C4/C4_Container>` / `Person(u, "User")` / `@enduml`. Without any include, the macros are unknown: `Syntax Error? (Assumed diagram type: sequence) (line: 1)`.
- The include decides which macros exist: `C4_Context` (Person, System, System_Ext, Enterprise_Boundary), `C4_Container` (adds Container, ContainerDb, ContainerQueue, System_Boundary, Container_Ext), `C4_Component` (adds Component, Container_Boundary), `C4_Dynamic`, `C4_Deployment` (Deployment_Node). `Container(...)` under `C4_Context` fails (`Assumed diagram type: component`); higher levels include the lower ones (`System` works under `C4_Container`).
- Macro names are case-sensitive: `ContainerDb` works, **`ContainerDB` fails**.
- Aliases (first argument) may contain letters, digits, `_` and `.`; **a hyphen fails** (`my-user` gave `Syntax Error? (Assumed diagram type: activity)`). Write labels and descriptions as double-quoted strings.
- **Backslash-escaped quotes inside a string fail**: `$descr="Handles \"x\""` gave `unquoted function/procedure cannot use expression.` Use single quotes or rephrase.
- Relationships: `Rel(from, to, "label", "technology")`, direction variants `Rel_R`, `Rel_D`, `Rel_L`, `Rel_U`, and `BiRel`. A boundary is `System_Boundary(id, "Name") { ... }` with the closing brace.
- Optional: `LAYOUT_LEFT_RIGHT()`, `SHOW_LEGEND()`, `title ...`, `AddElementTag("t", $bgColor="#ccc")` then `$tags="t"` on an element: all rendered.

## Identifiers
When an identifier exists (SysML v2 element id, schema.table, namespace/kind/name) use it as the macro alias (letters, digits, _ and . only) and the display name as the quoted label: `Container(n0002, "Engine", "Rust", "Computes thrust")`. Never invent identifiers.

## Verified examples (all render)
### system context
```c4plantuml
@startuml
!include <C4/C4_Context>
Person(cust, "Customer", "Buys things")
System(shop, "Web Shop", "Sells goods")
System_Ext(pay, "Payment Provider")
Rel(cust, shop, "Orders", "HTTPS")
Rel_R(shop, pay, "Charges")
@enduml
```
### containers in a boundary
```c4plantuml
@startuml
!include <C4/C4_Container>
LAYOUT_LEFT_RIGHT()
Person(u, "User")
System_Boundary(b, "Shop") {
  Container(web, "Web App", "React", "UI")
  ContainerDb(db, "Database", "Postgres", "Orders")
  ContainerQueue(q, "Events", "Kafka")
}
Rel(u, web, "Uses", "HTTPS")
Rel(web, db, "Reads/writes", "SQL")
Rel_D(web, q, "Publishes")
@enduml
```
### components
```c4plantuml
@startuml
!include <C4/C4_Component>
Container_Boundary(api, "API") {
  Component(ctl, "Controller", "Spring MVC")
  Component(svc, "Service", "Java")
  Rel(ctl, svc, "Calls")
}
ContainerDb(db, "DB", "Postgres")
Rel(svc, db, "Reads", "JDBC")
@enduml
```
### deployment
```c4plantuml
@startuml
!include <C4/C4_Deployment>
Deployment_Node(srv, "Server", "Ubuntu 22") {
  Container(app, "App", "Go", "API")
}
Deployment_Node(dbn, "DB host", "VM") {
  ContainerDb(pg, "Postgres", "PostgreSQL")
}
Rel(app, pg, "Reads", "TCP")
@enduml
```
### dynamic view
```c4plantuml
@startuml
!include <C4/C4_Dynamic>
Person(u, "User")
Container(web, "Web")
Container(api, "API")
Rel(u, web, "1. Opens page")
Rel(web, api, "2. Fetches data", "JSON")
@enduml
```
### tags legend and title
```c4plantuml
@startuml
!include <C4/C4_Container>
title Legacy flagged
AddElementTag("old", $bgColor="#cccccc")
Container(a, "Billing", "COBOL", "Legacy", $tags="old")
Container(b, "Portal", "Vue")
Rel(b, a, "Calls")
SHOW_LEGEND()
@enduml
```
