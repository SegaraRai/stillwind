//! Explanations of utility candidates that resolution rejects.
//!
//! [`diagnose_utility`] runs only after resolution failed. It resolves variations of
//! the candidate, such as the candidate without its modifier, so each explanation
//! agrees with what resolution accepts. [`check_candidate_variants`] locates the first
//! variant of a candidate that does not resolve, and in document context checks that a
//! variant selects the element to style.

use std::fmt;
use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_diagnostics::MarkerUseErrorKind;
use stillwind_syntax::{Candidate, Utility, Value, decode_arbitrary_value, parse_candidate};

use super::super::suggestions::closest_spelling;
use super::super::values::named_domain_values;
use super::super::variants::{
    DocumentTargetFailure, MarkerUse, VariantContext, VariantFailureReason, document_target,
    marker_use, resolve_variant,
};
use super::matching::resolve_named_utility;
use super::ranges::CandidateLayout;
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
    /// No utility definition matches the name, which is a marker class that the
    /// candidate cannot add in its form or position.
    #[error("`{class}` is a marker class and cannot be added {reason}")]
    Marker {
        /// Marker class that the candidate names, without an unsupported modifier.
        class: LeanStr,
        /// Form or position of the candidate that prevents the class from being added.
        /// [`MarkerUseErrorKind::Placement`] means that the form is valid, so a caller
        /// that adds the class to an element can drop the failure.
        reason: MarkerUseErrorKind,
    },
    /// A variant of the candidate does not resolve.
    #[error(transparent)]
    Variant(VariantFailureReason),
    /// In document context, no variant selects the element that the candidate styles,
    /// or a variant refers to `&` before an earlier variant selects an element.
    #[error("{}", MissingTargetMessage(.variant.as_deref()))]
    MissingDocumentTarget {
        /// Text of the variant that refers to `&` too early, or `None` when no variant
        /// selects an element.
        variant: Option<LeanStr>,
    },
}

/// Message of [`UtilityFailureReason::MissingDocumentTarget`].
struct MissingTargetMessage<'a>(Option<&'a str>);

impl fmt::Display for MissingTargetMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(variant) => write!(
                formatter,
                "`{variant}` refers to the styled element, but no earlier variant selects one"
            ),
            None => formatter.write_str(
                "no variant selects the element to style; start with a variant such as `host:` or `[:root]:`",
            ),
        }
    }
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

/// Resolves every variant of `candidate` in order and, in [`VariantContext::Document`],
/// checks that a variant selects the element to style, as [`document_target`]
/// describes.
///
/// # Errors
///
/// Returns the first variant that does not resolve, with its range located in
/// `candidate.raw`. In document context, returns
/// [`UtilityFailureReason::MissingDocumentTarget`] at the variant that refers to `&`
/// too early, or at the whole candidate when no variant selects an element.
pub fn check_candidate_variants(
    candidate: &Candidate,
    theme: &ThemeIndex,
    context: VariantContext,
) -> Result<(), UtilityFailure> {
    let layout = CandidateLayout::new(&candidate.raw, candidate);
    let mut applications = Vec::new();
    for (variant, &start) in candidate.variants.iter().zip(&layout.variants) {
        match resolve_variant(variant, theme) {
            Ok(application) => {
                applications.push(application);
            }
            Err(failure) => {
                let failure = failure.shifted(start);
                return Err(UtilityFailure {
                    range: failure.range,
                    reason: UtilityFailureReason::Variant(failure.reason),
                });
            }
        }
    }
    if context == VariantContext::Element {
        return Ok(());
    }
    match document_target(&applications) {
        Ok(_) => Ok(()),
        Err(DocumentTargetFailure::NoTarget) => Err(UtilityFailure {
            range: 0..candidate.raw.len(),
            reason: UtilityFailureReason::MissingDocumentTarget { variant: None },
        }),
        Err(DocumentTargetFailure::BeforeTarget(index)) => {
            let variant = &candidate.variants[index];
            let start = layout.variants[index];
            Err(UtilityFailure {
                range: start..start + variant.raw.len(),
                reason: UtilityFailureReason::MissingDocumentTarget {
                    variant: Some(variant.raw.clone()),
                },
            })
        }
    }
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
        Utility::ArbitraryProperty {
            property, value, ..
        } => UtilityFailure {
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

/// Returns the failure for a candidate `raw` that names a marker class and no utility,
/// or `None` when it names no marker class. The range covers the whole candidate,
/// because its variants, `!`, `-`, or modifier can each prevent the class.
fn marker_failure(raw: &str, theme: &ThemeIndex) -> Option<UtilityFailure> {
    let (class, reason) = match marker_use(raw, theme)? {
        MarkerUse::Class(class) => (class, MarkerUseErrorKind::Placement),
        MarkerUse::Invalid { class, reason } => (class, reason),
    };
    Some(UtilityFailure {
        range: 0..raw.len(),
        reason: UtilityFailureReason::Marker {
            class: class.as_str().into(),
            reason,
        },
    })
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
    if definitions.is_empty()
        && let Some(failure) = marker_failure(raw, theme)
    {
        return failure;
    }
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
                value: decode_arbitrary_value(arbitrary).into(),
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

/// Returns the Tailwind-style and Stillwind definitions whose names match `lookup`.
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
            None => {
                names.push(utility.name.to_lean_string());
            }
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
        let failure =
            check_candidate_variants(&candidate, &theme, VariantContext::Element).unwrap_err();
        assert_eq!(failure.range, 6..9);
        assert!(failure.reason.is_unknown() && failure.reason.is_variant());
    }

    #[test]
    fn document_context_requires_a_variant_that_selects_the_element() {
        let theme = theme();
        let check = |raw: &str, context| {
            check_candidate_variants(&parse_candidate(raw).unwrap(), &theme, context).map_err(
                |failure| {
                    (
                        raw[failure.range.clone()].to_lean_string(),
                        failure.reason.to_lean_string(),
                    )
                },
            )
        };
        for raw in [
            "size-full",
            "hover:flex",
            "not-hover:flex",
            "host:flex",
            "host:hover:flex",
        ] {
            assert_eq!(check(raw, VariantContext::Element), Ok(()), "{raw}");
        }
        for raw in [
            "host:flex",
            "print:host:hover:flex",
            "[:hover]:flex",
            "[:root]:dark:flex",
            "[:host(.compact)]:not-hover:flex",
        ] {
            assert_eq!(check(raw, VariantContext::Document), Ok(()), "{raw}");
        }
        let missing = "no variant selects the element to style; start with a variant such as `host:` or `[:root]:`";
        for raw in ["size-full", "print:flex", "supports-grid:dark:flex"] {
            assert_eq!(
                check(raw, VariantContext::Document),
                Err((raw.to_lean_string(), missing.to_lean_string())),
                "{raw}"
            );
        }
        for (raw, variant) in [
            ("hover:flex", "hover"),
            ("print:hover:host:flex", "hover"),
            ("not-hover:host:flex", "not-hover"),
            ("[&:hover]:flex", "[&:hover]"),
        ] {
            assert_eq!(
                check(raw, VariantContext::Document),
                Err((
                    variant.to_lean_string(),
                    format_args!(
                        "`{variant}` refers to the styled element, but no earlier variant selects one"
                    )
                    .to_lean_string()
                )),
                "{raw}"
            );
        }
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
            let invalid = resolved.diagnostic_data.invalid_value.unwrap();
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
            let reason = resolved.diagnostic_data.invalid_value.unwrap().reason;
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
            assert_eq!(resolved.diagnostic_data.invalid_value, None, "{raw}");
        }
    }
}
