//! Attachment and property interference checks over composed supplies.

use std::collections::BTreeSet;

use stillwind_css::properties_interfere;
use stillwind_diagnostics::{Label, LintDiagnostic, LintFinding};
use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, Span};

use super::composition::ResolvedSupply;
use super::{ResolvedStyle, RuleSubject, SupplyKind};

pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let mut reported = BTreeSet::new();
    for current in &style.supplies {
        if let SupplyKind::Provider { reference, name } = &current.supply.kind {
            let attached = style.supplies.iter().any(|other| {
                matches!(&other.supply.kind, SupplyKind::Marker(identity) if identity == reference)
                    && current.coexists(other)
            });
            if !attached
                && !style.unknown_can_coexist(current)
                && reported.insert((
                    "attachment",
                    current.supply.site.clone(),
                    current.supply.site.clone(),
                ))
            {
                findings.push(LintFinding::new(
                    LintDiagnostic::MissingRefAttachment { name: name.clone() },
                    local_span(style, current),
                ));
            }
        }
    }
    findings.extend(check_interference(style));
    findings
}

fn check_interference(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    if !style.supplies.iter().any(|s| {
        s.base.is_some()
            || matches!(
                &s.supply.kind,
                SupplyKind::Declaration { provider: true, .. }
            )
    }) {
        return Vec::new();
    }
    let mut findings = Vec::new();
    let mut reported = BTreeSet::new();
    for (index, current) in style.supplies.iter().enumerate() {
        let SupplyKind::Declaration {
            property: a,
            context: ac,
            provider: ap,
            gate: ag,
            ..
        } = &current.supply.kind
        else {
            continue;
        };
        for previous in &style.supplies[..index] {
            let SupplyKind::Declaration {
                property: b,
                context: bc,
                provider: bp,
                gate: bg,
                ..
            } = &previous.supply.kind
            else {
                continue;
            };
            if current.same_supply(previous)
                || !current.coexists(previous)
                || !properties_interfere(a, b)
            {
                continue;
            }
            let properties = describe_properties(a, b);
            if (*ap || *bp)
                && current.supply.site != previous.supply.site
                && ac.subject == RuleSubject::Target
                && bc.subject == RuleSubject::Target
                && !exclusive_markers(style, ag.as_ref(), bg.as_ref())
                && reported.insert((
                    "provider",
                    previous.supply.site.clone(),
                    current.supply.site.clone(),
                ))
            {
                findings.push(provider_finding(
                    style,
                    previous,
                    current,
                    properties.clone(),
                ));
            }
            if ac == bc
                && let Some(diagnostic) = base_interference(style, previous, current, properties)
                && reported.insert((
                    "base",
                    previous.supply.site.clone(),
                    current.supply.site.clone(),
                ))
            {
                findings.push(LintFinding::new(diagnostic, style.root.span));
            }
        }
    }
    findings
}

fn local_span(style: &ResolvedStyle<'_>, supply: &ResolvedSupply<'_>) -> Span {
    if supply.owner == style.root.identity {
        supply.supply.site.span
    } else {
        style.root.span
    }
}

fn exclusive_markers(
    style: &ResolvedStyle<'_>,
    a: Option<&IdentityId>,
    b: Option<&IdentityId>,
) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    if a == b {
        return false;
    }
    let a_sites: Vec<_> = style
        .supplies
        .iter()
        .filter(|s| matches!(&s.supply.kind,SupplyKind::Marker(id) if id == a))
        .collect();
    let b_sites: Vec<_> = style
        .supplies
        .iter()
        .filter(|s| matches!(&s.supply.kind,SupplyKind::Marker(id) if id == b))
        .collect();
    !a_sites.is_empty()
        && !b_sites.is_empty()
        && a_sites
            .iter()
            .all(|a| b_sites.iter().all(|b| !a.coexists(b)))
}

fn provider_finding(
    style: &ResolvedStyle<'_>,
    previous: &ResolvedSupply<'_>,
    current: &ResolvedSupply<'_>,
    properties: String,
) -> LintFinding {
    let mut finding = LintFinding::new(
        LintDiagnostic::ProviderConflict { properties },
        local_span(style, current),
    );
    finding.info.labels.push(Label {
        span: previous.supply.site.span,
        message: Some("overlapping property source".into()),
    });
    finding
}
fn imported_module(
    style: &ResolvedStyle<'_>,
    previous: &ResolvedSupply<'_>,
    current: &ResolvedSupply<'_>,
) -> ModuleId {
    if current.owner.module_id == style.root.identity.module_id {
        previous.owner.module_id.clone()
    } else {
        current.owner.module_id.clone()
    }
}

fn interference_kind(
    style: &ResolvedStyle<'_>,
    previous: &ResolvedSupply<'_>,
    current: &ResolvedSupply<'_>,
) -> &'static str {
    if current.owner.module_id == style.root.identity.module_id
        && previous.owner.module_id == style.root.identity.module_id
    {
        "order"
    } else {
        "imported"
    }
}

fn describe_properties(a: &str, b: &str) -> String {
    match a.cmp(b) {
        std::cmp::Ordering::Equal => a.into(),
        std::cmp::Ordering::Less => format!("{a} and {b}"),
        std::cmp::Ordering::Greater => format!("{b} and {a}"),
    }
}

fn base_interference(
    style: &ResolvedStyle<'_>,
    previous: &ResolvedSupply<'_>,
    current: &ResolvedSupply<'_>,
    properties: String,
) -> Option<LintDiagnostic> {
    let (Some(left), Some(right)) = (&previous.base, &current.base) else {
        return [&previous, &current]
            .iter()
            .any(|s| s.base.is_some() && s.owner.module_id != style.root.identity.module_id)
            .then(|| LintDiagnostic::ImportedBaseInterference {
                module: imported_module(style, previous, current),
                properties,
            });
    };
    if left == right || style.reaches(left, right) || style.reaches(right, left) {
        return None;
    }
    Some(
        if interference_kind(style, previous, current) == "imported" {
            LintDiagnostic::ImportedBaseInterference {
                module: imported_module(style, previous, current),
                properties,
            }
        } else {
            LintDiagnostic::AmbiguousBaseOrder { properties }
        },
    )
}
