//! Spelling suggestions for unknown utility and variant names.
//!
//! A suggestion is offered only for a likely typo: the candidate must be within a small
//! edit distance of the unknown text, and the caller confirms that the suggested
//! spelling resolves before it is reported.

use lean_string::LeanString;

/// Maximum number of close spellings that [`closest_spelling`] passes to `resolves`.
///
/// Resolving a candidate evaluates definition bodies, so a broad registry must not turn
/// one unknown name into hundreds of resolutions.
const MAX_CHECKED: usize = 8;

/// Returns the spelling in `candidates` that is closest to `unknown` and for which
/// `resolves` returns true.
///
/// Spellings are compared by Levenshtein distance over characters. Text of up to three
/// characters accepts a distance of 1, and longer text accepts a distance of 2. A
/// spelling equal to `unknown` is never returned. Spellings with the same distance are
/// tried in lexicographic order, so `red-999` prefers `red-900` to `red-950`.
pub(in crate::evaluation) fn closest_spelling(
    unknown: &str,
    candidates: impl IntoIterator<Item = LeanString>,
    mut resolves: impl FnMut(&str) -> bool,
) -> Option<LeanString> {
    let limit = if unknown.chars().count() <= 3 { 1 } else { 2 };
    let mut close = candidates
        .into_iter()
        .filter(|candidate| candidate != unknown)
        .filter_map(|candidate| {
            let distance = edit_distance(unknown, &candidate);
            (distance <= limit).then_some((distance, candidate))
        })
        .collect::<Vec<_>>();
    close.sort();
    close.dedup();
    close
        .into_iter()
        .take(MAX_CHECKED)
        .map(|(_, candidate)| candidate)
        .find(|candidate| resolves(candidate))
}

/// Returns the number of character insertions, deletions, and substitutions that turn
/// `left` into `right`.
fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (row, left) in left.chars().enumerate() {
        current[0] = row + 1;
        for (column, right) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left != *right);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_prefer_the_nearest_resolving_spelling() {
        assert_eq!(edit_distance("red-999", "red-900"), 2);
        assert_eq!(edit_distance("flx", "flex"), 1);
        let candidates = ["red-950", "red-900", "red-50", "blue-900"].map(LeanString::from);
        assert_eq!(
            closest_spelling("red-999", candidates.clone(), |_| true).as_deref(),
            Some("red-900")
        );
        assert_eq!(
            closest_spelling("red-999", candidates, |candidate| candidate != "red-900").as_deref(),
            Some("red-950")
        );
        assert_eq!(
            closest_spelling("hv", ["hover".into()], |_| true),
            None,
            "a short name accepts one edit"
        );
        assert_eq!(closest_spelling("flex", ["flex".into()], |_| true), None);
    }
}
