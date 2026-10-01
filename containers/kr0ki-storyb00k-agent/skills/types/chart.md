# Skill: Data chart (`chart`, format `vegalite`)

## Choose it when
- The reader wants to compare quantities, see a trend over time, or spot an outlier.
- There is real data to plot (counts, durations, rates) and one clear comparison.
- A single takeaway can be written as a title ("Cache misses doubled in March").

## Not when
- You are showing structure or connections, not numbers: use `component` or `erd`.
- You are showing ordered steps or messages: use `sequence` or `activity`.
- You have no data, only a concept: do not invent numbers; draw a structural type instead.

## Anatomy
- Data rows are inline `values`; each row is one observation with named fields.
- A mark (`bar`, `line`, `point`, `arc`) is the shape that encodes a row.
- Encoding channels (`x`, `y`, `color`, `theta`) map fields to position or colour; each needs a `type`: `nominal` (category), `ordinal` (ordered category), `quantitative` (number), `temporal` (time).
- Axis titles and units say what the numbers measure; the chart `title` says the point.

## What makes it good
- One comparison per chart; the title states the finding, not just the topic.
- Pick the mark for the question: bars for comparing categories, lines for change over time, points for relationships.
- Bars start at zero; if a baseline is truncated, say so in the title or an axis note.
- Axes carry titles with units; categories are sorted deliberately (by value or by natural order), not by accident.
- Limit series to about 5 colours, use direct value labels for few bars, and keep a category's key stable (id as the nominal field, display name in a label or tooltip field).

## What makes it bad
- Pie or donut charts with many slices, or with values too close to compare.
- A truncated y-axis that exaggerates a small difference.
- No title, no units, so the numbers mean nothing.
- Too many series or colours, a rainbow where one highlight colour would do.
- Plotting invented or rounded-to-fiction numbers; a chart must say where the data comes from.

## Questions to ask
- What single comparison or trend should the reader take away?
- What are the data points (categories, time range, units), and where do they come from?
- Should one item be highlighted, such as the latest period or a target?

## Contrast
Bad:
```vegalite
{"data":{"values":[{"c":"hit","n":940},{"c":"miss","n":60}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"n","type":"quantitative"}}}
```
Good:
```vegalite
{"data":{"values":[{"c":"hit","n":940},{"c":"miss","n":60}]},"title":"Cache hits dominate misses (requests)","width":200,"height":140,"layer":[{"mark":"bar","encoding":{"x":{"field":"c","type":"nominal","title":"Outcome","sort":"-y"},"y":{"field":"n","type":"quantitative","title":"Requests"}}},{"mark":{"type":"text","dy":-6},"encoding":{"x":{"field":"c","type":"nominal","sort":"-y"},"y":{"field":"n","type":"quantitative"},"text":{"field":"n","type":"quantitative"}}}]}
```
The good version states the finding in the title, labels axes with units, sorts by value, and prints the values on the bars.
