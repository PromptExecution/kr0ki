//! Deterministic content hashing for a [`ModelSnapshot`](crate::ModelSnapshot).
//!
//! Neither Flexo `flexo-mms-sysmlv2` nor Eclipse SysON returns a server-side content
//! hash / ETag for a commit's element set, so kr0ki derives its own. This hash is the
//! model-side cache key for PRD-KR0KI-001 FR5 and MUST be stable across:
//!
//! * the order the server returns elements in (JSON array order is not significant),
//! * cosmetic JSON key ordering within an element.
//!
//! It MUST change when any element `@id`, `@type`, field value, or the root set changes.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::model::Element;

/// Serialize `v` to bytes with **object keys sorted recursively**. Array element order
/// is preserved (it is semantically significant in JSON); only object member order is
/// normalized. Scalars use `serde_json`'s compact form.
///
/// Known limitation: numbers are emitted as `serde_json` parses them, so `1` and `1.0`
/// hash differently. OMG PSM element payloads do not rely on that distinction.
pub fn canonical_json(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical(v, &mut out);
    out
}

fn write_canonical(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Object(map) => {
            out.push(b'{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(
                    serde_json::to_string(k)
                        .expect("a string always serializes")
                        .as_bytes(),
                );
                out.push(b':');
                write_canonical(map.get(*k).expect("key came from this map"), out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        scalar => out.extend_from_slice(
            &serde_json::to_vec(scalar).expect("a scalar Value always serializes"),
        ),
    }
}

/// The object hashed for one element: its `fields` plus `@type` re-inserted (so a
/// metaclass change is caught). `@id` is fed to the hasher separately, framed by
/// `0x1f`, so it is not repeated here.
fn element_hash_object(e: &Element) -> Value {
    let mut m: Map<String, Value> = e.fields.clone();
    m.insert("@type".to_string(), Value::String(e.at_type.clone()));
    Value::Object(m)
}

/// Lowercase-hex SHA-256 over:
///   `b"kr0ki-sysmlv2/v1"`
///   then, for each element **sorted by `@id`**: `0x1f`, `@id`, `0x1f`, `canonical_json(fields + @type)`
///   then `0x1e`
///   then each root id **sorted**, joined by `0x1f`.
pub fn compute_content_hash(elements: &[Element], roots: &[String]) -> String {
    let mut ordered: Vec<&Element> = elements.iter().collect();
    ordered.sort_by(|a, b| a.at_id.cmp(&b.at_id));

    let mut h = Sha256::new();
    h.update(b"kr0ki-sysmlv2/v1");
    for e in &ordered {
        h.update([0x1f]);
        h.update(e.at_id.as_bytes());
        h.update([0x1f]);
        h.update(canonical_json(&element_hash_object(e)));
    }
    h.update([0x1e]);

    let mut root_ids: Vec<&String> = roots.iter().collect();
    root_ids.sort_unstable();
    for (i, r) in root_ids.iter().enumerate() {
        if i > 0 {
            h.update([0x1f]);
        }
        h.update(r.as_bytes());
    }

    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn el(id: &str, ty: &str, fields: Value) -> Element {
        let mut obj = json!({ "@id": id, "@type": ty });
        if let (Value::Object(dst), Value::Object(src)) = (&mut obj, fields) {
            dst.extend(src);
        }
        serde_json::from_value(obj).unwrap()
    }

    #[test]
    fn canonical_json_sorts_object_keys_recursively() {
        let a = canonical_json(&json!({ "b": 1, "a": { "y": 2, "x": 3 } }));
        let b = canonical_json(&json!({ "a": { "x": 3, "y": 2 }, "b": 1 }));
        assert_eq!(a, b);
        assert_eq!(
            String::from_utf8(a).unwrap(),
            r#"{"a":{"x":3,"y":2},"b":1}"#
        );
    }

    #[test]
    fn canonical_json_preserves_array_order() {
        let a = canonical_json(&json!([1, 2, 3]));
        let b = canonical_json(&json!([3, 2, 1]));
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_order_independent() {
        let e1 = el("e1", "PartUsage", json!({ "name": "pump" }));
        let e2 = el("e2", "PartUsage", json!({ "name": "valve" }));
        let forward = compute_content_hash(&[e1.clone(), e2.clone()], &["e1".into()]);
        let reverse = compute_content_hash(&[e2, e1], &["e1".into()]);
        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), 64);
    }

    #[test]
    fn hash_changes_on_field_type_and_roots() {
        let base = compute_content_hash(
            &[el("e1", "PartUsage", json!({ "name": "pump" }))],
            &["e1".into()],
        );
        let field_changed = compute_content_hash(
            &[el("e1", "PartUsage", json!({ "name": "PUMP" }))],
            &["e1".into()],
        );
        let type_changed = compute_content_hash(
            &[el("e1", "PartDefinition", json!({ "name": "pump" }))],
            &["e1".into()],
        );
        let roots_changed = compute_content_hash(
            &[el("e1", "PartUsage", json!({ "name": "pump" }))],
            &["e2".into()],
        );
        assert_ne!(base, field_changed);
        assert_ne!(base, type_changed);
        assert_ne!(base, roots_changed);
    }
}
