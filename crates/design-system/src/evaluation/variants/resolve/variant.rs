//! Resolution of one variant and the failures that it reports.

use std::fmt;
use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_syntax::{Variant, VariantKind};

use super::super::super::builtins::builtin_variants;
use super::super::super::suggestions::closest_spelling;
use super::super::super::values::named_domain_values;
use super::super::application::{VariantApplication, VariantObstacle};
use super::super::order::variant_sort_namespace;
use super::super::substitution::registered_variant_candidates;
use super::arbitrary::{
    arbitrary_variant_application, decode_variant_template, is_relative_selector,
    parse_nested_variant,
};
use super::definition::{resolve_registered_variant_candidate, split_variant_modifier};
use crate::ThemeIndex;

/// Why a variant does not resolve, and which part of its text causes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantFailure {
    /// Byte range of the responsible text within [`Variant::raw`], such as the inner
    /// variant `sm` of `group-sm` or the modifier `/foo` of `sm/foo`.
    pub range: Range<usize>,
    /// The cause, with the text that its message names.
    pub reason: VariantFailureReason,
}

impl VariantFailure {
    pub(super) fn new(range: Range<usize>, reason: VariantFailureReason) -> Self {
        Self { range, reason }
    }

    /// Moves the range by `offset` bytes, for a failure of an inner variant whose text
    /// starts at `offset` in the outer variant.
    #[must_use]
    pub fn shifted(mut self, offset: usize) -> Self {
        self.range = self.range.start + offset..self.range.end + offset;
        self
    }
}

/// The cause of a [`VariantFailure`].
///
/// [`Self::Unknown`] and [`Self::UnknownValue`] mean that no definition or theme token
/// has the written name. The other reasons mean that a definition exists but rejects
/// the value, the modifier, or the combination with an inner variant.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VariantFailureReason {
    /// No definition has this name.
    #[error("unknown variant `{variant}`")]
    Unknown {
        /// Variant text as written.
        variant: LeanStr,
    },
    /// A wildcard definition matches, but no theme token or keyword has the captured
    /// name.
    #[error("`{variant}` has no value `{value}`")]
    UnknownValue {
        /// Definition prefix, such as `min-`.
        variant: LeanStr,
        /// Captured name as written.
        value: LeanStr,
    },
    /// The definition rejects the captured value.
    #[error("`{variant}` does not accept `{value}`")]
    InvalidValue {
        /// Definition prefix, such as `min-` or `nth-`.
        variant: LeanStr,
        /// Captured value, decoded when it was written in brackets.
        value: LeanStr,
    },
    /// The captured value is a `var()` reference in a condition that cannot contain
    /// one.
    #[error("`var()` cannot be used in {context}")]
    VariableInCondition {
        /// Construct that would contain the reference.
        context: ConditionContext,
    },
    /// The definition requires a `/modifier`.
    #[error("`{variant}` requires a modifier")]
    MissingModifier {
        /// Definition prefix, such as `group-`.
        variant: LeanStr,
    },
    /// The definition rejects the modifier text.
    #[error("`{variant}` does not accept the modifier `{modifier}`")]
    InvalidModifier {
        /// Definition prefix, such as `group-`.
        variant: LeanStr,
        /// Modifier text after `/`.
        modifier: LeanStr,
    },
    /// The variant resolves without its modifier, but no definition accepts one.
    #[error("`{variant}` does not accept a modifier")]
    UnexpectedModifier {
        /// Variant text before `/`.
        variant: LeanStr,
    },
    /// An exact definition or a definition without a usable form produces no selector
    /// or condition.
    #[error("the definition of `{variant}` does not produce a valid selector or condition")]
    InvalidDefinition {
        /// Definition name.
        variant: LeanStr,
    },
    /// Arbitrary variant text is not a valid selector list.
    #[error("`{selector}` is not a valid selector")]
    InvalidSelector {
        /// Decoded selector text.
        selector: LeanStr,
    },
    /// A selector that starts with a combinator appears where no `:has()` anchors it.
    #[error("the relative selector `{selector}` can only be used in a `has-` variant")]
    RelativeSelector {
        /// Decoded selector text, starting with `>`, `+`, or `~`.
        selector: LeanStr,
    },
    /// Arbitrary variant text that starts with `@` is not a conditional at-rule.
    #[error("`{rule}` is not a valid at-rule condition")]
    InvalidAtRule {
        /// Decoded at-rule prelude, including its `@` name.
        rule: LeanStr,
    },
    /// The capture of a runtime-condition variant is malformed.
    #[error("`{condition}` is not a valid runtime condition")]
    InvalidCondition {
        /// Captured condition text.
        condition: LeanStr,
    },
    /// A negating variant cannot express the absence of its inner variant.
    #[error("`{variant}` cannot negate `{inner}`, which {obstacle}")]
    CannotNegate {
        /// Definition prefix, such as `not-`.
        variant: LeanStr,
        /// Inner variant text as written.
        inner: LeanStr,
        /// Part of the inner variant that has no negated form.
        obstacle: VariantObstacle,
    },
    /// A relationship variant cannot place its inner variant in its selector.
    #[error("{}", NestingMessage { variant, inner, obstacle: *obstacle })]
    CannotNest {
        /// Definition prefix, such as `group-` or `has-`.
        variant: LeanStr,
        /// Inner variant text as written.
        inner: LeanStr,
        /// Part of the inner variant that the selector cannot contain.
        obstacle: VariantObstacle,
    },
}

impl VariantFailureReason {
    /// Whether the variant or theme token name does not exist, as opposed to a known
    /// variant that rejects its value, modifier, or inner variant.
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. } | Self::UnknownValue { .. })
    }

    /// Returns a spelling that replaces the failing text of an unknown name and
    /// resolves, when it differs from the written text by a likely typo.
    ///
    /// The search resolves several spellings, so callers request it only for a failure
    /// that they report.
    #[must_use]
    pub fn suggestion(&self, theme: &ThemeIndex) -> Option<LeanString> {
        match self {
            Self::Unknown { variant } => closest_spelling(variant, variant_names(theme), |name| {
                resolve_variant(&parse_nested_variant(name), theme).is_ok()
            }),
            Self::UnknownValue { variant, value } => {
                let written = format_args!("{variant}{value}").to_lean_string();
                let names = theme
                    .variants()
                    .iter()
                    .chain(builtin_variants(theme))
                    .filter(|custom| custom.capture(&written) == Some(value.as_str()))
                    .flat_map(|custom| named_domain_values(&custom.body, theme))
                    .collect::<Vec<_>>();
                closest_spelling(value, names, |name| {
                    let candidate = format_args!("{variant}{name}").to_lean_string();
                    resolve_variant(&parse_nested_variant(&candidate), theme).is_ok()
                })
            }
            _ => None,
        }
    }

    /// Ranks failures of different definitions that match one variant name, so that the
    /// definition that came closest to resolving explains the failure.
    ///
    /// A missing modifier only shows that the variant was not written for that
    /// definition. An unknown captured name of a broad wildcard, such as `min-x` under
    /// `@*`, is weaker than a failure of a definition that recognizes the value.
    const fn relevance(&self) -> u8 {
        match self {
            Self::MissingModifier { .. } => 0,
            Self::UnknownValue { .. } => 1,
            _ => 2,
        }
    }
}

/// The construct that a variant value would be inserted into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionContext {
    /// An `@media` query.
    Media,
    /// An `@container` query.
    Container,
    /// An `@supports` condition.
    Supports,
    /// A selector.
    Selector,
}

impl ConditionContext {
    /// Classifies a definition body by its outermost construct.
    pub(super) fn of_body(body: &str) -> Self {
        let body = body.trim_start();
        if body.starts_with("@media") {
            Self::Media
        } else if body.starts_with("@container") {
            Self::Container
        } else if body.starts_with("@supports") {
            Self::Supports
        } else {
            Self::Selector
        }
    }
}

impl fmt::Display for ConditionContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Media => "media queries",
            Self::Container => "container queries",
            Self::Supports => "`@supports` conditions",
            Self::Selector => "selectors",
        })
    }
}

/// Message of [`VariantFailureReason::CannotNest`], which names at-rule variants
/// directly.
struct NestingMessage<'a> {
    variant: &'a str,
    inner: &'a str,
    obstacle: VariantObstacle,
}

impl fmt::Display for NestingMessage<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            variant,
            inner,
            obstacle,
        } = self;
        if *obstacle == VariantObstacle::AtRule {
            write!(
                formatter,
                "`{variant}` cannot contain the at-rule variant `{inner}`"
            )
        } else {
            write!(
                formatter,
                "`{variant}` cannot contain `{inner}`, which {obstacle}"
            )
        }
    }
}

/// Resolves one variant against the theme.
///
/// Arbitrary selectors become selector templates as described by
/// [`arbitrary_variant_application`]; arbitrary at-rules use the same structural body
/// resolver as named definitions. A relative selector such as `[>img]` fails, because
/// only a `:has()` slot gives it an anchor element. A named variant is matched against
/// project definitions first and built-ins second, and the first definition that
/// resolves it is used. A CSS definition with `[if-condition]` and the
/// `builtin-if-variant` intrinsic produces a `Conditional` application; its public
/// spelling comes only from the registered pattern. A `[variant]` definition with the
/// `builtin-not-variant` intrinsic produces the negation of its inner variant.
///
/// # Errors
///
/// Returns a [`VariantFailure`] whose range lies within `variant.raw`. An arbitrary
/// variant fails as a whole. A named variant that no definition captures is
/// [`VariantFailureReason::Unknown`], unless it resolves without its `/modifier`. When
/// several definitions capture the name, the failure of the definition that came
/// closest to resolving is returned. A failure of an inner variant, such as `foo` in
/// `group-foo`, keeps its own reason and points at the inner text.
pub fn resolve_variant(
    variant: &Variant,
    theme: &ThemeIndex,
) -> Result<VariantApplication, VariantFailure> {
    match &variant.kind {
        VariantKind::Arbitrary(value) => {
            let value = decode_variant_template(value);
            let reason = if is_relative_selector(&value) {
                VariantFailureReason::RelativeSelector {
                    selector: value.trim().to_lean_str(),
                }
            } else {
                match arbitrary_variant_application(&value) {
                    Ok(application) => return Ok(application),
                    Err(reason) => reason,
                }
            };
            Err(VariantFailure::new(0..variant.raw.len(), reason))
        }
        VariantKind::Named(value) => resolve_named_variant(value, theme),
    }
}

/// Resolves a named variant through project definitions, built-ins, and the registry's
/// bare token form, and explains a failure as [`resolve_variant`] describes.
fn resolve_named_variant(
    value: &str,
    theme: &ThemeIndex,
) -> Result<VariantApplication, VariantFailure> {
    let mut failure: Option<VariantFailure> = None;
    let mut keep = |candidate: VariantFailure| {
        if failure
            .as_ref()
            .is_none_or(|kept| candidate.reason.relevance() > kept.reason.relevance())
        {
            failure = Some(candidate);
        }
    };
    for definitions in [theme.variants(), builtin_variants(theme)] {
        for (custom, capture) in registered_variant_candidates(definitions, value) {
            let offset = custom
                .wildcard_parts()
                .map_or(0, |(prefix, _)| prefix.len());
            match resolve_registered_variant_candidate(custom, capture, value, offset, theme) {
                Ok(application) => return Ok(application),
                Err(candidate) => keep(candidate),
            }
        }
    }
    for custom in builtin_variants(theme) {
        let Some(namespace) = custom
            .sort
            .as_ref()
            .filter(|sort| sort.bare)
            .and_then(|sort| sort.key.as_deref())
            .and_then(variant_sort_namespace)
        else {
            continue;
        };
        if theme.token_in(namespace, value).is_none() {
            continue;
        }
        let Some((prefix, suffix)) = custom.wildcard_parts() else {
            continue;
        };
        let concrete = format_args!("{prefix}{value}{suffix}").to_lean_string();
        match resolve_registered_variant_candidate(custom, Some(value), &concrete, 0, theme) {
            Ok(application) => return Ok(application),
            Err(candidate) => keep(candidate),
        }
    }
    if let Some(failure) = failure {
        return Err(failure);
    }
    if let (base, Some(_)) = split_variant_modifier(value)
        && resolve_named_variant(base, theme).is_ok()
    {
        return Err(VariantFailure::new(
            base.len()..value.len(),
            VariantFailureReason::UnexpectedModifier {
                variant: base.to_lean_str(),
            },
        ));
    }
    Err(VariantFailure::new(
        0..value.len(),
        VariantFailureReason::Unknown {
            variant: value.to_lean_str(),
        },
    ))
}

/// Returns the exact variant names and the bare theme tokens that name variants, as
/// spelling suggestions for an unknown variant.
fn variant_names(theme: &ThemeIndex) -> Vec<LeanString> {
    let mut names = Vec::new();
    for custom in theme.variants().iter().chain(builtin_variants(theme)) {
        if custom.wildcard.is_none() {
            names.push(custom.name.to_lean_string());
        }
        if let Some(namespace) = custom
            .sort
            .as_ref()
            .filter(|sort| sort.bare)
            .and_then(|sort| sort.key.as_deref())
            .and_then(variant_sort_namespace)
        {
            names.extend(theme.tokens_in(namespace).into_iter().filter_map(
                |token| match &token.kind {
                    crate::ThemeTokenKind::Token { name, .. } => Some(name.to_lean_string()),
                    _ => None,
                },
            ));
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceFile};
    use stillwind_syntax::parse_candidate;

    use super::*;
    use crate::test_support::parse_fixture_theme;

    /// Variant text, whether the name is unknown, the text the failure covers, and the
    /// message.
    const VARIANT_FAILURES: &[(&str, bool, &str, &str)] = &[
        ("foo", true, "foo", "unknown variant `foo`"),
        ("min-[foo]", false, "[foo]", "`min-` does not accept `foo`"),
        ("@[foo]", false, "[foo]", "`@` does not accept `foo`"),
        ("nth-[foo]", false, "[foo]", "`nth-` does not accept `foo`"),
        ("min-mdd", true, "mdd", "`min-` has no value `mdd`"),
        (
            "min-(--x)",
            false,
            "(--x)",
            "`var()` cannot be used in media queries",
        ),
        (
            "@min-(--x)",
            false,
            "(--x)",
            "`var()` cannot be used in container queries",
        ),
        (
            "supports-(--x)",
            false,
            "(--x)",
            "`var()` cannot be used in `@supports` conditions",
        ),
        (
            "not-before",
            false,
            "before",
            "`not-` cannot negate `before`, which selects a pseudo-element",
        ),
        (
            "not-starting",
            false,
            "starting",
            "`not-` cannot negate `starting`, which applies `@starting-style`",
        ),
        (
            "not-*",
            false,
            "*",
            "`not-` cannot negate `*`, which styles descendants",
        ),
        (
            "not-[&_.foo]",
            false,
            "[&_.foo]",
            "`not-` cannot negate `[&_.foo]`, which styles descendants",
        ),
        (
            "not-hv",
            false,
            "hv",
            "`not-` cannot negate `hv`, which styles descendants",
        ),
        (
            "not-sm/foo",
            false,
            "/foo",
            "`sm` does not accept a modifier",
        ),
        (
            "group-sm",
            false,
            "sm",
            "`group-` cannot contain the at-rule variant `sm`",
        ),
        (
            "has-sm",
            false,
            "sm",
            "`has-` cannot contain the at-rule variant `sm`",
        ),
        (
            "in-*",
            false,
            "*",
            "`in-` cannot contain `*`, which styles descendants",
        ),
        (
            "peer-before",
            false,
            "before",
            "`peer-` cannot contain `before`, which selects a pseudo-element",
        ),
        ("group-foo", true, "foo", "unknown variant `foo`"),
        (
            "[color:red;]",
            false,
            "[color:red;]",
            "`color:red;` is not a valid selector",
        ),
        (
            "[>img]",
            false,
            "[>img]",
            "the relative selector `>img` can only be used in a `has-` variant",
        ),
    ];

    /// Each failure names its cause and covers the variant text responsible for it.
    #[test]
    fn failures_name_the_cause_and_the_responsible_text() {
        let theme = parse_fixture_theme(
            &SourceFile::new(
                FileId(1),
                None,
                None,
                "@custom-variant hv (:hover); @theme { --breakpoint-sm: 40rem; --breakpoint-md: 48rem; }",
            )
            .unwrap(),
        )
        .unwrap();
        for &(raw, unknown, text, message) in VARIANT_FAILURES {
            let variant = &parse_candidate(&format_args!("{raw}:flex").to_lean_string())
                .unwrap()
                .variants[0];
            let failure = resolve_variant(variant, &theme).unwrap_err();
            assert_eq!(failure.reason.is_unknown(), unknown, "{raw}");
            assert_eq!(&raw[failure.range.clone()], text, "{raw}");
            assert_eq!(failure.reason.to_lean_string(), message, "{raw}");
        }
    }

    #[test]
    fn unknown_names_suggest_spellings_that_resolve() {
        let theme = parse_fixture_theme(
            &SourceFile::new(FileId(1), None, None, "@theme { --breakpoint-md: 48rem; }").unwrap(),
        )
        .unwrap();
        for (raw, suggestion) in [
            ("hovr", Some("hover")),
            ("min-mdd", Some("md")),
            ("xyzzy", None),
        ] {
            let variant = &parse_candidate(&format_args!("{raw}:flex").to_lean_string())
                .unwrap()
                .variants[0];
            let failure = resolve_variant(variant, &theme).unwrap_err();
            assert_eq!(
                failure.reason.suggestion(&theme).as_deref(),
                suggestion,
                "{raw}"
            );
        }
    }
}
