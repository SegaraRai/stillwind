//! Selector conditions that test the element declaring a ref.

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{instantiate_selector, selector_is_element_condition, selector_uses_subject};
use stillwind_design_system::{ThemeIndex, VariantApplication};
use stillwind_diagnostics::{CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::RefCondition;

use super::super::symbols::ref_marker_name;
use super::super::variants::{VariantStep, variant_paths};
use super::prefix::collect_variant_and_applications;
use crate::identifiers::{IdentifierContext, RefMarker};

/// One resolved element predicate and its subject-independent environment guards.
pub(super) struct ElementCondition {
    /// Selector that tests the declaring element, with its marker class.
    pub(super) selector: LeanStr,
    /// Media, container, and support conditions that do not depend on the element.
    pub(super) conditions: Vec<VariantApplication>,
}

/// Resolves all identity and prefix alternatives of one element condition. The caller
/// supplies whether the selector will be inserted into `:has()`. Global media and
/// support conditions remain guards.
///
/// Returns `None` when the condition is false. An empty ref set adds
/// `EmptyRefCondition`. Target-only conditions, declaration effects, selectors that
/// omit their subject, pseudo-elements, and nested `:has()` selectors add
/// `UnsupportedRefVariant`. A ref marker naming failure adds a fatal diagnostic.
pub(super) fn element_conditions(
    context: &IdentifierContext,
    theme: &ThemeIndex,
    condition: &RefCondition,
    inside_has: bool,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> Option<Vec<ElementCondition>> {
    let (_, applications) =
        collect_variant_and_applications(theme, &condition.prefix, lint_findings)?;
    if condition.refs.is_empty() {
        lint_findings.push(LintFinding::new(
            LintDiagnostic::EmptyRefCondition,
            condition.prefix.span,
        ));
        return None;
    }
    let paths = variant_paths(&applications);
    let mut alternatives = Vec::new();
    for definition in &condition.refs {
        let marker = match ref_marker_name(context, definition) {
            Ok(marker) => marker,
            Err(error) => {
                diagnostics.push(CompileError::new(error, definition.call_span));
                return None;
            }
        };
        for path in &paths {
            let Some(predicate) = reference_path(&marker, path, inside_has) else {
                lint_findings.push(LintFinding::new(
                    LintDiagnostic::UnsupportedRefVariant,
                    condition.prefix.span,
                ));
                return None;
            };
            alternatives.push(predicate);
        }
    }
    alternatives.sort_by(|a, b| {
        a.selector.cmp(&b.selector).then_with(|| {
            format_args!("{:?}", a.conditions)
                .to_lean_string()
                .cmp(&format_args!("{:?}", b.conditions).to_lean_string())
        })
    });
    Some(alternatives)
}

/// Builds an element selector from a marker and a resolved variant path. Returns
/// `None` for steps whose meaning requires a styled target or declaration effects.
fn reference_path(
    marker: &RefMarker,
    path: &[VariantStep<'_>],
    inside_has: bool,
) -> Option<ElementCondition> {
    let mut reference = format_args!(":where({})", marker.selector()).to_lean_string();
    let mut conditions = Vec::new();
    for step in path {
        match step {
            VariantStep::SelectorSuffix(suffix) => {
                reference.push_str(suffix);
            }
            VariantStep::SelectorTemplate(template)
            | VariantStep::BareSelector {
                selector: template, ..
            } if selector_uses_subject(template) => {
                reference = instantiate_selector(template, &reference);
            }
            VariantStep::SelectorRule {
                selector,
                declarations,
            } if declarations.is_empty() && selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
            }
            VariantStep::SelectorMedia {
                selector,
                condition,
            } if selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
                conditions.push(VariantApplication::Media((*condition).to_lean_str()));
            }
            VariantStep::SelectorMediaRule {
                selector,
                declarations,
                condition,
            } if declarations.is_empty() && selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
                conditions.push(VariantApplication::Media((*condition).to_lean_str()));
            }
            VariantStep::Media(condition) => {
                conditions.push(VariantApplication::Media((*condition).to_lean_str()));
            }
            VariantStep::Supports(condition) => {
                conditions.push(VariantApplication::Supports((*condition).to_lean_str()));
            }
            _ => {
                return None;
            }
        }
    }
    if !selector_is_element_condition(&reference, inside_has) {
        return None;
    }
    // `:is` binds the full selector to one element before an ancestor or containment
    // relation is added. Its ancestors may lie outside the containing scope.
    Some(ElementCondition {
        selector: format_args!(":is({reference})").to_lean_str(),
        conditions,
    })
}

#[cfg(test)]
mod reference_path_tests {
    use super::*;

    #[test]
    fn element_conditions_keep_global_guards_and_reject_target_conditions() {
        let marker = RefMarker::new("menu").unwrap();
        let path = [
            VariantStep::Media("(min-width: 40rem)"),
            VariantStep::Supports("(display:grid)"),
        ];
        let predicate = reference_path(&marker, &path, false).unwrap();
        assert_eq!(
            predicate.conditions,
            [
                VariantApplication::Media("(min-width: 40rem)".into()),
                VariantApplication::Supports("(display:grid)".into())
            ]
        );
        assert!(
            reference_path(&marker, &[VariantStep::Container("(width > 10px)")], false).is_none()
        );
        assert!(reference_path(&marker, &[VariantStep::StartingStyle], false).is_none());
    }

    #[test]
    fn element_predicates_preserve_complex_selectors_and_check_has_placement() {
        let marker = RefMarker::new("menu").unwrap();
        let predicate = reference_path(
            &marker,
            &[VariantStep::SelectorTemplate(".a &, .b &")],
            true,
        )
        .unwrap();
        assert_eq!(
            predicate.selector,
            ":is(.a :where(.menu), .b :where(.menu))"
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix(":has(.x)")], true).is_none()
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix(":has(.x)")], false).is_some()
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix("::before")], false).is_none()
        );
        assert!(
            reference_path(
                &marker,
                &[VariantStep::SelectorTemplate(".document")],
                false
            )
            .is_none()
        );
    }
}
