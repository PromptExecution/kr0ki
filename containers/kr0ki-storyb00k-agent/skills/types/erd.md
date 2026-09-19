# Skill: entity-relationship diagrams (PlantUML ER, DBML)

Good at: entities, cardinality, foreign keys. Bad at: behaviour.

Gotchas:
- PlantUML ER: `entity` blocks + `||--o{` crow's-foot lines (one-to-many),
  `||--||` one-to-one, `}o--o{` many-to-many.
- DBML: `Table { column type [pk] }`, `Ref: a.id > b.author_id`.
- Name relationships with a verb phrase on the line label.

Minimal correct sample (PlantUML):

```plantuml
@startuml
entity "author" as a {
  * id : number
  --
  * name : text
}
entity "book" as b {
  * id : number
  --
  * author_id : number
  * title : text
}
a ||--o{ b : writes
@enduml
```

Render: POST source to `/render/plantuml` (or `/render/dbml` for DBML).
