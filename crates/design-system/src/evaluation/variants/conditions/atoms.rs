//! The simple conditions of a variant stack and the conversion of resolved variant
//! applications into branches of them.
//!
//! A branch is a conjunction of [`Condition`] values, and an application becomes a
//! disjunction of branches: a compound body, such as the result of `not-hover`,
//! contributes one branch per block. Conditions that this module cannot interpret keep
//! their normalized text, so two spellings of one condition compare equal only when
//! their text matches. [`super::queries`] reads media and container queries, and
//! [`super::selectors`] reads selectors.

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use super::super::VariantApplication;
use super::queries::{container_query, normalize, query_conditions, strip_not};
use super::selectors::selector_conditions;

/// The most branches that one candidate's conditions are expanded to. A larger
/// expansion is not analyzed.
pub(super) const MAX_BRANCHES: usize = 64;

/// One condition that a branch requires, with the variant that contributed it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Condition {
    /// Index of the contributing variant in candidate order.
    pub(super) variant: usize,
    /// What the condition tests.
    pub(super) test: Test,
    /// CSS text shown in messages, such as `@media (width < 40rem)` or `&:hover`.
    pub(super) display: LeanStr,
}

/// What a [`Condition`] tests.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Test {
    /// A comparison of a feature with a dimension, such as `(width >= 40rem)`.
    Range {
        /// The rule that evaluates the feature.
        scope: Scope,
        /// Lowercase feature name without a `min-` or `max-` prefix.
        feature: LeanStr,
        /// Direction and strictness of the comparison.
        bound: Bound,
        /// Numeric part of the dimension.
        value: f64,
        /// Lowercase unit of the dimension; empty for a number.
        unit: LeanStr,
    },
    /// A feature compared with one keyword, such as `(orientation: portrait)`, or a
    /// media type, whose feature is [`MEDIA_TYPE`].
    Discrete {
        /// The rule that evaluates the feature.
        scope: Scope,
        /// Lowercase feature name.
        feature: LeanStr,
        /// Lowercase keyword.
        value: LeanStr,
        /// The condition holds when the feature does not have `value`.
        negated: bool,
    },
    /// Any other condition, identified by its scope and normalized text.
    Opaque {
        /// Scope prefix and normalized text.
        key: LeanStr,
        /// The condition holds when the text does not match.
        negated: bool,
    },
}

/// Feature name that [`Test::Discrete`] uses for a media type such as `print`.
pub(super) const MEDIA_TYPE: &str = "<media-type>";

/// The rule that evaluates a feature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Scope {
    /// `@media`.
    Media,
    /// `@container`, with the container name or an empty name.
    Container(LeanStr),
}

/// Direction and strictness of a range comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Bound {
    /// `>=`.
    AtLeast,
    /// `>`.
    Above,
    /// `<=`.
    AtMost,
    /// `<`.
    Below,
}

impl Bound {
    /// Returns the comparison that holds exactly when this one does not.
    pub(super) const fn negated(self) -> Self {
        match self {
            Self::AtLeast => Self::Below,
            Self::Above => Self::AtMost,
            Self::AtMost => Self::Above,
            Self::Below => Self::AtLeast,
        }
    }

    /// Whether the comparison sets a lower bound.
    pub(super) const fn is_lower(self) -> bool {
        matches!(self, Self::AtLeast | Self::Above)
    }

    /// Whether the bound itself fails the comparison.
    pub(super) const fn is_strict(self) -> bool {
        matches!(self, Self::Above | Self::Below)
    }
}

/// Branches of one variant's application, or `None` when they exceed [`MAX_BRANCHES`].
///
/// `variant` is the index stored in each condition. Selector conditions are kept only
/// when the selector adds a compound condition to the styled element itself; other
/// selectors contribute no condition, so a branch never becomes stricter than the CSS.
pub(super) fn application_branches(
    application: &VariantApplication,
    variant: usize,
) -> Option<Vec<Vec<Condition>>> {
    let condition = |test, display: &str| Condition {
        variant,
        test,
        display: display.to_lean_str(),
    };
    let branches = match application {
        VariantApplication::Compound(children) => {
            let mut branches = Vec::new();
            for child in children {
                branches.extend(application_branches(child, variant)?);
                if branches.len() > MAX_BRANCHES {
                    return None;
                }
            }
            branches
        }
        VariantApplication::Sequence(children) => {
            let mut branches = vec![Vec::new()];
            for child in children {
                branches = conjoin(&branches, &application_branches(child, variant)?)?;
            }
            branches
        }
        VariantApplication::Media(query) => vec![query_conditions(query, &Scope::Media, variant)],
        VariantApplication::Container(query) => {
            let (name, query) = container_query(query);
            vec![query_conditions(query, &Scope::Container(name), variant)]
        }
        VariantApplication::Supports(prelude) => {
            let (negated, text) = strip_not(prelude);
            vec![vec![condition(
                Test::Opaque {
                    key: format_args!("@supports {}", normalize(text)).to_lean_str(),
                    negated,
                },
                &format_args!("@supports {prelude}").to_lean_string(),
            )]]
        }
        VariantApplication::Conditional(test) => {
            let (negated, name) = test
                .strip_prefix('!')
                .map_or((false, test.as_str()), |name| (true, name));
            vec![vec![condition(
                Test::Opaque {
                    key: format_args!("if {name}").to_lean_str(),
                    negated,
                },
                test,
            )]]
        }
        VariantApplication::SelectorSuffix(suffix) => {
            vec![selector_conditions(
                &format_args!("&{suffix}").to_lean_string(),
                variant,
            )]
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorCondition(selector)
        | VariantApplication::SelectorRule { selector, .. } => {
            vec![selector_conditions(selector, variant)]
        }
        VariantApplication::SelectorMedia {
            selector,
            condition,
        }
        | VariantApplication::SelectorMediaRule {
            selector,
            condition,
            ..
        } => {
            let mut conditions = selector_conditions(selector, variant);
            conditions.extend(query_conditions(condition, &Scope::Media, variant));
            vec![conditions]
        }
        VariantApplication::StartingStyle => vec![Vec::new()],
    };
    (branches.len() <= MAX_BRANCHES).then_some(branches)
}

/// Returns every combination of one branch from `left` and one from `right`, or `None`
/// when there are more than [`MAX_BRANCHES`].
pub(super) fn conjoin(
    left: &[Vec<Condition>],
    right: &[Vec<Condition>],
) -> Option<Vec<Vec<Condition>>> {
    if left.len().saturating_mul(right.len()) > MAX_BRANCHES {
        return None;
    }
    Some(
        left.iter()
            .flat_map(|left| {
                right.iter().map(move |right| {
                    let mut branch = left.clone();
                    branch.extend(right.iter().cloned());
                    branch
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tests(application: &VariantApplication) -> Vec<Vec<Test>> {
        application_branches(application, 0)
            .unwrap()
            .into_iter()
            .map(|branch| branch.into_iter().map(|condition| condition.test).collect())
            .collect()
    }

    #[test]
    fn compounds_add_branches_and_sequences_combine_them() {
        let not_hover = VariantApplication::Compound(vec![
            VariantApplication::SelectorTemplate("&:not(:hover)".into()),
            VariantApplication::Media("not (hover: hover)".into()),
        ]);
        assert_eq!(tests(&not_hover).len(), 2);
        let sequence = VariantApplication::Sequence(vec![not_hover.clone(), not_hover]);
        assert_eq!(tests(&sequence).len(), 4);
        let wide = VariantApplication::Compound(
            (0..=MAX_BRANCHES)
                .map(|_| VariantApplication::StartingStyle)
                .collect(),
        );
        assert_eq!(application_branches(&wide, 0), None);
    }
}
