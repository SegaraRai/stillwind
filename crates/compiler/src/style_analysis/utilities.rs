//! Duplicate and competing direct utilities under compatible source decisions.
//!
//! Candidate records group every declaration of one original token. Base and provider
//! comparisons remain in [`super::properties`]; identical shared declarations are
//! harmless.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_css::{properties_interfere_in, property_covers, reads_written_value};
use stillwind_diagnostics::{
    ConflictingUtilitiesConfig, Label, LabelScope, LintDiagnostic, LintFinding, LintKind,
};

use super::composition::ResolvedSupply;
use super::{DeclarationValue, ResolvedStyle, RuleContext, SupplyKind, UseId};
use crate::compiler::utility_name;
use crate::utility_conflicts::{PropertyPairs, conflict_notes, conflict_policy, describe_overlaps};

pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let settings = &style.root.lint_settings;
    let duplicate = settings.enabled_for_detection(LintKind::DuplicateUtility);
    let conflict = settings.enabled_for_detection(LintKind::ConflictingUtilities);
    if !duplicate && !conflict {
        return Vec::new();
    }
    let policy = conflict_policy(settings);
    let utilities: Vec<_> = style
        .supplies
        .iter()
        .filter(|supply| {
            supply.owner == style.root.identity
                && matches!(supply.supply.kind, SupplyKind::Utility { .. })
        })
        .collect();
    let declarations = declarations_by_site(style);
    let mut findings = Vec::new();
    let mut reported = BTreeSet::new();
    for (index, current) in utilities.iter().enumerate() {
        let SupplyKind::Utility {
            candidate,
            contexts,
        } = &current.supply.kind
        else {
            continue;
        };
        for previous in &utilities[..index] {
            let SupplyKind::Utility {
                candidate: previous_candidate,
                contexts: previous_contexts,
            } = &previous.supply.kind
            else {
                continue;
            };
            if current.supply.site == previous.supply.site
                || !current.coexists(previous)
                || !contexts
                    .iter()
                    .any(|context| previous_contexts.contains(context))
            {
                continue;
            }
            let mut notes = Vec::new();
            let diagnostic = if candidate == previous_candidate {
                if !duplicate {
                    continue;
                }
                LintDiagnostic::DuplicateUtility {
                    utility: candidate.clone(),
                }
            } else {
                if !conflict
                    || policy.ignores_utility(utility_name(candidate))
                    || policy.ignores_utility(utility_name(previous_candidate))
                {
                    continue;
                }
                let overlaps = overlapping_properties(
                    declarations
                        .get(&current.supply.site)
                        .map_or(&[][..], Vec::as_slice),
                    declarations
                        .get(&previous.supply.site)
                        .map_or(&[][..], Vec::as_slice),
                    &policy,
                );
                if overlaps.is_empty() {
                    continue;
                }
                notes = conflict_notes(&overlaps, policy.allow_shorthand_overrides);
                LintDiagnostic::ConflictingUtilities {
                    previous: previous_candidate.clone(),
                    utility: candidate.clone(),
                    properties: describe_overlaps(&overlaps),
                }
            };
            if reported.insert((
                previous.supply.site.clone(),
                current.supply.site.clone(),
                diagnostic.kind(),
            )) {
                let mut finding = LintFinding::new(diagnostic, current.supply.site.span);
                finding.info.labels.push(Label {
                    scope: LabelScope::Range,
                    span: previous.supply.site.span,
                    message: Some("previous utility".into()),
                });
                finding.info.notes = notes;
                findings.push(finding);
            }
        }
    }
    findings
}

fn declarations_by_site<'s, 'a>(
    style: &'s ResolvedStyle<'a>,
) -> BTreeMap<&'s UseId, Vec<&'s ResolvedSupply<'a>>> {
    let mut declarations = BTreeMap::new();
    for supply in &style.supplies {
        if supply.owner == style.root.identity
            && matches!(
                supply.supply.kind,
                SupplyKind::Declaration {
                    provider: false,
                    ..
                }
            )
        {
            declarations
                .entry(&supply.supply.site)
                .or_insert_with(Vec::new)
                .push(supply);
        }
    }
    declarations
}

fn overlapping_properties(
    current: &[&ResolvedSupply<'_>],
    previous: &[&ResolvedSupply<'_>],
    policy: &ConflictingUtilitiesConfig,
) -> PropertyPairs {
    let mut overlaps = BTreeSet::new();
    for a in current {
        let SupplyKind::Declaration {
            property: ap,
            value: av,
            important: ai,
            context: ac,
            provider: false,
            ..
        } = &a.supply.kind
        else {
            continue;
        };
        for b in previous {
            let SupplyKind::Declaration {
                property: bp,
                value: bv,
                important: bi,
                context: bc,
                provider: false,
                ..
            } = &b.supply.kind
            else {
                continue;
            };
            if ac != bc
                || !a.coexists(b)
                || !properties_interfere_in(ap, bp, &policy.writing_modes)
                || ap == bp && av == bv && ai == bi
                || policy.allow_shorthand_overrides
                    && (property_covers(ap, bp) || property_covers(bp, ap))
                || reads_site_value(ap, av, bp, bv, bc, previous)
                || reads_site_value(bp, bv, ap, av, ac, current)
            {
                continue;
            }
            overlaps.insert((bp.clone(), ap.clone()));
        }
    }
    overlaps
}

/// Returns whether the reader declaration computes the value of the writer declaration
/// in either order, because the writer's utility also sets the custom property that the
/// reader copies. `writer_site` holds every declaration of the writer's utility.
fn reads_site_value(
    reader_property: &str,
    reader_value: &DeclarationValue,
    writer_property: &str,
    writer_value: &DeclarationValue,
    writer_context: &RuleContext,
    writer_site: &[&ResolvedSupply<'_>],
) -> bool {
    let (DeclarationValue::Css(reader_value), DeclarationValue::Css(writer_value)) =
        (reader_value, writer_value)
    else {
        return false;
    };
    reads_written_value(
        reader_property,
        reader_value,
        writer_property,
        writer_value,
        |name| {
            writer_site
                .iter()
                .find_map(|supply| match &supply.supply.kind {
                    SupplyKind::Declaration {
                        property,
                        value: DeclarationValue::Css(value),
                        context,
                        ..
                    } if property.as_str() == name && context == writer_context => {
                        Some(value.as_str())
                    }
                    _ => None,
                })
        },
    )
}
