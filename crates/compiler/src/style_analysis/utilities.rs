//! Duplicate and competing direct utilities under compatible source decisions.
//!
//! Candidate records group every declaration of one original token. Base and provider
//! comparisons remain in `properties`; identical shared declarations are harmless.

use std::collections::BTreeSet;

use stillwind_css::{properties_interfere, property_covers};
use stillwind_diagnostics::{Label, LabelScope, LintConfig, LintDiagnostic, LintFinding, LintKind};

use super::{ResolvedStyle, SupplyKind, UseId};

pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    let settings = &style.root.lint_settings;
    let duplicate = settings.enabled_for_detection(LintKind::DuplicateUtilities);
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
                    style,
                    &current.supply.site,
                    &previous.supply.site,
                    allow,
                );
                if overlaps.is_empty() {
                    continue;
                }
                LintDiagnostic::ConflictingUtilities {
                    previous: previous_candidate.clone(),
                    utility: candidate.clone(),
                    properties: overlaps.into_iter().collect::<Vec<_>>().join(", "),
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

fn overlapping_properties(
    style: &ResolvedStyle<'_>,
    current: &UseId,
    previous: &UseId,
    allow: bool,
) -> BTreeSet<String> {
    let declarations = |site: &UseId| {
        let site = site.clone();
        style
            .supplies
            .iter()
            .filter(move |s| s.owner == style.root.identity && s.supply.site == site)
    };
    let mut overlaps = BTreeSet::new();
    for a in declarations(current) {
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
        for b in declarations(previous) {
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
                format!("{bp} and {ap}")
            });
        }
    }
    overlaps
}
