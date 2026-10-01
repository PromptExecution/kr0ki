---
name: kr0ki-structurizr
description: Use before writing or fixing Structurizr DSL for render_diagram (format structurizr): workspace/model/views layout, one-line block pitfall, element and relationship ordering, valid shapes and colours, worked examples, identifier rule.
---
# structurizr (render_diagram format `structurizr`)

## Rules (each checked against the renderer)
- Layout is `workspace { model { ... } views { ... } }`, keywords and braces each on their own line. Anything else at the top fails: `Unexpected tokens (expected: workspace)`. A workspace without a view fails: `Empty diagram, does not have any view.`
- **Do not put a block on one line.** `systemContext s { include * }` fails (`Too many tokens, expected: systemContext <software system identifier> [key] [description] {`); `model { u = person "U" }` also fails. Put `{` last on its line and the body on following lines.
- Elements: `id = person "Name" "description" "tag"`, `id = softwareSystem "Name" { c = container "Web" "desc" "Technology" }`, `component` inside a container. Identifiers may contain hyphens (`my-u` rendered) but must be unique: `The identifier "a" is already in use`.
- Relationships `a -> b "description" "technology"` must come after both elements are defined (`The source element "u" does not exist`), and an undefined target fails (`The destination element "ghost" does not exist`). Dotted paths like `s.web` do not resolve: use the element's own identifier (`web`).
- Views: `systemLandscape { include * }`, `systemContext <system id> { ... }`, `container <system id> { ... }`, `component <container id> { ... }`, `dynamic`, `deployment <system> "Env" { ... }`. `systemContext`/`container` need a software system id, not a person (`The element "u" is not a software system`). Two views with the same key fail (`A view with the key a already exists`).
- Inside a view use `include *`, `include a b`, `exclude x`, `autolayout lr` (or `tb 300 100`), `title "..."`.
- Styles go in `styles { element "Tag" { shape cylinder  background #ff0000 } relationship "Relationship" { dashed true } }`. Colours must be hex or a CSS name (`background red` ok; `notacolor` fails). **`shape database` is rejected** (`The shape "database" is not valid`); `cylinder`, `person`, `RoundedBox`, `Hexagon`, `pipe`, `WebBrowser` rendered. Tags: the 3rd string on an element (`"A,B"` for several) or a `tags "x"` line.
- Deployment: `live = deploymentEnvironment "Live" { n = deploymentNode "Server" { containerInstance web } }` plus `deployment s "Live" { include * }`.

## Identifiers
When an identifier exists (SysML v2 element id, schema.table, namespace/kind/name) use it as the DSL identifier (before `=`) and the display name as the quoted name: `n0002 = softwareSystem "Engine"`. Never invent identifiers.

## Verified examples (all render)
### system context
```structurizr
workspace {
  model {
    u = person "User" "Buys things"
    s = softwareSystem "Shop" "Sells goods"
    pay = softwareSystem "Payments" "3rd party" "Ext"
    u -> s "Orders" "HTTPS"
    s -> pay "Charges"
  }
  views {
    systemContext s {
      include *
      autolayout lr
    }
    styles {
      element "Ext" {
        background #999999
      }
      element "Person" {
        shape person
      }
    }
  }
}
```
### container view with cylinder
```structurizr
workspace {
  model {
    u = person "User"
    s = softwareSystem "Shop" {
      web = container "Web" "UI" "React"
      api = container "API" "Logic" "Go"
      db = container "DB" "Orders" "Postgres" "Database"
    }
    u -> web "Uses"
    web -> api "Calls" "JSON"
    api -> db "Reads" "SQL"
  }
  views {
    container s {
      include *
      autolayout tb
    }
    styles {
      element "Database" {
        shape cylinder
        background #ffcc00
      }
      relationship "Relationship" {
        dashed true
      }
    }
  }
}
```
### component view
```structurizr
workspace {
  model {
    s = softwareSystem "Shop" {
      api = container "API" {
        ctl = component "Controller"
        svc = component "Service"
      }
    }
    ctl -> svc "Calls"
  }
  views {
    component api {
      include *
      autolayout
    }
  }
}
```
### deployment
```structurizr
workspace {
  model {
    s = softwareSystem "Shop" {
      web = container "Web"
      db = container "DB"
    }
    web -> db "Reads"
    live = deploymentEnvironment "Live" {
      n = deploymentNode "Server" {
        containerInstance web
        containerInstance db
      }
    }
  }
  views {
    deployment s "Live" {
      include *
      autolayout
    }
  }
}
```
### dynamic view
```structurizr
workspace {
  model {
    u = person "User"
    s = softwareSystem "Shop" {
      web = container "Web"
      db = container "DB"
    }
    u -> web "Uses"
    web -> db "Reads"
  }
  views {
    dynamic s {
      u -> web "Opens page"
      web -> db "Queries"
      autolayout
    }
  }
}
```
### landscape with groups
```structurizr
workspace "Org" "Landscape" {
  model {
    group "Internal" {
      a = softwareSystem "CRM"
      b = softwareSystem "ERP"
    }
    u = person "Staff"
    u -> a "Uses"
    a -> b "Syncs"
  }
  views {
    systemLandscape {
      include *
      title "Landscape"
    }
  }
}
```
