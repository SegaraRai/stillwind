//! Lint dispatch over one resolved style and rule classification for replacement.
//!
//! Hosts resolve a call with `StyleResolver`, run `check_style`, and apply lint policy
//! and source suppression themselves. These checks reason about contributions that
//! can occur together; they do not compute the browser's complete cascade.

use stillwind_diagnostics::{LintFinding, LintKind};

use super::{ResolvedStyle, properties, utilities, view_transition};

/// Returns composition-dependent lint findings for a result from `StyleResolver`.
/// The findings retain diagnostic source spans, including labels in imported bases.
/// A compatible unknown path prevents absence findings and still permits known
/// conflicts. This function does not apply severity settings or source suppression.
pub fn check_style(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let mut findings = properties::check(style);
    findings.extend(view_transition::check(style));
    findings.extend(utilities::check(style));
    findings
}

/// Returns whether `kind` is computed from a style call's composed supplies.
/// Hosts use this classification to replace these findings after a base changes
/// while retaining findings produced by other compilation stages.
pub const fn is_style_analysis_lint(kind: LintKind) -> bool {
    matches!(
        kind,
        LintKind::RequireRefAttachment
            | LintKind::DuplicateUtilities
            | LintKind::ConflictingUtilities
            | LintKind::ConflictingProviderProperties
            | LintKind::RequireViewTransitionName
            | LintKind::SingleViewTransitionName
            | LintKind::SingleViewTransitionClass
            | LintKind::AmbiguousBaseOrder
            | LintKind::ImportedBaseInterference
    )
}

#[cfg(test)]
mod tests {
    use stillwind_semantic::{AssetKind, IdentityId};
    use stillwind_source::ModuleId;

    use super::super::test_support::{context, declaration, effects, supply};
    use super::super::{StyleEffect, StyleResolver, SupplyKind};
    use super::*;

    #[test]
    fn classification_selects_exactly_the_rules_dispatched_here() {
        let mut actual: Vec<_> = LintKind::ALL
            .iter()
            .copied()
            .filter(|kind| is_style_analysis_lint(*kind))
            .collect();
        let mut expected = vec![
            LintKind::DuplicateUtilities,
            LintKind::ConflictingUtilities,
            LintKind::RequireRefAttachment,
            LintKind::ConflictingProviderProperties,
            LintKind::RequireViewTransitionName,
            LintKind::SingleViewTransitionName,
            LintKind::SingleViewTransitionClass,
            LintKind::AmbiguousBaseOrder,
            LintKind::ImportedBaseInterference,
        ];
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn dispatch_combines_property_and_transition_findings_without_dropping_conflicts() {
        let root = effects(
            "root",
            StyleEffect::All(vec![
                supply(
                    "root",
                    1,
                    SupplyKind::Provider {
                        reference: IdentityId::new(
                            AssetKind::Ref,
                            ModuleId::from("root"),
                            20,
                            30,
                            "r",
                        ),
                        name: "r".into(),
                    },
                ),
                supply(
                    "root",
                    2,
                    SupplyKind::Transition {
                        identity: IdentityId::new(
                            AssetKind::ViewTransition,
                            ModuleId::from("root"),
                            30,
                            40,
                            "v",
                        ),
                        context: context("&"),
                    },
                ),
                supply("root", 3, declaration("view-transition-name", context("&"))),
                supply("root", 4, declaration("view-transition-name", context("&"))),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        let findings = check_style(&resolver.resolve(root.identity()).unwrap());
        assert_eq!(
            findings.iter().map(LintFinding::kind).collect::<Vec<_>>(),
            vec![
                LintKind::RequireRefAttachment,
                LintKind::SingleViewTransitionName
            ]
        );
    }
}
