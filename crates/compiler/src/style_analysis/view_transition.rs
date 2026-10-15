//! Participant name and class checks with inherited supplies and selection conditions.

use stillwind_diagnostics::{LintDiagnostic, LintFinding};

use super::composition::{ResolvedSupply, can_omit};
use super::{ResolvedStyle, RuleSubject, SupplyKind};

/// Returns name and class findings for transitions that affect the styled element.
/// A compatible name supply, unknown selector, or unknown base prevents a missing
/// name finding; names are not required on every runtime path. Duplicate names use
/// the largest compatible set of distinct source uses. Class findings detect when
/// a derived list can omit an inherited transition under compatible source decisions.
/// The caller applies lint policy and source suppression to the returned findings.
pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let transitions: Vec<_> = style
        .supplies
        .iter()
        .filter(|s| {
            matches!(&s.supply.kind, SupplyKind::Transition { context, .. }
                if context.subject == RuleSubject::Target)
        })
        .collect();
    if transitions.is_empty() {
        return Vec::new();
    }
    let names: Vec<_> = style
        .supplies
        .iter()
        .filter(|s| {
            matches!(&s.supply.kind, SupplyKind::Declaration { property, context, .. }
                if property == "view-transition-name" && context.subject == RuleSubject::Target)
        })
        .collect();
    let mut findings = Vec::new();
    let missing_name = transitions.iter().any(|transition| {
        !style.unknown_can_coexist(transition)
            && !names.iter().any(|name| transition.coexists(name))
            && !style.supplies.iter().any(|name| {
                transition.coexists(name)
                    && matches!(&name.supply.kind, SupplyKind::Declaration { property, context, .. }
                        if property == "view-transition-name" && context.subject == RuleSubject::Unknown)
            })
    });
    if missing_name {
        findings.push(LintFinding::new(
            LintDiagnostic::MissingViewTransitionName,
            style.root.span,
        ));
    }
    let count = maximum_names(&names);
    if count > 1 {
        findings.push(LintFinding::new(
            LintDiagnostic::DuplicateViewTransitionName { count },
            style.root.span,
        ));
    }
    let loses_class = transitions.iter().any(|base| {
        let SupplyKind::Transition {
            identity: lost,
            context: base_context,
        } = &base.supply.kind else {
            return false;
        };
        style.supplies.iter().any(|writer| {
            let SupplyKind::Declaration { property, context, .. } = &writer.supply.kind else {
                return false;
            };
            if property != "view-transition-class"
                || writer.owner == base.owner
                || !style.reaches(&writer.owner, &base.owner)
                || context.subject != RuleSubject::Target
            {
                return false;
            }
            let retained = transitions
                .iter()
                .copied()
                .filter(|retained| {
                    retained.owner == writer.owner
                        && matches!(&retained.supply.kind, SupplyKind::Transition { identity, context }
                            if identity == lost && context == base_context)
                })
                .collect::<Vec<_>>();
            can_omit(&[base, writer], &retained)
        })
    });
    if loses_class {
        findings.push(LintFinding::new(
            LintDiagnostic::SplitViewTransitionClass,
            style.root.span,
        ));
    }
    findings
}

/// Returns the maximum number of distinct name supplies that can coexist.
/// `names` contains name declarations with consistent finite source decisions.
/// CSS alternatives and repeated base paths share one source when their owner,
/// occurrence, and utility ordinal match. Search prunes sets that cannot improve
/// the current maximum; it does not infer correlations between different occurrences.
fn maximum_names(names: &[&ResolvedSupply<'_>]) -> usize {
    let mut best = 0;
    let mut pending = vec![(0, Vec::new())];
    while let Some((index, selected)) = pending.pop() {
        best = best.max(selected.len());
        if index == names.len() || selected.len() + names.len() - index <= best {
            continue;
        }
        pending.push((index + 1, selected.clone()));
        if selected.iter().all(|&other: &usize| {
            let a = names[other];
            let b = names[index];
            a.coexists(b)
                && !(a.owner == b.owner
                    && a.supply.site == b.supply.site
                    && a.supply.ordinal == b.supply.ordinal)
        }) {
            let mut selected = selected;
            selected.push(index);
            pending.push((index + 1, selected));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;
    use stillwind_semantic::{AssetKind, IdentityId};
    use stillwind_source::ModuleId;

    use super::super::test_support as f;
    use super::super::{RuleContext, StyleEffect, StyleResolver, StyleSupply};
    use super::*;

    fn transition(module: &str, offset: u32, context: RuleContext) -> StyleEffect {
        f::supply(
            module,
            offset,
            SupplyKind::Transition {
                identity: IdentityId::new(
                    AssetKind::ViewTransition,
                    ModuleId::from("definitions"),
                    0,
                    10,
                    "v",
                ),
                context,
            },
        )
    }

    fn name(module: &str, offset: u32) -> StyleEffect {
        f::supply(
            module,
            offset,
            f::declaration("view-transition-name", f::context("&")),
        )
    }

    #[test]
    fn name_maximum_counts_sources_and_utility_ordinals_instead_of_css_alternatives() {
        let root = f::effects(
            "root",
            StyleEffect::All(vec![name("root", 1), name("root", 1), name("root", 2)]),
        );
        let mut resolver = StyleResolver::new([&root]);
        let style = resolver.resolve(root.identity()).unwrap();
        assert_eq!(maximum_names(&[]), 0);
        assert_eq!(maximum_names(&style.supplies.iter().collect::<Vec<_>>()), 2);
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                name("root", 1),
                StyleEffect::Supply(StyleSupply {
                    site: f::site("root", 1),
                    ordinal: 0,
                    kind: f::declaration("view-transition-name", f::context("&:hover")),
                }),
                StyleEffect::Supply(StyleSupply {
                    site: f::site("root", 1),
                    ordinal: 1,
                    kind: f::declaration("view-transition-name", f::context("&")),
                }),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        let style = resolver.resolve(root.identity()).unwrap();
        assert_eq!(maximum_names(&style.supplies.iter().collect::<Vec<_>>()), 2);
    }

    #[test]
    fn name_maximum_selects_compatible_source_arms_and_finite_candidates() {
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                f::branch("root", 1, name("root", 2), name("root", 3)),
                name("root", 4),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        let style = resolver.resolve(root.identity()).unwrap();
        assert_eq!(maximum_names(&style.supplies.iter().collect::<Vec<_>>()), 2);
        let root = f::effects(
            "root",
            StyleEffect::Choice {
                site: f::site("root", 1),
                alternatives: vec![name("root", 2), name("root", 3), name("root", 4)],
            },
        );
        let mut resolver = StyleResolver::new([&root]);
        let style = resolver.resolve(root.identity()).unwrap();
        assert_eq!(maximum_names(&style.supplies.iter().collect::<Vec<_>>()), 1);
    }

    #[test]
    fn participant_checks_ignore_other_elements_and_accept_possible_names() {
        let other = RuleContext {
            subject: RuleSubject::Other,
            ..f::context("& > span")
        };
        let root = f::effects("root", transition("root", 1, other.clone()));
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                transition("root", 1, f::context("&")),
                f::supply("root", 2, f::declaration("view-transition-name", other)),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap())[0].kind(),
            LintKind::MissingViewTransitionName
        );
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                transition("root", 1, f::context("&")),
                f::branch("root", 2, name("root", 3), StyleEffect::All(Vec::new())),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn missing_names_only_consider_compatible_unknown_contributions() {
        let participant = transition("root", 2, f::context("&"));
        let unknown = StyleEffect::Base(f::identity("missing"));
        let root = f::effects(
            "root",
            f::branch("root", 1, participant.clone(), unknown.clone()),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap())[0].kind(),
            LintKind::MissingViewTransitionName
        );
        let root = f::effects("root", StyleEffect::All(vec![participant.clone(), unknown]));
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let uncertain = RuleContext {
            subject: RuleSubject::Unknown,
            ..f::context(":is(&, span)")
        };
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                participant,
                f::supply("root", 3, f::declaration("view-transition-name", uncertain)),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn duplicate_name_finding_records_the_maximum_simultaneous_count() {
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                transition("root", 1, f::context("&")),
                name("root", 2),
                f::branch(
                    "root",
                    3,
                    StyleEffect::All(vec![name("root", 4), name("root", 5)]),
                    name("root", 6),
                ),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        let findings = check(&resolver.resolve(root.identity()).unwrap());
        assert!(matches!(
            findings[0].diagnostic,
            LintDiagnostic::DuplicateViewTransitionName { count: 3 }
        ));
        assert_eq!(findings[0].info.labels[0].span, root.span);
    }

    #[test]
    fn inherited_class_retention_must_cover_every_compatible_source_arm() {
        let base = f::effects(
            "base",
            StyleEffect::All(vec![
                transition("base", 1, f::context("&")),
                name("base", 2),
            ]),
        );
        for (retained, loses) in [
            (StyleEffect::All(Vec::new()), true),
            (
                f::branch(
                    "root",
                    3,
                    transition("root", 4, f::context("&")),
                    StyleEffect::All(Vec::new()),
                ),
                true,
            ),
            (
                f::branch(
                    "root",
                    3,
                    transition("root", 4, f::context("&")),
                    transition("root", 5, f::context("&")),
                ),
                false,
            ),
        ] {
            let root = f::effects(
                "root",
                StyleEffect::All(vec![
                    StyleEffect::Base(base.identity.clone()),
                    f::supply(
                        "root",
                        2,
                        f::declaration("view-transition-class", f::context("&")),
                    ),
                    retained,
                ]),
            );
            let mut resolver = StyleResolver::new([&base, &root]);
            let findings = check(&resolver.resolve(root.identity()).unwrap());
            assert_eq!(
                findings
                    .iter()
                    .any(|f| f.kind() == LintKind::SplitViewTransitionClass),
                loses
            );
        }
    }

    #[test]
    fn class_replacement_requires_a_derived_writer_on_the_styled_element() {
        let base = f::effects(
            "base",
            StyleEffect::All(vec![
                transition("base", 1, f::context("&")),
                name("base", 2),
            ]),
        );
        let sibling = f::effects(
            "sibling",
            f::supply(
                "sibling",
                2,
                f::declaration("view-transition-class", f::context("&")),
            ),
        );
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                StyleEffect::Base(sibling.identity.clone()),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &sibling, &root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let other = RuleContext {
            subject: RuleSubject::Other,
            ..f::context("& > span")
        };
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                f::supply("root", 2, f::declaration("view-transition-class", other)),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &root]);
        assert_eq!(
            check(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }
}
