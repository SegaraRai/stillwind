//! Explanations of utility candidates that resolution rejects.
//!
//! [`diagnose_utility`] runs only after resolution failed. It resolves variations of the
//! candidate, such as the candidate without its modifier, so each explanation agrees
//! with what resolution accepts. [`check_candidate_variants`] locates the first variant
//! of a candidate that does not resolve.

use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_syntax::{Candidate, Utility, Value, parse_candidate};

use super::super::suggestions::closest_spelling;
use super::super::values::{decode_arbitrary, named_domain_values};
use super::super::variants::{VariantFailureReason, resolve_variant};
use super::matching::resolve_named_utility;
use crate::{CustomUtility, ThemeIndex};

/// Why a utility candidate does not resolve, and which part of its text causes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityFailure {
    /// Byte range of the responsible text within the candidate, such as the value
    /// `red-999` of `bg-red-999` or the variant `foo` of `foo:flex`.
    pub range: Range<usize>,
    /// The cause, with the text that its message names.
    pub reason: UtilityFailureReason,
}

impl UtilityFailure {
    /// Returns `candidate` with the failing text of an unknown name replaced by a
    /// spelling that resolves, when the two differ by a likely typo.
    ///
    /// `candidate` must be the text whose resolution returned this failure. The search
    /// resolves several spellings, so callers request it only for a failure that they
    /// report.
    #[must_use]
    pub fn suggested_candidate(&self, candidate: &str, theme: &ThemeIndex) -> Option<LeanString> {
        let replacement = match &self.reason {
            UtilityFailureReason::Variant(reason) => reason.suggestion(theme)?,
            UtilityFailureReason::UnknownName { utility } => {
                let modifier = parse_candidate(candidate).and_then(|parsed| match parsed.utility {
                    Utility::Named { modifier, .. } => modifier,
                    Utility::ArbitraryProperty { .. } => None,
                });
                closest_spelling(utility, utility_names(utility, theme), |name| {
                    resolves(name, modifier.as_ref(), theme)
                })?
            }
            UtilityFailureReason::UnknownValue { value, .. } => {
                let parsed = parse_candidate(candidate)?;
                let Utility::Named { modifier, .. } = &parsed.utility else {
                    return None;
                };
                let layout = CandidateLayout::new(candidate, &parsed);
                let start = layout
                    .negative
                    .map_or(layout.name.start, |range| range.start);
                let root = candidate.get(start..self.range.start)?;
                let names =
                    matching_definitions(&format_args!("{root}{value}").to_lean_string(), theme)
                        .into_iter()
                        .filter(|definition| definition.name.strip_suffix('*') == Some(root))
                        .flat_map(|definition| named_domain_values(&definition.body, theme))
                        .collect::<Vec<_>>();
                closest_spelling(value, names, |value| {
                    resolves(
                        &format_args!("{root}{value}").to_lean_string(),
                        modifier.as_ref(),
                        theme,
                    )
                })?
            }
            _ => return None,
        };
        let prefix = candidate.get(..self.range.start)?;
        let suffix = candidate.get(self.range.end..)?;
        Some(format_args!("{prefix}{replacement}{suffix}").to_lean_string())
    }
}

/// The cause of a [`UtilityFailure`].
///
/// [`Self::UnknownName`], [`Self::UnknownValue`], and an unknown variant mean that a
/// name does not exist. The other reasons mean that the candidate is malformed or that
/// an existing utility or variant rejects part of it.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UtilityFailureReason {
    /// The text is not candidate syntax, for example because of an unclosed bracket or
    /// quote.
    #[error("`{candidate}` is not valid utility syntax")]
    Malformed {
        /// Candidate text as written.
        candidate: LeanStr,
    },
    /// No utility definition matches the name.
    #[error("unknown utility `{utility}`")]
    UnknownName {
        /// Utility name as written, including a leading `-`.
        utility: LeanStr,
    },
    /// A wildcard definition matches, but no theme token or keyword has the value name.
    #[error("`{utility}` has no value `{value}`")]
    UnknownValue {
        /// Utility root without its trailing `-`, such as `bg`.
        utility: LeanStr,
        /// Value name as written.
        value: LeanStr,
    },
    /// The utility exists, but no definition accepts the value form.
    #[error("`{utility}` does not accept the value `{value}`")]
    UnsupportedValue {
        /// Utility root without its trailing `-`, or the property of an arbitrary
        /// property candidate.
        utility: LeanStr,
        /// Value text, decoded when it was written in brackets.
        value: LeanStr,
    },
    /// The candidate resolves without its modifier.
    #[error("`{utility}` does not accept the modifier `{modifier}`")]
    UnsupportedModifier {
        /// Utility name as written, without the modifier.
        utility: LeanStr,
        /// Modifier text after `/`.
        modifier: LeanStr,
    },
    /// The utility exists, but it has no negative form.
    #[error("`{utility}` does not accept a negative value")]
    UnsupportedNegative {
        /// Utility name without the leading `-`.
        utility: LeanStr,
    },
    /// A variant of the candidate does not resolve.
    #[error(transparent)]
    Variant(VariantFailureReason),
}

impl UtilityFailureReason {
    /// Whether a utility, theme token, or variant name does not exist, as opposed to a
    /// malformed candidate or a known name that rejects part of the candidate.
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        match self {
            Self::UnknownName { .. } | Self::UnknownValue { .. } => true,
            Self::Variant(reason) => reason.is_unknown(),
            _ => false,
        }
    }

    /// Whether a variant of the candidate causes the failure.
    #[must_use]
    pub const fn is_variant(&self) -> bool {
        matches!(self, Self::Variant(_))
    }
}

/// Byte ranges of the parts of a parsed candidate.
struct CandidateLayout {
    /// Start of each variant, in candidate order.
    variants: Vec<usize>,
    /// The leading `-` of a negative utility.
    negative: Option<Range<usize>>,
    /// The utility name or arbitrary property, without `!`, `-`, or a modifier.
    name: Range<usize>,
    /// The `/` and modifier text.
    modifier: Option<Range<usize>>,
}

impl CandidateLayout {
    fn new(raw: &str, candidate: &Candidate) -> Self {
        let mut offset = 0;
        let variants = candidate
            .variants
            .iter()
            .map(|variant| {
                let start = offset;
                offset += variant.raw.len() + 1;
                start
            })
            .collect();
        let mut end = raw.len();
        if raw[offset..].starts_with('!') {
            offset += 1;
        } else if candidate.important {
            end -= 1;
        }
        let negative = candidate.negative.then(|| {
            offset += 1;
            offset - 1..offset
        });
        let name_end = match &candidate.utility {
            Utility::Named {
                name,
                modifier: Some(_),
            } => offset + name.len(),
            _ => end,
        };
        Self {
            variants,
            negative,
            name: offset..name_end,
            modifier: (name_end < end).then_some(name_end..end),
        }
    }
}

/// Returns the byte range of the arbitrary value of `candidate`: the last bracketed or
/// parenthesized value in a utility name, such as `[1/2]` in `md:w-[1/2]` or `(--x)`
/// in `bg-(--x)`, or the text after the colon of an arbitrary property, such as `1/2`
/// in `[width:1/2]`. Returns the range of the whole utility when it has no such value.
/// `raw` is the text that `candidate` was parsed from.
pub(super) fn arbitrary_value_range(raw: &str, candidate: &Candidate) -> Range<usize> {
    let name = CandidateLayout::new(raw, candidate).name;
    let text = &raw[name.clone()];
    let start = match candidate.utility {
        Utility::ArbitraryProperty { .. } => text.find(':').map(|colon| (colon + 1, 1)),
        Utility::Named { .. } if text.ends_with(')') => text.rfind("-(").map(|dash| (dash + 1, 0)),
        Utility::Named { .. } => text.rfind("-[").map(|dash| (dash + 1, 0)),
    };
    start.map_or(name.clone(), |(start, trailing)| {
        name.start + start..name.end - trailing
    })
}

/// Returns the byte range of the value of the utility in `raw`: the value that
/// [`arbitrary_value_range`] finds, or the text after the last `-` of a named utility
/// without a bracketed or parenthesized value, such as `150` in `opacity-150`. Returns
/// `None` when `raw` is not candidate syntax or a named utility has no `-`.
#[must_use]
pub fn utility_value_range(raw: &str) -> Option<Range<usize>> {
    let candidate = parse_candidate(raw)?;
    let name = CandidateLayout::new(raw, &candidate).name;
    let text = &raw[name.clone()];
    match candidate.utility {
        Utility::Named { .. } if !text.ends_with([']', ')']) => {
            let dash = text.rfind('-')?;
            Some(name.start + dash + 1..name.end)
        }
        _ => Some(arbitrary_value_range(raw, &candidate)),
    }
}

/// Returns the byte range of the modifier of `candidate` after its `/`, or `None` when
/// it has no modifier. `raw` is the text that `candidate` was parsed from.
pub(super) fn modifier_value_range(raw: &str, candidate: &Candidate) -> Option<Range<usize>> {
    let modifier = CandidateLayout::new(raw, candidate).modifier?;
    Some(modifier.start + 1..modifier.end)
}

/// Resolves every variant of `candidate` in order.
///
/// # Errors
///
/// Returns the first variant that does not resolve, with its range located in
/// `candidate.raw`.
pub fn check_candidate_variants(
    candidate: &Candidate,
    theme: &ThemeIndex,
) -> Result<(), UtilityFailure> {
    let layout = CandidateLayout::new(&candidate.raw, candidate);
    for (variant, start) in candidate.variants.iter().zip(layout.variants) {
        if let Err(failure) = resolve_variant(variant, theme) {
            let failure = failure.shifted(start);
            return Err(UtilityFailure {
                range: failure.range,
                reason: UtilityFailureReason::Variant(failure.reason),
            });
        }
    }
    Ok(())
}

/// Explains why the utility part of `raw` does not resolve. `candidate` is the parse of
/// `raw`, or `None` when `raw` is not candidate syntax. Variants are not checked.
///
/// The caller must have observed that resolution of `raw` failed; otherwise the
/// returned reason does not describe a real failure. A variation of the candidate whose
/// definition is malformed counts as not resolving.
pub(super) fn diagnose_utility(
    raw: &str,
    candidate: Option<&Candidate>,
    theme: &ThemeIndex,
) -> UtilityFailure {
    let Some(candidate) = candidate else {
        return UtilityFailure {
            range: 0..raw.len(),
            reason: UtilityFailureReason::Malformed {
                candidate: raw.to_lean_str(),
            },
        };
    };
    let layout = CandidateLayout::new(raw, candidate);
    match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => UtilityFailure {
            range: layout.name,
            reason: UtilityFailureReason::UnsupportedValue {
                utility: property.clone(),
                value: value.clone(),
            },
        },
        Utility::Named { name, .. } => {
            diagnose_named_utility(raw, &layout, name, candidate.negative, theme)
        }
    }
}

/// Explains why the named utility `name` of `raw` does not resolve. `negative` tells
/// whether the candidate has a leading `-`, and `layout` locates its parts.
fn diagnose_named_utility(
    raw: &str,
    layout: &CandidateLayout,
    name: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> UtilityFailure {
    let lookup = if negative {
        format_args!("-{name}").to_lean_string()
    } else {
        name.to_lean_string()
    };
    let lookup_range = layout
        .negative
        .as_ref()
        .map_or(layout.name.start, |range| range.start)..layout.name.end;
    let definitions = matching_definitions(&lookup, theme);
    if definitions.is_empty() {
        if let Some(range) = layout.negative.clone()
            && !matching_definitions(name, theme).is_empty()
        {
            return UtilityFailure {
                range,
                reason: UtilityFailureReason::UnsupportedNegative {
                    utility: name.to_lean_str(),
                },
            };
        }
        return UtilityFailure {
            range: lookup_range,
            reason: UtilityFailureReason::UnknownName {
                utility: lookup.to_lean_str(),
            },
        };
    }
    if let Some(range) = layout.modifier.clone()
        && resolves(&lookup, None, theme)
    {
        return UtilityFailure {
            range: range.start + 1..range.end,
            reason: UtilityFailureReason::UnsupportedModifier {
                utility: lookup.to_lean_str(),
                modifier: raw[range.start + 1..range.end].to_lean_str(),
            },
        };
    }
    let Some(root) = definitions
        .iter()
        .filter_map(|definition| definition.name.strip_suffix('*'))
        .filter(|prefix| lookup.starts_with(prefix))
        .max_by_key(|prefix| prefix.len())
    else {
        return UtilityFailure {
            range: lookup_range,
            reason: UtilityFailureReason::UnknownName {
                utility: lookup.to_lean_str(),
            },
        };
    };
    let value = &lookup[root.len()..];
    let range = lookup_range.start + root.len()..lookup_range.end;
    let utility = root.strip_suffix('-').unwrap_or(root).to_lean_str();
    if let Some(arbitrary) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return UtilityFailure {
            range,
            reason: UtilityFailureReason::UnsupportedValue {
                utility,
                value: decode_arbitrary(arbitrary).into(),
            },
        };
    }
    if value.starts_with('(') || value.is_empty() {
        return UtilityFailure {
            range,
            reason: UtilityFailureReason::UnsupportedValue {
                utility,
                value: value.to_lean_str(),
            },
        };
    }
    UtilityFailure {
        range,
        reason: UtilityFailureReason::UnknownValue {
            utility,
            value: value.to_lean_str(),
        },
    }
}

/// Returns the project and built-in definitions whose names match `lookup`.
fn matching_definitions<'a>(lookup: &str, theme: &'a ThemeIndex) -> Vec<&'a CustomUtility> {
    let mut definitions = theme.matching_utilities(lookup);
    definitions.extend(theme.registry().matching_utilities(lookup));
    definitions
}

/// Whether the utility name `lookup`, including a leading `-` for a negative utility,
/// resolves with `modifier`. A malformed definition counts as not resolving.
fn resolves(lookup: &str, modifier: Option<&Value>, theme: &ThemeIndex) -> bool {
    let (negative, name) = lookup
        .strip_prefix('-')
        .map_or((false, lookup), |name| (true, name));
    matches!(
        resolve_named_utility(name, modifier, negative, theme),
        Ok(Some(_))
    )
}

/// Returns spellings to compare with the unknown utility name `lookup`: every exact
/// utility name, and every wildcard prefix followed by the text after the first `-` of
/// `lookup`, so `bgg-red-500` can become `bg-red-500`.
fn utility_names(lookup: &str, theme: &ThemeIndex) -> Vec<LeanString> {
    let tail = lookup
        .trim_start_matches('-')
        .split_once('-')
        .map(|(_, tail)| tail);
    let mut names = Vec::new();
    for utility in theme.utilities().iter().chain(theme.registry().utilities()) {
        match utility.name.strip_suffix('*') {
            Some(prefix) => {
                if let Some(tail) = tail {
                    names.push(format_args!("{prefix}{tail}").to_lean_string());
                }
            }
            None => names.push(utility.name.to_lean_string()),
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::evaluation::utilities::{InvalidValueReason, resolve_utility};
    use crate::test_support::parse_fixture_theme;

    fn theme() -> ThemeIndex {
        parse_fixture_theme(
            &SourceFile::new(
                FileId(1),
                None,
                None,
                "@theme { --color-red-900: #7f1d1d; --color-red-950: #450a0a; }",
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Each failure names its cause and covers the candidate text responsible for it.
    #[test]
    fn failures_name_the_cause_and_the_responsible_text() {
        let theme = theme();
        for (raw, unknown, text, message) in [
            (
                "unknown-x",
                true,
                "unknown-x",
                "unknown utility `unknown-x`",
            ),
            ("bg-red-999", true, "red-999", "`bg` has no value `red-999`"),
            (
                "p-[calc(1rem+)]",
                false,
                "[calc(1rem+)]",
                "`p` does not accept the value `calc(1rem + )`",
            ),
            (
                "-flex",
                false,
                "-",
                "`flex` does not accept a negative value",
            ),
            (
                "bg-[url(x.png)]/50",
                false,
                "50",
                "`bg-[url(x.png)]` does not accept the modifier `50`",
            ),
            (
                "data-[a=\"b]:flex",
                false,
                "data-[a=\"b]:flex",
                "`data-[a=\"b]:flex` is not valid utility syntax",
            ),
            (
                "hover:!-unknown-x",
                true,
                "-unknown-x",
                "unknown utility `-unknown-x`",
            ),
        ] {
            let failure = resolve_utility(raw, &theme).unwrap().unwrap_err();
            assert_eq!(failure.reason.is_unknown(), unknown, "{raw}");
            assert_eq!(&raw[failure.range.clone()], text, "{raw}");
            assert_eq!(failure.reason.to_lean_string(), message, "{raw}");
        }
        let candidate = parse_candidate("hover:foo:flex").unwrap();
        let failure = check_candidate_variants(&candidate, &theme).unwrap_err();
        assert_eq!(failure.range, 6..9);
        assert!(failure.reason.is_unknown() && failure.reason.is_variant());
    }

    #[test]
    fn unknown_names_suggest_corrected_candidates() {
        let theme = theme();
        for (raw, suggestion) in [
            ("bg-red-999", Some("bg-red-900")),
            ("hover:bg-red-999/50", Some("hover:bg-red-900/50")),
            ("flx", Some("flex")),
            ("bgg-red-900", Some("bg-red-900")),
            ("xyzzy-plugh", None),
        ] {
            let failure = resolve_utility(raw, &theme).unwrap().unwrap_err();
            assert_eq!(
                failure.suggested_candidate(raw, &theme).as_deref(),
                suggestion,
                "{raw}"
            );
        }
    }

    /// An arbitrary value that the grammar of its property rejects is emitted as
    /// written, as Tailwind emits it, and recorded with its property, its range, and
    /// the type hints of a utility that chose the property as a fallback between value
    /// types.
    #[test]
    fn values_that_no_grammar_accepts_are_emitted_and_recorded() {
        let theme = theme();
        for (raw, declaration, property, value, text, hints) in [
            ("w-[1/2]", "width:1/2;", "width", "1/2", "[1/2]", &[][..]),
            ("md:w-[red]", "width:red;", "width", "red", "[red]", &[]),
            (
                "text-[12px_foo]",
                "color:12px foo;",
                "color",
                "12px foo",
                "[12px_foo]",
                &["color", "length"],
            ),
            ("[width:1/2]", "width:1/2;", "width", "1/2", "1/2", &[]),
            (
                "hover:[color:12px_foo]",
                "color:12px foo;",
                "color",
                "12px foo",
                "12px_foo",
                &[],
            ),
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(resolved.declaration, declaration, "{raw}");
            let invalid = resolved.invalid_value.unwrap();
            assert_eq!(
                (invalid.property.as_str(), invalid.value.as_str()),
                (property, value),
                "{raw}"
            );
            assert_eq!(&raw[invalid.range], text, "{raw}");
            assert_eq!(
                invalid.reason,
                InvalidValueReason::Grammar {
                    fallback_type_hints: hints.iter().map(|&hint| hint.into()).collect(),
                },
                "{raw}"
            );
        }
        for root in [
            "bg",
            "border",
            "stroke",
            "shadow",
            "decoration",
            "outline",
            "ring",
            "text",
        ] {
            let raw = format_args!("{root}-[12px_foo]").to_lean_string();
            let resolved = resolve_utility(&raw, &theme).unwrap().unwrap();
            let reason = resolved.invalid_value.unwrap().reason;
            assert!(
                matches!(&reason, InvalidValueReason::Grammar { fallback_type_hints } if fallback_type_hints.len() > 1),
                "{raw}: {reason:?}"
            );
        }
        for raw in [
            "w-[2px]",
            "w-(--size)",
            "w-1/2",
            "[width:2px]",
            "[--x:1/2]",
            "[width:var(--x)]",
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(resolved.invalid_value, None, "{raw}");
        }
    }
}
