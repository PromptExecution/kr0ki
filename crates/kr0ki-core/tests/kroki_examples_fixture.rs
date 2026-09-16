//! Validates the vendored kroki.io example catalogue (kr0ki#19).
//!
//! `crates/kr0ki-core/fixtures/kroki-examples.json` is produced by
//! `scripts/sync-kroki-examples.sh` from `yuzutech/kroki.io`'s
//! `assets/examples/data.json` (MPL-2.0, see `/NOTICE`) — a real, working
//! example source per diagram type, vendored because `yuzutech/kroki` (the
//! server repo the issue originally pointed at) turned out to have no clean,
//! individually extractable per-format fixture set.
//!
//! This fixture is **not yet consumed** by kr0ki's render pipeline or
//! playbook (`fixtures/playbook-examples.json` is untouched) — that's a
//! separate follow-up decision, not this test's concern. This only proves
//! the vendored data itself is sound: well-formed, HTML-entity-decoded, and
//! covers every companion-free format kr0ki supports.

use kr0ki_core::format::DiagramFormat;
use serde_json::Value;

fn load_fixture() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/kroki-examples.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

#[test]
fn fixture_covers_every_companion_free_format() {
    let fixture = load_fixture();
    let types = fixture["types"].as_array().expect("types array");
    let vendored_slugs: std::collections::HashSet<&str> = types
        .iter()
        .map(|t| t["type"].as_str().expect("type field is a string"))
        .collect();

    let missing: Vec<&str> = DiagramFormat::ALL
        .iter()
        .map(|f| f.kroki_slug())
        .filter(|slug| !vendored_slugs.contains(slug))
        .collect();
    assert!(
        missing.is_empty(),
        "kroki.io's example catalogue is missing fixtures for: {missing:?}"
    );
}

#[test]
fn every_vendored_example_has_non_empty_decoded_source() {
    let fixture = load_fixture();
    for t in fixture["types"].as_array().unwrap() {
        let type_name = t["type"].as_str().unwrap();
        let examples = t["examples"]
            .as_array()
            .unwrap_or_else(|| panic!("{type_name} has no examples array"));
        assert!(
            !examples.is_empty(),
            "{type_name} has zero examples in the vendored fixture"
        );
        for ex in examples {
            let source = ex["source"].as_str().expect("example source is a string");
            assert!(
                !source.trim().is_empty(),
                "{type_name} example {:?} has empty source",
                ex["anchor"]
            );
            // Regression guard on the sync script's HTML-entity decode step —
            // data.json escapes source for the website's own display. A
            // *double*-escaped remnant (`&amp;lt;`) proves a decode pass was
            // skipped; a single `&lt;`/`&gt;`/`&amp;` is not itself a bug —
            // XML-based sources (umlet, bpmn) legitimately contain those as
            // part of valid, already-correctly-decoded XML content (e.g.
            // umlet's `lt=&lt;-` line-type attribute).
            assert!(
                !source.contains("&amp;lt;")
                    && !source.contains("&amp;gt;")
                    && !source.contains("&amp;amp;"),
                "{type_name} example {:?} has a double-escaped HTML entity — a decode pass was skipped",
                ex["anchor"]
            );
        }
    }
}

#[test]
fn fixture_records_its_own_provenance() {
    let fixture = load_fixture();
    assert_eq!(
        fixture["_source"].as_str(),
        Some("https://github.com/yuzutech/kroki.io (MPL-2.0)")
    );
    assert!(fixture["_source_rev"]
        .as_str()
        .is_some_and(|s| s.len() == 40));
}
