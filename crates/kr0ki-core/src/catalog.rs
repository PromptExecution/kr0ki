//! Intent-first diagram-type catalog (Plan 005 §1).
//!
//! The taxonomy is hardcoded here in Rust — never in JS or Python (architectural
//! rule) — and exported to the playbook via `/api/catalog` and to the agent's
//! skills via generated text. Every entry has a distinct, addressible `id` and
//! intent-oriented `use_cases` drawn from the controlled vocabulary below: the
//! ergonomics contract is that users know what they want to *convey*, not which
//! syntax fits, so browsing and discovery both go intent → type.

/// Controlled vocabulary for intent tags. Gallery filter offers exactly these;
/// multi-select is OR.
pub const USE_CASES: &[&str] = &[
    "process flow",
    "interaction",
    "data model",
    "network",
    "state machine",
    "architecture",
    "chart",
    "sketch",
    "timeline",
    "hardware",
];

pub struct DiagramType {
    /// Addressible name, stable across releases (deep links: ?type=<id>).
    pub id: &'static str,
    /// kr0ki render format the sample renders through.
    pub syntax: &'static str,
    /// Type-specific playbook fixture rendered in this card.
    pub example_id: &'static str,
    /// Human name shown on the card.
    pub name: &'static str,
    /// Intent tags from [`USE_CASES`].
    pub use_cases: &'static [&'static str],
    /// One sentence: when to choose this type.
    pub blurb: &'static str,
    /// Sample prompt for the gallery's Agent button (names the type).
    pub sample_prompt: &'static str,
}

/// The full taxonomy. Order is the gallery's default (All) order: grouped by
/// use case family — flow, interaction, structure, data, sketch/hardware.
pub const TYPES: &[DiagramType] = &[
    // ---- flow ----
    DiagramType {
        id: "flowchart",
        syntax: "d2",
        example_id: "d2-rust-flow",
        name: "Flowchart",
        use_cases: &["process flow", "sketch"],
        blurb: "Steps and decision branches in a process, drawn as boxes and arrows.",
        sample_prompt: "Draw a flowchart of the process I'm about to describe. Ask me about the steps and decision points first.",
    },
    DiagramType {
        id: "activity",
        syntax: "plantuml",
        example_id: "plantuml-activity",
        name: "Activity diagram",
        use_cases: &["process flow"],
        blurb: "A workflow with branches, merges, and swimlanes for who does what.",
        sample_prompt: "Draw an activity diagram (PlantUML) of the workflow I describe. Ask me about the steps, decisions, and who performs them first.",
    },
    DiagramType {
        id: "use-case",
        syntax: "plantuml",
        example_id: "plantuml-use-case",
        name: "Use case diagram",
        use_cases: &["process flow", "interaction"],
        blurb: "Actors outside a system boundary and the goals they can achieve.",
        sample_prompt: "Draw a use case diagram (PlantUML) for the system I describe. Ask me about the actors and their goals first.",
    },
    DiagramType {
        id: "state",
        syntax: "plantuml",
        example_id: "plantuml-state",
        name: "State machine",
        use_cases: &["state machine", "process flow"],
        blurb: "The states one thing can be in and the events that move it between them.",
        sample_prompt: "Draw a state machine diagram (PlantUML) for the thing I describe. Ask me about its states and transitions first.",
    },
    // ---- interaction ----
    DiagramType {
        id: "sequence",
        syntax: "plantuml",
        example_id: "plantuml-sequence",
        name: "Sequence diagram",
        use_cases: &["interaction", "process flow"],
        blurb: "Who talks to whom, in what order, over time — request/response flows.",
        sample_prompt: "Draw a sequence diagram (PlantUML) of the interaction I describe. Ask me about the participants and the order of messages first.",
    },
    DiagramType {
        id: "component",
        syntax: "plantuml",
        example_id: "plantuml-component",
        name: "Component diagram",
        use_cases: &["architecture"],
        blurb: "Software building blocks and the interfaces between them.",
        sample_prompt: "Draw a component diagram (PlantUML) of the system I describe. Ask me about the components and their interfaces first.",
    },
    DiagramType {
        id: "c4-context",
        syntax: "c4plantuml",
        example_id: "c4-context",
        name: "C4 context diagram",
        use_cases: &["architecture"],
        blurb: "Your system as a box in the middle, surrounded by users and dependencies.",
        sample_prompt: "Draw a C4 context diagram (C4-PlantUML) for the system I describe. Ask me about the system, its users, and external dependencies first.",
    },
    DiagramType {
        id: "class",
        syntax: "plantuml",
        example_id: "plantuml-class",
        name: "Class diagram",
        use_cases: &["data model", "architecture"],
        blurb: "Types, their fields, and inheritance/association relationships.",
        sample_prompt: "Draw a class diagram (PlantUML) for the domain I describe. Ask me about the classes and their relationships first.",
    },
    // ---- data ----
    DiagramType {
        id: "erd",
        syntax: "plantuml",
        example_id: "plantuml-erd",
        name: "Entity-relationship diagram",
        use_cases: &["data model"],
        blurb: "Database entities and how they reference each other (1:N, M:N).",
        sample_prompt: "Draw an entity-relationship diagram (PlantUML) for the data I describe. Ask me about the entities and their relationships first.",
    },
    DiagramType {
        id: "dbml",
        syntax: "dbml",
        example_id: "dbml-cache-entry",
        name: "Database schema (DBML)",
        use_cases: &["data model"],
        blurb: "Tables, columns, and foreign keys in a database-flavoured notation.",
        sample_prompt: "Draw a DBML database schema for the data model I describe. Ask me about the tables, columns, and foreign keys first.",
    },
    DiagramType {
        id: "chart",
        syntax: "vegalite",
        example_id: "vegalite-cache-outcomes",
        name: "Data chart (Vega-Lite)",
        use_cases: &["chart"],
        blurb: "Bars, lines, and points for quantitative data.",
        sample_prompt: "Draw a Vega-Lite chart for the data I describe. Ask me about the data shape and the comparison I want to show first.",
    },
    // ---- network / hardware ----
    DiagramType {
        id: "network",
        syntax: "nwdiag",
        example_id: "nwdiag-topology",
        name: "Network diagram",
        use_cases: &["network"],
        blurb: "Segments, nodes, and addresses — how machines connect.",
        sample_prompt: "Draw a network diagram (nwdiag) of the infrastructure I describe. Ask me about the segments, nodes, and addressing first.",
    },
    DiagramType {
        id: "k8s-topology",
        syntax: "k8s-topology",
        example_id: "k8s-topology-web-service",
        name: "Kubernetes topology",
        use_cases: &["network", "architecture"],
        blurb: "How workloads and services in a cluster relate, from a manifest.",
        sample_prompt: "Render the Kubernetes topology (k8s-topology) for the manifest I provide. Ask me for the manifest first if I haven't pasted one.",
    },
    DiagramType {
        id: "rack",
        syntax: "rackdiag",
        example_id: "rackdiag-deployment",
        name: "Rack diagram",
        use_cases: &["hardware", "network"],
        blurb: "Server racks with units stacked in order.",
        sample_prompt: "Draw a rack diagram (rackdiag) of the hardware I describe. Ask me about the rack units first.",
    },
    DiagramType {
        id: "packet",
        syntax: "packetdiag",
        example_id: "packetdiag-cache-key",
        name: "Packet structure",
        use_cases: &["hardware"],
        blurb: "Byte-level packet or register layouts.",
        sample_prompt: "Draw a packet structure diagram (packetdiag) for the protocol I describe. Ask me about the fields and widths first.",
    },
    DiagramType {
        id: "wiring",
        syntax: "wireviz",
        example_id: "wireviz-harness",
        name: "Wiring diagram",
        use_cases: &["hardware"],
        blurb: "Cables and pin-to-pin connections between connectors.",
        sample_prompt: "Draw a wiring diagram (WireViz) for the harness I describe. Ask me about the connectors and cables first.",
    },
    DiagramType {
        id: "bytefield",
        syntax: "bytefield",
        example_id: "bytefield-cache-key",
        name: "Bytefield",
        use_cases: &["hardware"],
        blurb: "Memory and protocol layouts, byte by byte.",
        sample_prompt: "Draw a bytefield diagram for the layout I describe. Ask me about the fields and byte boundaries first.",
    },
    // ---- sketch ----
    DiagramType {
        id: "block",
        syntax: "blockdiag",
        example_id: "blockdiag-pipeline",
        name: "Block diagram",
        use_cases: &["sketch", "architecture"],
        blurb: "Simple labelled blocks and arrows — the whiteboard classic.",
        sample_prompt: "Draw a block diagram (blockdiag) of what I describe. Ask me about the blocks and connections first.",
    },
    DiagramType {
        id: "sketch",
        syntax: "ditaa",
        example_id: "ditaa-artifact-flow",
        name: "ASCII sketch (ditaa)",
        use_cases: &["sketch"],
        blurb: "Hand-drawn look from ASCII-art boxes.",
        sample_prompt: "Turn my ASCII sketch into a ditaa diagram. Ask me to paste the ASCII first.",
    },
    DiagramType {
        id: "svgbob",
        syntax: "svgbob",
        example_id: "svgbob-ascii-pipeline",
        name: "ASCII diagram (svgbob)",
        use_cases: &["sketch"],
        blurb: "Crisp rendering of ASCII-art diagrams.",
        sample_prompt: "Render my ASCII art with svgbob. Ask me to paste the ASCII first.",
    },
    DiagramType {
        id: "timing",
        syntax: "plantuml",
        example_id: "plantuml-timing",
        name: "Timing diagram",
        use_cases: &["timeline", "state machine"],
        blurb: "Signal levels and state over a shared time axis.",
        sample_prompt: "Draw a timing diagram (PlantUML) for the signals I describe. Ask me about the signals and their timing first.",
    },
    DiagramType {
        id: "wavedrom",
        syntax: "wavedrom",
        example_id: "wavedrom-cache-timing",
        name: "Waveform (WaveDrom)",
        use_cases: &["timeline", "hardware"],
        blurb: "Digital signal waveforms with clock edges.",
        sample_prompt: "Draw a WaveDrom waveform for the signals I describe. Ask me about the signal names and clock cycles first.",
    },
    DiagramType {
        id: "gantt",
        syntax: "plantuml",
        example_id: "plantuml-gantt",
        name: "Gantt chart",
        use_cases: &["timeline", "process flow"],
        blurb: "Tasks on a calendar with dependencies and milestones.",
        sample_prompt: "Draw a Gantt chart (PlantUML) for the plan I describe. Ask me about the tasks, durations, and dependencies first.",
    },
    DiagramType {
        id: "deployment",
        syntax: "plantuml",
        example_id: "plantuml-deployment",
        name: "Deployment diagram",
        use_cases: &["architecture", "network"],
        blurb: "Artifacts placed on nodes — what runs where.",
        sample_prompt: "Draw a deployment diagram (PlantUML) for the system I describe. Ask me about the nodes and artifacts first.",
    },
];

/// All use-case tags actually referenced by [`TYPES`], in [`USE_CASES`] order.
/// The gallery filter renders exactly these (no empty filters).
pub fn used_use_cases() -> Vec<&'static str> {
    USE_CASES
        .iter()
        .copied()
        .filter(|tag| TYPES.iter().any(|t| t.use_cases.contains(tag)))
        .collect()
}

pub fn by_id(id: &str) -> Option<&'static DiagramType> {
    TYPES.iter().find(|t| t.id == id)
}

pub fn by_use_case(tag: &str) -> Vec<&'static DiagramType> {
    TYPES
        .iter()
        .filter(|t| t.use_cases.contains(&tag))
        .collect()
}

/// The type-discovery series (Plan 005 §2.2) as agent-consumable text: the
/// intent vocabulary and one line per use case so the LLM asks grounded
/// questions instead of inventing categories.
pub fn discovery_guide() -> String {
    let mut out = String::from(
        "DIAGRAM TYPE VOCABULARY for discovery questions — map the user's intent \
to these families, then recommend a type by id:\n",
    );
    for tag in USE_CASES {
        let types = by_use_case(tag);
        let names: Vec<&str> = types.iter().map(|t| t.id).collect();
        out.push_str(&format!("- {tag}: {}\n", names.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::examples;

    #[test]
    fn ids_are_unique_and_addressible() {
        let mut ids: Vec<&str> = TYPES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len, "duplicate typeId in catalog");
        for id in &ids {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
                "typeId {id} must be deep-link-safe (lowercase/digits/dashes)"
            );
        }
    }

    #[test]
    fn every_use_case_is_in_the_vocabulary() {
        for t in TYPES {
            for tag in t.use_cases {
                assert!(
                    USE_CASES.contains(tag),
                    "{} uses tag '{tag}' outside the controlled vocabulary",
                    t.id
                );
            }
            assert!(!t.use_cases.is_empty(), "{} has no use cases", t.id);
        }
    }

    #[test]
    fn every_type_resolves_to_a_known_format_and_names_itself_in_the_prompt() {
        // Formats kr0ki actually renders (from the live fixture catalog).
        let known = [
            "d2",
            "plantuml",
            "c4plantuml",
            "dbml",
            "vegalite",
            "nwdiag",
            "k8s-topology",
            "rackdiag",
            "packetdiag",
            "wireviz",
            "bytefield",
            "blockdiag",
            "ditaa",
            "svgbob",
            "wavedrom",
        ];
        for t in TYPES {
            assert!(
                known.contains(&t.syntax),
                "{} renders through unknown format {}",
                t.id,
                t.syntax
            );
            let lower = t.sample_prompt.to_lowercase();
            assert!(
                lower.contains(
                    t.name
                        .to_lowercase()
                        .split('(')
                        .next()
                        .unwrap()
                        .trim()
                        .split(' ')
                        .next()
                        .unwrap()
                ),
                "{} sample_prompt must name the type explicitly",
                t.id
            );
        }
    }

    #[test]
    fn every_type_points_to_a_type_specific_fixture() {
        for diagram_type in TYPES {
            let fixture = examples::ALL
                .iter()
                .find(|example| example.id == diagram_type.example_id)
                .unwrap_or_else(|| panic!("{} fixture missing", diagram_type.id));
            assert_eq!(
                fixture.format, diagram_type.syntax,
                "{} fixture syntax",
                diagram_type.id
            );
        }
    }

    #[test]
    fn used_use_cases_exclude_empty_tags() {
        let used = used_use_cases();
        assert!(used.contains(&"process flow"));
        assert!(used.contains(&"interaction"));
        for tag in used {
            assert!(!by_use_case(tag).is_empty());
        }
    }

    #[test]
    fn by_id_round_trip_and_deep_link_shape() {
        assert_eq!(by_id("sequence").unwrap().syntax, "plantuml");
        assert!(by_id("no-such-type").is_none());
    }

    #[test]
    fn discovery_guide_lists_every_vocabulary_family() {
        let guide = discovery_guide();
        for tag in USE_CASES {
            assert!(guide.contains(tag), "guide missing family {tag}");
        }
        assert!(guide.contains("sequence"));
    }
}
