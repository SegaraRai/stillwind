//! Attachment and property interference checks over composed supplies.

use std::collections::BTreeSet;

use stillwind_css::properties_interfere;
use stillwind_diagnostics::{Label, LabelScope, LintDiagnostic, LintFinding, LintKind};
use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, Span};

use super::composition::ResolvedSupply;
use super::{ResolvedStyle, RuleSubject, SupplyKind};

/// Returns missing attachment and property interference findings for `style`.
/// Compatible markers and unknown paths prevent missing-attachment findings.
/// Each source pair contributes at most one finding per interference rule.
/// Hosts apply lint policy and suppression after these findings are returned.
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
                    LintKind::RequireRefAttachment,
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

/// Compares coexisting declarations after excluding repeated original supplies.
/// Provider conflicts require target declarations and compatible marker gates.
/// Base-order checks require identical expanded CSS contexts. Both comparisons
/// use Webref property relationships and report the first overlap for each source pair.
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
                    LintKind::ConflictingProviderProperties,
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
                    LintKind::AmbiguousBaseOrder,
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

/// Returns the original use span for a direct supply or the consuming call's span
/// for an inherited supply, so source suppression runs in the consuming module.
fn local_span(style: &ResolvedStyle<'_>, supply: &ResolvedSupply<'_>) -> Span {
    if supply.owner == style.root.identity {
        supply.supply.site.span
    } else {
        style.root.span
    }
}

/// Returns whether two distinct required markers cannot be attached together.
/// Missing marker occurrences, equal identities, and absent requirements return
/// `false`; exclusivity requires known occurrences for both markers and no
/// compatible pair among them.
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

/// Builds a provider conflict at the current use or its consuming call.
/// `properties` describes an overlap already verified by the caller. The previous
/// original use becomes a related label, including when it belongs to another module.
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
        scope: LabelScope::Range,
        span: previous.supply.site.span,
        message: Some("overlapping property source".into()),
    });
    finding
}

/// Returns the producer module for an overlap involving an imported supply.
/// At least one owner must differ from the root's module. When both are imported,
/// the current supply's owner identifies the diagnostic's producer.
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

/// Classifies an overlapping pair as local order ambiguity or imported interference.
/// Classification uses the original supply owners, including transitive bases.
fn imported_interference(
    style: &ResolvedStyle<'_>,
    previous: &ResolvedSupply<'_>,
    current: &ResolvedSupply<'_>,
) -> bool {
    !(current.owner.module_id == style.root.identity.module_id
        && previous.owner.module_id == style.root.identity.module_id)
}

/// Returns one property name for equal inputs or an ordered pair for an overlap.
/// Inputs are decoded property names; lexicographic order stabilizes diagnostics.
fn describe_properties(a: &str, b: &str) -> String {
    match a.cmp(b) {
        std::cmp::Ordering::Equal => a.into(),
        std::cmp::Ordering::Less => format!("{a} and {b}"),
        std::cmp::Ordering::Greater => format!("{b} and {a}"),
    }
}

/// Returns the order diagnostic for an already verified declaration overlap.
/// Callers must first verify source coexistence, property overlap, and equal CSS
/// contexts. Direct declarations only warn against an imported base. Two distinct
/// direct bases warn unless one transitively consumes the other.
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
    Some(if imported_interference(style, previous, current) {
        LintDiagnostic::ImportedBaseInterference {
            module: imported_module(style, previous, current),
            properties,
        }
    } else {
        LintDiagnostic::AmbiguousBaseOrder { properties }
    })
}

#[cfg(test)]
mod tests {
    use stillwind_semantic::AssetKind;

    use super::super::test_support as f;
    use super::super::{RuleContext, StyleEffect, StyleEffects, StyleResolver};
    use super::*;

    fn reference(label: &str) -> IdentityId {
        IdentityId::new(AssetKind::Ref, ModuleId::from("root"), 80, 90, label)
    }

    fn provider(property: &str, gate: Option<IdentityId>) -> SupplyKind {
        let mut kind = f::declaration(property, f::context("&"));
        if let SupplyKind::Declaration {
            provider,
            gate: requirement,
            ..
        } = &mut kind
        {
            *provider = true;
            *requirement = gate;
        }
        kind
    }

    fn local_style(offset: u32, root: StyleEffect) -> StyleEffects {
        let mut effects = f::effects("root", root);
        effects.identity = IdentityId::new(
            AssetKind::Style,
            ModuleId::from("root"),
            offset,
            offset + 10,
            "",
        );
        effects.span = Span::new(effects.span.file, offset, offset + 10);
        effects
    }

    #[test]
    fn attachment_findings_require_compatible_identity_and_keep_consuming_spans() {
        let r = reference("r");
        let requirement = f::supply(
            "base",
            5,
            SupplyKind::Provider {
                reference: r.clone(),
                name: "r".into(),
            },
        );
        let base = f::effects("base", requirement);
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                f::supply("root", 2, SupplyKind::Marker(reference("other"))),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &root]);
        let findings = check(&resolver.resolve(root.identity()).unwrap());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind(), LintKind::RequireRefAttachment);
        assert_eq!(findings[0].info.labels[0].span, root.span);
        let attached = f::effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                f::supply("root", 2, SupplyKind::Marker(r)),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &attached]);
        assert_eq!(
            check(&resolver.resolve(attached.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn exclusive_markers_require_known_distinct_and_disjoint_attachment_paths() {
        let a = reference("a");
        let b = reference("b");
        let root = f::effects(
            "root",
            f::branch(
                "root",
                1,
                f::supply("root", 2, SupplyKind::Marker(a.clone())),
                f::supply("root", 3, SupplyKind::Marker(b.clone())),
            ),
        );
        let mut resolver = StyleResolver::new([&root]);
        let style = resolver.resolve(root.identity()).unwrap();
        assert!(exclusive_markers(&style, Some(&a), Some(&b)));
        assert!(!exclusive_markers(&style, Some(&a), Some(&a)));
        assert!(!exclusive_markers(
            &style,
            Some(&a),
            Some(&reference("missing"))
        ));
        assert!(!exclusive_markers(&style, None, Some(&b)));
        let together = f::effects(
            "root",
            StyleEffect::All(vec![
                root.root.clone(),
                f::supply("root", 4, SupplyKind::Marker(a.clone())),
            ]),
        );
        let mut resolver = StyleResolver::new([&together]);
        assert!(!exclusive_markers(
            &resolver.resolve(together.identity()).unwrap(),
            Some(&a),
            Some(&b)
        ));
    }

    #[test]
    fn provider_conflicts_use_property_relationships_and_original_related_labels() {
        let base = f::effects("base", f::supply("base", 5, provider("margin", None)));
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                f::supply("root", 2, f::declaration("margin-top", f::context("&"))),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &root]);
        let style = resolver.resolve(root.identity()).unwrap();
        let findings = check_interference(&style);
        assert_eq!(
            findings.iter().map(LintFinding::kind).collect::<Vec<_>>(),
            vec![
                LintKind::ConflictingProviderProperties,
                LintKind::ImportedBaseInterference
            ]
        );
        let provider = &findings[0];
        assert_eq!(provider.info.labels[0].span, f::site("root", 2).span);
        assert!(
            provider
                .info
                .labels
                .iter()
                .any(|label| label.span == f::site("base", 5).span)
        );
    }

    #[test]
    fn provider_conflicts_skip_other_subjects_and_exclusive_marker_gates() {
        let a = reference("a");
        let b = reference("b");
        let marker_paths = f::branch(
            "root",
            1,
            f::supply("root", 2, SupplyKind::Marker(a.clone())),
            f::supply("root", 3, SupplyKind::Marker(b.clone())),
        );
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                marker_paths,
                f::supply("root", 4, provider("container-type", Some(a))),
                f::supply("root", 5, provider("container-type", Some(b))),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check_interference(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let mut other_context = f::context("& > span");
        other_context.subject = RuleSubject::Other;
        let root = f::effects(
            "root",
            StyleEffect::All(vec![
                f::supply("root", 4, provider("container-type", None)),
                f::supply("root", 5, f::declaration("container-type", other_context)),
            ]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check_interference(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn repeated_sources_and_opposite_arms_do_not_add_provider_conflicts() {
        let one = f::supply("root", 2, provider("anchor-name", None));
        let other = f::supply("root", 3, f::declaration("anchor-name", f::context("&")));
        let repeated = f::effects(
            "root",
            StyleEffect::All(vec![one.clone(), one.clone(), other.clone(), other.clone()]),
        );
        let exclusive = f::effects("exclusive", f::branch("root", 1, one, other));
        let mut resolver = StyleResolver::new([&repeated, &exclusive]);
        assert_eq!(
            check_interference(&resolver.resolve(repeated.identity()).unwrap()).len(),
            1
        );
        assert_eq!(
            check_interference(&resolver.resolve(exclusive.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let plain = f::effects(
            "plain",
            StyleEffect::All(vec![
                f::supply("plain", 1, f::declaration("padding", f::context("&"))),
                f::supply("plain", 2, f::declaration("padding", f::context("&"))),
            ]),
        );
        let mut resolver = StyleResolver::new([&plain]);
        assert_eq!(
            check_interference(&resolver.resolve(plain.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn base_classification_respects_local_imported_and_ordered_ancestry() {
        let left = local_style(
            10,
            f::supply("root", 11, f::declaration("padding", f::context("&"))),
        );
        let right = local_style(
            30,
            f::supply("root", 31, f::declaration("padding", f::context("&"))),
        );
        let root = local_style(
            50,
            StyleEffect::All(vec![
                StyleEffect::Base(left.identity.clone()),
                StyleEffect::Base(right.identity.clone()),
            ]),
        );
        let mut resolver = StyleResolver::new([&left, &right, &root]);
        assert_eq!(
            check_interference(&resolver.resolve(root.identity()).unwrap())[0].kind(),
            LintKind::AmbiguousBaseOrder
        );
        let ordered = local_style(
            30,
            StyleEffect::All(vec![
                StyleEffect::Base(left.identity.clone()),
                right.root.clone(),
            ]),
        );
        let mut resolver = StyleResolver::new([&left, &ordered, &root]);
        assert_eq!(
            check_interference(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
        let imported = f::effects("imported", right.root.clone());
        let imported_root = local_style(
            50,
            StyleEffect::All(vec![
                StyleEffect::Base(left.identity.clone()),
                StyleEffect::Base(imported.identity.clone()),
            ]),
        );
        let mut resolver = StyleResolver::new([&left, &imported, &imported_root]);
        let findings = check_interference(&resolver.resolve(imported_root.identity()).unwrap());
        assert!(
            matches!(&findings[0].diagnostic, LintDiagnostic::ImportedBaseInterference { module, .. } if module == &ModuleId::from("imported"))
        );
    }

    #[test]
    fn base_overlaps_require_equal_expanded_selectors_and_at_rules() {
        for second_context in [
            f::context("&:hover"),
            RuleContext {
                at_rules: vec!["@media (width > 10px)".into()],
                ..f::context("&")
            },
        ] {
            let left = local_style(
                10,
                f::supply("root", 11, f::declaration("padding", f::context("&"))),
            );
            let right = local_style(
                30,
                f::supply("root", 31, f::declaration("padding", second_context)),
            );
            let root = local_style(
                50,
                StyleEffect::All(vec![
                    StyleEffect::Base(left.identity.clone()),
                    StyleEffect::Base(right.identity.clone()),
                ]),
            );
            let mut resolver = StyleResolver::new([&left, &right, &root]);
            assert_eq!(
                check_interference(&resolver.resolve(root.identity()).unwrap()),
                [] as [LintFinding; 0]
            );
        }
    }

    #[test]
    fn direct_local_writes_do_not_add_order_findings_against_local_bases() {
        let base = local_style(
            10,
            f::supply("root", 11, f::declaration("padding", f::context("&"))),
        );
        let root = local_style(
            50,
            StyleEffect::All(vec![
                StyleEffect::Base(base.identity.clone()),
                f::supply("root", 51, f::declaration("padding", f::context("&"))),
            ]),
        );
        let mut resolver = StyleResolver::new([&base, &root]);
        assert_eq!(
            check_interference(&resolver.resolve(root.identity()).unwrap()),
            [] as [LintFinding; 0]
        );
    }

    #[test]
    fn diagnostic_property_names_are_stable_for_equal_and_reversed_pairs() {
        assert_eq!(describe_properties("padding", "padding"), "padding");
        assert_eq!(
            describe_properties("margin-top", "margin"),
            "margin and margin-top"
        );
        assert_eq!(
            describe_properties("margin", "margin-top"),
            "margin and margin-top"
        );
    }
}
