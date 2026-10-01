---
name: kr0ki-umlet
description: Use before writing or fixing UMLet XML diagrams for render_diagram (format umlet): XML structure, element ids, required escaping of < > &, relation attributes, worked examples, identifier rule.
---
# umlet (render_diagram format `umlet`)

## Rules (each checked against the renderer)
- Source must be UMLet XML: `<diagram program="umlet" version="14.3.0">` containing `<element><id>UMLClass</id><coordinates><x/><y/><w/><h/></coordinates><panel_attributes>text</panel_attributes><additional_attributes></additional_attributes></element>`. Plain text fails: `Content is not allowed in prolog`; truncated XML fails: `XML document structures must start and end within the same entity`.
- `<diagram><element></element></diagram>` (missing children) fails with a `NullPointerException`. The XML declaration and the `program` attribute can be omitted.
- An unknown `<id>` (e.g. `UMLFoo`) and an empty `<diagram>` do NOT error: both return a blank 770-byte SVG. Use known ids such as `UMLClass`, `UMLInterface`, `UMLNote`, `UMLActor`, `UMLUseCase`, `UMLState`, `UMLPackage`, `Relation`, `Text`, `UMLSequenceAllInOne` (all verified to draw).
- `<`, `>` and `&` inside `panel_attributes` must be escaped. `lt=<-` fails: `The content of elements must consist of well-formed character data or markup`; `A&B` fails: `The reference to entity "B" must end with the ";" delimiter`. Write `lt=&lt;-`, `&amp;`, `&lt;&lt;interface&gt;&gt;`, or wrap the text in `<![CDATA[...]]>`.
- Compartments are separated by a `--` line; `bg=orange` / `bg=#ff8800` set the fill; `_x_` underlines, `/x/` italicises, `*x*` bolds a line.
- Relations are `Relation` elements: `lt=` sets line/arrow type (`lt=&lt;-`, `lt=-`, `lt=&lt;&lt;&lt;.`), optional `m1=`/`m2=`; `additional_attributes` is `x;y;x;y` points relative to the element. Layout is absolute: you must compute x/y/w/h yourself.

## Identifiers
Put the identifier as the first line of the element text and the display name after it, e.g. `<panel_attributes>n0002\nengine\n--\n+rpm: int</panel_attributes>`; UMLet has no separate key, so keep the identifier text exactly as given. Never invent identifiers.

## Verified examples (all render)
### class with compartments
```umlet
<?xml version="1.0" encoding="UTF-8" standalone="no"?><diagram program="umlet" version="14.3.0"><element><id>UMLClass</id><coordinates><x>10</x><y>10</y><w>160</w><h>90</h></coordinates><panel_attributes>Foo
--
+name: String
--
+run()</panel_attributes><additional_attributes></additional_attributes></element></diagram>
```
### interface and implementing class with relation
```umlet
<?xml version="1.0" encoding="UTF-8" standalone="no"?><diagram program="umlet" version="14.3.0"><element><id>UMLInterface</id><coordinates><x>10</x><y>10</y><w>130</w><h>60</h></coordinates><panel_attributes>&lt;&lt;interface&gt;&gt;
IFoo
--
+f()</panel_attributes><additional_attributes></additional_attributes></element><element><id>UMLClass</id><coordinates><x>10</x><y>140</y><w>130</w><h>60</h></coordinates><panel_attributes>Foo
--
+f()</panel_attributes><additional_attributes></additional_attributes></element><element><id>Relation</id><coordinates><x>60</x><y>70</y><w>30</w><h>70</h></coordinates><panel_attributes>lt=&lt;&lt;&lt;.</panel_attributes><additional_attributes>10;50;10;0</additional_attributes></element></diagram>
```
### use case with actor
```umlet
<?xml version="1.0" encoding="UTF-8" standalone="no"?><diagram program="umlet" version="14.3.0"><element><id>UMLActor</id><coordinates><x>10</x><y>20</y><w>60</w><h>90</h></coordinates><panel_attributes>User</panel_attributes><additional_attributes></additional_attributes></element><element><id>UMLUseCase</id><coordinates><x>160</x><y>30</y><w>130</w><h>60</h></coordinates><panel_attributes>Login</panel_attributes><additional_attributes></additional_attributes></element><element><id>Relation</id><coordinates><x>70</x><y>55</y><w>90</w><h>30</h></coordinates><panel_attributes>lt=-</panel_attributes><additional_attributes>0;10;90;10</additional_attributes></element></diagram>
```
### state and note with colour
```umlet
<?xml version="1.0" encoding="UTF-8" standalone="no"?><diagram program="umlet" version="14.3.0"><element><id>UMLState</id><coordinates><x>10</x><y>10</y><w>140</w><h>70</h></coordinates><panel_attributes>Idle
--
entry/init
bg=orange</panel_attributes><additional_attributes></additional_attributes></element><element><id>UMLNote</id><coordinates><x>200</x><y>10</y><w>120</w><h>60</h></coordinates><panel_attributes>a note
bg=#ffee88</panel_attributes><additional_attributes></additional_attributes></element></diagram>
```
### styled text lines
```umlet
<?xml version="1.0" encoding="UTF-8" standalone="no"?><diagram program="umlet" version="14.3.0"><element><id>UMLClass</id><coordinates><x>10</x><y>10</y><w>160</w><h>80</h></coordinates><panel_attributes>_underlined_
/italic/
*bold*
--
plain</panel_attributes><additional_attributes></additional_attributes></element></diagram>
```
