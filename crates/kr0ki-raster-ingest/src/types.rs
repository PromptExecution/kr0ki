//! Data contracts: what the model is asked to produce and what a run reports.
//!
//! [`Verdict`] and [`Description`] have committed JSON Schemas under `docs/schemas/plan-007/`; a test
//! keeps the files in sync with these types (regenerate with `UPDATE_SCHEMAS=1 cargo test`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
#[serde(deny_unknown_fields)]
pub struct DescribedEdge {
    pub from: String,
    pub to: String,
}

/// The model's structured reading of the original image, produced once per run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub diagram_kind: DiagramKind,
    /// Every piece of visible text, one string each.
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub nodes: Vec<String>,
    #[serde(default)]
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
#[serde(deny_unknown_fields)]
pub struct EdgeDiff {
    pub from: String,
    pub to: String,
    pub issue: EdgeIssue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabelDiff {
    pub expected: String,
    pub got: String,
}

/// The judge's comparison of the original with a render, completed by the deterministic label check.
///
/// `label_recall` / `label_precision` are always overwritten by the loop (or cleared when the renderer
/// cannot supply text), never trusted from the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub missing_nodes: Vec<String>,
    #[serde(default)]
    pub extra_nodes: Vec<String>,
    #[serde(default)]
    pub wrong_edges: Vec<EdgeDiff>,
    #[serde(default)]
    pub label_errors: Vec<LabelDiff>,
    #[serde(default)]
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
    fn verdict_uses_the_wire_name_match_and_rejects_unknown_fields() {
        let ok: Verdict = serde_json::from_str(r#"{"match": true, "score": 0.9}"#).unwrap();
        assert!(ok.matches && ok.missing_nodes.is_empty());
        assert!(
            serde_json::from_str::<Verdict>(r#"{"match": true, "score": 1, "extra": 1}"#).is_err()
        );
        assert!(serde_json::to_string(&ok)
            .unwrap()
            .contains(r#""match":true"#));
    }

    #[test]
    fn unknown_diagram_kinds_degrade_to_other_instead_of_failing() {
        let d: Description = serde_json::from_str(r#"{"diagram_kind": "gantt-ish"}"#).unwrap();
        assert_eq!(d.diagram_kind, DiagramKind::Other);
        let none: Description = serde_json::from_str(r#"{"diagram_kind": "none"}"#).unwrap();
        assert_eq!(none.diagram_kind, DiagramKind::None);
    }
}
