# Skill: Database schema (DBML) (`dbml`, format `dbml`)

## Choose it when
- The reader wants the concrete tables, columns, types and foreign keys of a database.
- A schema is being reviewed before a migration, or documented for developers.
- You want relationships kept next to the column that carries the key.

## Not when
- The audience wants a conceptual picture with cardinality and verbs only: use `erd`.
- You are showing how a request flows through services: use `sequence`.
- You are showing metrics over time or counts: use `chart`.

## Anatomy
- A `Table` is a database table; each line is a column with a type and settings in brackets.
- `[pk]` marks the primary key, `[not null]`, `[unique]` and `[default: ...]` carry constraints.
- A `Ref` (or inline `[ref: > t.id]`) is a foreign key; `>` is many-to-one, `<` one-to-many, `-` one-to-one.
- A schema prefix (`core.users`) groups tables; a `TableGroup` groups them visually by subject.
- An `Enum` names a closed set of values used as a column type.

## What makes it good
- Aim for about 6-15 tables per diagram; use `TableGroup` or separate diagrams per subject area beyond that.
- Every table has a primary key, and every foreign-key column has a `Ref` that points at a real column.
- Use the real schema.table and column names as written in the database; put friendly names in `note`. Identifiers are the key, display text the label.
- Name foreign-key columns consistently (`user_id` references `users.id`) and keep one naming case.
- Show only columns that explain the structure (keys, constraints, enums), and say in a note when others are omitted.

## What makes it bad
- Tables with no primary key, so rows cannot be told apart.
- Foreign-key columns present but no `Ref`, so the relationships are invisible.
- Types left vague or inconsistent (`varchar` here, `text` there for the same idea).
- A single wall of 40 tables with no groups or schema prefixes.
- Join tables for many-to-many hidden or omitted, so the model looks one-to-many.

## Questions to ask
- Which tables or subject area should this cover, and is there a real database to mirror?
- Which columns matter to the reader (keys, constraints, enums), and which can be left out?
- Should related tables be grouped by schema or by business area?

## Contrast
Bad:
```dbml
Table users {
  id integer
  name varchar
}
Table posts {
  id integer
  user_id integer
  title varchar
}
```
Good:
```dbml
Table users {
  id integer [pk, increment]
  name varchar(255) [not null]
  Note: 'Account holders'
}
Table posts {
  id integer [pk, increment]
  user_id integer [not null, ref: > users.id]
  title varchar(200) [not null]
}
```
The good version adds primary keys, constraints, and a real foreign-key reference, so the many-to-one link from posts to users is drawn.
