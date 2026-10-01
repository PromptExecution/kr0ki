---
name: kr0ki-erd
description: Use before writing or fixing erd (Haskell erd) entity-relationship source for render_diagram (format erd): [Entity] blocks, attribute markers, valid cardinalities, quoted names and options, worked examples, identifier rule.
---
# erd (render_diagram format `erd`)

## Rules (each checked against the renderer)
- Entities are `[Name]` on their own line, followed by one attribute per line until the next entity. An attribute before any entity fails: `Attribute '"id"' comes before first entity.`
- Attribute markers: `*id` primary key, `+loc_id` foreign key, `*+id` both. An attribute with a space must be quoted (`first name` fails: `expecting relationship or attribute`; `"first name"` and `*"first name"` render).
- Entity names: letters, digits and `_` only. **`[My Entity]`, `[core.users]` and `[my-ent]` all fail** (`expecting "]"` / `expecting letter, digit or underscore or "]"`). `["My Entity"]` (quoted) renders and is referenced as `"My Entity" 1--* B`.
- Relationships go on their own line: `Person *--1 Location`. Each side is one of `1`, `*`, `?`, `+`: **`0` and `n` fail** (`unexpected '0' expecting relationship or attribute`; same for `2--3`). The separator must be `--`; `1->*` fails.
- Both entities must exist: `Unknown entity 'Ghost' in relationship.` (declaring them after the relationship line is fine).
- Options go in braces with quoted values: `A 1--* B {label: "has", color: "#ff0000"}`, `[A] {bgcolor: "#ececfc", size: "20", border: "2", border-color: "#0000ff"}`, attribute `*id {label: "varchar, not null"}`, and a title line `title {label: "My DB", size: "30", color: "#0000ff"}`. **Unquoted values fail** (`{bgcolor: red}`), unknown option names fail: `Option 'colour' does not exist.` (`headlabel`, `taillabel`, `fontsize` also do not exist on relationships).
- `#` starts a comment line. No `;` terminators (`A 1--* B;` fails). SQL like `Table a (id int)` is not valid: `not a valid directive`.

## Identifiers
When an identifier exists (schema.table) use it as the entity name with dots turned into underscores or quoted (`["core.users"]`), and the display name as a label option: `[core_users] {label: "Users"}`. Never invent identifiers.

## Verified examples (all render)
### basic with keys
```erd
[Person]
*id
name
+location_id

[Location]
*id
city

Person *--1 Location
```
### cardinalities and labels
```erd
[Customer]
*id
[Order]
*id
+customer_id
[Invoice]
*id
[Note]
*id

Customer 1--* Order {label: "places"}
Order 1--? Invoice {label: "billed by", color: "#ff0000"}
Order +--* Note
```
### title and entity colours
```erd
title {label: "Shop schema", size: "24", color: "#1b4f72"}

[Product] {bgcolor: "#ececfc", size: "18"}
*sku
price

[Category] {border: "2", border-color: "#0000ff"}
*id
name

Product *--1 Category
```
### quoted entity and attributes
```erd
["Order Line"]
*+order_id
*"line no"
qty {label: "int, not null"}

[Order]
*id

"Order Line" *--1 Order
```
### composite key and many-to-many
```erd
[student]
*id
name
[course]
*id
title
[enrolment]
*+student_id
*+course_id
grade

enrolment *--1 student
enrolment *--1 course
```
