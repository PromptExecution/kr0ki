---
name: kr0ki-plantuml
description: Use before writing or fixing PlantUML source for render_diagram (format plantuml): sequence, activity (:action; if/else), state, use case, component, deployment, gantt (@startgantt), timing and class diagrams; quoting and wrapper rules, worked examples, identifier rule.
---
# plantuml (render_diagram format `plantuml`)

## Rules (each checked against the renderer)
- The renderer wraps plain source in `@startuml ... @enduml` for you, but write the wrapper anyway. **Gantt must use `@startgantt ... @endgantt`**: gantt lines inside `@startuml` fail with `Syntax Error? (Assumed diagram type: sequence) (line: 1)`. `@startmindmap`/`@startwbs` also render.
- A bad line gives `Syntax Error? (Assumed diagram type: <type>) (line: N)`: the type shows what PlantUML guessed; fix line N, do not rewrite everything.
- **Names with spaces or hyphens must be declared with a quoted alias**: `participant "Web App" as W`, `state "Idle mode" as s1`, `class "Engine" as e2`. Bare `Web App -> DB`, `class My Class`, `participant my-w` and `[*] --> my-state` all fail; `class my-class` rendered.
- Sequence: `A -> B : msg`, `-->` dotted return, `->>` async, `activate`/`deactivate`, `alt x ... else y ... end`, `loop`, `opt`, `group`, `autonumber`, `note right of B : t`, `note over A,B : t`.
- Activity (new syntax): `start`, `:action;`, `if (c?) then (yes) ... else (no) ... endif`, `while (c?) ... endwhile`, `repeat ... repeat while (c?)`, `fork ... fork again ... end fork`, `|Lane|` swimlanes, `partition "P" { }`, `stop`. **Every action needs the closing `;`** (`:Read input` fails).
- State: `[*] --> A`, `A --> B : event`, nested `state X { ... }`, `state f <<fork>>`, `<<choice>>`, `A : description`. Use case: `actor U`, `usecase "Login" as UC1`, `(Logout) as UC2`, `rectangle Sys { }`, `.>` for include/extend labels.
- Component/deployment: `[Svc A] --> [Svc B]`, `component`, `interface`, `package`, `node`, `cloud`, `database`, `artifact`, nesting in braces.
- Timing: `concise "Web" as W`, `robust`, `binary`, `clock clk with period 50`, `@0` / `W is Idle`, `@100`.
- Gantt: `Project starts 2024-01-01` (needed for absolute dates, else `No starting date for the project`), `[Design] lasts 5 days`, `[Build] starts at [Design]'s end`, `then [Test] lasts 3 days`, `[A] is colored in Lime/Green`, `saturday are closed`. A reference to a missing task fails: `No such task Ghost`; `[B] starts after [A]` fails.
- Class: `class A { +f : T }`, `abstract class`, `interface`, `enum`, `<|--`, `*--`, `o--`, `-->`, `..>`, generics `class Box<T>`, `<<Entity>>`, `package "P" { }`, `hide empty members`, `skinparam` lines first.

## Identifiers
When an identifier exists (SysML v2 element id, schema.table, namespace/kind/name) use it as the alias and the display name as the quoted title: `class "Engine" as e0002`, `participant "Web App" as w0001`, `state "Idle mode" as s0001`. Never invent identifiers.

## Verified examples (all render)
### sequence
```plantuml
@startuml
actor User
participant "Web App" as W
database DB
autonumber
User -> W : login
activate W
alt valid
  W -> DB : query
  DB --> W : rows
  W --> User : ok
else invalid
  W --> User : 401
end
deactivate W
note over User,W : TLS
@enduml
```
### activity
```plantuml
@startuml
|User|
start
:Submit form;
|System|
if (valid?) then (yes)
  :Save;
else (no)
  :Reject;
  stop
endif
repeat
  :Notify;
repeat while (retry?) is (yes)
stop
@enduml
```
### state
```plantuml
@startuml
[*] --> Idle
state "Running mode" as r1 {
  [*] --> Warm
  Warm --> Hot : load
}
state c <<choice>>
Idle --> r1 : start
r1 --> c
c --> Idle : [ok]
c --> [*] : [fail]
Idle : waits for jobs
@enduml
```
### use case
```plantuml
@startuml
left to right direction
actor Customer
actor "Shop Admin" as adm
rectangle Shop {
  usecase "Place order" as UC1
  usecase "Pay" as UC2
  (Refund) as UC3
  Customer --> UC1
  UC1 .> UC2 : include
  adm --> UC3
}
@enduml
```
### component and deployment
```plantuml
@startuml
package "Backend" {
  [Auth API] --> [Users DB] : sql
}
interface REST
[Auth API] - REST
node Server {
  artifact app.jar
}
cloud Net
database PG
Server --> PG
Server -- Net
@enduml
```
### gantt
```plantuml
@startgantt
Project starts 2024-01-01
[Design] lasts 5 days
[Build] starts at [Design]'s end
[Build] lasts 10 days
[Build] is colored in Lime/Green
then [Test] lasts 3 days
saturday are closed
sunday are closed
@endgantt
```
### timing
```plantuml
@startuml
concise "Web" as W
robust "DB" as D
clock clk with period 50
@0
W is Idle
D is Ready
@100
W is Busy
D is Busy
@300
W is Idle
@enduml
```
### class with id alias
```plantuml
@startuml
hide empty members
class "Engine" as e0002 <<Entity>> {
  +rpm : int
  -start()
}
abstract class "Part" as e0001
interface Drawable
e0001 <|-- e0002
e0002 *-- "2..*" Wheel
e0002 ..> Drawable
note right of e0002 : id is the alias
@enduml
```
