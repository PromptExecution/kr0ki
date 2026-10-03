# Rust behavioral evidence contract

`RustBehaviorIr` is the versioned source-evidence input to the existing
`ufo-types::SysGraph` pipeline. It is not a second semantic ontology. Deserialize
with `RustBehaviorIr::from_json` and validate before lifting or rendering;
`canonical_json` refuses any error finding and sorts sets for repeatable output.
The exported Draft 2020-12 `json_schema()` describes the wire format. Hashes,
UTF-8 byte spans, endpoint compatibility, and transition integrity additionally
require the Rust validator.

Source paths are relative, portable paths. Anchors point into embedded,
SHA-256-checked source content. The full Git revision, source tree SHA-256,
toolchain, extractor, and build configuration identify the analysis. Inferred
and unresolved edges remain explicit deterministic findings. Loop termination
and dynamic dispatch without proven targets remain warnings. Embedded extractor
errors prevent serialization as a valid artifact.

`StateMachineRuntime` and `StateMachine::to_scxml` consume the same transition
table. Applications bind named guards and effects through `RuntimeContext`;
guards are pure predicates. Ambiguous enabled transitions fail before effects.
A failed effect leaves the executor state and trace unchanged; the application
is responsible for any external side effects of its callback. Terminal states
reject further events.

SCXML output uses standard states, transitions and final states with application
guard bindings in `cond`, extension effect elements, and original identifiers
and source anchors in the `kr0ki` namespace. There is no emitted script. An
external SCXML executor must bind those guards and extension effects. Tests
parse the XML and verify table equivalence; they do not claim full SCXML grammar
or external-executor conformance.

The JSON fixtures are reviewed synthetic test facts and a runnable OODA-like
table. `tests/fixtures/state_machine` consumes that exact table and prints the
execution trace compared with `ooda-trace.json`. Its lockfile is tracked;
generated artifacts and fixture build directories remain ignored.
