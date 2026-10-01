//! Key for caching a *finished* run. Attempts are never cached, and uploads are never stored: only this key
//! and the result.
//!
//! Fields are length-prefixed rather than delimiter-joined, so no value can be crafted to collide with another
//! field boundary. The key covers every setting that changes what a run *concludes*. Time budgets (`max_wall`,
//! `call_timeout`) are deliberately not in it, because they make an outcome depend on machine load; the rule that
//! follows is that only results for which [`crate::LoopResult::is_cacheable`] is true may be stored under this
//! key (an `Exhausted` run might simply have been cut short).

use crate::{normalize::NormalizedImage, prompts::PROMPT_VERSION, run::LoopConfig};
use sha2::{Digest, Sha256};

const DOMAIN: &str = "kr0ki-raster/v1";

fn put(h: &mut Sha256, field: &str) {
    h.update((field.len() as u64).to_le_bytes());
    h.update(field.as_bytes());
}

/// `renderer_id` names the renderer and its version (for example the Kroki image tag): acceptance depends on what
/// the renderer produced, so a different renderer must not be served a result earned against another.
pub fn result_cache_key(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    proposer_id: &str,
    judge_id: &str,
    renderer_id: &str,
) -> String {
    key_for(
        image,
        cfg,
        [proposer_id, judge_id, renderer_id],
        PROMPT_VERSION,
    )
}

fn key_for(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    ids: [&str; 3],
    prompt_version: &str,
) -> String {
    let [proposer_id, judge_id, renderer_id] = ids;
    let mut h = Sha256::new();
    for field in [
        DOMAIN,
        &image.sha256,
        &cfg.format,
        proposer_id,
        judge_id,
        renderer_id,
        prompt_version,
        &cfg.max_iterations.to_string(),
        &cfg.max_tokens.to_string(),
        &cfg.stall_window.to_string(),
        &cfg.max_consecutive_model_errors.to_string(),
        &format!("{:.4}", cfg.min_match_score),
        &cfg.max_source_chars.to_string(),
        &format!("{:.4}", cfg.label_recall_threshold),
    ] {
        put(&mut h, field);
    }
    crate::hash::hex(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::SourceFormat;

    fn image() -> NormalizedImage {
        NormalizedImage {
            png: bytes::Bytes::new(),
            width: 1,
            height: 1,
            sha256: "a".repeat(64),
            source_sha256: "b".repeat(64),
            source_format: SourceFormat::Png,
        }
    }

    #[test]
    fn the_prompt_version_is_part_of_the_key() {
        let cfg = LoopConfig::new("d2");
        assert_ne!(
            key_for(&image(), &cfg, ["p", "j", "r"], "v1"),
            key_for(&image(), &cfg, ["p", "j", "r"], "v2")
        );
        assert_eq!(
            result_cache_key(&image(), &cfg, "p", "j", "r"),
            key_for(&image(), &cfg, ["p", "j", "r"], PROMPT_VERSION)
        );
    }

    #[test]
    fn the_minimum_match_score_changes_outcomes_so_it_is_part_of_the_key() {
        let a = LoopConfig::new("d2");
        let mut b = a.clone();
        b.min_match_score = 0.95;
        assert_ne!(
            result_cache_key(&image(), &a, "p", "j", "r"),
            result_cache_key(&image(), &b, "p", "j", "r")
        );
    }

    #[test]
    fn the_consecutive_error_cap_changes_outcomes_so_it_is_part_of_the_key() {
        let a = LoopConfig::new("d2");
        let mut b = a.clone();
        b.max_consecutive_model_errors = 9;
        assert_ne!(
            result_cache_key(&image(), &a, "p", "j", "r"),
            result_cache_key(&image(), &b, "p", "j", "r")
        );
    }

    #[test]
    fn the_renderer_identity_is_part_of_the_key() {
        let cfg = LoopConfig::new("d2");
        assert_ne!(
            result_cache_key(&image(), &cfg, "p", "j", "kroki-0.28"),
            result_cache_key(&image(), &cfg, "p", "j", "kroki-0.29"),
            "a result accepted against one renderer build must not be served for another"
        );
    }
}
