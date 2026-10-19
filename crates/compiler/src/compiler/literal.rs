//! Utility literal resolution into canonical class inputs and CSS contributions.
//!
//! [`LiteralContribution::resolve`] omits unresolved candidates and reports lint
//! findings while applying semantic wrappers to resolved utilities.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    MarkerUse, ThemeIndex, UtilityFailureReason, VariantApplication, VariantContext,
    is_tailwind_relationship_candidate, marker_use, repeated_subject_selector, resolve_variant,
    unsatisfiable_variants, utility_value_range,
};
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintDiagnostic, LintFinding, LintKind, LintSettings,
    MarkerUseErrorKind,
};
use stillwind_source::{ReplaceText, Span, Spanned};
use stillwind_syntax::{Utility, Variant, parse_candidate};

use super::conditionals::{UnsupportedCondition, check_candidate};
use super::lexer::lex_utilities;
use super::style_variants::StyleVariantPlan;
use super::utilities::{
    CompiledUtility, clamped_value, invalid_property_value_finding, marker_use_finding,
    resolve_utility_targets, unresolved_lint_kind, unresolved_utility_finding, written_span,
};
use super::variants::document_scoped_selector;

/// Rules and class names collected before they are assigned to a class unit.
///
/// [`Self::canonical`] contributes to the class hash assembled by `ClassUnits`.
/// Separate lists keep compatibility and asset markers apart from rules. Transition
/// uses keep their CSS context until the owning unit chooses whether to write tokens
/// directly or through slots.
#[derive(Default)]
pub(super) struct LiteralContribution {
    /// Hash fragment for rule-bearing candidates and their wrappers, in source order.
    pub(super) canonical: LeanString,
    /// Utilities to emit as this leaf's rule.
    pub(super) utilities: Vec<CompiledUtility>,
    /// Compatibility markers, emitted instead of a rule.
    pub(super) markers: Vec<LeanStr>,
    /// Names with no rule of their own, such as asset markers.
    pub(super) plain: Vec<LeanStr>,
    /// Transition uses with their tokens, CSS contexts, and names, in encounter order.
    pub(super) view_transitions: Vec<TransitionContribution>,
}

impl LiteralContribution {
    /// Appends the canonical input, utilities, and transition uses from `other` when
    /// it contains rules. Their order is preserved.
    ///
    /// Marker and plain class names are discarded. The caller must extract any names it
    /// needs before passing `other` to this function.
    pub(super) fn append_rules(&mut self, other: Self) {
        if !other.utilities.is_empty() || !other.view_transitions.is_empty() {
            self.canonical.push_str(&other.canonical);
            self.utilities.extend(other.utilities);
            self.view_transitions.extend(other.view_transitions);
        }
    }

    /// Lowers one utility literal into its canonical input, resolved utilities, and
    /// classes.
    ///
    /// A candidate that names a marker class adds the class as written when it has no
    /// variants, and it still contributes the utilities that it resolves to. Variants
    /// of `plan`, including ref conditions, prevent the class, and `invalid-marker-use`
    /// reports a candidate that cannot add its class whether or not it resolves. Any
    /// other unresolved candidate is omitted from class output and canonical input;
    /// enabled lint rules report it. A resolved candidate whose own variants would
    /// apply to the document is reported without changing its output, because the host
    /// decides that policy's severity. A candidate with a malformed capture of a
    /// CSS-registered runtime condition or a malformed `--if()` function is omitted as
    /// an invalid utility. With `use_css_if` false, a runtime condition that the
    /// fallback strategy cannot represent omits its candidate and reports
    /// `runtime-condition-requires-css-if`. A malformed utility requirement in the
    /// design system appends a fatal diagnostic, and callers must discard the
    /// compilation if diagnostics are present.
    pub(super) fn resolve(
        value: &Spanned<LeanStr>,
        plan: &StyleVariantPlan,
        theme: &ThemeIndex,
        lint_settings: &LintSettings,
        use_css_if: bool,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> LiteralContribution {
        let mut contribution = LiteralContribution::default();
        let mut decoded_offset = 0;
        for utility in lex_utilities(value, diagnostics) {
            let start = value.value[decoded_offset..]
                .find(utility.value.raw.as_str())
                .map_or(decoded_offset, |offset| decoded_offset + offset);
            decoded_offset = start + utility.value.raw.len();
            let source_span = Span::new(
                value.span.file,
                value.span.start.0 + u32::try_from(start).unwrap_or_default(),
                value.span.start.0 + u32::try_from(decoded_offset).unwrap_or_default(),
            );
            let raw = if plan.prefix.is_empty() {
                utility.value.raw.clone()
            } else {
                ToLeanStr::to_lean_str(&format_args!("{}{}", plan.prefix, utility.value.raw))
            };
            if let Some(diagnostic) =
                runtime_condition_lint(&raw, &utility.value.raw, theme, use_css_if)
            {
                if lint_settings.enabled_for_detection(diagnostic.kind()) {
                    lint_findings.push(LintFinding::new(diagnostic, utility.span));
                }
                continue;
            }
            if lint_settings.enabled_for_detection(LintKind::PreferTypedRelationships)
                && is_tailwind_relationship_candidate(&raw, theme)
            {
                lint_findings.push(LintFinding::new(
                    LintDiagnostic::TailwindRelationshipUtility {
                        utility: (raw.clone()),
                    },
                    utility.span,
                ));
            }
            contribution.markers.extend(added_marker(&raw, theme, plan));
            if lint_settings.enabled_for_detection(LintKind::InvalidMarkerUse) {
                lint_findings.extend(marker_use_finding(
                    &raw,
                    utility.value.raw.len(),
                    utility.span,
                    theme,
                    plan.has_refs.then_some(MarkerUseErrorKind::Conditional),
                ));
            }
            match resolve_utility_targets(&raw, theme, VariantContext::Element) {
                Ok(Ok(mut resolved)) => {
                    let canonical_candidate =
                        format_args!("{}{}", plan.canonical, utility.value.raw).to_lean_string();
                    contribution.canonical.push_str(&canonical_candidate);
                    contribution.canonical.push('\0');
                    lint_findings.extend(variant_selector_lints(
                        &utility.value.raw,
                        theme,
                        lint_settings,
                        utility.span,
                        0,
                    ));
                    lint_findings.extend(resolved_candidate_lints(
                        &raw,
                        utility.value.raw.len(),
                        &resolved,
                        theme,
                        lint_settings,
                        utility.span,
                    ));
                    for resolved_utility in &mut resolved {
                        resolved_utility.diagnostic_data.span = Some(source_span);
                        resolved_utility.diagnostic_data.variant_spans =
                            variant_spans(&utility.value.raw, source_span);
                        plan.apply(resolved_utility, theme);
                    }
                    contribution.utilities.append(&mut resolved);
                }
                // `marker_use_finding` above reports a candidate that names a marker.
                Ok(Err(failure)) => {
                    if !matches!(failure.reason, UtilityFailureReason::Marker { .. })
                        && lint_settings.enabled_for_detection(unresolved_lint_kind(&failure))
                    {
                        lint_findings.push(unresolved_utility_finding(
                            &raw,
                            utility.value.raw.len(),
                            &failure,
                            utility.span,
                            theme,
                        ));
                    }
                }
                Err(error) => diagnostics.push(CompileError::new(
                    CompileDiagnostic::MalformedUtilityRequirement {
                        utility: error.name,
                        offset: error.source.offset,
                    },
                    utility.span,
                )),
            }
        }
        contribution
    }
}

/// Returns the marker class that the candidate `raw` adds to the element, or `None`
/// when it names no marker class, cannot add it, or `plan` applies ref conditions.
fn added_marker(raw: &str, theme: &ThemeIndex, plan: &StyleVariantPlan) -> Option<LeanStr> {
    match marker_use(raw, theme)? {
        MarkerUse::Class(class) if !plan.has_refs => Some(class.as_str().into()),
        _ => None,
    }
}

/// A transition token with its CSS application context and name supply.
pub(super) struct TransitionContribution {
    /// Inclusive UTF-8 byte start of the original use receiver in this module, used to
    /// retain slot names when other call inputs are omitted from an isolated preview.
    pub(super) source_offset: u32,
    /// Canonical generated token, escaped when serialized into CSS.
    pub(super) token: LeanStr,
    /// Escaped static participant name.
    pub(super) name: Option<LeanStr>,
    /// Custom property that holds a dynamic name controlled by a CSS variant.
    /// Requires `plan` and a runtime value for this property from the same use.
    pub(super) dynamic_name_property: Option<LeanStr>,
    /// CSS application context; absence permits a base token to be emitted literally.
    pub(super) plan: Option<StyleVariantPlan>,
}

/// Returns the enabled findings for variant selectors of the candidate `raw`.
///
/// The first `unchecked` variants are not checked; a document-level style passes the
/// variants up to the one that selects its element. An unparsable candidate returns no
/// findings. See [`variant_list_lints`] for the reported rules.
pub(super) fn variant_selector_lints(
    raw: &str,
    theme: &ThemeIndex,
    lint_settings: &LintSettings,
    span: Span,
    unchecked: usize,
) -> Vec<LintFinding> {
    let Some(candidate) = parse_candidate(raw) else {
        return Vec::new();
    };
    // Escaped source text has no byte-for-byte mapping to `raw`; its variants share
    // `span`.
    let spans = if span.len() == u32::try_from(raw.len()).ok() {
        variant_spans(raw, span)
    } else {
        vec![span; candidate.variants.len()]
    };
    variant_list_lints(
        &candidate.variants[unchecked.min(candidate.variants.len())..],
        &spans[unchecked.min(spans.len())..],
        theme,
        lint_settings,
    )
}

/// Returns the enabled findings for the selectors of `variants`, each written at the
/// span with the same index in `spans`.
///
/// Each rule reports at most one finding at the span of the responsible variant: a
/// selector that does not depend on the styled element, and a selector that uses `&`
/// more than once in one complex selector.
pub(super) fn variant_list_lints(
    variants: &[Variant],
    spans: &[Span],
    theme: &ThemeIndex,
    lint_settings: &LintSettings,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    if lint_settings.enabled_for_detection(LintKind::DocumentScopedSelector)
        && let Some((diagnostic, span)) = variants
            .iter()
            .zip(spans)
            .find_map(|(variant, span)| Some((document_scoped_selector(variant, theme)?, *span)))
    {
        findings.push(LintFinding::new(diagnostic, span));
    }
    if lint_settings.enabled_for_detection(LintKind::RepeatedNestingSelector)
        && let Some((selector, span)) = variants
            .iter()
            .zip(spans)
            .find_map(|(variant, span)| Some((repeated_subject_selector(variant, theme)?, *span)))
    {
        findings.push(LintFinding::new(
            LintDiagnostic::RepeatedNestingSelector {
                selector: selector.into(),
            },
            span,
        ));
    }
    findings
}

/// Returns the enabled findings for the resolved candidate `raw`, whose last `written`
/// bytes the author wrote at `span`.
///
/// `invalid-property-value` reports a value that its property rejects.
/// `unsatisfiable-variants` reports variants whose combined condition never holds, at
/// the range from the first to the last contradicting variant. `clamped-value` reports
/// the first literal value that browsers clamp, and `invalid-property-value` reports a
/// negative filter amount, at the value of the utility. `resolved` holds every rule of
/// the candidate.
pub(super) fn resolved_candidate_lints(
    raw: &str,
    written: usize,
    resolved: &[CompiledUtility],
    theme: &ThemeIndex,
    lint_settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    if lint_settings.enabled_for_detection(LintKind::InvalidPropertyValue)
        && let Some(value) = resolved
            .first()
            .and_then(|first| first.diagnostic_data.invalid_value.as_ref())
    {
        findings.push(invalid_property_value_finding(raw, written, value, span));
    }
    if lint_settings.enabled_for_detection(LintKind::UnsatisfiableVariants)
        && let Some(candidate) = parse_candidate(raw)
        && let Some(found) = unsatisfiable_variants(&candidate, theme)
    {
        let [first, second] = found.conditions;
        findings.push(LintFinding::new(
            LintDiagnostic::UnsatisfiableVariants {
                variants: raw.get(found.range.clone()).unwrap_or(raw).to_lean_str(),
                first,
                second,
                branches: u32::try_from(found.branches).unwrap_or(u32::MAX),
            },
            written_span(raw.len(), written, &found.range, span),
        ));
    }
    if [LintKind::ClampedValue, LintKind::InvalidPropertyValue]
        .into_iter()
        .any(|kind| lint_settings.enabled_for_detection(kind))
    {
        let declarations = resolved.iter().flat_map(|utility| {
            std::iter::once(utility.declaration.as_str())
                .chain(utility.nested.iter().map(|rule| rule.declaration.as_str()))
        });
        let utility = raw.get(raw.len().saturating_sub(written)..).unwrap_or(raw);
        if let Some(diagnostic) = clamped_value(declarations, utility)
            && lint_settings.enabled_for_detection(diagnostic.kind())
        {
            let span = utility_value_range(raw)
                .map_or(span, |range| written_span(raw.len(), written, &range, span));
            findings.push(LintFinding::new(diagnostic, span));
        }
    }
    findings
}

fn variant_spans(raw: &str, source_span: Span) -> Vec<Span> {
    let Some(candidate) = parse_candidate(raw) else {
        return Vec::new();
    };
    let mut offset = source_span.start.0;
    candidate
        .variants
        .iter()
        .map(|variant| {
            let end = offset + u32::try_from(variant.raw.len()).unwrap_or_default();
            let span = Span::new(source_span.file, offset, end);
            offset = end + 1;
            span
        })
        .collect()
}

/// Returns the lint for a runtime condition that omits the candidate `raw`.
///
/// `utility` is the authored spelling reported by the lint. An invalid `--if()`
/// function selects `invalid-utility`. With `use_css_if` false, a condition that the
/// fallback strategy cannot represent selects `runtime-condition-requires-css-if`.
/// Returns `None` when the candidate can be lowered or does not parse. A conditional
/// variant with a malformed capture does not resolve, so utility resolution reports it.
fn runtime_condition_lint(
    raw: &str,
    utility: &str,
    theme: &ThemeIndex,
    use_css_if: bool,
) -> Option<LintDiagnostic> {
    let candidate = parse_candidate(raw)?;
    let value = match &candidate.utility {
        Utility::Named { name, .. } => name
            .find('[')
            .and_then(|start| name[start + 1..].strip_suffix(']'))
            .map(|value| value.replace_char('_', " ")),
        Utility::ArbitraryProperty { value, .. } => Some(value.to_lean_string()),
    };
    let conditions = candidate
        .variants
        .iter()
        .filter_map(|variant| match resolve_variant(variant, theme).ok() {
            Some(VariantApplication::Conditional(condition)) => Some(condition),
            _ => None,
        })
        .collect::<Vec<_>>();
    match check_candidate(
        value.as_deref(),
        conditions.iter().map(LeanStr::as_str),
        use_css_if,
    ) {
        Ok(()) => None,
        Err(UnsupportedCondition::Syntax) => Some(LintDiagnostic::InvalidUtility {
            utility: utility.to_lean_str(),
            detail: "an `--if()` function requires a condition and two values".into(),
        }),
        Err(UnsupportedCondition::RequiresNative(detail)) => {
            Some(LintDiagnostic::RuntimeConditionRequiresCssIf {
                utility: utility.to_lean_str(),
                detail: detail.to_lean_str(),
            })
        }
    }
}
