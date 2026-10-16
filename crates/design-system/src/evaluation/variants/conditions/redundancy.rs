//! Variants that have no effect because another variant of the same candidate already
//! requires their condition.

use std::ops::Range;

use stillwind_syntax::Candidate;

use super::atoms::{Condition, Test, application_branches};
use super::selectors::{is_filter, moves_subject};
use super::stack::{StackVariant, resolve_stack};
use crate::ThemeIndex;

/// A variant whose condition another variant of the same candidate already requires.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedundantVariant {
    /// Index of the redundant variant in candidate order.
    pub index: usize,
    /// Byte range of the redundant variant in the candidate text, without its `:`.
    pub range: Range<usize>,
    /// Text of the redundant variant.
    pub variant: String,
    /// Text of the variant that makes it redundant.
    pub kept: String,
    /// Why the variant is redundant.
    pub reason: VariantRedundancy,
}

/// Why a [`RedundantVariant`] has no effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariantRedundancy {
    /// An earlier variant has the same resolved application.
    Duplicate,
    /// The other variant compares the same feature in the same unit with a stricter
    /// bound, as `md` does for `sm`.
    Implied,
}

/// Returns the redundant variants of `candidate`, in candidate order.
///
/// A later variant is a duplicate when an earlier variant resolves to the same
/// application, the application only adds conditions to the styled element, and no
/// variant between them selects another element. A variant is implied when its whole
/// application is one range comparison and another variant's whole application is a
/// stricter comparison of the same feature in the same unit and rule. The function
/// returns no findings when a variant does not resolve.
pub fn redundant_variants(candidate: &Candidate, theme: &ThemeIndex) -> Vec<RedundantVariant> {
    if candidate.variants.len() < 2 {
        return Vec::new();
    }
    let Some(stack) = resolve_stack(candidate, theme) else {
        return Vec::new();
    };
    let mut redundant: Vec<RedundantVariant> = Vec::new();
    let report = |redundant: &mut Vec<RedundantVariant>, index: usize, kept: usize, reason| {
        if redundant.iter().all(|found| found.index != index) {
            redundant.push(RedundantVariant {
                index,
                range: stack[index].range.clone(),
                variant: stack[index].raw.to_owned(),
                kept: stack[kept].raw.to_owned(),
                reason,
            });
        }
    };
    for (index, variant) in stack.iter().enumerate() {
        if let Some(kept) = earlier_duplicate(&stack, index) {
            report(&mut redundant, index, kept, VariantRedundancy::Duplicate);
            continue;
        }
        let Some(range) = single_range(variant, index) else {
            continue;
        };
        let stricter = stack.iter().enumerate().find(|&(other, candidate)| {
            other != index
                && redundant.iter().all(|found| found.index != other)
                && single_range(candidate, other).is_some_and(|other| implies(&other, &range))
        });
        if let Some((kept, _)) = stricter {
            report(&mut redundant, index, kept, VariantRedundancy::Implied);
        }
    }
    redundant.sort_by_key(|found| found.index);
    redundant
}

/// Returns the index of an earlier variant that `stack[index]` repeats.
fn earlier_duplicate(stack: &[StackVariant<'_>], index: usize) -> Option<usize> {
    let application = &stack[index].application;
    if !is_filter(application) {
        return None;
    }
    let selector = moves_subject(application) || application.uses_subject();
    (0..index).rev().find(|&earlier| {
        stack[earlier].application == *application
            && (!selector
                || !stack[earlier + 1..index]
                    .iter()
                    .any(|between| moves_subject(&between.application)))
    })
}

/// The only condition of a variant, when it is a range comparison.
fn single_range(variant: &StackVariant<'_>, index: usize) -> Option<Test> {
    let mut branches = application_branches(&variant.application, index)?;
    let [branch] = branches.as_mut_slice() else {
        return None;
    };
    let [Condition { test, .. }] = branch.as_mut_slice() else {
        return None;
    };
    matches!(test, Test::Range { .. }).then(|| test.clone())
}

/// Whether range comparison `stricter` implies range comparison `weaker` and they
/// differ.
fn implies(stricter: &Test, weaker: &Test) -> bool {
    let (
        Test::Range {
            scope,
            feature,
            bound,
            value,
            unit,
        },
        Test::Range {
            scope: weaker_scope,
            feature: weaker_feature,
            bound: weaker_bound,
            value: weaker_value,
            unit: weaker_unit,
        },
    ) = (stricter, weaker)
    else {
        return false;
    };
    if scope != weaker_scope
        || feature != weaker_feature
        || unit != weaker_unit
        || bound.is_lower() != weaker_bound.is_lower()
        || stricter == weaker
    {
        return false;
    }
    let order = value.total_cmp(weaker_value);
    let tighter = if bound.is_lower() {
        order.is_gt()
    } else {
        order.is_lt()
    };
    // `> 40` implies `>= 40`, but `>= 40` does not imply `> 40`.
    tighter || (order.is_eq() && (bound.is_strict() || !weaker_bound.is_strict()))
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

    fn redundant(theme: &ThemeIndex, raw: &str) -> Vec<(String, String, VariantRedundancy)> {
        redundant_variants(&parse_candidate(raw).unwrap(), theme)
            .into_iter()
            .map(|found| {
                assert_eq!(&raw[found.range.clone()], found.variant);
                (found.variant, found.kept, found.reason)
            })
            .collect()
    }

    #[test]
    fn repeated_filters_and_weaker_ranges_are_redundant() {
        let theme = theme();
        let duplicate = |variant: &str, kept: &str| {
            (
                variant.to_owned(),
                kept.to_owned(),
                VariantRedundancy::Duplicate,
            )
        };
        for (raw, expected) in [
            ("hover:hover:flex", vec![duplicate("hover", "hover")]),
            ("sm:sm:flex", vec![duplicate("sm", "sm")]),
            ("dark:dark:flex", vec![duplicate("dark", "dark")]),
            (
                "group-hover:group-hover:flex",
                vec![duplicate("group-hover", "group-hover")],
            ),
            ("hover:focus:hover:flex", vec![duplicate("hover", "hover")]),
            ("@md:@md:flex", vec![duplicate("@md", "@md")]),
            ("md:*:md:flex", vec![duplicate("md", "md")]),
        ] {
            assert_eq!(redundant(&theme, raw), expected, "{raw}");
        }
        let implied = |variant: &str, kept: &str| {
            (
                variant.to_owned(),
                kept.to_owned(),
                VariantRedundancy::Implied,
            )
        };
        for (raw, expected) in [
            ("md:sm:flex", vec![implied("sm", "md")]),
            ("sm:md:flex", vec![implied("sm", "md")]),
            ("max-sm:max-md:flex", vec![implied("max-md", "max-sm")]),
            ("@min-md:@sm:flex", vec![implied("@sm", "@min-md")]),
            ("@max-sm:@max-md:flex", vec![implied("@max-md", "@max-sm")]),
            ("min-[50rem]:md:flex", vec![implied("md", "min-[50rem]")]),
        ] {
            assert_eq!(redundant(&theme, raw), expected, "{raw}");
        }
    }

    #[test]
    fn variants_that_select_other_elements_or_compare_other_units_are_kept() {
        let theme = theme();
        for raw in [
            "*:*:flex",
            "hover:*:hover:flex",
            "before:before:flex",
            "min-[600px]:sm:flex",
            "sm:max-md:flex",
            "sm:@md:flex",
            "@md/main:@sm:flex",
            "hover:focus:flex",
            "unknown:hover:hover:flex",
        ] {
            assert_eq!(redundant(&theme, raw), [], "{raw}");
        }
    }
}
