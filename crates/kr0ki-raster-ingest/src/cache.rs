//! Key for caching a *finished* run. Attempts are never cached, and uploads are never stored: only this key
//! and the result.
//!
//! Fields are length-prefixed rather than delimiter-joined, so no value can be crafted to collide with another
//! field boundary. Only settings that change the outcome are included; wall-clock and per-call timeouts are
//! operational and deliberately left out.

use crate::{normalize::NormalizedImage, prompts::PROMPT_VERSION, run::LoopConfig};
use sha2::{Digest, Sha256};

const DOMAIN: &str = "kr0ki-raster/v1";

fn put(h: &mut Sha256, field: &str) {
    h.update((field.len() as u64).to_le_bytes());
    h.update(field.as_bytes());
}

pub fn result_cache_key(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    proposer_id: &str,
    judge_id: &str,
) -> String {
    let mut h = Sha256::new();
    for field in [
        DOMAIN,
        &image.sha256,
        &cfg.format,
        proposer_id,
        judge_id,
        PROMPT_VERSION,
        &cfg.max_iterations.to_string(),
        &cfg.max_tokens.to_string(),
        &cfg.stall_window.to_string(),
        &cfg.max_source_chars.to_string(),
        &format!("{:.4}", cfg.label_recall_threshold),
    ] {
        put(&mut h, field);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
