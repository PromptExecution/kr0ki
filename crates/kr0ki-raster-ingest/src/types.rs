//! Data contracts: what the model is asked to produce and what a run reports.
//!
//! [`Verdict`] and [`Description`] have committed JSON Schemas under `docs/schemas/plan-007/`; a test
//! keeps the files in sync with these types (regenerate with `UPDATE_SCHEMAS=1 cargo test`).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// Models often send `null` for an empty list; treat it as empty. Unknown keys are likewise ignored on input
/// (a stray `"reasoning"` or an edge `"label"` must not discard a whole reply). Output only ever carries the
/// fields below, so consumers can rely on the committed schemas.
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// What kind of picture the image is. `None` means "not a diagram" and ends a run without looping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagramKind {
    None,
    Flowchart,
    Sequence,
    Class,
    Er,
    State,
    Architecture,
    Network,
    #[serde(other)]
    Other,
}

/// A directed connection read off the image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DescribedEdge {
    pub from: String,
    pub to: String,
}

/// The model's structured reading of the original image, produced once per run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Description {
    pub diagram_kind: DiagramKind,
    /// Every piece of visible text, one string each.
    #[serde(default, deserialize_with = "null_as_default")]
    pub labels: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub nodes: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub edges: Vec<DescribedEdge>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeIssue {
    Missing,
    Extra,
    Reversed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EdgeDiff {
    pub from: String,
    pub to: String,
    pub issue: EdgeIssue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LabelDiff {
    pub expected: String,
    pub got: String,
}

/// The judge's comparison of the original with a render, completed by the deterministic label check.
///
/// `label_recall` / `label_precision` are always overwritten by the loop (or cleared when the renderer
/// cannot supply text), never trusted from the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// Same nodes, labels and directed connections. Styling and layout alone do not count.
    #[serde(rename = "match")]
    pub matches: bool,
    /// 0.0 (unrelated) to 1.0 (identical in meaning).
    pub score: f64,
    #[serde(default)]
    pub label_recall: Option<f64>,
    #[serde(default)]
    pub label_precision: Option<f64>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub missing_nodes: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub extra_nodes: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub wrong_edges: Vec<EdgeDiff>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub label_errors: Vec<LabelDiff>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub layout_notes: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid verdict: {0}")]
pub struct VerdictError(pub String);

impl Verdict {
    /// Range-check the numeric fields (a model can return `1.7` or `NaN`).
    pub fn validate(&self) -> Result<(), VerdictError> {
        let unit = |name: &str, v: f64| {
            if v.is_finite() && (0.0..=1.0).contains(&v) {
                Ok(())
            } else {
                Err(VerdictError(format!(
                    "{name} must be within 0..=1, got {v}"
                )))
            }
        };
        unit("score", self.score)?;
        for (name, v) in [
            ("label_recall", self.label_recall),
            ("label_precision", self.label_precision),
            ("confidence", self.confidence),
        ] {
            if let Some(v) = v {
                unit(name, v)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(score: f64) -> Verdict {
        Verdict {
            matches: true,
            score,
            label_recall: None,
            label_precision: None,
            missing_nodes: vec![],
            extra_nodes: vec![],
            wrong_edges: vec![],
            label_errors: vec![],
            layout_notes: vec![],
            confidence: None,
        }
    }

    #[test]
    fn validate_rejects_out_of_range_and_non_finite_numbers() {
        assert!(verdict(0.0).validate().is_ok() && verdict(1.0).validate().is_ok());
        for bad in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
            assert!(verdict(bad).validate().is_err(), "{bad}");
        }
        let mut v = verdict(0.5);
        v.confidence = Some(2.0);
        assert!(v.validate().is_err());
    }

    #[test]
    fn verdict_uses_the_wire_name_match() {
        let ok: Verdict = serde_json::from_str(r#"{"match": true, "score": 0.9}"#).unwrap();
        assert!(ok.matches && ok.missing_nodes.is_empty());
        assert!(serde_json::to_string(&ok)
            .unwrap()
            .contains(r#""match":true"#));
    }

    #[test]
    fn model_replies_with_extra_keys_or_null_lists_are_still_understood() {
        let v: Verdict = serde_json::from_str(r#"{"match": true, "score": 1, "reasoning": "looks right", "missing_nodes": null, "layout_notes": null}"#).unwrap();
        assert!(v.missing_nodes.is_empty() && v.layout_notes.is_empty());
        let d: Description = serde_json::from_str(
            r#"{"diagram_kind":"flowchart","title":"Checkout","labels":null,"nodes":["a"],"edges":[{"from":"a","to":"b","label":"HTTP"}]}"#,
        )
        .unwrap();
        assert_eq!((d.labels.len(), d.edges.len()), (0, 1));
        // ...but what we emit is exactly the contract
        let out = serde_json::to_value(&d).unwrap();
        assert!(out.get("title").is_none() && out["edges"][0].get("label").is_none());
    }

    #[test]
    fn unknown_diagram_kinds_degrade_to_other_instead_of_failing() {
        let d: Description = serde_json::from_str(r#"{"diagram_kind": "gantt-ish"}"#).unwrap();
        assert_eq!(d.diagram_kind, DiagramKind::Other);
        let none: Description = serde_json::from_str(r#"{"diagram_kind": "none"}"#).unwrap();
        assert_eq!(none.diagram_kind, DiagramKind::None);
    }
}
