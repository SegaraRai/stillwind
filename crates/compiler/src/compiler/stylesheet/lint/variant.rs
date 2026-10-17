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
use crate::compiler::literal::variant_list_lints;
use crate::compiler::utilities::{unresolved_lint_kind, unresolved_utility_finding};

/// Checks each sequence of the `@variant` prelude at the byte range `prelude` and
/// returns the prefixes for the contents of the block.
///
/// `prefixes` holds the prefix of each enclosing sequence. The result holds one prefix
/// for each pair of an enclosing prefix and a sequence that resolves. When no sequence
/// resolves, expansion discards the block, and the result is `prefixes` so that the
/// contents are still checked.
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
        let Some(variants) = resolved_sequence(checker, written, range.clone()) else {
            continue;
        };
        let mut offset = range.start;
        let spans = variants
            .iter()
            .map(|variant| {
                let span = checker.span(offset..offset + variant.raw.len());
                offset += variant.raw.len() + 1;
                span
            })
            .collect::<Vec<_>>();
        checker.report_all(variant_list_lints(
            &variants,
            &spans,
            checker.theme,
            checker.settings,
        ));
        for prefix in prefixes {
            check_stack(checker, prefix, range.clone());
            inner.push(format_args!("{prefix}{written}:").to_lean_string());
        }
    }
    if inner.is_empty() {
        prefixes.to_vec()
    } else {
        inner
    }
}

/// Returns the variants of the sequence `written` at `range` when each one resolves.
///
/// Otherwise reports `invalid-variant` for text that is not a variant list, or the
/// finding of the first variant that does not resolve, and returns `None`.
fn resolved_sequence(
    checker: &mut Checker<'_>,
    written: &str,
    range: Range<usize>,
) -> Option<Vec<Variant>> {
    let span = checker.span(range);
    let Some(variants) = parse_variant_sequence(written) else {
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
    };
    let mut start = 0;
    for variant in &variants {
        if let Err(failure) = resolve_variant(variant, checker.theme) {
            let failure = failure.shifted(start);
            let failure = UtilityFailure {
                range: failure.range,
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
    Some(variants)
}

/// Reports `redundant-variant` and `unsatisfiable-variants` for the sequence at `range`
/// after the enclosing `prefix`.
///
/// A redundant variant that another variant of the sequence accompanies is removed
/// with its separating `:`. A sequence of one redundant variant has no fix, because
/// removing it would leave the block without a prelude.
fn check_stack(checker: &mut Checker<'_>, prefix: &str, range: Range<usize>) {
    let redundant = checker.enabled(LintKind::RedundantVariant);
    let unsatisfiable = checker.enabled(LintKind::UnsatisfiableVariants);
    if !redundant && !unsatisfiable {
        return;
    }
    let written = &checker.source.text()[range.clone()];
    // Both checks resolve only the variants of the candidate. The grammar requires
    // a utility suffix, whose name is never looked up.
    let stack = format_args!("{prefix}{written}:utility").to_lean_string();
    let Some(candidate) = parse_candidate(&stack) else {
        return;
    };
    // Returns the source range of the part of `part` that lies in the sequence.
    let local = |part: &Range<usize>| {
        let end = part.end.checked_sub(prefix.len()).filter(|end| *end > 0)?;
        let start = part.start.saturating_sub(prefix.len());
        Some(range.start + start..range.start + end.min(written.len()))
    };
    if redundant {
        for found in redundant_variants(&candidate, checker.theme) {
            if found.range.start < prefix.len() {
                continue;
            }
            let Some(variant) = local(&found.range) else {
                continue;
            };
            let removal = if variant.end < range.end {
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
