//! Generate the static kr0ki mdb00k/playb00k bundle.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("site"));

    kr0ki_core::docgen::export_static_mdb00k(&output)?;
    println!("generated static mdb00k at {}", output.display());
    Ok(())
}
