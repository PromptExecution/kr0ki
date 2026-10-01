//! Deterministic scoring: how well the text in a render covers the labels read off the original.
//!
//! This is the signal that does not depend on any model's opinion (PLAN-KR0KI-007 §7). Labels are compared
//! after normalization, with a small edit-distance tolerance because the same text read twice rarely matches
//! to the byte.

use crate::types::Verdict;

#[derive(Debug, Clone, PartialEq)]
pub struct LabelScore {
    /// Fraction of target labels found in the render.
    pub recall: f64,
    /// Fraction of rendered labels that correspond to a target label.
    pub precision: f64,
    /// Target labels (as read from the original image) that nothing in the render matched. This is what the
    /// next proposal is told to fix when the deterministic check refuses a match.
    pub missing: Vec<String>,
}

/// Punctuation that merely wraps or ends a label ("Auth service:", "(DB)"). Symbols that can be part of the
/// name (`+`, `#`, `.`, `/`, `->`) are kept: trimming them would make `C++`, `C#` and `C` the same label.
const WRAPPING_PUNCTUATION: [char; 12] =
    ['"', '\'', '`', ':', ';', ',', '(', ')', '[', ']', '{', '}'];

/// Case-fold, collapse whitespace, trim wrapping punctuation and a trailing full stop (repeated until stable, so
/// `(DB).` and `Node.:` reduce the same way as `DB` and `Node`).
pub fn normalize_label(s: &str) -> String {
    let lowered = s.to_lowercase();
    let mut cur = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    loop {
        let next = cur
            .trim_matches(|c: char| WRAPPING_PUNCTUATION.contains(&c))
            .trim()
            .trim_end_matches('.')
            .to_string();
        if next == cur {
            return cur;
        }
        cur = next;
    }
}

/// Minimum normalized-Levenshtein similarity for two labels to count as the same text. At 0.85 a single misread
/// character is forgiven only in labels of 7 or more characters; shorter labels must match exactly, which is what
/// we want because one character changes their meaning ("DB" vs "D8", "v1" vs "v2").
const FUZZY_THRESHOLD: f64 = 0.85;

fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn words(s: &str) -> Vec<&str> {
    s.split([' ', '-', '_']).filter(|w| !w.is_empty()).collect()
}

/// Same text, allowing for a misread character, but never across an index or a one-letter qualifier:
/// `Worker 1` vs `Worker 2`, `Server A` vs `Server B`, `Worker-A` vs `Worker-B` and `ServerA` vs `ServerB`
/// are *different nodes*, though a character-level similarity would call them 0.86-0.88 alike.
fn similar(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if digits(a) != digits(b) {
        return false;
    }
    let (wa, wb) = (words(a), words(b));
    if wa.len() == wb.len() && wa.len() > 1 {
        // Multi-word labels: every word must match exactly, or be a long word with a small misread.
        return wa.iter().zip(&wb).all(|(x, y)| {
            x == y
                || (x.chars().count().min(y.chars().count()) >= 7
                    && strsim::normalized_levenshtein(x, y) >= FUZZY_THRESHOLD)
        });
    }
    // A single token that differs only in its final character is how `ServerA`/`ServerB` look; a real misread
    // is far more often a dropped or doubled letter, so substitution at the very end is not forgiven.
    let (ca, cb): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if ca.len() == cb.len() && ca[..ca.len() - 1] == cb[..cb.len() - 1] {
        return false;
    }
    strsim::normalized_levenshtein(a, b) >= FUZZY_THRESHOLD
}

/// Greedy one-to-one matching: exact matches are taken first, then the best remaining fuzzy ones.
pub fn label_scores(target: &[String], rendered: &[String]) -> LabelScore {
    let target: Vec<(&String, String)> = target
        .iter()
        .map(|t| (t, normalize_label(t)))
        .filter(|(_, n)| !n.is_empty())
        .collect();
    let rendered: Vec<String> = rendered
        .iter()
        .map(|s| normalize_label(s))
        .filter(|s| !s.is_empty())
        .collect();
    let mut used = vec![false; rendered.len()];
    let mut matched = vec![false; target.len()];

    for (ti, (_, t)) in target.iter().enumerate() {
        if let Some((ri, _)) = rendered
            .iter()
            .enumerate()
            .find(|(ri, r)| !used[*ri] && *r == t)
        {
            used[ri] = true;
            matched[ti] = true;
        }
    }
    let pending: Vec<usize> = (0..target.len()).filter(|ti| !matched[*ti]).collect();
    for ti in pending {
        let t = &target[ti].1;
        let best = rendered
            .iter()
            .enumerate()
            .filter(|(ri, r)| !used[*ri] && similar(t, r))
            .max_by(|a, b| {
                strsim::normalized_levenshtein(t, a.1)
                    .total_cmp(&strsim::normalized_levenshtein(t, b.1))
            });
        if let Some((ri, _)) = best {
            used[ri] = true;
            matched[ti] = true;
        }
    }

    let hits = matched.iter().filter(|m| **m).count();
    let ratio = |num: usize, den: usize| {
        if den == 0 {
            1.0
        } else {
            num as f64 / den as f64
        }
    };
    LabelScore {
        recall: ratio(hits, target.len()),
        precision: ratio(hits, rendered.len()),
        missing: target
            .iter()
            .zip(&matched)
            .filter(|(_, m)| !**m)
            .map(|((orig, _), _)| (*orig).clone())
            .collect(),
    }
}

/// One number used to pick the best attempt: 0 for a failed render, otherwise the judge's score, averaged with
/// label recall when labels were available.
pub fn composite_score(
    render_ok: bool,
    verdict: Option<&Verdict>,
    labels: Option<&LabelScore>,
) -> f64 {
    if !render_ok {
        return 0.0;
    }
    let judge = verdict.map_or(0.0, |v| v.score);
    match labels {
        Some(l) => (judge + l.recall) / 2.0,
        None => judge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn normalization_folds_case_whitespace_and_wrapping_punctuation_only() {
        assert_eq!(normalize_label("  Auth   Service: "), "auth service");
        assert_eq!(normalize_label("(DB)"), "db");
        assert_eq!(normalize_label("Node."), "node");
        // meaningful symbols survive
        assert_eq!(normalize_label("C++"), "c++");
        assert_eq!(normalize_label("C#"), "c#");
        assert_eq!(normalize_label(".NET"), ".net");
        assert_eq!(normalize_label("→"), "→");
    }

    #[test]
    fn labels_that_differ_only_by_a_meaningful_symbol_do_not_match() {
        for (a, b) in [("C++", "C#"), ("C++", "C"), (".NET", "NET"), ("v1", "v2")] {
            assert_eq!(label_scores(&s(&[a]), &s(&[b])).recall, 0.0, "{a} vs {b}");
        }
        assert_eq!(label_scores(&s(&["C++"]), &s(&["c++"])).recall, 1.0);
    }

    #[test]
    fn identical_label_sets_score_one() {
        let sc = label_scores(&s(&["Client", "API", "DB"]), &s(&["db", "client", "api"]));
        assert_eq!((sc.recall, sc.precision), (1.0, 1.0));
    }

    #[test]
    fn one_misread_character_is_forgiven_only_in_labels_of_seven_or_more_characters() {
        // 7 chars, one edit: 1 - 1/7 = 0.857 >= 0.85
        assert_eq!(label_scores(&s(&["Gateway"]), &s(&["Gatewy"])).recall, 1.0);
        // 5 chars, one edit: 0.8 < 0.85
        assert_eq!(label_scores(&s(&["Cache"]), &s(&["Cachs"])).recall, 0.0);
        assert_eq!(label_scores(&s(&["DB"]), &s(&["D8"])).recall, 0.0);
    }

    #[test]
    fn missing_and_extra_labels_move_recall_and_precision_separately() {
        let sc = label_scores(
            &s(&["a-node", "b-node", "c-node", "d-node"]),
            &s(&["a-node", "b-node", "zzzzzz"]),
        );
        assert_eq!(sc.recall, 0.5);
        assert!((sc.precision - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn each_rendered_label_can_satisfy_only_one_target() {
        let sc = label_scores(&s(&["server", "server"]), &s(&["server"]));
        assert_eq!(sc.recall, 0.5);
    }

    #[test]
    fn empty_sides_are_defined_not_nan() {
        let none = label_scores(&[], &s(&["x-label"]));
        assert_eq!((none.recall, none.precision), (1.0, 0.0));
        let blank = label_scores(&s(&["x-label"]), &[]);
        assert_eq!((blank.recall, blank.precision), (0.0, 1.0));
        let both = label_scores(&[], &[]);
        assert_eq!((both.recall, both.precision), (1.0, 1.0));
    }

    #[test]
    fn indexed_and_lettered_siblings_are_different_labels() {
        for (a, b) in [
            ("Worker 1", "Worker 2"),
            ("Web Server 1", "Web Server 2"),
            ("Server A", "Server B"),
            ("Zone 1a", "Zone 1b"),
        ] {
            assert_eq!(label_scores(&s(&[a]), &s(&[b])).recall, 0.0, "{a} vs {b}");
        }
        // a duplicated sibling cannot stand in for the missing one
        assert_eq!(
            label_scores(&s(&["Worker 1", "Worker 2"]), &s(&["Worker 1", "Worker 1"])).recall,
            0.5
        );
        // but spacing and a misread in a long word are still forgiven
        assert_eq!(
            label_scores(&s(&["Worker 1"]), &s(&["Worker1"])).recall,
            1.0
        );
        assert_eq!(
            label_scores(&s(&["Auth Service"]), &s(&["Auth Servise"])).recall,
            1.0
        );
    }

    #[test]
    fn dangling_punctuation_is_removed_in_any_order() {
        assert_eq!(normalize_label("(DB)."), "db");
        assert_eq!(normalize_label("Node.:"), "node");
        assert_eq!(normalize_label("\"Auth.\""), "auth");
    }

    #[test]
    fn unmatched_target_labels_are_reported_in_their_original_form() {
        let sc = label_scores(
            &s(&["Auth Service", "Cache", "Queue"]),
            &s(&["auth service"]),
        );
        assert_eq!(sc.missing, s(&["Cache", "Queue"]));
        assert!(label_scores(&s(&["A-node"]), &s(&["a-node"]))
            .missing
            .is_empty());
    }

    #[test]
    fn single_token_siblings_that_differ_by_one_letter_or_digit_are_different_labels() {
        for (a, b) in [
            ("ServerA", "ServerB"),
            ("Worker-A", "Worker-B"),
            ("Worker1", "Worker2"),
            ("node_a", "node_b"),
        ] {
            assert_eq!(label_scores(&s(&[a]), &s(&[b])).recall, 0.0, "{a} vs {b}");
        }
        // a dropped letter is still forgiven
        assert_eq!(label_scores(&s(&["Gateway"]), &s(&["Gatewy"])).recall, 1.0);
    }
}
