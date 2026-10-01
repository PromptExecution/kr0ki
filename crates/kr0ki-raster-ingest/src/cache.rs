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

pub fn result_cache_key(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    proposer_id: &str,
    judge_id: &str,
) -> String {
    key_for(image, cfg, proposer_id, judge_id, PROMPT_VERSION)
}

fn key_for(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    proposer_id: &str,
    judge_id: &str,
    prompt_version: &str,
) -> String {
    let mut h = Sha256::new();
    for field in [
        DOMAIN,
        &image.sha256,
        &cfg.format,
        proposer_id,
        judge_id,
        prompt_version,
        &cfg.max_iterations.to_string(),
        &cfg.max_tokens.to_string(),
        &cfg.stall_window.to_string(),
        &cfg.max_consecutive_model_errors.to_string(),
        &cfg.max_source_chars.to_string(),
        &format!("{:.4}", cfg.label_recall_threshold),
    ] {
        put(&mut h, field);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::SourceFormat;

    fn image() -> NormalizedImage {
        NormalizedImage {
            png: vec![],
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
            key_for(&image(), &cfg, "p", "j", "v1"),
            key_for(&image(), &cfg, "p", "j", "v2")
        );
        assert_eq!(
            result_cache_key(&image(), &cfg, "p", "j"),
            key_for(&image(), &cfg, "p", "j", PROMPT_VERSION)
        );
    }

    #[test]
    fn the_consecutive_error_cap_changes_outcomes_so_it_is_part_of_the_key() {
        let a = LoopConfig::new("d2");
        let mut b = a.clone();
        b.max_consecutive_model_errors = 9;
        assert_ne!(
            result_cache_key(&image(), &a, "p", "j"),
            result_cache_key(&image(), &b, "p", "j")
        );
    }
}
