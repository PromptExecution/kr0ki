use serde_json::{json, Value};

fn reference(name: &str) -> Value {
    json!({"$ref": format!("#/$defs/{name}")})
}
fn nullable(value: Value) -> Value {
    json!({"anyOf": [value, {"type": "null"}]})
}
fn array(value: Value) -> Value {
    json!({"type": "array", "items": value})
}
fn enumeration(values: &[&str]) -> Value {
    json!({"type": "string", "enum": values})
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "additionalProperties": false, "properties": properties, "required": required})
}
fn text() -> Value {
    json!({"type": "string"})
}
fn hash() -> Value {
    json!({"type": "string", "pattern": "^[0-9a-f]{64}$"})
}

/// Draft 2020-12 wire schema. Cross-field graph, digest and span constraints
/// belong to `RustBehaviorIr::validate`; schema validation alone is insufficient.
pub fn json_schema() -> Value {
    let anchor = object(
        json!({
            "file": text(), "symbol": text(),
            "start": {"type": "integer", "minimum": 0, "maximum": u32::MAX},
            "end": {"type": "integer", "minimum": 0, "maximum": u32::MAX}
        }),
        &["file", "symbol", "start", "end"],
    );
    let provenance = object(
        json!({
            "revision": {"type": "string", "pattern": "^([0-9a-f]{40}|[0-9a-f]{64})$"},
            "tree_digest": hash(), "toolchain": text(), "extractor": text(),
            "config": {"type": "object", "additionalProperties": {"type": "string"}}
        }),
        &[
            "revision",
            "tree_digest",
            "toolchain",
            "extractor",
            "config",
        ],
    );
    let source = object(
        json!({"path": text(), "sha256": hash(), "content": text()}),
        &["path", "sha256", "content"],
    );
    let node = object(
        json!({"id": text(), "name": text(), "kind": reference("NodeKind"), "anchor": nullable(reference("Anchor"))}),
        &["id", "name", "kind"],
    );
    let edge = object(
        json!({
            "id": text(), "from": text(), "to": text(), "kind": reference("EdgeKind"),
            "guard": nullable(text()), "resolution": reference("Resolution"), "anchor": reference("Anchor")
        }),
        &["id", "from", "to", "kind", "resolution", "anchor"],
    );
    let diagnostic = object(
        json!({
            "code": text(), "severity": reference("Severity"), "message": text(), "anchor": nullable(reference("Anchor"))
        }),
        &["code", "severity", "message"],
    );
    let state = object(
        json!({"id": text(), "terminal": {"type": "boolean"}, "anchor": nullable(reference("Anchor"))}),
        &["id", "terminal"],
    );
    let transition = object(
        json!({
            "id": text(), "from": text(), "to": text(), "event": text(),
            "guard": nullable(text()), "effect": nullable(text()), "anchor": nullable(reference("Anchor"))
        }),
        &["id", "from", "to", "event"],
    );
    let machine = object(
        json!({
            "id": text(), "name": text(), "initial": text(),
            "states": array(reference("State")), "transitions": array(reference("Transition"))
        }),
        &["id", "name", "initial", "states", "transitions"],
    );
    let mut schema = object(
        json!({
            "schema_version": {"type": "integer", "const": crate::SCHEMA_VERSION},
            "provenance": reference("Provenance"), "sources": array(reference("SourceFile")),
            "nodes": array(reference("Node")), "edges": array(reference("Edge")),
            "diagnostics": array(reference("Diagnostic")), "machines": array(reference("StateMachine"))
        }),
        &[
            "schema_version",
            "provenance",
            "sources",
            "nodes",
            "edges",
            "diagnostics",
            "machines",
        ],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["$id"] = json!("urn:kr0ki:rust-behavior-ir:v1");
    schema["title"] = json!("RustBehaviorIR v1");
    schema["$defs"] = json!({
        "Anchor": anchor, "Provenance": provenance, "SourceFile": source,
        "Node": node, "Edge": edge, "Diagnostic": diagnostic,
        "StateMachine": machine, "State": state, "Transition": transition,
        "NodeKind": enumeration(&["module", "type", "field", "associated_type", "trait", "function", "action", "decision", "merge", "loop", "exit", "dispatch", "state", "external"]),
        "EdgeKind": enumeration(&["contains", "satisfies", "requires", "governed_by", "calls", "flow", "branch", "back", "exit", "transition"]),
        "Resolution": enumeration(&["resolved", "inferred", "unresolved"]),
        "Severity": enumeration(&["error", "warning", "info"])
    });
    schema
}
