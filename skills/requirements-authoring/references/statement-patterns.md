# Statement patterns

Guidance from the NASA Systems Engineering Handbook, Appendix C: use **shall** for
requirements, **will** for facts and **should** for goals; give each requirement an intelligible
rationale; state *what* is needed, not *how*; make it verifiable (can it be tested,
demonstrated, inspected or analysed?). One `shall` per requirement is this repository's own
authoring rule, not NASA's wording.

| | statement | why |
|---|---|---|
| good | The tool gateway shall deny operations outside the caller identity's capability grant. | one `shall`, a component, an observable refusal |
| good | The verification runner shall bind each result to the model, implementation and configuration revisions used. | testable: change a revision, see the result stale |
| bad | The gateway denies writes. | no `shall`: a fact, not a requirement |
| bad | The gateway shall deny writes and shall log them. | two obligations; split into two requirements |
| bad | Shall deny writes. | no responsible component |
| bad | The UI shall be user-friendly and/or fast, TBD. | vague, open, untestable |

## What the lint sees (and what it cannot)

Sees: missing `shall`; more than one `shall`; nothing before `shall`; the terms `TBD`, `TBC`,
`etc`, `and/or`, `as appropriate`, `user-friendly`, `adequate`, `if possible`, `where practical`.

Cannot see: whether the behaviour is observable, whether two requirements contradict, whether a
term is defined, whether the statement leaks a design choice. That is your judgment; quote the
words and propose a rewrite.
