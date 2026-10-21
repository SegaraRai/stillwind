//! `bypassed-undefinition`: stylesheet definitions that reach the `keyframe` or
//! `view-transition` registry although an earlier undefinition of that registry removed
//! the definitions of their name.
//!
//! Statements outside every `@stillwind-registry` block reach `default`, `keyframe`,
//! and `view-transition` in registration order. A built-in block that replaces
//! `duration-*` in `view-transition` therefore removes only the definitions before it,
//! and a later `@utility duration-slow` sets `transition-duration` in a view
//! transition. Definitions inside blocks name their registries explicitly and are not
//! reported.

use lean_string::{ToLeanStr, ToLeanString};

use stillwind_design_system::CssSourceGraph;
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};
use stillwind_source::FileId;

use super::analysis::Checker;
use super::definitions::{Definition, DefinitionKind};

/// Reports each of `definitions`, the definitions of the checked source, that bypasses
/// an undefinition. `graph` maps the source to the expanded stylesheet that the
/// registry statements address.
pub(super) fn check_bypassed_undefinitions(
    checker: &mut Checker<'_>,
    graph: &CssSourceGraph,
    definitions: &[Definition<'_>],
) {
    if !checker.enabled(LintKind::BypassedUndefinition) {
        return;
    }
    let text = checker.source.text();
    for definition in definitions {
        let utility = match definition.kind {
            DefinitionKind::Utility => true,
            DefinitionKind::Variant => false,
            DefinitionKind::StillwindVariant => continue,
        };
        let Some(offset) =
            generated_offset(graph, checker.source.id(), definition.node.range.start)
        else {
            continue;
        };
        let name = &text[definition.name.clone()];
        let Some(bypassed) = checker.theme.bypassed_undefinition(utility, name, offset) else {
            continue;
        };
        let finding = LintFinding::new(
            LintDiagnostic::BypassedUndefinition {
                directive: format_args!("@{}", definition.directive)
                    .to_lean_string()
                    .into(),
                name: name.to_lean_str(),
                context: bypassed.context.name().into(),
                undefinition: if utility {
                    "@stillwind-undef-utility"
                } else {
                    "@stillwind-undef-variant"
                }
                .into(),
                pattern: bypassed.pattern,
            },
            checker.span(definition.name.clone()),
        );
        checker.report(finding);
    }
}

/// Returns the first offset in the expanded stylesheet of the byte `offset` of the
/// source `file`, or `None` when the expansion does not include it.
fn generated_offset(graph: &CssSourceGraph, file: FileId, offset: usize) -> Option<usize> {
    graph
        .mappings
        .iter()
        .filter(|mapping| mapping.file == file)
        .filter_map(|mapping| {
            let relative = offset.checked_sub(mapping.source_start)?;
            (relative < mapping.generated.len()).then_some(mapping.generated.start + relative)
        })
        .min()
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

    #[test]
    fn reports_root_definitions_after_context_undefinitions() {
        assert_eq!(
            default_findings(
                "@utility duration-slow { transition-duration: 1s; }
@stillwind-registry default { @utility duration-fast { transition-duration: 0.1s; } }
@utility other { color: red; }
@stillwind-registry keyframe { @stillwind-undef-variant print-*; }
@custom-variant print-wide { @media print { @slot; } }
@stillwind-registry keyframe, default { @stillwind-undef-utility tone-*; }
@utility tone-a { color: blue; }
@utility translate-hidden { translate: 0 -100%; }",
            ),
            [
                finding(LintKind::BypassedUndefinition, "duration-slow"),
                finding(LintKind::BypassedUndefinition, "print-wide"),
                finding(LintKind::BypassedUndefinition, "translate-hidden"),
            ]
        );
    }
}
