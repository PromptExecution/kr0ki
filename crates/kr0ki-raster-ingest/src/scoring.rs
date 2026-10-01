//! Deterministic scoring: how well the text in a render covers the labels read off the original.
//!
//! This is the signal that does not depend on any model's opinion (PLAN-KR0KI-007 §7). Labels are compared
//! after normalization, with a small edit-distance tolerance because the same text read twice rarely matches
//! to the byte.

use crate::types::Verdict;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelScore {
    /// Fraction of target labels found in the render.
    pub recall: f64,
    /// Fraction of rendered labels that correspond to a target label.
    pub precision: f64,
}

/// Punctuation that merely wraps or ends a label ("Auth service:", "(DB)"). Symbols that can be part of the
/// name (`+`, `#`, `.`, `/`, `->`) are kept: trimming them would make `C++`, `C#` and `C` the same label.
const WRAPPING_PUNCTUATION: [char; 12] =
    ['"', '\'', '`', ':', ';', ',', '(', ')', '[', ']', '{', '}'];

/// Case-fold, collapse whitespace, trim wrapping punctuation and a trailing full stop.
pub fn normalize_label(s: &str) -> String {
    let lowered = s.to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|c: char| WRAPPING_PUNCTUATION.contains(&c))
        .trim()
        .trim_end_matches('.')
        .to_string()
}

/// Minimum normalized-Levenshtein similarity for two labels to count as the same text. At 0.85 a single misread
/// character is forgiven only in labels of 7 or more characters; shorter labels must match exactly, which is what
/// we want because one character changes their meaning ("DB" vs "D8", "v1" vs "v2").
const FUZZY_THRESHOLD: f64 = 0.85;

fn similar(a: &str, b: &str) -> bool {
    a == b || strsim::normalized_levenshtein(a, b) >= FUZZY_THRESHOLD
}

/// Greedy one-to-one matching: exact matches are taken first, then the best remaining fuzzy ones.
pub fn label_scores(target: &[String], rendered: &[String]) -> LabelScore {
    let target: Vec<String> = target
        .iter()
        .map(|s| normalize_label(s))
        .filter(|s| !s.is_empty())
        .collect();
    let rendered: Vec<String> = rendered
        .iter()
        .map(|s| normalize_label(s))
        .filter(|s| !s.is_empty())
        .collect();
    let mut used = vec![false; rendered.len()];
    let mut matched = 0usize;

    let mut pending: Vec<usize> = Vec::new();
    for (ti, t) in target.iter().enumerate() {
        match rendered
            .iter()
            .enumerate()
            .find(|(ri, r)| !used[*ri] && *r == t)
        {
            Some((ri, _)) => {
                used[ri] = true;
                matched += 1;
            }
            None => pending.push(ti),
        }
    }
    for ti in pending {
        let best = rendered
            .iter()
            .enumerate()
            .filter(|(ri, r)| !used[*ri] && similar(&target[ti], r))
            .max_by(|a, b| {
                let sa = strsim::normalized_levenshtein(&target[ti], a.1);
                let sb = strsim::normalized_levenshtein(&target[ti], b.1);
                sa.total_cmp(&sb)
            });
        if let Some((ri, _)) = best {
            used[ri] = true;
            matched += 1;
        }
    }

    let ratio = |num: usize, den: usize| {
        if den == 0 {
            1.0
        } else {
            num as f64 / den as f64
        }
    };
    LabelScore {
        recall: ratio(matched, target.len()),
        precision: ratio(matched, rendered.len()),
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
}
