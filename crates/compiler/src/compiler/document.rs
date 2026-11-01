//! Document-level styles, where variants select the element that a candidate styles.
//!
//! A document-level style has no generated class, so its candidates resolve in
//! [`VariantContext::Document`]: the first variant that selects an element, such as
//! `host:` or `[:root]:`, sets the element, and later variants apply relative to it. A
//! candidate without such a variant is omitted and reported as
//! `missing-document-target`. `document-scoped-selector` checks only the variants after
//! the one that selects the element.
//!
//! `style.css()` calls use [`emit_document_rules`], and the editor query role
//! [`crate::CandidateContext::Document`] uses [`emit_document_style`].

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{ThemeIndex, VariantContext, document_target, resolve_variant};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintFinding, LintKind, LintSettings};
use stillwind_source::Spanned;
use stillwind_syntax::parse_candidate;

use super::RuleBoundaries;
use super::lexer::lex_utilities;
use super::literal::{
    definition_condition_lint, omit, resolved_candidate_lints, runtime_condition_lint,
    variant_selector_lints,
};
use super::output::CssContribution;
use super::utilities::{
    emit_utility_rules, resolve_utility_targets, unresolved_lint_kind, unresolved_utility_finding,
};
use crate::identifiers::IdentifierContext;

/// Resolves the whitespace-separated candidates of a document-level style and returns
/// their rules in emission order.
///
/// A candidate that does not resolve, or that does not select an element as
/// [`VariantContext::Document`] describes, is omitted, and enabled lint rules report it
/// in `lint_findings`. A candidate whose runtime condition cannot be lowered with
/// `use_css_if` is omitted in the same way. A malformed utility requirement in the
/// design system appends a diagnostic to `diagnostics`; callers must discard the result
/// if diagnostics are present. Runtime conditions in the returned CSS are not lowered
/// yet.
///
/// # Errors
///
/// Returns the diagnostic of a runtime condition that cannot be rendered.
pub(crate) fn emit_document_style(
    input: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    lint_settings: &LintSettings,
    use_css_if: bool,
    identifiers: &IdentifierContext,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> Result<LeanString, CompileDiagnostic> {
    let contributions = emit_document_rules(
        &[DocumentInput {
            prefix: "",
            utilities: input,
        }],
        None,
        theme,
        lint_settings,
        use_css_if,
        identifiers,
        diagnostics,
        lint_findings,
    )?;
    let mut boundaries = RuleBoundaries::new(
        format_args!("preview\0{}", input.value).to_lean_string(),
        identifiers,
    );
    contributions
        .iter()
        .map(|contribution| boundaries.render(&contribution.css))
        .collect()
}

/// One utility literal of a document-level style.
pub(crate) struct DocumentInput<'a> {
    /// Variants of enclosing groups, each ending in `:`, prepended to every candidate.
    /// The author did not write them at the literal's span.
    pub(crate) prefix: &'a str,
    /// Whitespace-separated candidates at their source span.
    pub(crate) utilities: &'a Spanned<LeanStr>,
}

/// Resolves the candidates of `inputs` as one document-level style and returns their
/// rules in emission order, as [`emit_document_style`] describes.
///
/// `selector` is the element that a candidate styles when its own variants select
/// none. Such a candidate resolves as if it started with the arbitrary variant
/// `[selector]:`. Without `selector`, it is omitted and reported as
/// `missing-document-target`.
///
/// # Errors
///
/// Returns the diagnostic of a runtime condition that cannot be rendered.
#[allow(clippy::too_many_arguments)] // One borrow per compilation-wide destination.
pub(crate) fn emit_document_rules(
    inputs: &[DocumentInput<'_>],
    selector: Option<&str>,
    theme: &ThemeIndex,
    lint_settings: &LintSettings,
    use_css_if: bool,
    identifiers: &IdentifierContext,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> Result<Vec<CssContribution>, CompileDiagnostic> {
    let target =
        selector.map(|selector| format_args!("[{}]:", encode_variant(selector)).to_lean_string());
    let mut utilities = Vec::new();
    for input in inputs {
        for token in lex_utilities(input.utilities, diagnostics) {
            let written = &token.value.raw;
            let mut raw = format_args!("{}{written}", input.prefix).to_lean_string();
            let mut result = resolve_utility_targets(&raw, theme, VariantContext::Document);
            if let (Some(target), Ok(Err(failure))) = (&target, &result)
                && unresolved_lint_kind(failure) == LintKind::MissingDocumentTarget
            {
                raw = format_args!("{target}{raw}").to_lean_string();
                result = resolve_utility_targets(&raw, theme, VariantContext::Document);
            }
            if let Ok(Ok(resolved)) = &result
                && omit(
                    runtime_condition_lint(&raw, written, theme, use_css_if)
                        .or_else(|| definition_condition_lint(written, resolved, use_css_if)),
                    token.span,
                    lint_settings,
                    lint_findings,
                )
            {
                continue;
            }
            match result {
                Ok(Ok(mut resolved)) => {
                    lint_findings.extend(variant_selector_lints(
                        &raw,
                        theme,
                        lint_settings,
                        token.span,
                        target_variant(&raw, theme).map_or(0, |index| index + 1),
                    ));
                    lint_findings.extend(resolved_candidate_lints(
                        &raw,
                        written.len(),
                        &resolved,
                        theme,
                        lint_settings,
                        token.span,
                    ));
                    for utility in &mut resolved {
                        utility.diagnostic_data.span = Some(token.span);
                    }
                    utilities.append(&mut resolved);
                }
                Ok(Err(failure)) => {
                    if lint_settings.enabled_for_detection(unresolved_lint_kind(&failure)) {
                        lint_findings.push(unresolved_utility_finding(
                            &raw,
                            written.len(),
                            &failure,
                            token.span,
                            theme,
                        ));
                    }
                }
                Err(error) => {
                    diagnostics.push(CompileError::new(
                        CompileDiagnostic::MalformedUtilityRequirement {
                            utility: error.name,
                            offset: error.source.offset,
                        },
                        token.span,
                    ));
                }
            }
        }
    }
    let mut contributions = Vec::new();
    emit_utility_rules::<0>(&mut contributions, "&", &utilities, theme, identifiers)?;
    Ok(contributions)
}

/// Spells `selector` as the value of an arbitrary variant: a space becomes `_`, and a
/// literal `_` is escaped.
fn encode_variant(selector: &str) -> LeanString {
    let mut output = LeanString::new();
    for character in selector.trim().chars() {
        match character {
            '_' => {
                output.push_str(r"\_");
            }
            character if character.is_whitespace() => {
                output.push('_');
            }
            character => {
                output.push(character);
            }
        }
    }
    output
}

/// Returns the index of the variant of `raw` that selects the element of a
/// document-level style, or `None` when `raw` does not parse or its variants do not
/// resolve.
fn target_variant(raw: &str, theme: &ThemeIndex) -> Option<usize> {
    let applications = parse_candidate(raw)?
        .variants
        .iter()
        .map(|variant| resolve_variant(variant, theme).ok())
        .collect::<Option<Vec<_>>>()?;
    document_target(&applications).ok()
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::{LintDiagnostic, LintKind};
    use stillwind_source::{CssSourceName, FileId, Span};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::identifiers::{DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierMode};

    fn snapshot() -> DesignSystemSnapshot {
        let source = css_source(
            FileId(1),
            "@import 'stillwind';\
             @custom-variant hv (:hover);\
             @custom-variant shell scope(document) (:host);",
        )
        .unwrap();
        DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap()
    }

    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    /// Renders `input` as a document-level style and returns the CSS with the kind and
    /// covered text of each finding.
    fn document(theme: &ThemeIndex, input: &str) -> (LeanString, Vec<(LintKind, LeanString)>) {
        let span = Span::new(FileId(2), 0, u32::try_from(input.len()).unwrap());
        let mut diagnostics = Vec::new();
        let mut findings = Vec::new();
        let css = emit_document_style(
            &Spanned::new(LeanStr::from(input), span),
            theme,
            &LintSettings::with_defaults(),
            false,
            &identifiers(),
            &mut diagnostics,
            &mut findings,
        )
        .unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let findings = findings
            .iter()
            .map(|finding| {
                let span = finding.labels[0].span;
                (
                    finding.kind(),
                    input[span.start.0 as usize..span.end.0 as usize].to_lean_string(),
                )
            })
            .collect();
        (css, findings)
    }

    /// Renders `candidate` for the generated class `.c`.
    fn element(theme: &ThemeIndex, candidate: &str) -> LeanString {
        let utilities = resolve_utility_targets(candidate, theme, VariantContext::Element)
            .unwrap()
            .unwrap();
        let mut contributions = Vec::new();
        emit_utility_rules::<0>(&mut contributions, ".c", &utilities, theme, &identifiers())
            .unwrap();
        contributions
            .iter()
            .map(|contribution| contribution.css.as_str())
            .collect()
    }

    /// Each kind of variant applies relative to the generated class where one exists,
    /// and in document context either selects the element or needs an earlier variant
    /// that does.
    #[test]
    fn variants_apply_relative_to_the_anchor_in_both_contexts() {
        let snapshot = snapshot();
        let theme = snapshot.theme();
        for (candidate, in_element, in_document) in [
            ("[&_.x]:flex", ".c .x{display:flex;}", None),
            (
                "[:hover]:flex",
                ".c:is(:hover){display:flex;}",
                Some(":hover{display:flex;}"),
            ),
            (
                "[:host(.compact)]:flex",
                ":host(.compact){display:flex;}",
                Some(":host(.compact){display:flex;}"),
            ),
            (
                "[:root]:flex",
                ":root{display:flex;}",
                Some(":root{display:flex;}"),
            ),
            ("hv:flex", ".c :hover{display:flex;}", None),
            (
                "hover:flex",
                "@media (hover: hover){.c:hover{display:flex;}}",
                None,
            ),
            (
                "host:flex",
                ":host{display:flex;}",
                Some(":host{display:flex;}"),
            ),
            (
                "shell:flex",
                ":host{display:flex;}",
                Some(":host{display:flex;}"),
            ),
            ("print:flex", "@media print{.c{display:flex;}}", None),
            (
                "print:host:flex",
                "@media print{:host{display:flex;}}",
                Some("@media print{:host{display:flex;}}"),
            ),
        ] {
            assert_eq!(element(theme, candidate), in_element, "{candidate}");
            let (css, findings) = document(theme, candidate);
            if let Some(expected) = in_document {
                assert_eq!(css, expected, "{candidate}");
                assert_eq!(findings, [], "{candidate}");
            } else {
                assert_eq!(css, "", "{candidate}");
                assert_eq!(
                    findings.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
                    [LintKind::MissingDocumentTarget],
                    "{candidate}"
                );
            }
        }
    }

    /// Later variants refer to the element that the first selector variant selects.
    #[test]
    fn later_variants_apply_to_the_selected_element() {
        let snapshot = snapshot();
        let theme = snapshot.theme();
        for (candidate, expected) in [
            (
                "host:hover:flex",
                "@media (hover: hover){:host(:hover){display:flex;}}",
            ),
            ("host:[:hover]:flex", ":host(:is(:hover)){display:flex;}"),
            ("host:hv:flex", ":host :hover{display:flex;}"),
            (
                "host:not-hover:flex",
                ":host(:not(:hover)){display:flex;}@media not (hover: hover){:host{display:flex;}}",
            ),
            ("host:focus:*:flex", ":is(:host(:focus) > *){display:flex;}"),
            ("[:hover,:focus]:flex", ":hover,:focus{display:flex;}"),
            (
                "[:hover,:focus]:[&_.x]:flex",
                ":is(:hover,:focus) .x{display:flex;}",
            ),
            ("host:*:flex", ":is(:host > *){display:flex;}"),
        ] {
            let (css, findings) = document(theme, candidate);
            assert_eq!(css, expected, "{candidate}");
            assert_eq!(findings, [], "{candidate}");
        }
        let (css, _) = document(theme, "host:if(--on):flex");
        assert!(
            css.starts_with("@supports (x:revert-rule) and (--"),
            "{css}"
        );
        let (css, _) = document(theme, "host:space-x-4");
        assert!(
            css.starts_with(":where(:host > :not(:last-child)){"),
            "{css}"
        );
    }

    /// `missing-document-target` covers the variant that refers to the element too
    /// early, or the whole candidate when no variant selects one.
    /// `document-scoped-selector` applies only after the element is selected.
    #[test]
    fn reports_candidates_without_a_selected_element() {
        let snapshot = snapshot();
        let theme = snapshot.theme();
        let (css, findings) = document(theme, "size-full not-hover:flex print:hover:host:flex");
        assert_eq!(css, "");
        assert_eq!(
            findings,
            [
                (
                    LintKind::MissingDocumentTarget,
                    "size-full".to_lean_string()
                ),
                (
                    LintKind::MissingDocumentTarget,
                    "not-hover".to_lean_string()
                ),
                (LintKind::MissingDocumentTarget, "hover".to_lean_string()),
            ]
        );
        let (css, findings) = document(theme, "host:[:root]:flex");
        assert_eq!(css, ":root{display:flex;}");
        assert_eq!(
            findings,
            [(LintKind::DocumentScopedSelector, "[:root]".to_lean_string())]
        );

        let mut diagnostics = Vec::new();
        let mut findings = Vec::new();
        emit_document_style(
            &Spanned::new(LeanStr::from("hover:flex"), Span::new(FileId(2), 0, 10)),
            theme,
            &LintSettings::with_defaults(),
            false,
            &identifiers(),
            &mut diagnostics,
            &mut findings,
        )
        .unwrap();
        assert!(matches!(
            &findings[0].diagnostic,
            LintDiagnostic::MissingDocumentTarget { detail, .. }
                if detail == "`hover` refers to the styled element, but no earlier variant selects one"
        ));
    }
}
