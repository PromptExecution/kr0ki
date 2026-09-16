//! The executable playb00k example catalog.
//!
//! One fixture per supported [`crate::format::DiagramFormat`] slug — same source used
//! by the live HTTP contract test (`kr0ki-server/tests/playbook_live.rs`), the static
//! mdb00k export (`docgen::export_static_mdb00k`), and the `/playbook/` Vue UI
//! (`GET /api/examples`). Regenerated from `fixtures/playbook-examples.json` — keep the
//! two in lockstep; the fixture is the editable source of truth.

use serde::Serialize;

/// One renderable example: enough to drive a render call and describe it in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct PlaybookExample {
    pub id: &'static str,
    /// A [`crate::format::DiagramFormat`] kroki slug, e.g. `"d2"`.
    pub format: &'static str,
    pub title: &'static str,
    pub input_kind: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    /// Output kinds this example renders as, e.g. `&["svg", "png"]`.
    pub outputs: &'static [&'static str],
}

/// The full catalog: one example per [`crate::format::DiagramFormat::ALL`] entry.
pub const ALL: &[PlaybookExample] = &[
    PlaybookExample {
        id: "plantuml-request",
        format: "plantuml",
        title: "Operator render request",
        input_kind: "procedural code",
        description: "A request crosses kr0ki and its artifact cache.",
        source: "@startuml\nactor Operator\nrectangle kr0ki\ndatabase Cache\nOperator -> kr0ki : render source\nkr0ki -> Cache : read/write artifact\n@enduml\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "c4-context",
        format: "c4plantuml",
        title: "Renderer context",
        input_kind: "architecture model",
        description: "Operator, kr0ki, and the Kroki backend in a C4 context.",
        source: "@startuml\n!include <C4/C4_Context>\nPerson(operator, \"Operator\")\nSystem(kr0ki, \"kr0ki\", \"cached diagram renderer\")\nSystem_Ext(kroki, \"Kroki\", \"diagram backend\")\nRel(operator, kr0ki, \"submits source\")\nRel(kr0ki, kroki, \"renders\")\n@enduml\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "graphviz-pipeline",
        format: "graphviz",
        title: "IAC to artifact pipeline",
        input_kind: "IAC / code",
        description: "The shared IAC → procedural diagram → Kroki → artifact story.",
        source: "digraph kr0ki {\n  rankdir=LR;\n  input [label=\"IAC / code\"];\n  diagram [label=\"procedural diagram\"];\n  kroki [label=\"Kroki\"];\n  artifact [label=\"SVG / PNG\"];\n  input -> diagram -> kroki -> artifact;\n}\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "d2-rust-flow",
        format: "d2",
        title: "Rust render flow",
        input_kind: "Rust code flow",
        description: "The Box-5 route, cache, backend, and artifact flow used by /docs.",
        source: "# kr0ki Box-5 Rust code-flow — executable playb00k view source.\ndirection: right\nclient: HTTP caller { shape: person }\nrouter: Axum router { shape: hexagon }\nservice: RenderService { shape: rectangle; style.fill: \"#d8eaff\" }\ncache: FsCache { shape: cylinder; style.fill: \"#e9f7df\" }\nbackend: HttpKrokiBackend { shape: rectangle; style.fill: \"#fff0cc\" }\nkroki: Kroki renderer { shape: cloud }\nartifact: SVG or PNG { shape: document; style.fill: \"#f7e6ff\" }\nclient -> router: POST /render/:format\nrouter -> service: route + auth\nservice -> cache: lookup\ncache -> service: hit or miss\nservice -> backend: miss only\nbackend -> kroki: render\nkroki -> backend: bytes\nbackend -> service: artifact\nservice -> cache: atomic write\nservice -> client: headers + bytes\nclient -> artifact: receives\n",
        outputs: &["svg"],
    },
    PlaybookExample {
        id: "vegalite-cache-outcomes",
        format: "vegalite",
        title: "Cache outcome metrics",
        input_kind: "structured telemetry",
        description: "A small declarative visualization of cache hit/miss outcomes.",
        source: "{\n  \"$schema\": \"https://vega.github.io/schema/vega-lite/v5.json\",\n  \"description\": \"kr0ki render cache outcomes\",\n  \"data\": {\"values\": [{\"status\":\"miss\",\"count\":1},{\"status\":\"hit\",\"count\":3}]},\n  \"mark\": \"bar\",\n  \"encoding\": {\n    \"x\": {\"field\":\"status\",\"type\":\"nominal\"},\n    \"y\": {\"field\":\"count\",\"type\":\"quantitative\"},\n    \"color\": {\"field\":\"status\",\"type\":\"nominal\"}\n  }\n}\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "ditaa-artifact-flow",
        format: "ditaa",
        title: "ASCII artifact flow",
        input_kind: "ASCII IAC sketch",
        description: "A text-only source converted to a procedural diagram.",
        source: "+---------+      +--------+      +---------+\n| IAC/code|----->| kr0ki  |----->| SVG/PNG |\n+---------+      +--------+      +---------+\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "nomnoml-components",
        format: "nomnoml",
        title: "Renderer components",
        input_kind: "component model",
        description: "A concise component relation view of the render loop.",
        source: "[IAC / code]->[RenderService]\n[RenderService]->[FsCache]\n[RenderService]->[Kroki]\n[Kroki]->[SVG / PNG]\n",
        outputs: &["svg", "png"],
    },
    PlaybookExample {
        id: "wavedrom-cache-timing",
        format: "wavedrom",
        title: "Cache timing",
        input_kind: "procedural timing model",
        description: "The request, cache, and backend sequence as a timing diagram.",
        source: "{ \"signal\": [\n  { \"name\": \"render request\", \"wave\": \"01..\" },\n  { \"name\": \"cache\", \"wave\": \"x3.4\", \"data\": [\"lookup\", \"hit\", \"artifact\"] },\n  { \"name\": \"backend\", \"wave\": \"0.1.\" }\n]}\n",
        outputs: &["svg"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::DiagramFormat;

    #[test]
    fn covers_every_diagram_format_exactly_once() {
        for &format in DiagramFormat::ALL {
            let matches: Vec<_> = ALL
                .iter()
                .filter(|example| example.format == format.kroki_slug())
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "expected exactly one example for {}, found {}",
                format.kroki_slug(),
                matches.len()
            );
        }
        assert_eq!(ALL.len(), DiagramFormat::ALL.len());
    }

    #[test]
    fn every_example_format_and_output_are_recognised() {
        for example in ALL {
            example
                .format
                .parse::<DiagramFormat>()
                .unwrap_or_else(|error| panic!("{}: {error}", example.id));
            assert!(
                !example.outputs.is_empty(),
                "{}: no outputs listed",
                example.id
            );
            for output in example.outputs {
                assert!(
                    crate::cache::OutputKind::from_param(output).is_some(),
                    "{}: unrecognised output {output}",
                    example.id
                );
            }
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = ALL.iter().map(|example| example.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len(), "duplicate example id");
    }
}
