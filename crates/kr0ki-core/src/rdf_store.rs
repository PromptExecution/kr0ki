//! Requirements graph as RDF in an in-memory oxigraph store, queried with read-only SPARQL.
//!
//! PLAN-KR0KI-008 WP7. The playbook's `buildRequirementGraph` result is the input, so the browser and the
//! server answer the same questions over the same shape. Cost is an *attribution line* (`code`, `share`),
//! never money (decision 3), and the SHACL-style checks are SPARQL ASK/SELECT (EVAL-rdf-stores: oxigraph's
//! SPARQL is correct; the SHACL engines evaluated were not).
//!
//! The store is a disposable cache built per request from the graph; nothing is persisted here.

use oxigraph::model::{vocab::rdf, GraphNameRef, Literal, NamedNode, QuadRef};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const NS: &str = "urn:kr0ki:req#";
/// Rows returned by one query; a hostile or careless query cannot ship an unbounded result.
pub const MAX_ROWS: usize = 10_000;
pub const MAX_QUERY_BYTES: usize = 16 * 1024;
pub const MAX_NODES: usize = 50_000;

#[derive(Debug, thiserror::Error)]
pub enum RdfError {
    #[error("graph too large: {0} nodes (max {MAX_NODES})")]
    TooLarge(usize),
    #[error("query too large (max {MAX_QUERY_BYTES} bytes)")]
    QueryTooLarge,
    #[error("invalid SPARQL: {0}")]
    Parse(String),
    #[error("only SELECT and ASK queries are accepted")]
    NotReadOnly,
    #[error("query failed: {0}")]
    Eval(String),
    #[error("store error: {0}")]
    Store(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Attribution {
    pub code: String,
    #[serde(default = "one")]
    pub share: f64,
}
fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Tag {
    pub tag: String,
    #[serde(default)]
    pub basis: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReqNode {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub attributions: Vec<Attribution>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub satisfied_by: Vec<String>,
    #[serde(default)]
    pub verified_by: Vec<String>,
    #[serde(default)]
    pub allocated_to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReqEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ReqGraph {
    #[serde(default)]
    pub nodes: Vec<ReqNode>,
    #[serde(default)]
    pub edges: Vec<ReqEdge>,
}

/// An IRI for a user-supplied identifier. Percent-encodes everything outside a conservative set, so an id can never
/// break out of the IRI (`>`, spaces, quotes) or collide with another id.
fn iri(kind: &str, id: &str) -> NamedNode {
    let mut out = format!("{NS}{kind}/");
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    NamedNode::new_unchecked(out)
}
fn pred(name: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{NS}{name}"))
}

pub struct RdfStore {
    store: Store,
}

impl RdfStore {
    pub fn from_graph(g: &ReqGraph) -> Result<Self, RdfError> {
        if g.nodes.len() > MAX_NODES {
            return Err(RdfError::TooLarge(g.nodes.len()));
        }
        let store = Store::new().map_err(|e| RdfError::Store(e.to_string()))?;
        let put = |s: &NamedNode, p: &NamedNode, o: oxigraph::model::TermRef<'_>| {
            store
                .insert(QuadRef::new(s, p, o, GraphNameRef::DefaultGraph))
                .map_err(|e| RdfError::Store(e.to_string()))
        };
        let requirement = pred("Requirement");
        for n in &g.nodes {
            let s = iri("req", &n.id);
            put(&s, &NamedNode::from(rdf::TYPE), requirement.as_ref().into())?;
            put(
                &s,
                &pred("id"),
                Literal::new_simple_literal(&n.id).as_ref().into(),
            )?;
            put(
                &s,
                &pred("title"),
                Literal::new_simple_literal(&n.title).as_ref().into(),
            )?;
            for (i, a) in n.attributions.iter().enumerate() {
                // A blank-free, stable line node so one requirement can carry several lines on the same code.
                let line = iri("line", &format!("{}#{i}", n.id));
                put(&s, &pred("attribution"), line.as_ref().into())?;
                put(
                    &line,
                    &pred("code"),
                    Literal::new_simple_literal(&a.code).as_ref().into(),
                )?;
                put(
                    &line,
                    &pred("share"),
                    Literal::from(a.share).as_ref().into(),
                )?;
            }
            for t in &n.tags {
                put(
                    &s,
                    &pred("tag"),
                    Literal::new_simple_literal(&t.tag).as_ref().into(),
                )?;
            }
            for (p, list) in [
                ("satisfiedBy", &n.satisfied_by),
                ("verifiedBy", &n.verified_by),
                ("allocatedTo", &n.allocated_to),
            ] {
                for x in list {
                    put(&s, &pred(p), iri("elt", x).as_ref().into())?;
                }
            }
        }
        for e in &g.edges {
            // `derive` is child -> parent, so `derivedFrom` points up the tree. Unknown kinds are kept under their own name.
            let kind: String = e
                .kind
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            if kind.is_empty() {
                continue;
            }
            let name = if kind == "derive" {
                "derivedFrom".to_string()
            } else {
                kind
            };
            put(
                &iri("req", &e.from),
                &pred(&name),
                iri("req", &e.to).as_ref().into(),
            )?;
        }
        Ok(Self { store })
    }

    pub fn len(&self) -> usize {
        self.store.len().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Run a SELECT or ASK. Updates cannot be expressed through `parse_query`, so this is read-only by construction;
    /// CONSTRUCT/DESCRIBE are refused to keep the response shape simple.
    pub fn query(&self, sparql: &str) -> Result<Value, RdfError> {
        if sparql.len() > MAX_QUERY_BYTES {
            return Err(RdfError::QueryTooLarge);
        }
        let prepared = SparqlEvaluator::new()
            .parse_query(sparql)
            .map_err(|e| RdfError::Parse(e.to_string()))?;
        match prepared
            .on_store(&self.store)
            .execute()
            .map_err(|e| RdfError::Eval(e.to_string()))?
        {
            QueryResults::Boolean(b) => Ok(json!({"boolean": b})),
            QueryResults::Solutions(sols) => {
                let vars: Vec<String> = sols
                    .variables()
                    .iter()
                    .map(|v| v.as_str().to_string())
                    .collect();
                let mut rows = Vec::new();
                let mut truncated = false;
                for sol in sols {
                    let sol = sol.map_err(|e| RdfError::Eval(e.to_string()))?;
                    if rows.len() >= MAX_ROWS {
                        truncated = true;
                        break;
                    }
                    let mut row = serde_json::Map::new();
                    for (v, t) in sol.iter() {
                        row.insert(v.as_str().to_string(), term_json(t));
                    }
                    rows.push(Value::Object(row));
                }
                Ok(json!({"vars": vars, "rows": rows, "truncated": truncated}))
            }
            _ => Err(RdfError::NotReadOnly),
        }
    }

    /// The built-in shapes, each a SPARQL SELECT of violating requirement ids. Starter rule pack for
    /// `/sparql/shapes`; project-specific shapes are plain queries sent to `query`.
    pub fn check_shapes(&self) -> Result<Vec<ShapeResult>, RdfError> {
        let mut out = Vec::new();
        for shape in SHAPES {
            let v = self.query(shape.sparql)?;
            let ids: Vec<String> = v["rows"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .filter_map(|r| r["id"]["value"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            out.push(ShapeResult {
                shape: shape.name.to_string(),
                severity: shape.severity.to_string(),
                message: shape.message.to_string(),
                violations: ids,
            });
        }
        Ok(out)
    }

    /// Attribution rolled up by code, over a scenario of included requirements when given (None = all).
    /// `prefix_depth` folds hierarchical codes (`CC-4410/WBS-2.3` at depth 1 is `CC-4410`).
    pub fn attribution_rollup(&self, prefix_depth: Option<usize>) -> Result<Value, RdfError> {
        let v = self.query(&format!(
            "PREFIX r: <{NS}> SELECT ?code (SUM(?share) AS ?total) (COUNT(DISTINCT ?req) AS ?requirements) \
             WHERE {{ ?req r:attribution ?l . ?l r:code ?code ; r:share ?share }} GROUP BY ?code ORDER BY ?code"
        ))?;
        let Some(depth) = prefix_depth else {
            return Ok(v);
        };
        // Folding by prefix is a string operation on the code; done here so the SPARQL stays portable.
        let mut folded: std::collections::BTreeMap<String, f64> = Default::default();
        for r in v["rows"].as_array().into_iter().flatten() {
            let code = r["code"]["value"].as_str().unwrap_or_default();
            let key = code
                .split('/')
                .take(depth.max(1))
                .collect::<Vec<_>>()
                .join("/");
            *folded.entry(key).or_default() += r["total"]["value"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
        }
        Ok(
            json!({"rows": folded.into_iter().map(|(code, total)| json!({"code": code, "total": total})).collect::<Vec<_>>()}),
        )
    }
}

#[derive(Debug, Serialize)]
pub struct ShapeResult {
    pub shape: String,
    pub severity: String,
    pub message: String,
    pub violations: Vec<String>,
}

struct Shape {
    name: &'static str,
    severity: &'static str,
    message: &'static str,
    sparql: &'static str,
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "unsatisfied",
        severity: "warning",
        message: "requirement has no satisfying element",
        sparql: "PREFIX r: <urn:kr0ki:req#> SELECT ?id WHERE { ?q a r:Requirement ; r:id ?id . FILTER NOT EXISTS { ?q r:satisfiedBy ?s } } ORDER BY ?id",
    },
    Shape {
        name: "unverified",
        severity: "warning",
        message: "requirement has no verification",
        sparql: "PREFIX r: <urn:kr0ki:req#> SELECT ?id WHERE { ?q a r:Requirement ; r:id ?id . FILTER NOT EXISTS { ?q r:verifiedBy ?s } } ORDER BY ?id",
    },
    Shape {
        name: "unattributed",
        severity: "info",
        message: "requirement carries no cost attribution code",
        sparql: "PREFIX r: <urn:kr0ki:req#> SELECT ?id WHERE { ?q a r:Requirement ; r:id ?id . FILTER NOT EXISTS { ?q r:attribution ?l } } ORDER BY ?id",
    },
    Shape {
        name: "over-allocated",
        severity: "error",
        message: "attribution shares sum to more than 1",
        sparql: "PREFIX r: <urn:kr0ki:req#> SELECT ?id (SUM(?s) AS ?total) WHERE { ?q r:id ?id ; r:attribution ?l . ?l r:share ?s } GROUP BY ?id HAVING (SUM(?s) > 1.0000001) ORDER BY ?id",
    },
    Shape {
        name: "derivation-cycle",
        severity: "error",
        message: "requirement is (transitively) derived from itself",
        sparql: "PREFIX r: <urn:kr0ki:req#> SELECT DISTINCT ?id WHERE { ?q r:id ?id ; r:derivedFrom+ ?q } ORDER BY ?id",
    },
];

fn term_json(t: &oxigraph::model::Term) -> Value {
    use oxigraph::model::Term;
    match t {
        Term::NamedNode(n) => json!({"type": "uri", "value": n.as_str()}),
        Term::BlankNode(b) => json!({"type": "bnode", "value": b.as_str()}),
        Term::Literal(l) => {
            json!({"type": "literal", "value": l.value(), "datatype": l.datatype().as_str()})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> ReqNode {
        ReqNode {
            id: id.into(),
            ..Default::default()
        }
    }
    fn graph() -> ReqGraph {
        let mut r1 = node("R1");
        r1.satisfied_by = vec!["Engine".into()];
        r1.verified_by = vec!["T1".into()];
        r1.attributions = vec![
            Attribution {
                code: "CC-4410/WBS-2.3".into(),
                share: 0.6,
            },
            Attribution {
                code: "CC-4410/WBS-2.4".into(),
                share: 0.4,
            },
        ];
        let mut r2 = node("R2");
        r2.attributions = vec![
            Attribution {
                code: "CC-9000".into(),
                share: 0.7,
            },
            Attribution {
                code: "CC-9001".into(),
                share: 0.5,
            },
        ];
        ReqGraph {
            nodes: vec![r1, r2, node("R3")],
            edges: vec![ReqEdge {
                from: "R2".into(),
                to: "R1".into(),
                kind: "derive".into(),
            }],
        }
    }

    fn ids(shapes: &[ShapeResult], name: &str) -> Vec<String> {
        shapes
            .iter()
            .find(|s| s.shape == name)
            .unwrap()
            .violations
            .clone()
    }

    #[test]
    fn shapes_find_exactly_the_violating_requirements() {
        let s = RdfStore::from_graph(&graph())
            .unwrap()
            .check_shapes()
            .unwrap();
        assert_eq!(ids(&s, "unsatisfied"), ["R2", "R3"]);
        assert_eq!(ids(&s, "unverified"), ["R2", "R3"]);
        assert_eq!(ids(&s, "unattributed"), ["R3"]);
        assert_eq!(ids(&s, "over-allocated"), ["R2"]); // 0.7 + 0.5; R1's 0.6 + 0.4 is exactly 1 and passes
        assert!(ids(&s, "derivation-cycle").is_empty());
    }

    #[test]
    fn a_derivation_cycle_is_reported_for_every_member() {
        let mut g = graph();
        g.edges.push(ReqEdge {
            from: "R1".into(),
            to: "R2".into(),
            kind: "derive".into(),
        });
        let s = RdfStore::from_graph(&g).unwrap().check_shapes().unwrap();
        assert_eq!(ids(&s, "derivation-cycle"), ["R1", "R2"]);
    }

    #[test]
    fn rollup_sums_shares_by_code_and_folds_by_prefix() {
        let st = RdfStore::from_graph(&graph()).unwrap();
        let flat = st.attribution_rollup(None).unwrap();
        let rows = flat["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        let folded = st.attribution_rollup(Some(1)).unwrap();
        let cc4410 = folded["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == "CC-4410")
            .unwrap();
        assert!((cc4410["total"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn property_paths_and_sub_selects_work() {
        let st = RdfStore::from_graph(&graph()).unwrap();
        let v = st
            .query(&format!("PREFIX r: <{NS}> SELECT ?id WHERE {{ ?q r:derivedFrom+ ?p . ?p r:id 'R1' . ?q r:id ?id }}"))
            .unwrap();
        assert_eq!(v["rows"][0]["id"]["value"], "R2");
    }

    #[test]
    fn hostile_identifiers_cannot_break_out_of_their_iri() {
        let mut g = ReqGraph::default();
        g.nodes.push(node("a> . <x:evil> <x:p> <x:o"));
        g.nodes.push(node("a b"));
        let st = RdfStore::from_graph(&g).unwrap();
        let v = st
            .query(&format!(
                "PREFIX r: <{NS}> SELECT ?q WHERE {{ ?q a r:Requirement }}"
            ))
            .unwrap();
        assert_eq!(v["rows"].as_array().unwrap().len(), 2);
        assert_eq!(
            st.query("ASK { <x:evil> ?p ?o }").unwrap()["boolean"],
            false
        );
    }

    #[test]
    fn only_select_and_ask_are_accepted() {
        let st = RdfStore::from_graph(&graph()).unwrap();
        assert!(matches!(
            st.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }"),
            Err(RdfError::NotReadOnly)
        ));
        assert!(matches!(
            st.query("INSERT DATA { <x:a> <x:b> <x:c> }"),
            Err(RdfError::Parse(_))
        ));
        assert!(matches!(
            st.query("SELECT nonsense"),
            Err(RdfError::Parse(_))
        ));
        assert_eq!(st.len(), RdfStore::from_graph(&graph()).unwrap().len());
    }

    #[test]
    fn result_size_and_query_size_are_bounded() {
        let st = RdfStore::from_graph(&graph()).unwrap();
        assert!(matches!(
            st.query(&" ".repeat(MAX_QUERY_BYTES + 1)),
            Err(RdfError::QueryTooLarge)
        ));
        let big = ReqGraph {
            nodes: (0..MAX_NODES + 1).map(|i| node(&i.to_string())).collect(),
            edges: vec![],
        };
        assert!(matches!(
            RdfStore::from_graph(&big),
            Err(RdfError::TooLarge(_))
        ));
    }
}
