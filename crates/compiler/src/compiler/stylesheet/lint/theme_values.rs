//! `incompatible-theme-value`: `@theme` token values that the type of their namespace
//! rejects.
//!
//! Resolution uses a token without checking its value, so `--color-brand: 12px` makes
//! `bg-brand` emit `background-color: var(--color-brand)`. The finding covers the
//! value, and the output stays unchanged.

use lean_string::ToLeanStr;

use stillwind_css::{CssNode, CssNodeKind};
use stillwind_design_system::{ThemeValueRole, theme_value_mismatch};
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};

use super::analysis::Checker;

/// Reports the token values of the `@theme` blocks among `nodes` and inside wrappers
/// such as `@layer`. Style rules and other definitions are not searched.
pub(super) fn check_theme_values(checker: &mut Checker<'_>, nodes: &[CssNode]) {
    if !checker.enabled(LintKind::IncompatibleThemeValue) {
        return;
    }
    for node in nodes {
        let CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(block),
            ..
        } = &node.kind
        else {
            continue;
        };
        if name == "theme" {
            if ThemeValueRole::parse(&checker.source.text()[prelude.clone()])
                == ThemeValueRole::Asset
            {
                continue;
            }
            for declaration in &block.children {
                check_token(checker, declaration);
            }
        } else if !super::analysis::is_definition(name) {
            check_theme_values(checker, &block.children);
        }
    }
}

fn check_token(checker: &mut Checker<'_>, node: &CssNode) {
    let CssNodeKind::Declaration { property, value } = &node.kind else {
        return;
    };
    let text = checker.source.text();
    let (token, written) = (&text[property.clone()], &text[value.clone()]);
    let Some(mismatch) = theme_value_mismatch(token, written, checker.theme) else {
        return;
    };
    let finding = LintFinding::new(
        LintDiagnostic::IncompatibleThemeValue {
            token: token.to_lean_str(),
            value: written.to_lean_str(),
            namespace: mismatch.namespace.as_str().into(),
            expected: mismatch.expected,
        },
        checker.span(value.clone()),
    );
    checker.report(finding);
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

    #[test]
    fn asset_values_do_not_acquire_types_from_theme_namespaces() {
        assert_eq!(
            default_findings(
                "@theme asset {--color-picture:url(data:image/svg+xml,a); --breakpoint-data:blue;}
            @theme {--color-brand:12px;}"
            ),
            [finding(LintKind::IncompatibleThemeValue, "12px")]
        );
    }

    #[test]
    fn token_values_must_match_their_namespace() {
        assert_eq!(
            default_findings(
                "@theme { --color-brand: 12px; --breakpoint-tab: blue; --color-ok: red; }\
                 @layer theme { @theme inline { --spacing: red; } }"
            ),
            [
                finding(LintKind::IncompatibleThemeValue, "12px"),
                finding(LintKind::IncompatibleThemeValue, "blue"),
                finding(LintKind::IncompatibleThemeValue, "red"),
            ]
        );
        assert_eq!(
            default_findings(
                "@theme {\
                   --color-a: var(--x);\
                   --color-b: --theme(--color-red-500);\
                   --color-*: initial;\
                   --text-big: 3rem;\
                   --text-big--line-height: 1;\
                   --size-4: blue;\
                 }\
                 @utility box-* { width: --value(--size-*); }\
                 .card { --color-brand: 12px; }"
            ),
            []
        );
    }
}
