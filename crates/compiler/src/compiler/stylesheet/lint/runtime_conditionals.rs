//! `invalid-runtime-condition`: authored declarations whose `--if()` cannot be lowered.
//!
//! Rendering leaves such a declaration out of the stylesheet instead of failing. This
//! module reports each one at its authored declaration, including declarations in
//! utility definitions and theme blocks.

use lean_string::ToLeanStr;

use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};

use super::super::super::conditionals::{check_value, declaration_values};
use super::analysis::Checker;

/// Reports each declaration of the checked source whose `--if()` functions cannot be
/// lowered with `use_css_if`.
pub(super) fn check_runtime_conditionals(checker: &mut Checker<'_>, use_css_if: bool) {
    if !checker.enabled(LintKind::InvalidRuntimeCondition) {
        return;
    }
    let text = checker.source.text();
    if !text.contains("--if(") {
        return;
    }
    for (property, value) in declaration_values(text) {
        let Err(failure) = check_value(&text[value.clone()], use_css_if) else {
            continue;
        };
        let name = &text[property.clone()];
        let start = property.end - name.trim_start().len();
        let finding = LintFinding::new(
            LintDiagnostic::InvalidRuntimeCondition {
                property: name.trim().to_lean_str(),
                detail: failure.detail().into(),
            },
            checker.span(start..value.end),
        );
        checker.report(finding);
    }
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

    #[test]
    fn reports_declarations_whose_conditions_cannot_be_lowered() {
        assert_eq!(
            default_findings(
                "@utility tone { color: --if(--on, red); }
@theme { --brand: --if(--on:dark, black, white); }
.ok { color: --if(--on, red, blue); }"
            ),
            [
                finding(LintKind::InvalidRuntimeCondition, "color: --if(--on, red)"),
                finding(
                    LintKind::InvalidRuntimeCondition,
                    "--brand: --if(--on:dark, black, white)"
                ),
            ]
        );
    }
}
