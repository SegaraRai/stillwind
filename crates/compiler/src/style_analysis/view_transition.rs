//! Participant name and class checks with inherited supplies and selection conditions.

use stillwind_diagnostics::{LintDiagnostic, LintFinding};

use super::composition::{ResolvedSupply, can_omit};
use super::{ResolvedStyle, RuleSubject, SupplyKind};

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
            LintDiagnostic::SplitViewTransitionClasses,
            style.root.span,
        ));
    }
    findings
}

/// Finds the maximum compatible set of distinct authored name supplies. Shared
/// CSS alternatives and repeated base paths contribute one source. Decisions are
/// compared symbolically; runtime outcomes are not enumerated in advance.
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
