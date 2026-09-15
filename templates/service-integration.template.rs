// service-integration.template.rs
// ------------------------------------------------------------------------------
// How a sibling b00t service integrates with kr0ki-core programmatically.
// This is a *template snippet* — copy into your crate's src/ and adjust paths.
//
// Dependencies (Cargo.toml):
//   kr0ki-core = { git = "https://github.com/PromptExecution/kr0ki" }
//   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//
// Invariants:
//   - The service NEVER calls Kroki HTTP directly; it goes through RenderService.
//   - The service NEVER invents a parallel cache key scheme; it reuses cache_key().
//   - The service NEVER defines its own DiagramFormat; it uses kr0ki-core's enum.
// ------------------------------------------------------------------------------

use kr0ki_core::{
    cache::{cache_key, FsCache, OutputKind},
    format::DiagramFormat,
    render::HttpKrokiBackend,
    RenderService,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Build the backend + cache exactly as kr0ki-server does.
    let backend = HttpKrokiBackend::new("https://kroki.io");
    let cache = FsCache::new("./.render-cache");
    let service = RenderService::new(backend, cache);

    // 2. Render an executable diagram (this template's D2 source).
    let d2_source = include_str!("../templates/b00t-stack-orchestration.d2");
    let rendered = service
        .render(DiagramFormat::D2, OutputKind::Svg, d2_source)
        .await?;

    println!(
        "cache={} key={} bytes={}",
        if rendered.status == kr0ki_core::cache::CacheStatus::Hit {
            "hit"
        } else {
            "miss"
        },
        rendered.key,
        rendered.bytes.len()
    );

    // 3. The key is deterministic: same D2 source → same key → byte-identical SVG.
    //    This is the contract kr0ki's CDN tier (D5) will key on too.
    assert_eq!(
        rendered.key,
        cache_key(DiagramFormat::D2, OutputKind::Svg, d2_source)
    );

    // 4. Write the artifact (or serve it, or upload it to a docs site).
    tokio::fs::write("b00t-stack-orchestration.svg", &rendered.bytes).await?;
    Ok(())
}
