//! Duplicate and competing direct utilities under compatible source decisions.
//!
//! Candidate records group every declaration of one original token. Base and provider
//! comparisons remain in `properties`; identical shared declarations are harmless.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{properties_interfere, property_covers};
use stillwind_diagnostics::{Label, LabelScope, LintConfig, LintDiagnostic, LintFinding, LintKind};
use stillwind_source::JoinText;

use super::composition::ResolvedSupply;
use super::{ResolvedStyle, SupplyKind, UseId};

pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let settings = &style.root.lint_settings;
    let duplicate = settings.enabled_for_detection(LintKind::DuplicateUtility);
    let conflict = settings.enabled_for_detection(LintKind::ConflictingUtilities);
    if !duplicate && !conflict {
        return Vec::new();
    }
    let allow = matches!(settings.get(LintKind::ConflictingUtilities), Some(LintConfig::ConflictingUtilities(config)) if config.allow_shorthand_overrides);
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
            let diagnostic = if candidate == previous_candidate {
                if !duplicate {
                    continue;
                }
                LintDiagnostic::DuplicateUtility {
                    utility: candidate.clone(),
                }
            } else {
                if !conflict {
                    continue;
                }
                let overlaps = overlapping_properties(
                    declarations
                        .get(&current.supply.site)
                        .map_or(&[][..], Vec::as_slice),
                    declarations
                        .get(&previous.supply.site)
                        .map_or(&[][..], Vec::as_slice),
                    allow,
                );
                if overlaps.is_empty() {
                    continue;
                }
                LintDiagnostic::ConflictingUtilities {
                    previous: previous_candidate.clone(),
                    utility: candidate.clone(),
                    properties: (overlaps.into_iter().collect::<Vec<_>>().join_text(", ")).into(),
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
    allow: bool,
) -> BTreeSet<LeanStr> {
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
                || !properties_interfere(ap, bp)
                || ap == bp && av == bv && ai == bi
                || allow && (property_covers(ap, bp) || property_covers(bp, ap))
            {
                continue;
            }
            overlaps.insert(if ap == bp {
                ap.clone()
            } else {
                format_args!("{bp} and {ap}").to_lean_str()
            });
        }
    }
    overlaps
}
