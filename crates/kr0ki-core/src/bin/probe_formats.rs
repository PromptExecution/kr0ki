//! `just probe-formats [backend]` (kr0ki#18) — thin CLI over `kr0ki_core::probe`.

use kr0ki_core::probe::{self, ProbeOutcome};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:8010".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("kr0ki-probe-formats/", env!("CARGO_PKG_VERSION")))
        .build()?;

    println!("probing {backend}");
    let registered = probe::fetch_registered_converters(&client, &backend).await?;
    println!(
        "{} converters registered at {backend}/health",
        registered.len()
    );

    let mut candidates = probe::candidates(&registered);
    candidates.sort();
    if candidates.is_empty() {
        println!("no candidates: every registered converter is already in DiagramFormat::ALL");
        return Ok(());
    }
    println!(
        "{} candidate(s) not yet in DiagramFormat::ALL:\n",
        candidates.len()
    );

    let mut companion_free = Vec::new();
    for slug in &candidates {
        let outcome = probe::probe_one(&client, &backend, slug).await?;
        let version = registered.get(slug).map(String::as_str).unwrap_or("?");
        match &outcome {
            ProbeOutcome::CompanionFree => {
                println!("  {slug:<16} v{version:<10} OK — companion-free, real SVG");
                companion_free.push(slug.clone());
            }
            ProbeOutcome::CompanionRequired => {
                println!("  {slug:<16} v{version:<10} 503 — needs a companion, correctly excluded");
            }
            ProbeOutcome::NoProbeSource => {
                println!("  {slug:<16} v{version:<10} skipped — no vendored source in fixtures/kroki-examples.json");
            }
            ProbeOutcome::NeedsInvestigation { status, detail } => {
                println!("  {slug:<16} v{version:<10} {status} — needs investigation: {detail}");
            }
        }
    }

    if !companion_free.is_empty() {
        println!(
            "\ncompanion-free candidates ready to add to DiagramFormat::ALL: {companion_free:?}"
        );
    }

    Ok(())
}
