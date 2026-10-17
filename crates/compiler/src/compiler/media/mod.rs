//! Lowering of `style.media()` calls into media query list strings.
//!
//! Each argument is a variant list whose variants must resolve only to media
//! conditions. The variants of one argument must all hold, and the arguments are
//! alternatives, so the call matches where any argument matches. An argument that does
//! not resolve, or that contains another kind of condition, matches no environment and
//! produces a lint finding. Removing only the failing variant would match more
//! environments than the author wrote.
//!
//! [`media_query`] lowers one call, and [`query`] combines media query lists.

mod query;

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{
    ThemeIndex, UtilityFailure, UtilityFailureReason, VariantApplication, resolve_variant,
};
use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::MediaCall;
use stillwind_source::Spanned;

use self::query::MediaQueryList;
use super::style_variants::parse_prefix;
use super::utilities::{unresolved_utility_finding, written_span};
use super::variants::{VariantStep, variant_paths};

/// Returns the media query list that `call` lowers to, without `@media` or braces.
///
/// Queries keep the spelling of their variant definitions when one variant supplies
/// them. Several variants of one argument combine into one query per alternative.
/// Returns `not all` when no argument can match. Findings for unsupported arguments are
/// appended to `lint_findings`.
pub(crate) fn media_query(
    call: &MediaCall,
    theme: &ThemeIndex,
    lint_findings: &mut Vec<LintFinding>,
) -> LeanString {
    let mut queries: Vec<LeanString> = Vec::new();
    for argument in &call.arguments {
        for query in argument_queries(argument, theme, lint_findings) {
            if !queries.contains(&query) {
                queries.push(query);
            }
        }
    }
    if queries.is_empty() {
        return "not all".to_lean_string();
    }
    queries.join(", ").to_lean_string()
}

/// Returns the media query list of one decoded `style.media()` argument, or `None` when
/// the argument produces a lint finding or matches no environment.
///
/// Editors use this to validate and preview one argument without its call.
pub(crate) fn media_argument_query(text: &str, theme: &ThemeIndex) -> Option<LeanString> {
    let argument = Spanned::new(
        LeanStr::from(text),
        stillwind_source::Span::new(
            stillwind_source::FileId(0),
            0,
            u32::try_from(text.len()).unwrap_or(u32::MAX),
        ),
    );
    let mut findings = Vec::new();
    let queries = argument_queries(&argument, theme, &mut findings);
    (findings.is_empty() && !queries.is_empty()).then(|| queries.join(", ").to_lean_string())
}

/// Resolves one argument into the media queries of its alternatives.
///
/// Returns no queries, after pushing a finding, when a variant does not resolve or adds
/// a condition that is not a media query.
fn argument_queries(
    argument: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    lint_findings: &mut Vec<LintFinding>,
) -> Vec<LeanString> {
    let text = argument.value.as_str();
    if text.trim().is_empty() {
        lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidMediaArgument {
                reason: "the argument is empty".into(),
            },
            argument.span,
        ));
        return Vec::new();
    }
    let prefix = if text.ends_with(':') {
        argument.clone()
    } else {
        Spanned::new(
            format_args!("{text}:").to_lean_string().into(),
            argument.span,
        )
    };
    let Some(variants) = parse_prefix(&prefix) else {
        lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidVariantPrefix,
            argument.span,
        ));
        return Vec::new();
    };
    let mut applications = Vec::with_capacity(variants.len());
    let mut start = 0;
    for variant in &variants {
        let range = start..start + variant.raw.len();
        let application = match resolve_variant(variant, theme) {
            Ok(application) => application,
            Err(failure) => {
                let failure = failure.shifted(start);
                let failure = UtilityFailure {
                    range: failure.range,
                    reason: UtilityFailureReason::Variant(failure.reason),
                };
                lint_findings.push(unresolved_utility_finding(
                    text,
                    text.len(),
                    &failure,
                    argument.span,
                    theme,
                ));
                return Vec::new();
            }
        };
        if !media_only(std::slice::from_ref(&application)) {
            lint_findings.push(LintFinding::new(
                LintDiagnostic::NonMediaVariant {
                    variant: variant.raw.clone(),
                },
                written_span(text.len(), text.len(), &range, argument.span),
            ));
            return Vec::new();
        }
        applications.push(application);
        start = range.end + 1;
    }
    variant_paths(&applications)
        .iter()
        .filter_map(|path| path_query(path))
        .collect()
}

/// Whether every alternative of `applications` consists of media conditions.
fn media_only(applications: &[VariantApplication]) -> bool {
    variant_paths(applications)
        .iter()
        .flatten()
        .all(|step| matches!(step, VariantStep::Media(_)))
}

/// Returns the media query list that requires every condition of `path`, or `None`
/// when no environment satisfies them all.
///
/// One condition keeps its spelling. An empty path matches every environment.
fn path_query(path: &[VariantStep<'_>]) -> Option<LeanString> {
    let conditions = path.iter().filter_map(|step| match step {
        VariantStep::Media(condition) => Some(*condition),
        _ => None,
    });
    let mut lists = conditions.map(MediaQueryList::parse);
    let Some(first) = lists.next() else {
        return Some("all".to_lean_string());
    };
    if path.len() == 1 {
        return path.iter().find_map(|step| match step {
            VariantStep::Media(condition) => Some(condition.trim().to_lean_string()),
            _ => None,
        });
    }
    let mut combined = first?;
    for list in lists {
        combined = combined.and(&list?);
    }
    combined.render()
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId, Span};
    use stillwind_test_support::css_source;

    use super::*;

    /// Lowers a call with `arguments` against the standard package and a custom variant
    /// `wide` whose two media alternatives must stay alternatives.
    fn lower(arguments: &[&str]) -> (LeanString, Vec<LintFinding>) {
        let source = css_source(
            FileId(2),
            "@import 'stillwind';             @custom-variant wide { @media (width >= 90rem) { @slot; } @media print { @slot; } }",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let theme = snapshot.theme();
        let call = MediaCall {
            span: Span::new(FileId(1), 0, 0),
            arguments: arguments
                .iter()
                .map(|argument| {
                    Spanned::new(
                        LeanStr::from(*argument),
                        Span::new(FileId(1), 0, u32::try_from(argument.len()).unwrap()),
                    )
                })
                .collect(),
        };
        let mut findings = Vec::new();
        let query = media_query(&call, theme, &mut findings);
        (query, findings)
    }

    #[test]
    fn variants_of_one_argument_combine_and_arguments_are_alternatives() {
        for (arguments, expected) in [
            (&["sm"][..], "(width >= 40rem)"),
            (&["sm:"], "(width >= 40rem)"),
            (&["motion-reduce:"], "(prefers-reduced-motion: reduce)"),
            (
                &["sm:motion-reduce:"],
                "(width >= 40rem) and (prefers-reduced-motion: reduce)",
            ),
            (
                &["sm:", "motion-reduce:"],
                "(width >= 40rem), (prefers-reduced-motion: reduce)",
            ),
            (&["print:sm"], "print and (width >= 40rem)"),
            (&["not-sm:"], "not (width >= 40rem)"),
            (&["sm", "sm:"], "(width >= 40rem)"),
            (&["wide"], "(width >= 90rem), print"),
            (
                &["wide:motion-reduce"],
                "(width >= 90rem) and (prefers-reduced-motion: reduce), print and (prefers-reduced-motion: reduce)",
            ),
        ] {
            let (query, findings) = lower(arguments);
            assert_eq!(query, expected, "{arguments:?}");
            assert!(findings.is_empty(), "{arguments:?}: {findings:?}");
        }
    }

    #[test]
    fn invalid_arguments_match_nothing_and_keep_valid_alternatives() {
        let (query, findings) = lower(&["sm:future:", "motion-reduce"]);
        assert_eq!(query, "(prefers-reduced-motion: reduce)");
        assert!(matches!(
            &findings[..],
            [LintFinding {
                diagnostic: LintDiagnostic::UnknownVariant { .. },
                ..
            }]
        ));
        assert_eq!(findings[0].labels[0].span, Span::new(FileId(1), 3, 9));

        let (query, findings) = lower(&["sm:hover:"]);
        assert_eq!(query, "not all");
        assert!(matches!(
            &findings[..],
            [LintFinding {
                diagnostic: LintDiagnostic::NonMediaVariant { variant },
                ..
            }] if variant == "hover"
        ));
        assert_eq!(findings[0].labels[0].span, Span::new(FileId(1), 3, 8));

        for argument in ["", "  ", "sm::"] {
            let (query, findings) = lower(&[argument]);
            assert_eq!(query, "not all", "{argument:?}");
            assert_eq!(findings.len(), 1, "{argument:?}");
        }
        assert_eq!(lower(&[]).0, "not all");
    }

    #[test]
    fn selectors_containers_and_supports_are_not_media_conditions() {
        for argument in [
            "hover",
            "focus",
            "@sm",
            "supports-grid",
            "starting",
            "[&:hover]",
        ] {
            let (query, findings) = lower(&[argument]);
            assert_eq!(query, "not all", "{argument}");
            assert!(
                matches!(
                    &findings[..],
                    [LintFinding {
                        diagnostic: LintDiagnostic::NonMediaVariant { .. },
                        ..
                    }]
                ),
                "{argument}: {findings:?}"
            );
        }
    }
}
