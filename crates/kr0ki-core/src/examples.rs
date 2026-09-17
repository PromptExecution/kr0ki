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
    /// A [`crate::format::DiagramFormat`] kroki slug (e.g. `"d2"`) when
    /// `route` is `None`; a display label when `route` is `Some` (kr0ki#30 —
    /// a custom-route example isn't a Kroki-family diagram format, so this
    /// stops meaning "parses as `DiagramFormat`" for those entries).
    pub format: &'static str,
    pub title: &'static str,
    pub input_kind: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    /// Output kinds this example renders as, e.g. `&["svg", "png"]`.
    pub outputs: &'static [&'static str],
    /// `None` (every `DiagramFormat` example): the caller builds
    /// `/render/{format}` itself (`Gallery.vue`/`RendererPanel.vue`).
    /// `Some(path)`: use this exact path instead — for a capability like
    /// `POST /render/k8s-topology` whose input isn't diagram-format source
    /// text at all, so it can't be reached by templating `format` into
    /// `/render/{format}`.
    pub route: Option<&'static str>,
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
        route: None,
    },
    PlaybookExample {
        id: "c4-context",
        format: "c4plantuml",
        title: "Renderer context",
        input_kind: "architecture model",
        description: "Operator, kr0ki, and the Kroki backend in a C4 context.",
        source: "@startuml\n!include <C4/C4_Context>\nPerson(operator, \"Operator\")\nSystem(kr0ki, \"kr0ki\", \"cached diagram renderer\")\nSystem_Ext(kroki, \"Kroki\", \"diagram backend\")\nRel(operator, kr0ki, \"submits source\")\nRel(kr0ki, kroki, \"renders\")\n@enduml\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "graphviz-pipeline",
        format: "graphviz",
        title: "IAC to artifact pipeline",
        input_kind: "IAC / code",
        description: "The shared IAC → procedural diagram → Kroki → artifact story.",
        source: "digraph kr0ki {\n  rankdir=LR;\n  input [label=\"IAC / code\"];\n  diagram [label=\"procedural diagram\"];\n  kroki [label=\"Kroki\"];\n  artifact [label=\"SVG / PNG\"];\n  input -> diagram -> kroki -> artifact;\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "d2-rust-flow",
        format: "d2",
        title: "Rust render flow",
        input_kind: "Rust code flow",
        description: "The Box-5 route, cache, backend, and artifact flow used by /docs.",
        source: "# kr0ki Box-5 Rust code-flow — executable playb00k view source.\ndirection: right\nclient: HTTP caller { shape: person }\nrouter: Axum router { shape: hexagon }\nservice: RenderService { shape: rectangle; style.fill: \"#d8eaff\" }\ncache: FsCache { shape: cylinder; style.fill: \"#e9f7df\" }\nbackend: HttpKrokiBackend { shape: rectangle; style.fill: \"#fff0cc\" }\nkroki: Kroki renderer { shape: cloud }\nartifact: SVG or PNG { shape: document; style.fill: \"#f7e6ff\" }\nclient -> router: POST /render/:format\nrouter -> service: route + auth\nservice -> cache: lookup\ncache -> service: hit or miss\nservice -> backend: miss only\nbackend -> kroki: render\nkroki -> backend: bytes\nbackend -> service: artifact\nservice -> cache: atomic write\nservice -> client: headers + bytes\nclient -> artifact: receives\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "vegalite-cache-outcomes",
        format: "vegalite",
        title: "Cache outcome metrics",
        input_kind: "structured telemetry",
        description: "A small declarative visualization of cache hit/miss outcomes.",
        source: "{\n  \"$schema\": \"https://vega.github.io/schema/vega-lite/v5.json\",\n  \"description\": \"kr0ki render cache outcomes\",\n  \"data\": {\"values\": [{\"status\":\"miss\",\"count\":1},{\"status\":\"hit\",\"count\":3}]},\n  \"mark\": \"bar\",\n  \"encoding\": {\n    \"x\": {\"field\":\"status\",\"type\":\"nominal\"},\n    \"y\": {\"field\":\"count\",\"type\":\"quantitative\"},\n    \"color\": {\"field\":\"status\",\"type\":\"nominal\"}\n  }\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "ditaa-artifact-flow",
        format: "ditaa",
        title: "ASCII artifact flow",
        input_kind: "ASCII IAC sketch",
        description: "A text-only source converted to a procedural diagram.",
        source: "+---------+      +--------+      +---------+\n| IAC/code|----->| kr0ki  |----->| SVG/PNG |\n+---------+      +--------+      +---------+\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "nomnoml-components",
        format: "nomnoml",
        title: "Renderer components",
        input_kind: "component model",
        description: "A concise component relation view of the render loop.",
        source: "[IAC / code]->[RenderService]\n[RenderService]->[FsCache]\n[RenderService]->[Kroki]\n[Kroki]->[SVG / PNG]\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "wavedrom-cache-timing",
        format: "wavedrom",
        title: "Cache timing",
        input_kind: "procedural timing model",
        description: "The request, cache, and backend sequence as a timing diagram.",
        source: "{ \"signal\": [\n  { \"name\": \"render request\", \"wave\": \"01..\" },\n  { \"name\": \"cache\", \"wave\": \"x3.4\", \"data\": [\"lookup\", \"hit\", \"artifact\"] },\n  { \"name\": \"backend\", \"wave\": \"0.1.\" }\n]}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "vega-cache-outcomes-bar",
        format: "vega",
        title: "Cache outcomes, low-level grammar",
        input_kind: "declarative visualization",
        description: "The Vega grammar the vegalite fixture's spec compiles down to conceptually.",
        source: "{\n  \"$schema\": \"https://vega.github.io/schema/vega/v5.json\",\n  \"width\": 200, \"height\": 100, \"padding\": 5,\n  \"data\": [{\"name\": \"table\", \"values\": [{\"category\":\"miss\",\"amount\":1},{\"category\":\"hit\",\"amount\":3}]}],\n  \"scales\": [\n    {\"name\":\"xscale\",\"type\":\"band\",\"domain\":{\"data\":\"table\",\"field\":\"category\"},\"range\":\"width\",\"padding\":0.2},\n    {\"name\":\"yscale\",\"domain\":{\"data\":\"table\",\"field\":\"amount\"},\"nice\":true,\"range\":\"height\"}\n  ],\n  \"marks\": [{\"type\":\"rect\",\"from\":{\"data\":\"table\"},\"encode\":{\"enter\":{\"x\":{\"scale\":\"xscale\",\"field\":\"category\"},\"width\":{\"scale\":\"xscale\",\"band\":1},\"y\":{\"scale\":\"yscale\",\"field\":\"amount\"},\"y2\":{\"scale\":\"yscale\",\"value\":0}}}}]\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "blockdiag-pipeline",
        format: "blockdiag",
        title: "Render pipeline blocks",
        input_kind: "block diagram",
        description: "client -> router -> service -> cache as block-diag boxes.",
        source: "blockdiag {\n  client -> router -> service -> cache;\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "actdiag-request-flow",
        format: "actdiag",
        title: "Request activity flow",
        input_kind: "activity diagram",
        description: "The request's path through the service and cache as an activity diagram.",
        source: "actdiag {\n  client -> service -> cache;\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "seqdiag-request-sequence",
        format: "seqdiag",
        title: "Request sequence",
        input_kind: "sequence diagram",
        description: "client, service, cache, and backend as a seqdiag sequence.",
        source: "seqdiag {\n  client -> service -> cache -> backend;\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "nwdiag-topology",
        format: "nwdiag",
        title: "kr0ki / kroki network topology",
        input_kind: "network diagram",
        description: "The two services on one local network segment.",
        source: "nwdiag {\n  network kr0ki {\n    kr0ki;\n    kroki;\n  }\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "packetdiag-cache-key",
        format: "packetdiag",
        title: "Cache key packet layout",
        input_kind: "packet diagram",
        description: "A packet-style field layout, reused here for the cache key.",
        source: "packetdiag {\n  0-15: Cache Key Hash\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "rackdiag-deployment",
        format: "rackdiag",
        title: "Local pod rack layout",
        input_kind: "rack diagram",
        description: "kr0ki-server and kroki as rack units in the local k0s pod.",
        source: "rackdiag {\n  1: kr0ki-server\n  2: kroki\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "erd-render-request",
        format: "erd",
        title: "Render request entity",
        input_kind: "entity relationship",
        description: "A minimal entity-relationship fixture.",
        source: "[render] {label: \"render request\"}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "umlet-class",
        format: "umlet",
        title: "kr0ki class sketch",
        input_kind: "UML sketch",
        description: "A single UMLet class element.",
        source: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<diagram program=\"umlet\" version=\"15.1\">\n  <help_text></help_text>\n  <zoom_level>10</zoom_level>\n  <element>\n    <id>UMLClass</id>\n    <coordinates><x>10</x><y>10</y><w>140</w><h>50</h></coordinates>\n    <panel_attributes>kr0ki</panel_attributes>\n    <additional_attributes/>\n  </element>\n</diagram>\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "pikchr-pipeline",
        format: "pikchr",
        title: "kr0ki to kroki pipeline",
        input_kind: "pic-family diagram",
        description: "Two boxes and an arrow via pikchr's PIC-derived language.",
        source: "box \"kr0ki\"; arrow; box \"kroki\"\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "goat-ascii-pipeline",
        format: "goat",
        title: "ASCII pipeline (GoAT)",
        input_kind: "ASCII art diagram",
        description: "Hand-drawn ASCII boxes, rendered by GoAT.",
        source: "+-------+     +-------+\n| kr0ki | --> | kroki |\n+-------+     +-------+\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "bytefield-cache-key",
        format: "bytefield",
        title: "Cache key byte layout",
        input_kind: "byte field diagram",
        description: "A byte-field-style column header and box.",
        source: "(draw-column-headers)\n(draw-box \"cache key\")\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "dbml-cache-entry",
        format: "dbml",
        title: "Cache entry schema",
        input_kind: "database schema (DBML)",
        description: "The FsCache entry shape as a DBML table.",
        source: "Table cache_entry {\n  key varchar [pk]\n  bytes blob\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "tikz-line",
        format: "tikz",
        title: "Render pipeline (TikZ)",
        input_kind: "LaTeX/TikZ figure",
        description: "client -> service -> {cache, backend} as positioned TikZ nodes and arrows.",
        source: "\\begin{document}\n\\begin{tikzpicture}\n  \\node[draw, rounded corners] (client) at (0,0) {client};\n  \\node[draw, rounded corners] (service) at (3,0) {service};\n  \\node[draw, rounded corners] (cache) at (6,0) {cache};\n  \\node[draw, rounded corners] (backend) at (9,0) {backend};\n  \\draw[->] (client) -- (service);\n  \\draw[->] (service) -- (cache);\n  \\draw[->] (service) -- (backend);\n\\end{tikzpicture}\n\\end{document}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "svgbob-ascii-pipeline",
        format: "svgbob",
        title: "ASCII pipeline (svgbob)",
        input_kind: "ASCII art diagram",
        description: "The same two-box pipeline, rendered by svgbob.",
        source: "[kr0ki] --> [kroki]\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "wireviz-harness",
        format: "wireviz",
        title: "Renderer connector harness",
        input_kind: "wiring diagram (WireViz)",
        description: "Two connectors joined by a cable, in WireViz's YAML wiring-diagram DSL.",
        source: "connectors:\n  X1:\n    type: Molex KK 254\n    subtype: female\n    pinlabels: [GND, VCC]\n  X2:\n    type: Molex KK 254\n    subtype: male\n    pinlabels: [GND, VCC]\n\ncables:\n  W1:\n    colors: [BK, RD]\n\nconnections:\n  -\n    - X1: [1-2]\n    - W1: [1-2]\n    - X2: [1-2]\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "structurizr-context",
        format: "structurizr",
        title: "kr0ki system context",
        input_kind: "C4 model (Structurizr DSL)",
        description: "Operator, kr0ki (with its RenderService/FsCache containers), and Kroki as a Structurizr workspace with system-context and container views.",
        source: "workspace {\n    model {\n        user = person \"Operator\"\n        kr0ki = softwareSystem \"kr0ki\" {\n            renderer = container \"RenderService\"\n            cache = container \"FsCache\"\n        }\n        kroki = softwareSystem \"Kroki\"\n        user -> kr0ki \"Submits diagram source\"\n        kr0ki -> kroki \"Renders via\"\n        renderer -> cache \"Reads/writes\"\n    }\n    views {\n        systemContext kr0ki {\n            include *\n            autoLayout\n        }\n        container kr0ki {\n            include *\n            autoLayout\n        }\n    }\n}\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "symbolator-entity",
        format: "symbolator",
        title: "RenderService entity symbol",
        input_kind: "VHDL entity (Symbolator)",
        description: "RenderService's inputs and outputs as a VHDL entity black-box symbol.",
        source: "entity render_service is\n  Port ( source         : in  STD_LOGIC;\n         format_select  : in  STD_LOGIC;\n         request_output : in  STD_LOGIC;\n         artifact       : out STD_LOGIC;\n         cache_hit      : out STD_LOGIC);\nend render_service;\n",
        outputs: &["svg", "png"],
        route: None,
    },
    PlaybookExample {
        id: "k8s-topology-web-service",
        format: "k8s-topology",
        title: "Kubernetes topology (native recognizer)",
        input_kind: "Kubernetes manifests",
        description: "A Deployment, the ConfigMap it consumes, and the Service that selects it — recognized, lifted to SysML v2, and rendered as D2, not proxied through the vendored KubeDiagrams tool (see /render/kubediagram's own example... there isn't one; this is the only k8s example in this catalog, on purpose, to keep the two pipelines from being conflated).",
        source: "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app-config\n  namespace: demo\ndata:\n  LOG_LEVEL: info\n---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n  namespace: demo\n  labels:\n    app: web\nspec:\n  replicas: 2\n  selector:\n    matchLabels:\n      app: web\n  template:\n    metadata:\n      labels:\n        app: web\n    spec:\n      containers:\n        - name: web\n          image: web:latest\n          envFrom:\n            - configMapRef:\n                name: app-config\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: web\n  namespace: demo\nspec:\n  selector:\n    app: web\n  ports:\n    - port: 80\n",
        outputs: &["svg", "png"],
        route: Some("/render/k8s-topology"),
    },
    PlaybookExample {
        id: "rust-topology-render-backend",
        format: "rust-topology",
        title: "Rust code topology (native recognizer)",
        input_kind: "Rust source",
        description: "A render module — a Backend trait, a KrokiBackend that implements it and holds a Cache field, and a call from one free function to another — recognized, lifted to SysML v2, and rendered as D2, exercising all three PATTERNS-rust-source.md relation kinds (has_part, satisfies, flows_to) in one compact, self-contained file (single-file scope only — no cross-file resolution).",
        source: "mod render {\n    pub struct Artifact;\n    pub struct Cache;\n\n    pub trait Backend {\n        fn render(&self) -> Artifact;\n    }\n\n    pub struct KrokiBackend {\n        pub cache: Cache,\n    }\n\n    impl Backend for KrokiBackend {\n        fn render(&self) -> Artifact {\n            fetch()\n        }\n    }\n\n    fn fetch() -> Artifact {\n        Artifact\n    }\n\n    pub fn render_service() -> Artifact {\n        fetch()\n    }\n}\n",
        outputs: &["svg", "png"],
        route: Some("/render/rust-topology"),
    },
    PlaybookExample {
        id: "rust-isometric-render-backend",
        format: "rust-isometric",
        title: "Rust code topology (isometric, systhread-core)",
        input_kind: "Rust source",
        description: "The same relationships as the D2 recognizer example, rendered through a completely different backend: systhread-core's own layout (Cassowary/kasuari constraint solving) and SVG renderer (FR3), not Kroki — no output choice, always SVG.",
        source: "trait Drive {}\nstruct Engine;\nstruct Car {\n    engine: Engine,\n}\nimpl Drive for Car {}\nfn build() -> Car {\n    Car { engine: Engine }\n}\nfn main() {\n    build();\n}\n",
        outputs: &["svg"],
        route: Some("/render/rust-isometric"),
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
        let format_routed = ALL.iter().filter(|e| e.route.is_none()).count();
        assert_eq!(format_routed, DiagramFormat::ALL.len());
    }

    #[test]
    fn every_example_format_and_output_are_recognised() {
        for example in ALL {
            // A custom-route example's `format` is a display label, not a
            // `DiagramFormat` slug (see `PlaybookExample::route`'s docs) —
            // only format-routed examples must parse.
            if example.route.is_none() {
                example
                    .format
                    .parse::<DiagramFormat>()
                    .unwrap_or_else(|error| panic!("{}: {error}", example.id));
            }
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
    fn k8s_topology_example_actually_exercises_the_recognizer_pipeline() {
        use serde::Deserialize;
        let example = ALL
            .iter()
            .find(|e| e.id == "k8s-topology-web-service")
            .expect("k8s-topology-web-service example exists");

        let manifests: Vec<serde_json::Value> = serde_yaml::Deserializer::from_str(example.source)
            .map(serde_json::Value::deserialize)
            .collect::<Result<Vec<_>, _>>()
            .expect("example source is valid multi-doc YAML");
        assert_eq!(
            manifests.len(),
            3,
            "expected ConfigMap + Deployment + Service"
        );

        let recognizer = crate::k8s_recognizer::KubernetesRecognizer::new();
        let edges = recognizer.recognize(&manifests);
        assert!(
            !edges.is_empty(),
            "the example manifest produced zero recognized relationships -- \
             it would render as a bare unconnected node list, defeating the \
             point of a demo for this pipeline"
        );

        let relations: Vec<_> = crate::sysml_lift::lift_edges(&edges)
            .into_iter()
            .map(|lifted| lifted.relation)
            .collect();
        let d2 = crate::sysml_render::to_d2(&relations);
        assert!(
            d2.contains("->"),
            "expected at least one D2 edge line:\n{d2}"
        );
    }

    #[test]
    fn rust_topology_example_actually_exercises_the_recognizer_pipeline() {
        let example = ALL
            .iter()
            .find(|e| e.id == "rust-topology-render-backend")
            .expect("rust-topology-render-backend example exists");

        let (nodes, edges) = crate::rust_recognizer::recognize_source(example.source)
            .expect("example source is valid Rust");
        let graph = crate::rust_recognizer::to_sysgraph(&nodes, &edges);
        assert!(
            graph.dangling_edges().is_empty(),
            "every edge endpoint should resolve to a node: {:?}",
            graph.dangling_edges()
        );

        use ufo_types::ontology::UfoRelation;
        let relation_kinds: std::collections::HashSet<_> =
            graph.edges.iter().map(|e| e.relation).collect();
        assert!(
            relation_kinds.contains(&UfoRelation::HasPart),
            "expected at least one has_part edge (module containment or field composition)"
        );
        assert!(
            relation_kinds.contains(&UfoRelation::Satisfies),
            "expected the KrokiBackend -> Backend trait-impl edge"
        );
        assert!(
            relation_kinds.contains(&UfoRelation::FlowsTo),
            "expected the render_service -> fetch call edge"
        );

        let relations: Vec<_> = crate::sysml_lift::lift_edges(&graph.edges)
            .into_iter()
            .map(|lifted| lifted.relation)
            .collect();
        let d2 = crate::sysml_render::to_d2(&relations);
        assert!(
            d2.contains("->"),
            "expected at least one D2 edge line:\n{d2}"
        );
    }

    #[test]
    fn rust_isometric_example_actually_renders_through_systhread_core() {
        let example = ALL
            .iter()
            .find(|e| e.id == "rust-isometric-render-backend")
            .expect("rust-isometric-render-backend example exists");

        let (nodes, edges) = crate::rust_recognizer::recognize_source(example.source)
            .expect("example source is valid Rust");
        assert!(
            !edges.is_empty(),
            "the example should recognize at least one relationship"
        );
        let svg = crate::isometric::render_svg(example.title, &nodes, &edges);
        assert!(svg.contains("<svg"), "expected real SVG output:\n{svg}");
    }

    #[test]
    fn every_custom_route_example_has_a_non_empty_render_path() {
        for example in ALL.iter().filter(|e| e.route.is_some()) {
            let route = example.route.unwrap();
            assert!(
                route.starts_with("/render/"),
                "{}: custom route {route:?} doesn't look like a render endpoint",
                example.id
            );
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
