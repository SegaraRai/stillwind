//! `implicit-nesting-selector`: variant definition selectors that are relative to `&`
//! without writing it.
//!
//! Without `scope(document)`, a selector of a variant definition is a nested style rule
//! selector, so a branch without `&` means `& <branch>` in every context. The finding
//! covers the selector at the definition, and its fix inserts `& ` where the compiler
//! inserts it, so the fixed definition applies the same selector.

use std::ops::Range;

use lean_string::ToLeanStr;

use stillwind_css::{
    CssBlock, CssNodeKind, absolutize_nested_selector, implicit_nesting_positions,
    selector_is_valid,
};
use stillwind_diagnostics::{FixAction, LintDiagnostic, LintFinding, LintKind, SourceEdit};

use super::analysis::Checker;
use super::definitions::{Definition, DefinitionKind};

/// Reports the selectors of the variant definition `definition` that have a branch
/// without `&` or a branch that starts with a combinator.
///
/// Definitions with `scope(document)`, or `scope: document` in a `@stillwind-variant`
/// header, keep their selectors as written and are skipped. At-rule shorthands such as
/// `(@media print)` have no selector.
pub(super) fn check_nesting(checker: &mut Checker<'_>, definition: &Definition<'_>) {
    if !checker.enabled(LintKind::ImplicitNestingSelector) {
        return;
    }
    let text = checker.source.text();
    let rest = &text[definition.rest.clone()];
    match definition.kind {
        DefinitionKind::Utility => {
            return;
        }
        DefinitionKind::Variant if rest.starts_with("scope(") => {
            return;
        }
        DefinitionKind::StillwindVariant if declares_document_scope(rest) => {
            return;
        }
        DefinitionKind::Variant | DefinitionKind::StillwindVariant => {}
    }
    match definition.block {
        Some(block) => {
            check_body(checker, block);
        }
        None => {
            if let Some(selector) = rest
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
                .filter(|inner| !inner.trim_start().starts_with('@'))
            {
                let start = definition.rest.start + 1;
                let leading = selector.len() - selector.trim_start().len();
                let start = start + leading;
                check_selector(checker, start..start + selector.trim().len());
            }
        }
    }
}

/// Returns whether the `@stillwind-variant` header text after the name, such as
/// `(value: any, scope: document)`, declares the document scope.
fn declares_document_scope(rest: &str) -> bool {
    rest.trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .filter_map(|descriptor| descriptor.split_once(':'))
        .any(|(key, value)| key.trim() == "scope" && value.trim() == "document")
}

/// Checks the outermost style rules of a definition body, including rules inside
/// at-rules such as `@media`.
fn check_body(checker: &mut Checker<'_>, block: &CssBlock) {
    for node in &block.children {
        match &node.kind {
            CssNodeKind::StyleRule { selector, .. } => {
                check_selector(checker, selector.clone());
            }
            CssNodeKind::AtRule {
                block: Some(block), ..
            } => {
                check_body(checker, block);
            }
            _ => {}
        }
    }
}

/// Reports the selector at `range` when the compiler inserts `&` into it, with a fix
/// that inserts `& ` at each such branch.
fn check_selector(checker: &mut Checker<'_>, range: Range<usize>) {
    let selector = &checker.source.text()[range.clone()];
    let positions = implicit_nesting_positions(selector);
    let nested = absolutize_nested_selector(selector);
    if positions.is_empty() || !selector_is_valid(&nested) {
        return;
    }
    let span = checker.span(range.clone());
    let mut finding = LintFinding::new(
        LintDiagnostic::ImplicitNestingSelector {
            selector: selector.to_lean_str(),
            nested: nested.to_lean_str(),
        },
        span,
    );
    let edits = positions
        .into_iter()
        .map(|position| {
            let at = range.start + position;
            Some(SourceEdit {
                range: checker.edit_range(checker.span(at..at))?,
                replacement: "& ".into(),
            })
        })
        .collect::<Option<Vec<_>>>();
    if let Some(edits) = edits {
        finding.actions.push(FixAction {
            title: "Write the nesting selector `&`".into(),
            edits,
        });
    }
    checker.report(finding);
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::resolve_variant;
    use stillwind_diagnostics::LintKind;
    use stillwind_syntax::parse_candidate;

    use super::super::lint_stylesheets;
    use super::super::tests::{default_findings, fixed, snapshot};
    use crate::CompilerOptions;

    #[test]
    fn selectors_without_nesting_selector_get_it_inserted() {
        for (css, covered, expected) in [
            (
                "@custom-variant hv (:hover);",
                ":hover",
                "@custom-variant hv (& :hover);",
            ),
            (
                "@custom-variant child ( > .item );",
                "> .item",
                "@custom-variant child ( & > .item );",
            ),
            (
                "@custom-variant either (:hover, &.open,.x);",
                ":hover, &.open,.x",
                "@custom-variant either (& :hover, &.open,& .x);",
            ),
            (
                "@custom-variant blk { :hover { @slot; } }",
                ":hover",
                "@custom-variant blk { & :hover { @slot; } }",
            ),
            (
                "@custom-variant pm { @media print { .open { @slot; } } }",
                ".open",
                "@custom-variant pm { @media print { & .open { @slot; } } }",
            ),
            (
                "@variant legacy { .x { @slot; } }",
                ".x",
                "@variant legacy { & .x { @slot; } }",
            ),
        ] {
            assert_eq!(
                default_findings(css),
                [fixed(LintKind::ImplicitNestingSelector, covered, expected)],
                "{css}"
            );
        }
    }

    #[test]
    fn explicit_and_document_selectors_are_not_reported() {
        assert_eq!(
            default_findings(
                "@custom-variant hv (&:hover);\
                 @custom-variant shell scope(document) (:host);\
                 @custom-variant root scope(document) { :root { @slot; } }\
                 @custom-variant pr (@media print);\
                 .card { @variant hover { color: red; } }"
            ),
            []
        );
    }

    #[test]
    fn the_fix_keeps_the_applied_selector_and_names_the_document_scope() {
        let options = CompilerOptions::default();
        let resolve = |css: &str| {
            let snapshot = snapshot(css, &options);
            let variant = &parse_candidate("either:flex").unwrap().variants[0];
            resolve_variant(variant, snapshot.theme())
        };
        let written = "@custom-variant either (:hover, > .open);";
        assert_eq!(
            resolve(written),
            resolve("@custom-variant either (& :hover, & > .open);")
        );
        let snapshot = snapshot(written, &options);
        let lint = lint_stylesheets(&snapshot, &options);
        let message = lint.findings[0].diagnostic.to_string();
        assert!(
            message.contains("`& :hover, & > .open`") && message.contains("scope(document)"),
            "{message}"
        );
    }
}
