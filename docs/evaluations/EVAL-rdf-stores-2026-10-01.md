# H-rdf-stores: RDF store / SHACL evaluation for kr0ki

Date 2026-10-01. Everything below is VERIFIED by crates.io API, `gh`, source reading or a run, unless marked UNVERIFIED.

## What kr0ki has today
`crates/kr0ki-core/src/graph_store.rs` is NOT a triple store. It is a `Mutex<Vec<Triple>>` of `oxrdf` 0.3 types (data model only, no parser, no query engine). It offers two hard-coded query shapes (`TriplesAbout`, `RelatedVia`) and one structural check. Its own doc says "neither a triplestore nor a SPARQL endpoint". Cargo.toml comment: oxrdf only, "not the full Oxigraph store".

## Candidates

| | oxigraph | grafeo | zu (tamnd/zu, crate `zudb`) | rudof shacl_validation |
|---|---|---|---|---|
| Version/date | 0.5.11, 2026-09-02 | 0.5.43, 2026-09-27 | zudb not checked; repo created 2026-08-06; crates.io `zu` is an unrelated Yew crate (LGPL) | shacl_validation 0.2.12 (2026-04-22); rudof_lib 0.3.24 (2026-09-27) |
| License | MIT OR Apache-2.0 | Apache-2.0 | Apache-2.0 (repo) | MIT OR Apache-2.0 |
| Activity | 1969 stars, push 2026-10-01, since 2018, 812k downloads | 797 stars, push 2026-10-01, created 2026-01, 23k downloads, ~weekly releases | 2 stars; README: "Nothing is usable yet" | rudof: 130 stars, push 2026-09-30 |
| Query | SPARQL 1.1 (query, update, federated); SPARQL 1.2 optional (`rdf-12`) | GQL, Cypher, Gremlin, GraphQL, SPARQL 1.1, SQL/PGQ | property-graph, GQL-like MATCH; no RDF, no SPARQL | n/a (validator) |
| SHACL | none built in | built in (`shacl` feature, Core + SHACL-SPARQL claimed) | none | W3C SHACL Core; native engine + SPARQL engine |
| RDF I/O | Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD, N3 (oxrdfio) | Turtle, N-Triples (seen in code); RDF/XML, JSON-LD UNVERIFIED | none | Turtle etc. via rudof_rdf |
| Persistence | in-memory or RocksDB (default feature, C++ build) | memory, WAL, single file (`grafeo-file`), no C deps | zu1 file, SQLite, S3 | in-memory graph |
| Embed as crate | yes | yes | yes, but not usable | yes |
| WASM | yes: `js` feature, `cargo build --target wasm32-unknown-unknown` OK (53s, 0.5 MB wasm from the spike) | npm `@grafeo-db/wasm` exists, built from GQL/LPG plus optional `rdf-model`; SPARQL/SHACL in WASM UNVERIFIED. My direct build with `rdf,sparql,shacl` for wasm32 failed (getrandom needs `wasm_js`; fixable, not pursued) | n/a | `from_path` is cfg-gated off on wasm; WASM otherwise UNVERIFIED |
| Clean release build (4 cores) | 105 s (no RocksDB); RocksDB variant UNVERIFIED (C++) | 278 s (rdf+sparql+shacl) | not built | ~250 s total incl. failed attempts; 27 s once pinned |
| Binary (spike, release, stripped? no) | 6.9 MB | 9.4 MB | n/a | 16 MB (pulls rudof stack) |

zu: not an RDF store, pre-alpha. Dropped, no spike.
Others: sophia 0.10.0 (RDF API, no store or SHACL engine) and rdf-fusion 0.2.1 (DataFusion SPARQL, 1.2k downloads) were identified from crates.io only. Not spiked. Jena/Fuseki, QLever, pySHACL not evaluated (non-Rust, would be a sidecar).

## Spike data
12 requirements R1..R12 with `ex:id`, `ex:cost`, `ex:dependsOn` (R12 lacks cost, R13 has `derivedFrom` and lacks id, both on purpose). 50 triples.

## Spike results (release build, in-memory, timings are one run)

| Query | oxigraph 0.5.11 | grafeo 0.5.43 |
|---|---|---|
| (a) sub-select MAX(cost) then filter | OK 0.27 ms, R11=110 | OK 1.1 ms, R11=110 |
| (b) `?r ex:dependsOn+ ex:R1` | OK 8 rows, 0.08 ms | OK 8 rows, 106 ms |
| (c) `SUM(?c)` over that set | OK total=350 (xsd:integer), n=7, 0.1 ms | OK but returns Float64(350): integer typing lost; 98 ms |
| (c2) sub-select roots + `dependsOn*` + GROUP BY | OK 3 groups: R1=360, R9=300, R13=5 | WRONG: 2 groups, R13 (zero-length path, cost 5) missing |
| (c3) `(ex:dependsOn\|ex:derivedFrom)+` | OK 355, 0.09 ms | ERROR `semantic error: Complex property paths not yet supported` |
| (d) SHACL | none built in; use companion | OK, 0.15 ms, 2 violations (R12 no cost, R13 no id) |

Hand check: R1 dependants R2..R8,R12 give cost sum 20+...+80 = 350. Correct.

SHACL companion: rudof `shacl_validation` 0.2.12 with oxigraph-independent oxrdf graph. A plain `cargo add` FAILED to compile (semver break inside the rudof 0.2.x family: `iri_s::IriS` vs `rudof_iri::IriS`). Fix: pin `rudof_rdf=0.2.9, prefixmap=0.2.9, sparql_service=0.2.9, mie=0.2.9`. Then native mode: OK, same 2 violations as grafeo, 0.46 ms. SPARQL mode returned 0 results on the same data (disagrees with native; UNVERIFIED why). Validation runs on its own graph, so data must be handed over (serialize oxigraph store to Turtle, or re-parse). rudof_lib 0.3.24 is the newer, maintained facade but was not tried (UNVERIFIED whether it avoids the pin problem).

## Notes
- Oxigraph is the only candidate that is correct on every query, preserves xsd:integer, and handles `|` paths. Cost simulation needs exact numerics; grafeo's Float64 coercion and the missing zero-length-path row are real risks for a roll-up.
- grafeo is young (created Jan 2026) but the only one with SHACL in one dependency. Its gaps found here are query-engine gaps, not data-model ones.
- Oxigraph's `Store::new()` needs no RocksDB with `default-features=false`, so it is a pure-Rust dependency.

## RECOMMENDATION
1. Server: **oxigraph 0.5.x, `default-features=false`** (in-memory; add `rocksdb` only if persistence is needed). It is a drop-in upgrade for `GraphStore`: kr0ki already uses `oxrdf`, which oxigraph shares.
2. Browser: yes. Oxigraph builds for wasm32-unknown-unknown with the `js` feature (verified build; running it in a browser UNVERIFIED). Grafeo has an npm WASM package but SPARQL/SHACL in it is UNVERIFIED.
3. SHACL: a separate step. Use rudof `shacl_validation` (native mode) with the pinned versions above, running over a Turtle export of the oxigraph store. Alternative if Rust dependency churn hurts: grafeo only as a SHACL validator (it passed here), or sh:SPARQL-style checks written as plain SPARQL ASK/SELECT in oxigraph (no extra crate; recommended for the first iteration given the 2 simple shapes needed). Re-check rudof_lib 0.3.x before committing.
4. Reject zu (no RDF, not usable). Revisit grafeo once its property-path and numeric-typing bugs are fixed.

## Spike code
- /tmp/claude-1000/rdf-eval/data/reqs.ttl, shapes.ttl
- /tmp/claude-1000/rdf-eval/oxi (oxigraph, native + wasm), graf (grafeo), shacl (rudof). Target dirs: /tmp/claude-1000/rdf-eval/target-*
