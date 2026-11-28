//! Checks of the variant sequences of one `@variant` prelude.
//!
//! Nested `@variant` blocks apply their conditions together, so `redundant-variant`
//! and `unsatisfiable-variants` check each sequence after the sequences of the
//! enclosing blocks. Findings cover only the text of the checked sequence.

use std::ops::Range;

use lean_string::{LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    UtilityFailure, UtilityFailureReason, VariantRedundancy, redundant_variants, resolve_variant,
    unsatisfiable_variants,
};
use stillwind_diagnostics::{FixAction, LintDiagnostic, LintFinding, LintKind, SourceEdit};
use stillwind_syntax::{Variant, parse_candidate};

use super::super::authored_directives::{parse_variant_sequence, variant_sequences};
use super::analysis::Checker;
use crate::compiler::expand_prefix;
use crate::compiler::literal::{host_selector_lint, variant_list_lints};
use crate::compiler::utilities::{unresolved_lint_kind, unresolved_utility_finding};

/// One resolved sequence from a `@variant` prelude, after expanding any `or(...)`
/// alternatives.
struct ResolvedVariantSequence {
    /// Expanded variant prefix without a trailing `:`.
    text: LeanString,
    /// Parsed variants of `text`, each resolved against the active theme.
    variants: Vec<Variant>,
}

/// Checks each sequence of the `@variant` prelude at the byte range `prelude` and
/// returns the prefixes for the contents of the block.
///
/// `prefixes` holds the prefix of each enclosing sequence. The result holds one prefix
/// for each pair of an enclosing prefix and a sequence that resolves (after expanding
/// any `or(...)` alternatives). When no sequence resolves, expansion discards the block,
/// and the result is `prefixes` so that the contents are still checked.
pub(super) fn check_variant_block(
    checker: &mut Checker<'_>,
    prelude: Range<usize>,
    prefixes: &[LeanString],
) -> Vec<LeanString> {
    let source = checker.source;
    let mut inner = Vec::new();
    for sequence in variant_sequences(&source.text()[prelude.clone()]) {
        let range = prelude.start + sequence.start..prelude.start + sequence.end;
        let written = &source.text()[range.clone()];
        let Some((resolved, expanded)) = resolved_sequence(checker, written, range.clone()) else {
            continue;
        };
        for item in resolved {
            let spans = if expanded {
                vec![checker.span(range.clone()); item.variants.len()]
            } else {
                let mut offset = range.start;
                item.variants
                    .iter()
                    .map(|variant| {
                        let span = checker.span(offset..offset + variant.raw.len());
                        offset += variant.raw.len() + 1;
                        span
                    })
                    .collect::<Vec<_>>()
            };
            checker.report_all(variant_list_lints(
                &item.variants,
                &spans,
                checker.theme,
                checker.settings,
            ));
            checker.report_all(host_selector_lint(
                &item.variants,
                &spans,
                checker.theme,
                checker.settings,
            ));
            for prefix in prefixes {
                check_stack(checker, prefix, &item.text, range.clone(), !expanded);
                inner.push(format_args!("{prefix}{}:", item.text).to_lean_string());
            }
        }
    }
    if inner.is_empty() {
        prefixes.to_vec()
    } else {
        inner
    }
}

/// Returns the resolved sequences of `written` at `range` (expanding any `or(...)`
/// alternatives) and whether alternative expansion rewrote the spelling.
///
/// Otherwise reports `invalid-variant` for text that is not a variant list or fails
/// alternative expansion, or the finding of the first variant that does not resolve, and
/// returns `None`.
fn resolved_sequence(
    checker: &mut Checker<'_>,
    written: &str,
    range: Range<usize>,
) -> Option<(Vec<ResolvedVariantSequence>, bool)> {
    let span = checker.span(range);
    if parse_variant_sequence(written).is_none() {
        if checker.enabled(LintKind::InvalidVariant) {
            checker.report(LintFinding::new(
                LintDiagnostic::InvalidVariant {
                    variant: written.to_lean_str(),
                    detail: "a `@variant` sequence must be variants separated by `:`".into(),
                },
                span,
            ));
        }
        return None;
    }
    let prefixes = match expand_prefix(&format_args!("{written}:").to_lean_string(), checker.theme)
    {
        Ok(prefixes) => prefixes,
        Err(failure) => {
            if checker.enabled(unresolved_lint_kind(&failure)) {
                checker.report(unresolved_utility_finding(
                    written,
                    written.len(),
                    &failure,
                    span,
                    checker.theme,
                ));
            }
            return None;
        }
    };
    let expanded = prefixes.len() > 1 || prefixes[0].trim_end_matches(':') != written;
    let mut resolved = Vec::with_capacity(prefixes.len());
    for prefix in prefixes {
        let text = prefix.trim_end_matches(':');
        let variants = parse_variant_sequence(text)?;
        let mut start = 0;
        for variant in &variants {
            if let Err(failure) = resolve_variant(variant, checker.theme) {
                let failure_range = if expanded {
                    0..written.len()
                } else {
                    failure.range.start + start..failure.range.end + start
                };
                let failure = UtilityFailure {
                    range: failure_range,
                    reason: UtilityFailureReason::Variant(failure.reason),
                };
                if checker.enabled(unresolved_lint_kind(&failure)) {
                    checker.report(unresolved_utility_finding(
                        written,
                        written.len(),
                        &failure,
                        span,
                        checker.theme,
                    ));
                }
                return None;
            }
            start += variant.raw.len() + 1;
        }
        resolved.push(ResolvedVariantSequence {
            text: text.to_lean_string(),
            variants,
        });
    }
    Some((resolved, expanded))
}

/// Reports `redundant-variant` and `unsatisfiable-variants` for `sequence` at `range`
/// after the enclosing `prefix`.
///
/// When `fixable` is true, a redundant variant that another variant of the sequence
/// accompanies is removed with its separating `:`. Expanded `or(...)` sequences report
/// on the full sequence range without a removal edit. A sequence of one redundant
/// variant also has no fix, because removing it would leave the block without a prelude.
fn check_stack(
    checker: &mut Checker<'_>,
    prefix: &str,
    sequence: &str,
    range: Range<usize>,
    fixable: bool,
) {
    let redundant = checker.enabled(LintKind::RedundantVariant);
    let unsatisfiable = checker.enabled(LintKind::UnsatisfiableVariants);
    if !redundant && !unsatisfiable {
        return;
    }
    // Both checks resolve only the variants of the candidate. The grammar requires
    // a utility suffix, whose name is never looked up.
    let stack = format_args!("{prefix}{sequence}:utility").to_lean_string();
    let Some(candidate) = parse_candidate(&stack) else {
        return;
    };
    // Returns the source range of the part of `part` that lies in the sequence.
    let local = |part: &Range<usize>| {
        if !fixable {
            return (part.end > prefix.len()).then(|| range.clone());
        }
        let end = part.end.checked_sub(prefix.len()).filter(|end| *end > 0)?;
        let start = part.start.saturating_sub(prefix.len());
        Some(range.start + start..range.start + end.min(sequence.len()))
    };
    if redundant {
        for found in redundant_variants(&candidate, checker.theme) {
            if found.range.start < prefix.len() {
                continue;
            }
            let Some(variant) = local(&found.range) else {
                continue;
            };
            let removal = if !fixable {
                None
            } else if variant.end < range.end {
                Some(variant.start..variant.end + 1)
            } else if variant.start > range.start {
                Some(variant.start - 1..variant.end)
            } else {
                None
            };
            let mut finding = LintFinding::new(
                LintDiagnostic::RedundantVariant {
                    variant: found.variant.into(),
                    kept: found.kept.into(),
                    implied: found.reason == VariantRedundancy::Implied,
                },
                checker.span(variant),
            );
            if let Some(range) =
                removal.and_then(|removal| checker.edit_range(checker.span(removal)))
            {
                finding.actions.push(FixAction {
                    title: "Remove the redundant variant".into(),
                    edits: vec![SourceEdit {
                        range,
                        replacement: "".into(),
                    }],
                });
            }
            checker.report(finding);
        }
    }
    if unsatisfiable
        && let Some(found) = unsatisfiable_variants(&candidate, checker.theme)
        && let Some(part) = local(&found.range)
    {
        let [first, second] = found.conditions;
        checker.report(LintFinding::new(
            LintDiagnostic::UnsatisfiableVariants {
                variants: stack
                    .get(found.range.clone())
                    .unwrap_or_default()
                    .to_lean_str(),
                first,
                second,
                branches: u32::try_from(found.branches).unwrap_or(u32::MAX),
            },
            checker.span(part),
        ));
    }
}
