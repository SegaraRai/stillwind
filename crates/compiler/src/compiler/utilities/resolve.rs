//! Candidate resolution into compiled utilities.
//!
//! One candidate can produce several targets, for example a utility that expands to
//! more than one rule, and each target becomes its own [`CompiledUtility`] sharing the
//! candidate's parsed variants. A candidate that does not resolve keeps the design
//! system's explanation, and [`unresolved_utility_finding`] turns it into the lint
//! finding for the failing part of the candidate.

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    InvalidPropertyValue, InvalidValueReason, NestedUtilityRule, ResolvedUtilityTarget, ThemeIndex,
    UtilityArbitraryUse, UtilityFailure, UtilityFailureReason, UtilityOrder,
    UtilityResolutionError, UtilityTokenUse, VariableTypeSource, VariantApplication,
    VariantContext, check_candidate_variants, resolve_utility,
};
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};
use stillwind_source::Span;
use stillwind_syntax::{Variant, parse_candidate};

/// One resolved utility rule, ready to be ordered and rendered.
#[derive(Clone, Debug)]
pub(crate) struct CompiledUtility {
    /// Source occurrences and value selections retained for diagnostics.
    pub(in crate::compiler) diagnostic_data: CompiledUtilityDiagnosticData,
    /// Whether the rule starts from its class selector or an element selected by a
    /// variant.
    pub(in crate::compiler) context: VariantContext,
    /// Parsed variants in candidate order, resolved through the registry unless lowered
    /// applications are present.
    pub(in crate::compiler) variants: Vec<Variant>,
    /// Prelowered alternative paths, used instead of registry resolution when nonempty.
    /// Steps retain application order.
    pub(in crate::compiler) variant_applications: Vec<Vec<VariantApplication>>,
    /// Serialized declaration-list content without the outer rule braces.
    pub(in crate::compiler) declaration: LeanStr,
    /// Custom-property registration names required by this utility, independent of CSS
    /// references.
    pub(in crate::compiler) required_properties: Vec<LeanStr>,
    /// Optional selector template whose ampersands denote the generated class selector.
    pub(in crate::compiler) selector: Option<LeanStr>,
    /// Optional container query prelude without @container or braces.
    pub(in crate::compiler) container: Option<LeanStr>,
    /// Conditional utility rules in resolved emission order.
    pub(in crate::compiler) nested: Vec<NestedUtilityRule>,
    /// Snapshot-selected declaration metadata or authored registration position.
    pub(in crate::compiler) order: UtilityOrder,
    /// Original candidate spelling for ordering and diagnostics. Empty for synthetic
    /// utility rules.
    pub(in crate::compiler) candidate: LeanStr,
}

/// Source occurrences and value checks shared by the rules of one utility candidate.
#[derive(Clone, Debug, Default)]
pub(in crate::compiler) struct CompiledUtilityDiagnosticData {
    /// Candidate range, or `None` when no source occurrence is attached. Literal
    /// lowering initially uses decoded byte offsets, which the emitter maps to the
    /// original source.
    pub(in crate::compiler) span: Option<Span>,
    /// Direct selections preserved before inline theme values expand.
    pub(in crate::compiler) token_uses: Vec<UtilityTokenUse>,
    /// Accepted captures before source values expand into declaration text.
    pub(in crate::compiler) arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// Ranges of variants written in the utility literal, in candidate order after
    /// skipping `wrapper_variant_count`. They use the same offset basis as `span`.
    pub(in crate::compiler) variant_spans: Vec<Span>,
    /// Leading variants already checked at their wrapper source locations.
    pub(in crate::compiler) wrapper_variant_count: usize,
    /// Value that its property rejects. The declarations emit it as written.
    pub(in crate::compiler) invalid_value: Option<InvalidPropertyValue>,
}

/// Resolves a candidate and returns its first target, for callers that render a single
/// rule.
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
pub(in crate::compiler) fn resolve_first_target(
    utility: &str,
    theme: &ThemeIndex,
) -> Result<Result<CompiledUtility, UtilityFailure>, UtilityResolutionError> {
    Ok(
        resolve_utility_targets(utility, theme, VariantContext::Element)?
            .map(|mut targets| targets.swap_remove(0)),
    )
}

/// Resolves a candidate into every rule it expands to, in emission order.
///
/// Every variant must also resolve; an absent CSS registration never drops its guard
/// and turns the candidate into an unconditional utility. In
/// [`VariantContext::Document`], a variant must also select the element to style, as
/// [`check_candidate_variants`] describes. The inner error explains why the candidate
/// does not resolve; the caller decides whether that is a lint finding, an external
/// class name, or a diagnostic. A failing variant is reported before a failing utility
/// name, because it comes first in the candidate text.
///
/// Each target retains the complete candidate's diagnostic selections. Source ranges
/// remain unset until source lowering attaches an occurrence.
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
pub(crate) fn resolve_utility_targets(
    utility: &str,
    theme: &ThemeIndex,
    context: VariantContext,
) -> Result<Result<Vec<CompiledUtility>, UtilityFailure>, UtilityResolutionError> {
    let resolved = match resolve_utility(utility, theme)? {
        Ok(resolved) => resolved,
        Err(failure) => {
            let variants = parse_candidate(utility).map_or(Ok(()), |candidate| {
                check_candidate_variants(&candidate, theme, VariantContext::Element)
            });
            return Ok(Err(variants.err().unwrap_or(failure)));
        }
    };
    if let Err(failure) = check_candidate_variants(&resolved.candidate, theme, context) {
        return Ok(Err(failure));
    }
    let mut targets = vec![ResolvedUtilityTarget {
        declaration: resolved.declaration,
        selector: resolved.selector,
        nested: resolved.nested,
    }];
    targets.extend(resolved.additional);
    Ok(Ok(targets
        .into_iter()
        .map(|target| CompiledUtility {
            diagnostic_data: CompiledUtilityDiagnosticData {
                span: None,
                token_uses: resolved.diagnostic_data.token_uses.clone(),
                arbitrary_uses: resolved.diagnostic_data.arbitrary_uses.clone(),
                invalid_value: resolved.diagnostic_data.invalid_value.clone(),
                ..CompiledUtilityDiagnosticData::default()
            },
            context,
            variants: resolved.candidate.variants.clone(),
            variant_applications: Vec::new(),
            declaration: target.declaration,
            required_properties: resolved.required_properties.clone(),
            selector: target.selector,
            container: None,
            nested: target.nested,
            order: resolved.order.clone(),
            candidate: utility.to_lean_str(),
        })
        .collect()))
}

/// Returns the lint finding for a candidate that does not resolve.
///
/// `candidate` is the resolved text and `span` covers its last `written` bytes, which
/// the author wrote at this location; earlier bytes come from enclosing variant
/// wrappers. The finding covers the failing part of the written text. It covers all of
/// `span` when the failure lies in the wrapper text, or when `span` is not `written`
/// bytes long because the source spells the text with escapes. Unknown names select
/// `unknown-utility` or `unknown-variant` and carry a spelling suggestion when one
/// resolves. Other failures select `invalid-utility` or `invalid-variant`.
pub(crate) fn unresolved_utility_finding(
    candidate: &str,
    written: usize,
    failure: &UtilityFailure,
    span: Span,
    theme: &ThemeIndex,
) -> LintFinding {
    let prefix = candidate.len().saturating_sub(written);
    let utility = candidate[prefix..].to_lean_str();
    let detail = failure.reason.to_lean_string().into();
    let suggestion = || {
        failure
            .suggested_candidate(candidate, theme)
            .and_then(|suggestion| suggestion.get(prefix..).map(LeanStr::from))
    };
    let range = failure
        .range
        .start
        .checked_sub(prefix)
        .zip(failure.range.end.checked_sub(prefix));
    let span = range.map_or(span, |(start, end)| sub_span(span, written, start, end));
    let part = || {
        range.map_or_else(
            || utility.clone(),
            |(start, end)| utility.get(start..end).unwrap_or_default().to_lean_str(),
        )
    };
    let diagnostic = match unresolved_lint_kind(failure) {
        LintKind::UnknownVariant => LintDiagnostic::UnknownVariant {
            variant: part(),
            detail,
            suggestion: suggestion(),
        },
        LintKind::InvalidVariant => LintDiagnostic::InvalidVariant {
            variant: part(),
            detail,
        },
        LintKind::MissingDocumentTarget => LintDiagnostic::MissingDocumentTarget {
            candidate: part(),
            detail,
        },
        LintKind::UnknownUtility => LintDiagnostic::UnknownUtility {
            utility,
            detail,
            suggestion: suggestion(),
        },
        _ => LintDiagnostic::InvalidUtility { utility, detail },
    };
    LintFinding::new(diagnostic, span)
}

/// Returns the rule that reports `failure`: `missing-document-target` for a
/// document-level candidate without an element to style, `unknown-utility` or
/// `unknown-variant` for a name that does not exist, and `invalid-utility` or
/// `invalid-variant` otherwise.
pub(crate) const fn unresolved_lint_kind(failure: &UtilityFailure) -> LintKind {
    if let UtilityFailureReason::MissingDocumentTarget { .. } = failure.reason {
        return LintKind::MissingDocumentTarget;
    }
    match (failure.reason.is_variant(), failure.reason.is_unknown()) {
        (true, true) => LintKind::UnknownVariant,
        (true, false) => LintKind::InvalidVariant,
        (false, true) => LintKind::UnknownUtility,
        (false, false) => LintKind::InvalidUtility,
    }
}

/// Returns the `invalid-property-value` finding for a candidate that emits `value`.
///
/// `candidate`, `written`, and `span` have the meanings that
/// [`unresolved_utility_finding`] describes. When the property was chosen as a fallback
/// between value types, or when it rejects the known type of a variable, the finding
/// lists the written text with each applicable type hint added to the value.
pub(crate) fn invalid_property_value_finding(
    candidate: &str,
    written: usize,
    value: &InvalidPropertyValue,
    span: Span,
) -> LintFinding {
    let prefix = candidate.len().saturating_sub(written);
    let span = written_span(candidate.len(), written, &value.range, span);
    let utility = candidate.get(prefix..).unwrap_or(candidate).to_lean_str();
    let hinted = |hints: &[LeanStr]| {
        hints
            .iter()
            .filter_map(|hint| with_type_hint(candidate, prefix, &value.range, hint))
            .collect()
    };
    let diagnostic = match &value.reason {
        InvalidValueReason::Grammar {
            fallback_type_hints,
        } => LintDiagnostic::InvalidPropertyValue {
            alternatives: hinted(fallback_type_hints),
            utility,
            property: value.property.clone(),
            value: value.value.clone(),
        },
        InvalidValueReason::VariableType(mismatch) => LintDiagnostic::VariableTypeMismatch {
            alternatives: hinted(&mismatch.type_hints),
            utility,
            property: value.property.clone(),
            variable: mismatch.variable.clone(),
            value_type: mismatch.value_type.clone(),
            namespace: match &mismatch.source {
                VariableTypeSource::Theme(namespace) => Some(namespace.clone()),
                VariableTypeSource::Registration => None,
            },
        },
        InvalidValueReason::OpacityModifier => LintDiagnostic::OpacityModifierOutOfRange {
            modifier: candidate
                .get(value.range.clone())
                .unwrap_or_default()
                .to_lean_str(),
            percentage: value.value.clone(),
        },
    };
    LintFinding::new(diagnostic, span)
}

/// Returns `candidate` from byte `prefix` with `hint` added to the bracketed or
/// parenthesized value at `range`, such as `bg-(color:--x)` for `bg-(--x)`. Returns
/// `None` when the value is not delimited.
pub(crate) fn with_type_hint(
    candidate: &str,
    prefix: usize,
    range: &Range<usize>,
    hint: &str,
) -> Option<LeanStr> {
    let value = candidate.get(range.clone())?;
    let (open, inner, close) = if let Some(inner) = value.strip_prefix('[') {
        ('[', inner.strip_suffix(']')?, ']')
    } else {
        ('(', value.strip_prefix('(')?.strip_suffix(')')?, ')')
    };
    let before = candidate.get(prefix..range.start)?;
    let after = candidate.get(range.end..)?;
    Some(format_args!("{before}{open}{hint}:{inner}{close}{after}").to_lean_str())
}

/// Returns the part of `span` that covers `range` of a candidate whose last `written`
/// of `length` bytes the author wrote at `span`. A range that starts in the earlier
/// bytes is cut at the written text, and a range that lies wholly in them returns
/// `span`. See [`sub_span`] for escaped text.
pub(crate) fn written_span(
    length: usize,
    written: usize,
    range: &Range<usize>,
    span: Span,
) -> Span {
    let prefix = length.saturating_sub(written);
    match range.end.checked_sub(prefix) {
        Some(end) if end > 0 => sub_span(span, written, range.start.saturating_sub(prefix), end),
        _ => span,
    }
}

/// Returns the part of `span` from byte `start` to byte `end` of its `written` bytes of
/// text, or `span` itself when its length differs from `written`.
fn sub_span(span: Span, written: usize, start: usize, end: usize) -> Span {
    if span.len() != u32::try_from(written).ok() {
        return span;
    }
    let offset = |position: usize| {
        span.start
            .0
            .saturating_add(u32::try_from(position).unwrap_or(u32::MAX))
            .min(span.end.0)
    };
    Span::new(span.file, offset(start), offset(end))
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn unresolved_findings_cover_the_failing_part_of_the_written_text() {
        let theme = ThemeIndex::default();
        let span = Span::new(FileId(1), 10, 16);
        let failure = resolve_utility_targets("hover:", &theme, VariantContext::Element)
            .unwrap()
            .err()
            .unwrap();
        let finding = unresolved_utility_finding("hover:", 6, &failure, span, &theme);
        assert_eq!(finding.kind(), LintKind::InvalidUtility);
        assert_eq!(finding.labels[0].span, span);
        let failure = resolve_utility_targets("foo:x", &theme, VariantContext::Element)
            .unwrap()
            .err()
            .unwrap();
        let finding =
            unresolved_utility_finding("foo:x", 5, &failure, Span::new(FileId(1), 10, 15), &theme);
        assert_eq!(finding.kind(), LintKind::UnknownVariant);
        assert_eq!(finding.labels[0].span, Span::new(FileId(1), 10, 13));
        let finding =
            unresolved_utility_finding("foo:x", 1, &failure, Span::new(FileId(1), 14, 15), &theme);
        assert_eq!(finding.labels[0].span, Span::new(FileId(1), 14, 15));
        let escaped = Span::new(FileId(1), 10, 25);
        let finding = unresolved_utility_finding("foo:x", 5, &failure, escaped, &theme);
        assert_eq!(finding.labels[0].span, escaped);
    }
}
