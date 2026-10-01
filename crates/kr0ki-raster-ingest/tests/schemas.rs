//! The committed JSON Schemas must match the Rust types that define the contract.
//! Regenerate after an intentional change: `UPDATE_SCHEMAS=1 cargo test -p kr0ki-raster-ingest --test schemas`.

use kr0ki_raster_ingest::{Description, Verdict};
use schemars::{schema_for, JsonSchema};
use std::path::PathBuf;

fn check<T: JsonSchema>(name: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../docs/schemas/plan-007/{name}.schema.json"));
    let generated = serde_json::to_string_pretty(&schema_for!(T)).unwrap() + "\n";
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is missing; run with UPDATE_SCHEMAS=1", path.display()));
    assert_eq!(
        committed, generated,
        "{name}.schema.json is stale; run with UPDATE_SCHEMAS=1 and commit the result"
    );
}

#[test]
fn verdict_schema_is_in_sync() {
    check::<Verdict>("verdict");
}

#[test]
fn description_schema_is_in_sync() {
    check::<Description>("description");
}

#[test]
fn the_verdict_schema_names_the_wire_field_match_and_requires_it() {
    let schema = serde_json::to_value(schema_for!(Verdict)).unwrap();
    assert!(
        schema["properties"].get("match").is_some(),
        "the field is `match` on the wire, not `matches`"
    );
    assert!(schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "match"));
    assert!(schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "score"));
}
