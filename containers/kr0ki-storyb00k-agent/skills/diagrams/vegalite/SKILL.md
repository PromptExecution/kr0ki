---
name: kr0ki-vegalite
description: Use before writing or fixing Vega-Lite chart specs for render_diagram (format vegalite): strict-JSON rules, inline data only, valid encoding types, layer/transform/facet examples, identifier rule.
---
# vegalite (render_diagram format `vegalite`)

## Rules (each checked against the renderer)
- Source is one Vega-Lite spec as **strict JSON**: double-quoted keys, no trailing commas, no single quotes, no YAML. Errors: trailing comma -> `Expected double-quoted property name in JSON`; `{'data':...}` -> `Expected property name or '}'`; `mark: bar` -> `"mark: bar" is not valid JSON`.
- Data must be inline: `"data":{"values":[{...},{...}]}`. `"data":{"url":...}` fails with `Unable to load data from an URL while running in secure mode`. A CSV string only works with `"format":{"type":"csv"}`; a bare string in `values` fails to parse as JSON.
- Every encoding channel with a `field` should carry a valid `"type"`: `quantitative`, `nominal`, `ordinal` or `temporal`. `"type":"category"` fails with `Invalid field type "undefined"`.
- Mark names must be real Vega-Lite marks (`bar`, `line`, `point`, `arc`, `text`, ...). `"mark":"foo"` fails with `Cannot read properties of undefined (reading 'filled')`.
- A `field` name that does not exist in the data does NOT error: the chart renders wrong or empty. Copy field names exactly from the data.
- `$schema` is optional (renders with or without it). `title`, `width`, `height`, `layer`, `hconcat`, `transform` (`filter`, `calculate`), `aggregate` and `column` facets all render.
- Pie/donut uses `"mark":"arc"` with `theta` and `color` channels (`innerRadius` makes a donut).

## Identifiers
Put the identifier in its own data field and use it as the nominal key: `{"data":{"values":[{"id":"n0002","label":"engine","v":3}]},"mark":"bar","encoding":{"x":{"field":"id","type":"nominal"},"y":{"field":"v","type":"quantitative"},"tooltip":{"field":"label","type":"nominal"}}}`; show the display name from the `label` field (tooltip or text). Never invent identifiers.

## Verified examples (all render)
### bar chart with titles
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"width":200,"height":120,"title":"Sales","mark":"bar","encoding":{"x":{"field":"c","type":"nominal","title":"Item"},"y":{"field":"y","type":"quantitative","title":"Units"}}}
```
### line with points colored by series
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"mark":{"type":"line","point":true},"encoding":{"x":{"field":"x","type":"quantitative"},"y":{"field":"y","type":"quantitative"},"color":{"field":"g","type":"nominal"}}}
```
### layered bars with value labels
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"layer":[{"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"y","type":"quantitative"}}},{"mark":{"type":"text","dy":-6},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"y","type":"quantitative"},"text":{"field":"y","type":"quantitative"}}}]}
```
### filter, calculate and aggregate
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"transform":[{"filter":"datum.y > 2"},{"calculate":"datum.y * 2","as":"y2"}],"mark":"bar","encoding":{"x":{"field":"g","type":"nominal"},"y":{"field":"y2","type":"quantitative","aggregate":"sum"}}}
```
### donut with arc mark
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"mark":{"type":"arc","innerRadius":40},"encoding":{"theta":{"field":"y","type":"quantitative"},"color":{"field":"c","type":"nominal"}}}
```
### small multiples with column facet
```vegalite
{"data":{"values":[{"c":"A","x":1,"y":2,"g":"u"},{"c":"B","x":2,"y":5,"g":"v"},{"c":"C","x":3,"y":3,"g":"u"}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"y","type":"quantitative"},"column":{"field":"g","type":"nominal"}}}
```
