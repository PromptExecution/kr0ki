---
name: kr0ki-dbml
description: Use before writing or fixing DBML source for render_diagram (format dbml): Table/Ref/Enum syntax, quoting of names, schema-qualified references, the real reference errors, worked examples, identifier rule.
---
# dbml (render_diagram format `dbml`)

## Rules (each checked against the renderer)
- Top-level blocks only: `Table`, `Ref`, `Enum`, `TableGroup`, `Note`, `Project`. Anything else (SQL such as `CREATE TABLE`, or stray words) fails: `Could not parse input at line 1:1. Expected "/*", "//", "Enum", "Note", "Project", "Ref", "Table", "TableGroup"...`. Empty input fails with HTTP 400.
- Table syntax: `Table users { id integer [pk] \n name varchar(255) [not null, unique] }`. One column per line; **no `;` after a column** (`Could not parse input ... but ";" found`) and **no backticks around the type**. Types with parameters are fine: `decimal(10,2)`. Comments use `//`.
- **Names with spaces or hyphens must be double-quoted**: `Table my table {` fails (`Expected ".", "[", "as", "{"`), `Table my-table {` fails; `Table "my table" {` and a quoted column `"my col" int` render. Reserved-looking names such as `order` rendered unquoted.
- Foreign keys: inline `user_id integer [ref: > users.id]` or top level `Ref: posts.user_id > users.id` (`>` many-to-one, `<` one-to-many, `-` one-to-one). Optional name and settings: `Ref fk_name: a.x > b.y [delete: cascade]`. Composite: `Ref: b.(x,y) > a.(x,y)`.
- **Every Ref must point at an existing table and column.** `Table ghost does not exist` and `Column nope does not exist in table a` are the real errors.
- Schemas: `Table core.users { ... }` and refer to it as `core.users.id`. An unqualified `a.id` does not find `s.a` (`Table a does not exist`). Alias on an unqualified table works (`Table users as U { ... }` then `U.id`), but on a schema table `Table core.users as U` then `U.id` fails with `Table U does not exist`: refer to it as `core.users.id`.
- Also rendered: `Enum status { active \n inactive }` used as a column type, `TableGroup g { a \n b }`, `Table a [headercolor: #3498DB] {`, `indexes { (x, y) [pk] \n email [unique] }`, `Note: 'text'` inside a table, `Project p { database_type: 'PostgreSQL' }`, a column setting `default: \`now()\``.

## Identifiers
When an identifier exists (schema.table, column name) use it as the table and column name (`Table core.users`) and put the display name in a note: `Table core.users { id int [pk, note: 'User id'] \n Note: 'Users' }`. Never invent identifiers.

## Verified examples (all render)
### tables and inline refs
```dbml
Table users {
  id integer [pk, increment]
  email varchar(255) [not null, unique]
  created_at timestamp [default: `now()`]
}
Table posts {
  id integer [pk]
  user_id integer [not null, ref: > users.id]
  title varchar
}
```
### top-level refs and composite key
```dbml
Table orders {
  id int [pk]
  customer_id int
}
Table customers {
  id int [pk]
}
Table lines {
  order_id int
  sku varchar
  qty int
  indexes {
    (order_id, sku) [pk]
  }
}
Ref fk_order: orders.customer_id > customers.id [delete: cascade]
Ref: lines.order_id > orders.id
```
### enum and table group
```dbml
Enum order_status {
  pending
  shipped
  cancelled
}
Table orders {
  id int [pk]
  status order_status
}
Table audit {
  id int [pk]
}
TableGroup core {
  orders
  audit
}
```
### schemas and notes
```dbml
Table core.users {
  id int [pk]
  name varchar [note: 'Display name']
}
Table core.sessions {
  id int [pk]
  user_id int [ref: > core.users.id]
}
Table billing.invoices [headercolor: #3498DB] {
  id int [pk]
  user_id int [ref: > core.users.id]
}
```
### many to many with quoted names
```dbml
Table "student" {
  id int [pk]
  "full name" varchar
}
Table "course" {
  id int [pk]
}
Table "enrolment" {
  student_id int [ref: > student.id]
  course_id int [ref: > course.id]
  Note: 'join table'
}
```
