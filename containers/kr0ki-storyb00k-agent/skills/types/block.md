# Skill: Block diagram (`block`, format `blockdiag`)
## Choose it when
- The reader wants the big picture: which major parts exist and what hands off to what.
- You are sketching an idea or pipeline early, before details are settled.
- A single left-to-right chain with a few branches tells the whole story.
## Not when
- Component interfaces and dependencies matter: use `component`.
- Decisions and branches in a process matter: use `flowchart`.
- Messages over time between parties matter: use `sequence`.
## Anatomy
- A block is one part, stage or system; its label is a noun phrase.
- An arrow is a hand-off of data or control; its direction is the flow.
- A group boxes blocks that belong together (a team, a tier, a trust zone).
- Shape and colour are the only extra channels: use them to mark a kind, not to decorate.
## What makes it good
- Five to nine blocks; split into a second diagram past about twelve.
- Flow runs one direction (left to right, or top to bottom) with few crossing edges.
- Labels are short nouns; edge labels say what travels (`events`, `rows`), not "calls".
- Groups reflect a real boundary the reader cares about, and each block sits in at most one.
- Where an identifier exists, keep it as the node name and put the display name in `label`.
## What makes it bad
- A hub block with every other block wired to it, which shows nothing about order.
- Arrows with no meaning, or a mix of "depends on" and "sends data to" in one diagram.
- Every block the same colour, or every block a different colour.
- Verbs or sentences inside blocks.
- Mixing abstraction levels: a "Database" next to a "login button".
## Questions to ask
- What are the main parts, in the order data or work moves through them?
- Are there parts that belong together (a team, a layer, a zone)?
- What travels on the arrows?
## Contrast
Bad:
```blockdiag
blockdiag {
  A -> B;
  A -> C;
  A -> D;
  A -> E;
  B -> E;
}
```
Good:
```blockdiag
blockdiag {
  orientation = landscape;
  src [label = "Source files"];
  build [label = "Build"];
  test [label = "Test"];
  pub [label = "Publish"];
  src -> build [label = "code"];
  build -> test [label = "artifact"];
  test -> pub [label = "approved"];
  group ci {
    label = "CI";
    color = "#DDEEFF";
    build; test;
  }
}
```
The good version names each block, labels what travels on each arrow, and groups the stages that share a boundary.
