//! Variant stacks whose combined condition can never hold.

use std::ops::Range;

use lean_string::LeanStr;
use stillwind_syntax::Candidate;

use super::atoms::{Condition, MEDIA_TYPE, Test, application_branches, conjoin};
use super::selectors::moves_subject;
use super::stack::resolve_stack;
use crate::ThemeIndex;

/// Media features that have exactly one value at a time, so that two different values
/// exclude each other. `any-hover` and `any-pointer` are absent because several input
/// devices can match different values at once.
const SINGLE_VALUE_FEATURES: &[&str] = &[
    MEDIA_TYPE,
    "display-mode",
    "forced-colors",
    "hover",
    "inverted-colors",
    "orientation",
    "overflow-block",
    "overflow-inline",
    "pointer",
    "prefers-color-scheme",
    "prefers-contrast",
    "prefers-reduced-data",
    "prefers-reduced-motion",
    "prefers-reduced-transparency",
    "scripting",
    "update",
];

/// Variants of one candidate whose combined condition can never hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsatisfiableVariants {
    /// Byte range in the candidate text from the first to the last variant that takes
    /// part in a contradiction, without the trailing `:`.
    pub range: Range<usize>,
    /// Two conditions that exclude each other in the first branch, as CSS text in
    /// candidate order.
    pub conditions: [LeanStr; 2],
    /// Number of alternative branches of the combined condition; each one contains a
    /// contradiction.
    pub branches: usize,
}

/// Returns the contradiction of `candidate`'s variants, or `None` when some branch of
/// their combined condition can hold or the compiler cannot prove otherwise.
///
/// The combined condition is an OR of branches, and each branch is an AND of simple
/// conditions. A branch is impossible when it contains a condition and its negation,
/// two different values of a feature that has one value at a time, or range comparisons
/// of one feature in one unit that leave no value. Selector conditions are compared
/// only when every selector of the stack adds conditions to the styled element itself.
/// Conditions that the compiler cannot interpret never contradict different text. The
/// function returns `None` when a variant does not resolve or the condition has more
/// than 64 branches.
pub fn unsatisfiable_variants(
    candidate: &Candidate,
    theme: &ThemeIndex,
) -> Option<UnsatisfiableVariants> {
    if candidate.variants.len() < 2 {
        return None;
    }
    let stack = resolve_stack(candidate, theme)?;
    let compare_selectors = !stack
        .iter()
        .any(|variant| moves_subject(&variant.application));
    let mut branches = vec![Vec::new()];
    for (index, variant) in stack.iter().enumerate() {
        branches = conjoin(
            &branches,
            &application_branches(&variant.application, index)?,
        )?;
    }
    let mut first = None;
    let mut involved = Vec::new();
    for branch in &branches {
        let branch = branch
            .iter()
            .filter(|condition| {
                compare_selectors
                    || !matches!(&condition.test, Test::Opaque { key, .. } if key.starts_with("& "))
            })
            .collect::<Vec<_>>();
        let (left, right) = contradiction(&branch)?;
        involved.extend([left.variant, right.variant]);
        first.get_or_insert([left.display.clone(), right.display.clone()]);
    }
    let first_variant = *involved.iter().min()?;
    let last_variant = *involved.iter().max()?;
    Some(UnsatisfiableVariants {
        range: stack[first_variant].range.start..stack[last_variant].range.end,
        conditions: first?,
        branches: branches.len(),
    })
}

/// Returns two conditions of `branch` that exclude each other, earlier first.
fn contradiction<'a>(branch: &[&'a Condition]) -> Option<(&'a Condition, &'a Condition)> {
    for (index, right) in branch.iter().enumerate() {
        for left in &branch[..index] {
            if excludes(&left.test, &right.test) {
                return Some(if left.variant <= right.variant {
                    (left, right)
                } else {
                    (right, left)
                });
            }
        }
    }
    // Several lower and upper bounds can leave no value even when each pair overlaps
    // only through a third bound, so ranges compare the tightest pair.
    for (index, lower) in branch.iter().enumerate() {
        for upper in &branch[index + 1..] {
            for (lower, upper) in [(lower, upper), (upper, lower)] {
                if tightest(branch, lower)
                    && tightest(branch, upper)
                    && ranges_exclude(lower, upper)
                {
                    return Some(if lower.variant <= upper.variant {
                        (lower, upper)
                    } else {
                        (upper, lower)
                    });
                }
            }
        }
    }
    None
}

/// Whether `left` and `right` cannot hold together, ignoring range comparisons.
fn excludes(left: &Test, right: &Test) -> bool {
    match (left, right) {
        (
            Test::Opaque { key, negated },
            Test::Opaque {
                key: other,
                negated: other_negated,
            },
        ) => key == other && negated != other_negated,
        (
            Test::Discrete {
                scope,
                feature,
                value,
                negated,
            },
            Test::Discrete {
                scope: other_scope,
                feature: other_feature,
                value: other_value,
                negated: other_negated,
            },
        ) if scope == other_scope && feature == other_feature => {
            if value == other_value {
                negated != other_negated
            } else {
                !negated && !other_negated && SINGLE_VALUE_FEATURES.contains(&feature.as_str())
            }
        }
        _ => false,
    }
}

/// Whether no other comparison of `branch` on the same feature and unit is a tighter
/// bound in the same direction than `condition`.
fn tightest(branch: &[&Condition], condition: &Condition) -> bool {
    let Test::Range {
        scope,
        feature,
        bound,
        value,
        unit,
    } = &condition.test
    else {
        return false;
    };
    !branch.iter().any(|other| {
        let Test::Range {
            scope: other_scope,
            feature: other_feature,
            bound: other_bound,
            value: other_value,
            unit: other_unit,
        } = &other.test
        else {
            return false;
        };
        other_scope == scope
            && other_feature == feature
            && other_unit == unit
            && other_bound.is_lower() == bound.is_lower()
            && if bound.is_lower() {
                other_value > value
            } else {
                other_value < value
            }
    })
}

/// Whether lower bound `lower` and upper bound `upper` on one feature and unit leave no
/// value.
fn ranges_exclude(lower: &Condition, upper: &Condition) -> bool {
    let (
        Test::Range {
            scope,
            feature,
            bound,
            value,
            unit,
        },
        Test::Range {
            scope: upper_scope,
            feature: upper_feature,
            bound: upper_bound,
            value: upper_value,
            unit: upper_unit,
        },
    ) = (&lower.test, &upper.test)
    else {
        return false;
    };
    scope == upper_scope
        && feature == upper_feature
        && unit == upper_unit
        && bound.is_lower()
        && !upper_bound.is_lower()
        && match value.total_cmp(upper_value) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => bound.is_strict() || upper_bound.is_strict(),
            std::cmp::Ordering::Less => false,
        }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceFile};
    use stillwind_syntax::parse_candidate;

    use super::*;
    use crate::test_support::parse_fixture_theme;

    fn theme() -> ThemeIndex {
        parse_fixture_theme(&SourceFile::new(FileId(1), None, None, "").unwrap()).unwrap()
    }

    fn unsatisfiable(theme: &ThemeIndex, raw: &str) -> Option<(String, [String; 2], usize)> {
        unsatisfiable_variants(&parse_candidate(raw).unwrap(), theme).map(|found| {
            (
                raw[found.range].to_owned(),
                found.conditions.map(|condition| condition.to_string()),
                found.branches,
            )
        })
    }

    fn found(
        variants: &str,
        left: &str,
        right: &str,
        branches: usize,
    ) -> (String, [String; 2], usize) {
        (
            variants.to_owned(),
            [left.to_owned(), right.to_owned()],
            branches,
        )
    }

    #[test]
    fn contradicting_variants_report_every_branch() {
        let theme = theme();
        for (raw, expected) in [
            (
                "max-sm:sm:flex",
                found(
                    "max-sm:sm",
                    "@media (width < 40rem)",
                    "@media (width >= 40rem)",
                    1,
                ),
            ),
            (
                "sm:max-sm:flex",
                found(
                    "sm:max-sm",
                    "@media (width >= 40rem)",
                    "@media (width < 40rem)",
                    1,
                ),
            ),
            (
                "portrait:landscape:flex",
                found(
                    "portrait:landscape",
                    "@media (orientation: portrait)",
                    "@media (orientation: landscape)",
                    1,
                ),
            ),
            (
                "motion-safe:motion-reduce:flex",
                found(
                    "motion-safe:motion-reduce",
                    "@media (prefers-reduced-motion: no-preference)",
                    "@media (prefers-reduced-motion: reduce)",
                    1,
                ),
            ),
            (
                "pointer-fine:pointer-coarse:flex",
                found(
                    "pointer-fine:pointer-coarse",
                    "@media (pointer: fine)",
                    "@media (pointer: coarse)",
                    1,
                ),
            ),
            (
                "hover:not-hover:flex",
                found("hover:not-hover", "&:hover", "&:not(:hover)", 2),
            ),
            (
                "not-hover:hover:flex",
                found("not-hover:hover", "&:not(:hover)", "&:hover", 2),
            ),
            (
                "dark:not-dark:flex",
                found(
                    "dark:not-dark",
                    "@media (prefers-color-scheme: dark)",
                    "@media not (prefers-color-scheme: dark)",
                    1,
                ),
            ),
            (
                "print:not-print:flex",
                found("print:not-print", "@media print", "@media not print", 1),
            ),
            (
                "supports-grid:not-supports-grid:flex",
                found(
                    "supports-grid:not-supports-grid",
                    "@supports (grid: var(--tw))",
                    "@supports not (grid: var(--tw))",
                    1,
                ),
            ),
            (
                "sm:focus:max-sm:flex",
                found(
                    "sm:focus:max-sm",
                    "@media (width >= 40rem)",
                    "@media (width < 40rem)",
                    1,
                ),
            ),
            (
                "@md:@max-md:flex",
                found(
                    "@md:@max-md",
                    "@container (width >= 28rem)",
                    "@container (width < 28rem)",
                    1,
                ),
            ),
        ] {
            assert_eq!(unsatisfiable(&theme, raw), Some(expected), "{raw}");
        }
    }

    #[test]
    fn conditions_that_can_hold_together_or_cannot_be_compared_are_not_reported() {
        let theme = theme();
        for raw in [
            "sm:max-md:flex",
            "hover:focus:flex",
            "dark:portrait:flex",
            "min-[600px]:max-sm:flex",
            "any-pointer-fine:any-pointer-coarse:flex",
            "hover:*:not-hover:flex",
            "@md/main:@max-md:flex",
            "[@media(width>=40rem)]:not-[@media(width>=41rem)]:flex",
            "not-sm:max-md:flex",
            "unknown:max-sm:sm:flex",
        ] {
            assert_eq!(unsatisfiable(&theme, raw), None, "{raw}");
        }
    }
}
