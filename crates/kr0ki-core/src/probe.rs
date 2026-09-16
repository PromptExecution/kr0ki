//! `just probe-formats` (kr0ki#18): discover which of a Kroki backend's
//! registered converters are companion-free and not yet in
//! [`DiagramFormat::ALL`].
//!
//! Replaces the manual "POST a probe source, eyeball 200 vs 503" process
//! from PR #17 — a 20-minutes-per-format investigation with no tooling
//! behind it, and easy to get wrong: BusyBox `wget --post-file` on piped
//! stdin sent no `Content-Length` and produced a spurious `500` from
//! `symbolator` that a real buffered client (this module uses `reqwest`,
//! same as [`crate::render::HttpKrokiBackend`]) doesn't reproduce.
//!
//! Probe sources come from the vendored kroki.io example catalogue
//! (`fixtures/kroki-examples.json`, kr0ki#19) — real, working sources per
//! format, not ad hoc guesses. A candidate with no vendored source is
//! reported as such rather than probed with a guessed placeholder: a guess
//! that happens to fail proves nothing about whether the format itself
//! needs a companion (exactly the ambiguity #19 exists to remove).
//!
//! This talks to the Kroki backend directly — never through kr0ki-server's
//! own `/render/:format` — so probing never pollutes `FsCache` with
//! throwaway artifacts.

use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;

use crate::format::DiagramFormat;

/// The vendored kroki.io example catalogue, embedded at compile time.
const KROKI_EXAMPLES_JSON: &str = include_str!("../fixtures/kroki-examples.json");

#[derive(Debug, Deserialize)]
struct ExamplesFixture {
    types: Vec<FixtureType>,
}

#[derive(Debug, Deserialize)]
struct FixtureType {
    #[serde(rename = "type")]
    type_: String,
    examples: Vec<FixtureExample>,
}

#[derive(Debug, Deserialize)]
struct FixtureExample {
    source: String,
}

/// A known-good probe source per kroki.io type slug — each type's first
/// vendored example. `None` if the fixture is somehow malformed (never true
/// for the committed fixture; kept as a `Result` so a corrupt file is a
/// clear error, not a panic, if this is ever called against an edited copy).
fn probe_sources() -> Result<BTreeMap<String, String>, serde_json::Error> {
    let fixture: ExamplesFixture = serde_json::from_str(KROKI_EXAMPLES_JSON)?;
    Ok(fixture
        .types
        .into_iter()
        .filter_map(|t| t.examples.into_iter().next().map(|e| (t.type_, e.source)))
        .collect())
}

/// `GET {backend}/health`'s `version` map, minus the `"kroki"` server-info
/// entry — every converter Kroki has registered, *regardless* of whether a
/// companion is actually running for it (issue #18/#20's own caveat: this
/// alone is not the "does it need a companion" signal, just the candidate
/// list; probing each one is).
pub async fn fetch_registered_converters(
    client: &reqwest::Client,
    backend: &str,
) -> anyhow::Result<BTreeMap<String, String>> {
    let url = format!("{}/health", backend.trim_end_matches('/'));
    let body: serde_json::Value = client.get(&url).send().await?.json().await?;
    let versions = body
        .get("version")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("{url} response has no \"version\" object"))?;
    Ok(versions
        .iter()
        .filter(|(k, _)| k.as_str() != "kroki")
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect())
}

/// Registered converters not already in [`DiagramFormat::ALL`].
pub fn candidates(registered: &BTreeMap<String, String>) -> Vec<String> {
    let wired: HashSet<&str> = DiagramFormat::ALL.iter().map(|f| f.kroki_slug()).collect();
    registered
        .keys()
        .filter(|slug| !wired.contains(slug.as_str()))
        .cloned()
        .collect()
}

/// One candidate's probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// `200` with a real (non-empty, SVG-shaped) body — companion-free, safe
    /// to add to `DiagramFormat::ALL`.
    CompanionFree,
    /// `503` — needs a companion kr0ki doesn't run; correctly excluded.
    CompanionRequired,
    /// No vendored probe source exists for this slug (e.g. `dot`, an alias
    /// Kroki accepts alongside `graphviz` that kroki.io's own catalogue
    /// doesn't list separately) — not probed at all.
    NoProbeSource,
    /// Anything else: a genuinely unexpected status, or a `200` whose body
    /// isn't real SVG (the exact `symbolator` silent-0-byte-body failure
    /// mode this tool exists to catch) — needs a human look.
    NeedsInvestigation { status: u16, detail: String },
}

/// Probe one candidate slug against `backend`, using its vendored source if
/// one exists.
pub async fn probe_one(
    client: &reqwest::Client,
    backend: &str,
    slug: &str,
) -> anyhow::Result<ProbeOutcome> {
    let sources = probe_sources()?;
    let Some(source) = sources.get(slug) else {
        return Ok(ProbeOutcome::NoProbeSource);
    };
    Ok(probe_with_source(client, backend, slug, source).await)
}

async fn probe_with_source(
    client: &reqwest::Client,
    backend: &str,
    slug: &str,
    source: &str,
) -> ProbeOutcome {
    let url = format!("{}/{slug}/svg", backend.trim_end_matches('/'));
    let resp = match client
        .post(&url)
        .header("Content-Type", "text/plain")
        .body(source.to_string())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return ProbeOutcome::NeedsInvestigation {
                status: 0,
                detail: format!("request failed: {e}"),
            }
        }
    };

    let status = resp.status();
    if status.as_u16() == 503 {
        return ProbeOutcome::CompanionRequired;
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return ProbeOutcome::NeedsInvestigation {
            status: status.as_u16(),
            detail: body.chars().take(200).collect(),
        };
    }

    // "200 OK, real SVG — not just status" (issue #18's own phrasing): the
    // symbolator failure mode was exactly a 200 with a 0-byte body.
    let bytes = resp.bytes().await.unwrap_or_default();
    let looks_like_svg = bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg");
    if looks_like_svg {
        ProbeOutcome::CompanionFree
    } else {
        ProbeOutcome::NeedsInvestigation {
            status: 200,
            detail: format!(
                "200 OK but body doesn't look like SVG ({} bytes)",
                bytes.len()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn probe_sources_covers_every_wired_diagram_format() {
        let sources = probe_sources().unwrap();
        let missing: Vec<&str> = DiagramFormat::ALL
            .iter()
            .map(|f| f.kroki_slug())
            .filter(|slug| !sources.contains_key(*slug))
            .collect();
        assert!(
            missing.is_empty(),
            "no vendored probe source for: {missing:?}"
        );
    }

    #[tokio::test]
    async fn fetch_registered_converters_drops_the_kroki_entry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": { "kroki": {"number": "1.0"}, "d2": "0.7.1", "mermaid": "11.0" },
                "status": "pass"
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let registered = fetch_registered_converters(&client, &server.uri())
            .await
            .unwrap();
        assert_eq!(registered.len(), 2);
        assert_eq!(registered.get("d2"), Some(&"0.7.1".to_string()));
        assert!(!registered.contains_key("kroki"));
    }

    #[test]
    fn candidates_excludes_already_wired_formats() {
        let mut registered = BTreeMap::new();
        registered.insert("d2".to_string(), "0.7.1".to_string()); // already wired
        registered.insert("mermaid".to_string(), "11.0".to_string()); // companion-required, not wired
        registered.insert("dot".to_string(), "14.1.3".to_string()); // graphviz alias, not wired

        let found = candidates(&registered);
        assert!(!found.contains(&"d2".to_string()));
        assert!(found.contains(&"mermaid".to_string()));
        assert!(found.contains(&"dot".to_string()));
    }

    #[tokio::test]
    async fn probe_with_source_reports_companion_free_on_real_svg() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/d2/svg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"<?xml version=\"1.0\"?><svg></svg>".to_vec()),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let outcome = probe_with_source(&client, &server.uri(), "d2", "a -> b").await;
        assert_eq!(outcome, ProbeOutcome::CompanionFree);
    }

    #[tokio::test]
    async fn probe_with_source_reports_companion_required_on_503() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mermaid/svg"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let outcome = probe_with_source(&client, &server.uri(), "mermaid", "graph TD").await;
        assert_eq!(outcome, ProbeOutcome::CompanionRequired);
    }

    #[tokio::test]
    async fn probe_with_source_flags_a_200_with_empty_body_as_needs_investigation() {
        // The exact symbolator failure mode this tool exists to catch: a
        // silent 200 OK with a 0-byte body, no error at all.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/symbolator/svg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(Vec::<u8>::new()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let outcome = probe_with_source(&client, &server.uri(), "symbolator", "entity foo").await;
        assert!(matches!(
            outcome,
            ProbeOutcome::NeedsInvestigation { status: 200, .. }
        ));
    }

    #[tokio::test]
    async fn probe_one_reports_no_probe_source_for_an_unvendored_slug() {
        let server = MockServer::start().await;
        // No mock registered for /dot/svg at all — probe_one must never call
        // out for a slug with no vendored source.
        let client = reqwest::Client::new();
        let outcome = probe_one(&client, &server.uri(), "dot").await.unwrap();
        assert_eq!(outcome, ProbeOutcome::NoProbeSource);
    }
}
