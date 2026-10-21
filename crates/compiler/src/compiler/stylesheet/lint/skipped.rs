//! `invalid-css-import` and `invalid-css-directive`: statements that loading skipped.
//!
//! The loader leaves out an invalid import or directive instead of failing, and records
//! it in [`CssSourceGraph::skipped`]. This module reports the records whose span
//! addresses the checked source. Sources that the host does not mark for linting keep
//! their records unreported.

use lean_string::ToLeanString;

use stillwind_design_system::CssSourceGraph;
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};

use super::analysis::Checker;

/// Reports the statements of the checked source that loading skipped. `graph` is the
/// source graph of the snapshot that loaded the source.
pub(super) fn check_skipped_statements(checker: &mut Checker<'_>, graph: &CssSourceGraph) {
    for skipped in graph
        .skipped
        .iter()
        .filter(|skipped| skipped.span.file == checker.source.id())
    {
        let reason = skipped.reason.to_lean_string().into();
        let (kind, diagnostic) = if skipped.reason.is_import() {
            (
                LintKind::InvalidCssImport,
                LintDiagnostic::InvalidCssImport { reason },
            )
        } else {
            (
                LintKind::InvalidCssDirective,
                LintDiagnostic::InvalidCssDirective { reason },
            )
        };
        if checker.enabled(kind) {
            checker.report(LintFinding::new(diagnostic, skipped.span));
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

    #[test]
    fn reports_skipped_imports_and_directives_of_the_source() {
        assert_eq!(
            default_findings(
                "@import 42;
@media print { @utility tone { color: red; } }
@stillwind-slot unknown;
@stillwind-registry motion { }"
            ),
            [
                finding(LintKind::InvalidCssImport, "@import 42;"),
                finding(LintKind::InvalidCssDirective, "@utility"),
                finding(LintKind::InvalidCssDirective, "@stillwind-slot unknown;"),
                finding(LintKind::InvalidCssDirective, "@stillwind-registry motion"),
            ]
        );
    }
}
