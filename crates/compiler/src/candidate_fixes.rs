//! Source lints of one written utility that offer an edit: variants without an effect,
//! `var()` values without a type hint, and variant selectors that test the `class`
//! attribute for utility names.
//!
//! [`detect`] receives the written text and the variant prefix of enclosing wrappers.
//! It reports only parts of the written text, because an edit can change only that
//! text. Hosts apply the edits through the fix actions of the findings.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{ClassMatcher, selector_class_tests};
use stillwind_design_system::{ThemeIndex, VariantRedundancy, redundant_variants, resolve_utility};
use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, LintSettings, SourceEdit, SourceRange,
};
use stillwind_source::Span;
use stillwind_syntax::{Candidate, parse_candidate};

use crate::compiler::{utility_class_test, with_type_hint};

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

/// Returns the enabled `redundant-variant`, `prefer-typed-variables`, and
/// `utility-class-selector` findings for `written`, the utility text that the author
/// wrote under the variant prefix `prefix`.
///
/// A redundant variant is reported at its text and removed with its `:`. An untyped
/// variable is reported at its value, and the fix inserts the type hint of the
/// definition that receives it. A variant that tests the `class` attribute for a utility
/// name is reported at its text; see [`class_test_fix`] for the variants that have a fix.
/// A finding whose part cannot be mapped to the source covers the whole written text and
/// offers no fix.
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
    let class_tests = settings.enabled_for_detection(LintKind::UtilityClassSelector)
        && (written.contains("class") || written.contains('.'));
    if !redundant && !untyped && !class_tests {
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
    if class_tests {
        findings.extend(class_test_findings(&candidate, prefix.len(), theme, source));
    }
    if untyped
        && let Ok(Ok(resolved)) = resolve_utility(&raw, theme)
        && let Some(variable) = resolved.diagnostic_data.untyped_variable
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

/// Returns a `utility-class-selector` finding for each variant of `candidate` that is
/// written after the first `prefix` bytes and tests the `class` attribute for a utility
/// name.
fn class_test_findings(
    candidate: &Candidate,
    prefix: usize,
    theme: &ThemeIndex,
    source: &WrittenSource<'_>,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let utility = candidate
        .raw
        .get(
            candidate
                .variants
                .iter()
                .map(|variant| variant.raw.len() + 1)
                .sum::<usize>()..,
        )
        .unwrap_or_default();
    let mut offset = 0;
    for variant in &candidate.variants {
        let range = offset..offset + variant.raw.len();
        offset = range.end + 1;
        let Some(range) = written_range(&range, prefix) else {
            continue;
        };
        let Some(test) = utility_class_test(variant, theme) else {
            continue;
        };
        let diagnostic = LintDiagnostic::UtilityClassSelector { test };
        findings.push(match class_test_fix(&variant.raw, utility) {
            Some(replacement) => finding(
                diagnostic,
                range.clone(),
                "Apply the default with zero specificity",
                range,
                replacement,
                source,
            ),
            None => LintFinding::new(diagnostic, (source.part)(range).unwrap_or(source.span)),
        });
    }
    findings
}

/// Returns the replacement for an arbitrary variant `[<base>:not([class*=<root>-])]` or
/// `[<base>:not([class^=<root>-])]` whose `<base>` contains `&`, when `utility`, the
/// utility text after the variants, starts with `<root>-`. The variant applies the
/// utility unless the element has a utility of the same root, such as
/// `[&_svg:not([class*='size-'])]:size-4`.
///
/// The replacement `[:where(<base>)]` gives the rule zero specificity instead, so a class
/// on the element that sets the same property wins. Returns `None` for other variants.
fn class_test_fix(variant: &str, utility: &str) -> Option<LeanStr> {
    let inner = variant.strip_prefix('[')?.strip_suffix(']')?;
    let (base, test) = inner.strip_suffix(')')?.rsplit_once(":not(")?;
    if !base.contains('&') {
        return None;
    }
    let [class_test] = selector_class_tests(test).try_into().ok()?;
    let root = class_test.value.as_str();
    let utility = utility.trim_start_matches(['!', '-']);
    (class_test.range == (0..test.len())
        && matches!(
            class_test.matcher,
            ClassMatcher::Substring | ClassMatcher::Prefix
        )
        && root.ends_with('-')
        && utility.starts_with(root))
    .then(|| format_args!("[:where({base})]").to_lean_str())
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
