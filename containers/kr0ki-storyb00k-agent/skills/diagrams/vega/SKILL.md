---
name: kr0ki-vega
description: Use before writing or fixing Vega (not Vega-Lite) visualization specs for render_diagram (format vega): strict JSON, scales/marks wiring, inline data only, error meanings, worked examples, identifier rule.
---
# vega (render_diagram format `vega`)

## Rules (each checked against the renderer)
- Source is a full **Vega** spec as strict JSON (double quotes, no trailing commas). Vega-Lite specs (`mark`/`encoding`) belong to format `vegalite`: sent to `vega` they return HTTP 200 with an empty `width="0" height="0"` SVG and no error.
- Data must be inline `{"name":"t","values":[...]}`. `"url"` fails: `Unable to load data from an URL while running in secure mode`.
- Everything referenced must be declared: `Unrecognized scale name: "y"` (mark uses a scale you did not define), `Undefined data set name: "nope"` (bad `from.data`), `Unrecognized scale type: "ordinal2"`, `Unrecognized scale range value: "w"` (use `"width"`, `"height"`, or an array).
- Mark `type` must be a Vega mark (`rect`, `line`, `symbol`, `arc`, `text`, `area`, ...). `"type":"bar"` fails with `Cannot read properties of undefined (reading 'bound')`; use `rect` for bars.
- A wrong `field` name does not error; the marks just misplace or vanish. `width`/`height` may be omitted, and `$schema` is optional.
- Pie: data `transform` `{"type":"pie","field":"v"}` then an `arc` mark with `startAngle`/`endAngle` from the data. Stacked bars: `{"type":"stack","groupby":["c"],"field":"v"}` then `y0`/`y1` fields.

## Identifiers
Keep the identifier as a data field and map it through a scale: `"data":[{"name":"t","values":[{"id":"n0002","label":"engine","v":3}]}]` with `{"scale":"x","field":"id"}` for position and a `text` mark using `{"field":"label"}` for the display name. Never invent identifiers.

## Verified examples (all render)
### bar chart with band and linear scales
```vega
{"width":200,"height":100,"data":[{"name":"t","values":[{"c":"A","v":3,"g":"u"},{"c":"B","v":5,"g":"u"},{"c":"C","v":2,"g":"v"}]}],"scales":[{"name":"x","type":"band","domain":{"data":"t","field":"c"},"range":"width","padding":0.1},{"name":"y","type":"linear","domain":{"data":"t","field":"v"},"range":"height","nice":true}],"axes":[{"orient":"bottom","scale":"x"},{"orient":"left","scale":"y"}],"marks":[{"type":"rect","from":{"data":"t"},"encode":{"enter":{"x":{"scale":"x","field":"c"},"width":{"scale":"x","band":1},"y":{"scale":"y","field":"v"},"y2":{"scale":"y","value":0},"fill":{"value":"steelblue"}}}}]}
```
### line with symbols
```vega
{"width":200,"height":100,"data":[{"name":"t","values":[{"x":1,"y":2},{"x":2,"y":5},{"x":3,"y":3}]}],"scales":[{"name":"x","type":"linear","domain":{"data":"t","field":"x"},"range":"width"},{"name":"y","type":"linear","domain":{"data":"t","field":"y"},"range":"height","nice":true}],"axes":[{"orient":"bottom","scale":"x"},{"orient":"left","scale":"y"}],"marks":[{"type":"line","from":{"data":"t"},"encode":{"enter":{"x":{"scale":"x","field":"x"},"y":{"scale":"y","field":"y"},"stroke":{"value":"#c00"}}}},{"type":"symbol","from":{"data":"t"},"encode":{"enter":{"x":{"scale":"x","field":"x"},"y":{"scale":"y","field":"y"}}}}]}
```
### pie chart with pie transform and arc mark
```vega
{"width":160,"height":160,"data":[{"name":"t","values":[{"c":"A","v":3,"g":"u"},{"c":"B","v":5,"g":"u"},{"c":"C","v":2,"g":"v"}],"transform":[{"type":"pie","field":"v"}]}],"scales":[{"name":"col","type":"ordinal","domain":{"data":"t","field":"c"},"range":{"scheme":"category10"}}],"legends":[{"fill":"col"}],"marks":[{"type":"arc","from":{"data":"t"},"encode":{"enter":{"x":{"signal":"width/2"},"y":{"signal":"height/2"},"fill":{"scale":"col","field":"c"},"startAngle":{"field":"startAngle"},"endAngle":{"field":"endAngle"},"outerRadius":{"signal":"width/2"}}}}]}
```
### stacked bars with stack transform and color legend
```vega
{"width":200,"height":100,"data":[{"name":"t","values":[{"c":"A","g":"u","v":3},{"c":"A","g":"w","v":2},{"c":"B","g":"u","v":4},{"c":"B","g":"w","v":1}],"transform":[{"type":"stack","groupby":["c"],"field":"v"}]}],"scales":[{"name":"x","type":"band","domain":{"data":"t","field":"c"},"range":"width","padding":0.1},{"name":"y","type":"linear","domain":{"data":"t","field":"y1"},"range":"height","nice":true},{"name":"col","type":"ordinal","domain":{"data":"t","field":"g"},"range":"category"}],"axes":[{"orient":"bottom","scale":"x"},{"orient":"left","scale":"y"}],"legends":[{"fill":"col"}],"marks":[{"type":"rect","from":{"data":"t"},"encode":{"enter":{"x":{"scale":"x","field":"c"},"width":{"scale":"x","band":1},"y":{"scale":"y","field":"y0"},"y2":{"scale":"y","field":"y1"},"fill":{"scale":"col","field":"g"}}}}]}
```
### text labels from data
```vega
{"width":200,"height":60,"data":[{"name":"t","values":[{"c":"A","v":3,"g":"u"},{"c":"B","v":5,"g":"u"},{"c":"C","v":2,"g":"v"}]}],"scales":[{"name":"x","type":"point","domain":{"data":"t","field":"c"},"range":"width","padding":0.5}],"marks":[{"type":"text","from":{"data":"t"},"encode":{"enter":{"x":{"scale":"x","field":"c"},"y":{"value":30},"text":{"field":"v"},"align":{"value":"center"},"fontSize":{"value":18}}}}]}
```
