//! Expansion of variant alternatives into separate variant sequences.
//!
//! An alternatives variant, such as `or(a,b)`, and a [`StyleVariant::Alternatives`]
//! wrapper hold when any of their sequences holds. A variant is an alternatives variant
//! when [`variant_alternatives`] finds a registered definition that captures it. The
//! compiler expands them before it resolves any variant, so a contribution under
//! alternatives becomes one contribution for each combination of sequences. Each
//! expanded contribution is resolved, ordered, and checked as if it were written on its
//! own: `or(hover,md:focus):p-4` produces the same rules as `hover:p-4 md:focus:p-4`.
//!
//! Ordering each sequence on its own keeps the cascade of an alternative equal to the
//! cascade of the separate utilities. A single position for the whole alternative, as
//! a custom variant with several blocks has, would make `or(hover,md):p-4` win or lose
//! against `md:p-8` depending on where that position falls, which the author cannot
//! see.
//!
//! An alternatives variant must be a variant of the candidate or prefix itself. Inside
//! the value of another variant, such as `not-or(...)`, it is not expanded, and
//! resolution reports [`VariantFailureReason::InvalidAlternatives`].

use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    ThemeIndex, UtilityFailure, UtilityFailureReason, VariantFailureReason, variant_alternatives,
};
use stillwind_diagnostics::LintFinding;
use stillwind_ir::{RefCondition, StyleVariant};
use stillwind_source::Spanned;
use stillwind_syntax::{parse_candidate, parse_variant_prefix};

use super::super::utilities::unresolved_utility_finding;

/// The largest number of sequences that one candidate or wrapper list may expand to.
///
/// Alternatives multiply: three `or(...)` variants with four sequences each would
/// otherwise emit 64 rules for one utility.
const MAX_SEQUENCES: usize = 32;

/// Expands the alternatives variants of one candidate, such as `md:or(hover,focus):p-4`.
///
/// Returns the candidate itself when it has no alternatives or is not a candidate.
/// Otherwise returns one candidate for each combination of sequences, in the order of
/// the alternatives as written.
///
/// # Errors
///
/// Returns a failure when an alternatives variant is empty, contains an empty
/// sequence, has unbalanced delimiters, or when the candidate expands to more than
/// [`MAX_SEQUENCES`] candidates. The failure range covers the variant as written.
pub(in crate::compiler) fn expand_candidate(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Vec<LeanString>, UtilityFailure> {
    let Some(candidate) = parse_candidate(raw) else {
        return Ok(vec![raw.to_lean_string()]);
    };
    let variants = candidate
        .variants
        .iter()
        .map(|variant| variant.raw.as_str())
        .collect::<Vec<_>>();
    if !variants
        .iter()
        .any(|variant| variant_alternatives(variant, theme).is_some())
    {
        return Ok(vec![raw.to_lean_string()]);
    }
    let consumed = variants
        .iter()
        .map(|variant| variant.len() + 1)
        .sum::<usize>();
    let prefixes = expand_variants(&variants, 0, theme)?;
    Ok(prefixes
        .into_iter()
        .map(|prefix| format_args!("{prefix}{}", &raw[consumed..]).to_lean_string())
        .collect())
}

/// Expands the alternatives variants of a variant prefix that ends with `:`, such as
/// `md:or(hover,focus):`.
///
/// Returns the prefix itself when it has no alternatives or is malformed. Resolution
/// reports a malformed prefix.
///
/// # Errors
///
/// Returns the failures that [`expand_candidate`] describes.
fn expand_prefix(raw: &str, theme: &ThemeIndex) -> Result<Vec<LeanString>, UtilityFailure> {
    let Some(variants) = parse_variant_prefix(raw) else {
        return Ok(vec![raw.to_lean_string()]);
    };
    let variants = variants
        .iter()
        .map(|variant| variant.raw.as_str())
        .collect::<Vec<_>>();
    if !variants
        .iter()
        .any(|variant| variant_alternatives(variant, theme).is_some())
    {
        return Ok(vec![raw.to_lean_string()]);
    }
    expand_variants(&variants, 0, theme)
}

/// Expands variant spellings into prefixes that end with `:`. `offset` is the byte
/// offset of the first variant in the text that a failure range refers to.
fn expand_variants(
    variants: &[&str],
    offset: usize,
    theme: &ThemeIndex,
) -> Result<Vec<LeanString>, UtilityFailure> {
    let mut prefixes = vec![LeanString::new()];
    let mut start = offset;
    for variant in variants {
        let range = start..start + variant.len();
        let sequences = match variant_alternatives(variant, theme) {
            Some(value) => sequences(variant, value, &range, theme)?,
            None => vec![format_args!("{variant}:").to_lean_string()],
        };
        if prefixes.len() * sequences.len() > MAX_SEQUENCES {
            return Err(failure(
                variant,
                &format!("expands to more than {MAX_SEQUENCES} variant sequences"),
                range,
            ));
        }
        prefixes = prefixes
            .iter()
            .flat_map(|prefix| {
                sequences
                    .iter()
                    .map(move |sequence| format_args!("{prefix}{sequence}").to_lean_string())
            })
            .collect();
        start = range.end + 1;
    }
    Ok(prefixes)
}

/// Splits the captured value of an alternatives variant, such as `a,b` in `or(a,b)`,
/// into its expanded sequences, each ending with `:`. A sequence may contain further
/// alternatives.
fn sequences(
    variant: &str,
    value: &str,
    range: &Range<usize>,
    theme: &ThemeIndex,
) -> Result<Vec<LeanString>, UtilityFailure> {
    let malformed = || {
        failure(
            variant,
            "must list nonempty variant sequences separated by `,`",
            range.clone(),
        )
    };
    let mut expanded = Vec::new();
    for sequence in split_top_level(value).ok_or_else(malformed)? {
        let sequence = if sequence.ends_with(':') {
            sequence.to_lean_string()
        } else {
            format_args!("{sequence}:").to_lean_string()
        };
        if sequence == ":" || parse_variant_prefix(&sequence).is_none() {
            return Err(malformed());
        }
        let nested = expand_prefix(&sequence, theme).map_err(|nested| UtilityFailure {
            range: range.clone(),
            reason: nested.reason,
        })?;
        expanded.extend(nested);
    }
    Ok(expanded)
}

/// Splits `value` at commas outside brackets, parentheses, and quotes. Returns `None`
/// for unbalanced delimiters.
fn split_top_level(value: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut closers = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut start = 0;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, character) {
            (_, '\\') => escaped = true,
            (Some(open), _) if character == open => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, '[') => closers.push(']'),
            (None, '(') => closers.push(')'),
            (None, ']' | ')') => {
                if closers.pop() != Some(character) {
                    return None;
                }
            }
            (None, ',') if closers.is_empty() => {
                parts.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if !closers.is_empty() || quote.is_some() || escaped {
        return None;
    }
    parts.push(&value[start..]);
    Some(parts)
}

fn failure(variant: &str, problem: &str, range: Range<usize>) -> UtilityFailure {
    UtilityFailure {
        range,
        reason: UtilityFailureReason::Variant(VariantFailureReason::InvalidAlternatives {
            variant: variant.to_lean_str(),
            problem: problem.to_lean_str(),
        }),
    }
}

/// Expands the alternatives in `wrappers` into wrapper lists without alternatives.
///
/// [`StyleVariant::Alternatives`] contributes each of its sequences, and an alternatives
/// variant in a prefix or in the prefix of a ref condition contributes each of its
/// sequences. The lists keep the order of the wrappers, and the combinations keep the
/// order of the alternatives as written. A list without alternatives is returned
/// unchanged as the only list.
///
/// Returns `None` after adding a lint finding when an alternative is malformed or when
/// the wrappers expand to more than [`MAX_SEQUENCES`] lists.
pub(in crate::compiler) fn expand_wrappers(
    wrappers: &[StyleVariant],
    theme: &ThemeIndex,
    lint_findings: &mut Vec<LintFinding>,
) -> Option<Vec<Vec<StyleVariant>>> {
    let mut lists = vec![Vec::new()];
    for wrapper in wrappers {
        let alternatives = wrapper_alternatives(wrapper, theme, lint_findings)?;
        if lists.len() * alternatives.len() > MAX_SEQUENCES {
            let span = wrapper_span(wrapper)?;
            lint_findings.push(unresolved_utility_finding(
                "",
                0,
                &failure(
                    "or",
                    &format!("expands to more than {MAX_SEQUENCES} variant sequences"),
                    0..0,
                ),
                span,
                theme,
            ));
            return None;
        }
        lists = lists
            .iter()
            .flat_map(|list| {
                alternatives.iter().map(move |alternative| {
                    let mut list = list.clone();
                    list.extend(alternative.iter().cloned());
                    list
                })
            })
            .collect();
    }
    Some(lists)
}

/// Returns the wrapper lists that one wrapper expands to.
fn wrapper_alternatives(
    wrapper: &StyleVariant,
    theme: &ThemeIndex,
    lint_findings: &mut Vec<LintFinding>,
) -> Option<Vec<Vec<StyleVariant>>> {
    match wrapper {
        StyleVariant::Prefix(prefix) => Some(
            expand_spanned_prefix(prefix, theme, lint_findings)?
                .into_iter()
                .map(|prefix| vec![StyleVariant::Prefix(prefix)])
                .collect(),
        ),
        StyleVariant::Container { .. } => Some(vec![vec![wrapper.clone()]]),
        StyleVariant::Ref { condition, scopes } => {
            let mut conditions = vec![Vec::new()];
            for condition in std::iter::once(condition).chain(scopes) {
                let prefixes = expand_spanned_prefix(&condition.prefix, theme, lint_findings)?;
                conditions = conditions
                    .iter()
                    .flat_map(|list: &Vec<RefCondition>| {
                        prefixes.iter().map(move |prefix| {
                            let mut list = list.clone();
                            list.push(RefCondition {
                                refs: condition.refs.clone(),
                                prefix: prefix.clone(),
                            });
                            list
                        })
                    })
                    .collect();
            }
            Some(
                conditions
                    .into_iter()
                    .map(|mut list| {
                        let condition = list.remove(0);
                        vec![StyleVariant::Ref {
                            condition,
                            scopes: list,
                        }]
                    })
                    .collect(),
            )
        }
        StyleVariant::Alternatives(sequences) => {
            let mut lists = Vec::new();
            for sequence in sequences {
                lists.extend(expand_wrappers(sequence, theme, lint_findings)?);
            }
            Some(lists)
        }
    }
}

/// Expands the alternatives of one prefix wrapper. Every expanded prefix keeps the
/// span of the prefix as written.
///
/// Returns [`None`] after it reports a malformed alternatives variant or an expansion
/// that is too large to `lint_findings`.
pub(in crate::compiler) fn expand_spanned_prefix(
    prefix: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    lint_findings: &mut Vec<LintFinding>,
) -> Option<Vec<Spanned<LeanStr>>> {
    match expand_prefix(&prefix.value, theme) {
        Ok(prefixes) => Some(
            prefixes
                .into_iter()
                .map(|value| Spanned::new(value.to_lean_str(), prefix.span))
                .collect(),
        ),
        Err(failure) => {
            lint_findings.push(unresolved_utility_finding(
                &prefix.value,
                prefix.value.len(),
                &failure,
                prefix.span,
                theme,
            ));
            None
        }
    }
}

/// The source span of a wrapper, used to report an expansion that is too large.
fn wrapper_span(wrapper: &StyleVariant) -> Option<stillwind_source::Span> {
    match wrapper {
        StyleVariant::Prefix(prefix) => Some(prefix.span),
        StyleVariant::Container { query, .. } => Some(query.span),
        StyleVariant::Ref { condition, .. } => Some(condition.prefix.span),
        StyleVariant::Alternatives(sequences) => sequences.iter().flatten().find_map(wrapper_span),
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::*;

    fn snapshot() -> DesignSystemSnapshot {
        let source = css_source(
            FileId(1),
            r"@import 'stillwind';
              @stillwind-variant any-* (value: [variant-list]) { @stillwind-intrinsic builtin-or-variant; }",
        )
        .unwrap();
        DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap()
    }

    fn expanded(raw: &str, theme: &ThemeIndex) -> Vec<String> {
        expand_candidate(raw, theme)
            .unwrap()
            .into_iter()
            .map(|candidate| candidate.to_string())
            .collect()
    }

    #[test]
    fn candidates_expand_into_each_combination_in_written_order() {
        let snapshot = snapshot();
        let theme = snapshot.theme();
        assert_eq!(expanded("p-4", theme), ["p-4"]);
        assert_eq!(
            expanded("or(hover,focus):p-4", theme),
            ["hover:p-4", "focus:p-4"]
        );
        assert_eq!(
            expanded("md:or(hover,dark:focus:):!p-4", theme),
            ["md:hover:!p-4", "md:dark:focus:!p-4"]
        );
        assert_eq!(
            expanded("or(a,b):any-c,d:p-4", theme),
            ["a:c:p-4", "a:d:p-4", "b:c:p-4", "b:d:p-4"]
        );
        assert_eq!(
            expanded("or(has-[a,b],or(c,[&:is(d,e)])):p-4", theme),
            ["has-[a,b]:p-4", "c:p-4", "[&:is(d,e)]:p-4"]
        );
    }

    #[test]
    fn malformed_alternatives_fail_at_their_variant() {
        let snapshot = snapshot();
        let theme = snapshot.theme();
        for raw in [
            "md:or(hover,):p-4",
            "md:or(hover,:):p-4",
            "md:or(a,b):or(a,b):or(a,b):or(a,b):or(a,b):or(a,b):p-4",
        ] {
            let failure = expand_candidate(raw, theme).unwrap_err();
            assert!(
                matches!(
                    failure.reason,
                    UtilityFailureReason::Variant(VariantFailureReason::InvalidAlternatives { .. })
                ),
                "{raw}"
            );
            assert!(raw[failure.range.clone()].starts_with("or("), "{raw}");
        }
    }
}
