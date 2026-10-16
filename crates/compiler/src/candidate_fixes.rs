//! Source lints of one written utility that offer an edit: variants without an effect
//! and `var()` values without a type hint.
//!
//! [`detect`] receives the written text and the variant prefix of enclosing wrappers.
//! It reports only parts of the written text, because an edit can change only that
//! text. Hosts apply the edits through the fix actions of the findings.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{ThemeIndex, VariantRedundancy, redundant_variants, resolve_utility};
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, LintSettings, SourceEdit, SourceRange,
};
use stillwind_source::Span;
use stillwind_syntax::parse_candidate;

use crate::compiler::with_type_hint;

/// Maps the written text to the original source.
pub(crate) struct WrittenSource<'a> {
    /// Range of the whole written text, used when a part cannot be mapped.
    pub(crate) span: Span,
    /// Returns the source range of a byte range of the written text, or `None` when the
    /// source spells the text with escapes that do not map byte for byte.
    pub(crate) part: &'a dyn Fn(Range<usize>) -> Option<Span>,
    /// Returns the editable range of a source span, or `None` when it is unavailable.
    pub(crate) range: &'a dyn Fn(Span) -> Option<SourceRange>,
}

/// Returns the enabled `redundant-variant` and `prefer-typed-variables` findings for
/// `written`, the utility text that the author wrote under the variant prefix `prefix`.
///
/// A redundant variant is reported at its text and removed with its `:`. An untyped
/// variable is reported at its value, and the fix inserts the type hint of the
/// definition that receives it. A finding whose part cannot be mapped to the source
/// covers the whole written text and offers no fix.
pub(crate) fn detect(
    written: &str,
    prefix: &str,
    theme: &ThemeIndex,
    settings: &LintSettings,
    source: &WrittenSource<'_>,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let redundant = settings.enabled_for_detection(LintKind::RedundantVariant);
    let untyped = settings.enabled_for_detection(LintKind::PreferTypedVariables)
        && (written.contains("(--") || written.contains("var("));
    if !redundant && !untyped {
        return findings;
    }
    let raw = format_args!("{prefix}{written}").to_lean_str();
    let Some(candidate) = parse_candidate(&raw) else {
        return findings;
    };
    if redundant {
        for found in redundant_variants(&candidate, theme) {
            let Some(range) = written_range(&found.range, prefix.len()) else {
                continue;
            };
            let removal = range.start..range.end + 1;
            findings.push(finding(
                LintDiagnostic::RedundantVariant {
                    variant: found.variant.into(),
                    kept: found.kept.into(),
                    implied: found.reason == VariantRedundancy::Implied,
                },
                range,
                "Remove the redundant variant",
                removal,
                LeanStr::new(),
                source,
            ));
        }
    }
    if untyped
        && let Ok(Ok(resolved)) = resolve_utility(&raw, theme)
        && let Some(variable) = resolved.untyped_variable
        && let Some(range) = written_range(&variable.range, prefix.len())
        && let Some(replacement) =
            with_type_hint(&raw, prefix.len(), &variable.range, &variable.hint)
        && let Some(open) = written.get(range.start..range.start + 1)
    {
        let insertion = format_args!("{open}{}:", variable.hint).to_lean_str();
        findings.push(finding(
            LintDiagnostic::UntypedVariable {
                utility: written.to_lean_str(),
                hint: variable.hint,
                replacement,
            },
            range.clone(),
            "Add the type hint",
            range.start..range.start + 1,
            insertion,
            source,
        ));
    }
    findings
}

/// Returns `range` of the candidate text relative to the written text after `prefix`
/// bytes of wrapper variants, or `None` when it starts in the wrapper variants.
fn written_range(range: &Range<usize>, prefix: usize) -> Option<Range<usize>> {
    Some(range.start.checked_sub(prefix)?..range.end.checked_sub(prefix)?)
}

/// Returns a finding at `range` of the written text with a fix that replaces `edit`
/// with `replacement`, or without a fix when either range cannot be mapped.
fn finding(
    diagnostic: LintDiagnostic,
    range: Range<usize>,
    title: &str,
    edit: Range<usize>,
    replacement: LeanStr,
    source: &WrittenSource<'_>,
) -> LintFinding {
    let mut finding = LintFinding::new(diagnostic, (source.part)(range).unwrap_or(source.span));
    if let Some(range) = (source.part)(edit).and_then(|span| (source.range)(span)) {
        finding.actions.push(FixAction {
            title: title.into(),
            edits: vec![SourceEdit { range, replacement }],
        });
    }
    finding
}
