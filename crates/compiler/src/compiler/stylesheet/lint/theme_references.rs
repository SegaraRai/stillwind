//! `invalid-theme-reference`: theme references that cannot resolve.
//!
//! A `theme()` or `--theme()` call that names an inactive token without a fallback
//! contributes nothing: stylesheet rendering omits the declaration that contains it.
//! The finding covers the call, in authored rules, `@utility` bodies, and variant
//! definitions. `@theme` blocks are not checked, because a token whose value cannot
//! resolve is itself omitted.
//!
//! A `@utility` whose `--value()` calls read only theme namespaces that provide no
//! value can never resolve. The finding covers the definition name, and uses of the
//! utility still report `unknown-utility`.

use std::ops::Range;

use cssparser::Token;
use lean_string::ToLeanStr;

use stillwind_css::{CssNode, CssNodeKind, find_css_functions, split_once_top_level};
use stillwind_design_system::{suggested_theme_token, unresolvable_value_namespaces};
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};
use stillwind_source::JoinText;

use super::analysis::Checker;
use super::definitions::{Definition, DefinitionKind};

/// Reports unresolved references in `nodes` and their descendants. `in_rule` tells
/// whether a style rule encloses `nodes`; outside one, `@theme` blocks are skipped.
pub(super) fn check_theme_references(checker: &mut Checker<'_>, nodes: &[CssNode], in_rule: bool) {
    if !checker.enabled(LintKind::InvalidThemeReference) {
        return;
    }
    for node in nodes {
        match &node.kind {
            CssNodeKind::StyleRule { selector, block } => {
                check_text(checker, selector.clone());
                check_theme_references(checker, &block.children, true);
            }
            CssNodeKind::AtRule { name, .. }
                if name == "apply" || (!in_rule && name == "theme") => {}
            CssNodeKind::AtRule { prelude, block, .. } => {
                check_text(checker, prelude.clone());
                if let Some(block) = block {
                    check_theme_references(checker, &block.children, in_rule);
                }
            }
            CssNodeKind::Declaration { value, .. } => {
                check_text(checker, value.clone());
            }
        }
    }
}

/// Reports each call in the source text at `range` that names an inactive token
/// without a fallback. A `--theme()` fallback of `initial` omits the declaration on
/// purpose and is not reported.
fn check_text(checker: &mut Checker<'_>, range: Range<usize>) {
    let text = &checker.source.text()[range.clone()];
    for found in find_css_functions(text, &["theme", "--theme"], 0) {
        let Some(end) = found.end else {
            continue;
        };
        let (reference, fallback) = split_once_top_level(&text[found.arguments], |token| {
            matches!(token, Token::Comma)
        });
        let reference = reference.trim();
        if !reference.starts_with("--")
            || fallback.is_some_and(|fallback| !fallback.trim().is_empty())
            || checker.theme.token_property(reference).is_some()
        {
            continue;
        }
        let finding = LintFinding::new(
            LintDiagnostic::UnresolvedThemeReference {
                reference: text[found.start..end].to_lean_str(),
                suggestion: suggested_theme_token(reference, checker.theme).map(Into::into),
            },
            checker.span(range.start + found.start..range.start + end),
        );
        checker.report(finding);
    }
}

/// Reports the `@utility` definition `definition`, which has a capture, when no
/// domain of its `--value()` calls can resolve a value, as
/// [`unresolvable_value_namespaces`] describes.
pub(super) fn check_utility_namespaces(checker: &mut Checker<'_>, definition: &Definition<'_>) {
    let Some(block) = definition.block else {
        return;
    };
    let text = checker.source.text();
    if definition.kind != DefinitionKind::Utility
        || !definition.has_capture(text)
        || !checker.enabled(LintKind::InvalidThemeReference)
    {
        return;
    }
    let namespaces = unresolvable_value_namespaces(&text[block.contents.clone()], checker.theme);
    if namespaces.is_empty() {
        return;
    }
    let finding = LintFinding::new(
        LintDiagnostic::EmptyThemeNamespace {
            utility: text[definition.name.clone()].to_lean_str(),
            namespaces: namespaces.join_text(", ").to_lean_str(),
        },
        checker.span(definition.name.clone()),
    );
    checker.report(finding);
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::{LintDiagnostic, LintKind};

    use super::super::lint_stylesheets;
    use super::super::tests::{default_findings, finding, snapshot};
    use crate::CompilerOptions;

    #[test]
    fn references_without_token_or_fallback_are_reported() {
        assert_eq!(
            default_findings(
                ".a {\
                   color: --theme(--color-nope);\
                   background: --theme(--color-nope, red);\
                   border-color: --theme(--color-nope, initial);\
                   fill: --theme(--color-red-500);\
                   outline-color: theme(--color-rde-500);\
                 }\
                 @utility u { color: --theme(--color-gone); }\
                 @custom-variant v { @media (width >= --theme(--breakpoint-nope)) { @slot; } }\
                 @theme { --color-x: --theme(--color-skipped); }"
            ),
            [
                finding(LintKind::InvalidThemeReference, "--theme(--color-nope)"),
                finding(LintKind::InvalidThemeReference, "theme(--color-rde-500)"),
                finding(LintKind::InvalidThemeReference, "--theme(--color-gone)"),
                finding(
                    LintKind::InvalidThemeReference,
                    "--theme(--breakpoint-nope)"
                ),
            ]
        );
        let options = CompilerOptions::default();
        let snapshot = snapshot(".a { color: theme(--color-rde-500); }", &options);
        assert!(matches!(
            &lint_stylesheets(&snapshot, &options).findings[0].diagnostic,
            LintDiagnostic::UnresolvedThemeReference { suggestion: Some(suggestion), .. }
                if suggestion == "--color-red-500"
        ));
    }

    #[test]
    fn utilities_that_read_only_empty_namespaces_are_reported() {
        assert_eq!(
            default_findings("@utility box-* { width: --value(--size-*); }"),
            [finding(LintKind::InvalidThemeReference, "box-*")]
        );
        assert_eq!(
            default_findings(
                "@theme { --size-4: 1rem; --color-x: red; }                 @utility box-* { width: --value(--size-lg-*); }                 @utility tx-* { color: --value(--color-*-strong); }"
            ),
            [
                finding(LintKind::InvalidThemeReference, "box-*"),
                finding(LintKind::InvalidThemeReference, "tx-*"),
            ]
        );
        for css in [
            "@theme { --size-4: 1rem; } @utility box-* { width: --value(--size-*); }",
            "@theme { --size-lg-4: 1rem; } @utility box-* { width: --value(--size-lg-*); }",
            "@theme { --tab-4: 4; } @utility box-* { width: --value(--spacing-*); }",
            "@utility box-* { width: --value(--size-*, integer); }",
            "@utility box-* { width: --value(--size-*, [length]); }",
            "@utility tx-* { color: --value(--color-*); }",
            "@utility box { width: --value(--size-*); }",
        ] {
            assert_eq!(default_findings(css), [], "{css}");
        }
    }
}
