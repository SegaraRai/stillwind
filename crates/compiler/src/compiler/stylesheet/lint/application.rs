//! Checks of the candidates of one `@apply` statement.
//!
//! The rule selector of the enclosing style rule is the anchor of each candidate, so
//! candidates resolve in [`VariantContext::Element`]. Candidates are compared only
//! with the other candidates of the same statement.

use std::collections::BTreeSet;
use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{declaration_ranges, properties_interfere, property_covers};
use stillwind_design_system::VariantContext;
use stillwind_diagnostics::{
    FixAction, Label, LabelScope, LintConfig, LintDiagnostic, LintFinding, LintKind,
    MarkerUseErrorKind, SourceEdit,
};
use stillwind_source::{JoinText, Span};

use super::analysis::Checker;
use crate::candidate_fixes::{WrittenSource, detect as detect_fixes};
use crate::compiler::literal::{resolved_candidate_lints, variant_selector_lints};
use crate::compiler::utilities::{
    CompiledUtility, marker_use_finding, resolve_utility_targets, unresolved_lint_kind,
    unresolved_utility_finding,
};

/// Checks the `candidates` of one `@apply` statement, which are byte ranges of the
/// checker's source.
///
/// `prefixes` holds the variant prefix of each enclosing `@variant` sequence. Each
/// candidate is resolved alone, as expansion resolves it. The rules that combine
/// conditions, `redundant-variant` and `unsatisfiable-variants`, also check the
/// candidate under each prefix.
pub(super) fn check_application(
    checker: &mut Checker<'_>,
    candidates: &[Range<usize>],
    prefixes: &[LeanString],
) {
    let source = checker.source;
    let theme = checker.theme;
    let settings = checker.settings;
    let mut resolved = Vec::new();
    for range in candidates {
        let raw = &source.text()[range.clone()];
        let span = checker.span(range.clone());
        match resolve_utility_targets(raw, theme, VariantContext::Element) {
            Ok(Ok(utilities)) => {
                if checker.enabled(LintKind::InvalidMarkerUse)
                    && let Some(finding) = marker_use_finding(
                        raw,
                        raw.len(),
                        span,
                        theme,
                        Some(MarkerUseErrorKind::Placement),
                    )
                {
                    checker.report(finding);
                }
                checker.report_all(variant_selector_lints(raw, theme, settings, span, 0));
                for prefix in prefixes {
                    let combined = format_args!("{prefix}{raw}").to_lean_string();
                    let utilities = if prefix.is_empty() {
                        Ok(Ok(utilities.clone()))
                    } else {
                        resolve_utility_targets(&combined, theme, VariantContext::Element)
                    };
                    if let Ok(Ok(utilities)) = utilities {
                        checker.report_all(resolved_candidate_lints(
                            &combined,
                            raw.len(),
                            &utilities,
                            theme,
                            settings,
                            span,
                        ));
                    }
                    let part = |part: Range<usize>| {
                        Some(checker.span(range.start + part.start..range.start + part.end))
                    };
                    let edit_range = |span| checker.edit_range(span);
                    let fixes = detect_fixes(
                        raw,
                        prefix,
                        theme,
                        settings,
                        &WrittenSource {
                            span,
                            part: &part,
                            range: &edit_range,
                        },
                    );
                    checker.report_all(fixes);
                }
                check_canonical(checker, raw, span);
                resolved.push((range.clone(), utilities));
            }
            Ok(Err(failure)) => {
                if checker.enabled(unresolved_lint_kind(&failure)) {
                    checker.report(unresolved_utility_finding(
                        raw,
                        raw.len(),
                        &failure,
                        span,
                        theme,
                    ));
                }
            }
            // Compilation reports a malformed requirement of the design system.
            Err(_) => {}
        }
    }
    check_pairs(checker, &resolved);
}

/// Reports `prefer-canonical-utilities` for `raw` at `span` with a fix that writes the
/// canonical spelling.
fn check_canonical(checker: &mut Checker<'_>, raw: &str, span: Span) {
    if !checker.enabled(LintKind::PreferCanonicalUtilities) {
        return;
    }
    let Ok(Some(canonical)) = checker.query.canonical_candidate(raw) else {
        return;
    };
    if canonical == raw {
        return;
    }
    let Some(range) = checker.edit_range(span) else {
        return;
    };
    let mut finding = LintFinding::new(LintDiagnostic::NonCanonicalUtility, span);
    finding.actions.push(FixAction {
        title: "Use canonical utility spelling".into(),
        edits: vec![SourceEdit {
            range,
            replacement: canonical,
        }],
    });
    checker.report(finding);
}

/// Reports `duplicate-utility` and `conflicting-utilities` for each candidate of
/// `resolved` that repeats or competes with an earlier one, with a label at the earlier
/// candidate.
///
/// Candidates compete when rules with the same variants and selector write
/// interfering properties with different values. With `allow-shorthand-overrides`, a
/// shorthand and a property that it covers do not compete.
fn check_pairs(checker: &mut Checker<'_>, resolved: &[(Range<usize>, Vec<CompiledUtility>)]) {
    let duplicate = checker.enabled(LintKind::DuplicateUtility);
    let conflict = checker.enabled(LintKind::ConflictingUtilities);
    if !duplicate && !conflict {
        return;
    }
    let allow = matches!(
        checker.settings.get(LintKind::ConflictingUtilities),
        Some(LintConfig::ConflictingUtilities(config)) if config.allow_shorthand_overrides
    );
    let text = checker.source.text();
    for (index, (range, utilities)) in resolved.iter().enumerate() {
        let raw = &text[range.clone()];
        for (previous_range, previous) in &resolved[..index] {
            let previous_raw = &text[previous_range.clone()];
            let diagnostic = if raw == previous_raw {
                if !duplicate {
                    continue;
                }
                LintDiagnostic::DuplicateUtility {
                    utility: raw.to_lean_str(),
                }
            } else {
                let overlaps = if conflict {
                    overlapping_properties(utilities, previous, allow)
                } else {
                    BTreeSet::new()
                };
                if overlaps.is_empty() {
                    continue;
                }
                LintDiagnostic::ConflictingUtilities {
                    previous: previous_raw.to_lean_str(),
                    utility: raw.to_lean_str(),
                    properties: overlaps
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join_text(", ")
                        .into(),
                }
            };
            let mut finding = LintFinding::new(diagnostic, checker.span(range.clone()));
            finding.info.labels.push(Label {
                scope: LabelScope::Range,
                span: checker.span(previous_range.clone()),
                message: Some("previous utility".into()),
            });
            checker.report(finding);
        }
    }
}

/// Returns the competing properties of `current` and `previous`, written as `a` or as
/// `b and a` when the properties differ.
fn overlapping_properties(
    current: &[CompiledUtility],
    previous: &[CompiledUtility],
    allow: bool,
) -> BTreeSet<LeanStr> {
    let mut overlaps = BTreeSet::new();
    for a in current {
        for b in previous.iter().filter(|b| {
            b.selector == a.selector
                && b.variants
                    .iter()
                    .map(|variant| &variant.raw)
                    .eq(a.variants.iter().map(|variant| &variant.raw))
        }) {
            for (ap, av) in declarations(&a.declaration) {
                for (bp, bv) in declarations(&b.declaration) {
                    if !properties_interfere(&ap, &bp)
                        || ap == bp && av == bv
                        || allow && (property_covers(&ap, &bp) || property_covers(&bp, &ap))
                    {
                        continue;
                    }
                    overlaps.insert(if ap == bp {
                        ap.to_lean_str()
                    } else {
                        format_args!("{bp} and {ap}").to_lean_str()
                    });
                }
            }
        }
    }
    overlaps
}

/// Returns the lowercase property and trimmed value, including any `!important`, of
/// each declaration of `css`.
fn declarations(css: &str) -> Vec<(LeanString, &str)> {
    declaration_ranges(css)
        .into_iter()
        .filter_map(|range| {
            let (property, value) = css[range].split_once(':')?;
            Some((property.trim().to_ascii_lowercase().into(), value.trim()))
        })
        .collect()
}
