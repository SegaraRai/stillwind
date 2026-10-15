//! Utility literal resolution into canonical class inputs and CSS contributions.
//!
//! [`LiteralContribution::resolve`] omits unresolved candidates and reports lint
//! findings while applying semantic wrappers to resolved utilities.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    ThemeIndex, VariantApplication, compatibility_marker_class, is_condition_variant,
    is_tailwind_relationship_candidate, resolve_variant,
};
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintDiagnostic, LintFinding, LintKind, LintSettings,
    UtilityFailure,
};
use stillwind_source::{ReplaceText, Span, Spanned};
use stillwind_syntax::{Utility, parse_candidate};

use super::conditionals::{UnsupportedCondition, check_candidate};
use super::lexer::lex_utilities;
use super::style_variants::StyleVariantPlan;
use super::utilities::{CompiledUtility, resolve_utility_targets, unresolved_utility_kind};
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
    /// A compatibility marker is emitted instead of utilities when the candidate only
    /// marks a Tailwind relationship. An unresolved candidate is omitted from class
    /// output and canonical input; enabled lint rules report it. A resolved candidate
    /// whose own variants would apply to the document is reported without changing its
    /// output, because the host decides that policy's severity. A candidate with a
    /// malformed capture of a CSS-registered runtime condition or a malformed `--if()`
    /// function is omitted as an invalid utility. With `use_css_if` false, a runtime
    /// condition that the fallback strategy cannot represent omits its candidate and
    /// reports `runtime-condition-requires-native`. A malformed utility requirement in
    /// the design system appends a fatal diagnostic, and callers must discard the
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
                    LintDiagnostic::PreferTypedRelationships {
                        utility: (raw.clone()),
                    },
                    utility.span,
                ));
            }
            if !plan.has_refs
                && let Some(marker) = compatibility_marker_class(&raw, theme)
            {
                contribution.markers.push((marker).into());
                continue;
            }
            match resolve_utility_targets(&raw, theme) {
                Ok(Some(mut resolved)) => {
                    let canonical_candidate =
                        format_args!("{}{}", plan.canonical, utility.value.raw).to_lean_string();
                    contribution.canonical.push_str(&canonical_candidate);
                    contribution.canonical.push('\0');
                    if lint_settings.enabled_for_detection(LintKind::RequireElementScope)
                        && let Some(candidate) = parse_candidate(&utility.value.raw)
                        && let Some(selector) = candidate
                            .variants
                            .iter()
                            .find_map(|variant| document_scoped_selector(variant, theme))
                    {
                        lint_findings.push(LintFinding::new(
                            LintDiagnostic::DocumentScopedSelector {
                                selector: (selector).into(),
                            },
                            utility.span,
                        ));
                    }
                    for resolved_utility in &mut resolved {
                        resolved_utility.source.span = Some(source_span);
                        resolved_utility.source.variant_spans =
                            variant_spans(&utility.value.raw, source_span);
                        plan.apply(resolved_utility, theme);
                    }
                    contribution.utilities.append(&mut resolved);
                }
                Ok(None) => {
                    if lint_settings.has_enabled_rules() {
                        let classification = unresolved_utility_kind(&utility.value.raw);
                        if lint_settings.enabled_for_detection(classification.lint_kind()) {
                            lint_findings.push(LintFinding::new(
                                LintDiagnostic::UnresolvedUtility {
                                    utility: (utility.value.raw.clone()),
                                    classification,
                                },
                                utility.span,
                            ));
                        }
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
/// `utility` is the authored spelling reported by the lint. A conditional variant whose
/// capture is malformed and an invalid `--if()` function select `invalid-utility`. With
/// `use_css_if` false, a condition that the fallback strategy cannot represent selects
/// `runtime-condition-requires-native`. Returns `None` when the candidate can be
/// lowered or does not parse.
fn runtime_condition_lint(
    raw: &str,
    utility: &str,
    theme: &ThemeIndex,
    use_css_if: bool,
) -> Option<LintDiagnostic> {
    let candidate = parse_candidate(raw)?;
    let invalid = || LintDiagnostic::UnresolvedUtility {
        utility: utility.to_lean_str(),
        classification: UtilityFailure::Invalid,
    };
    if candidate.variants.iter().any(|variant| {
        resolve_variant(variant, theme).is_none() && is_condition_variant(&variant.raw, theme)
    }) {
        return Some(invalid());
    }
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
        .filter_map(|variant| match resolve_variant(variant, theme) {
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
        Err(UnsupportedCondition::Syntax) => Some(invalid()),
        Err(UnsupportedCondition::RequiresNative(detail)) => {
            Some(LintDiagnostic::RuntimeConditionRequiresNative {
                utility: utility.to_lean_str(),
                detail: detail.to_lean_str(),
            })
        }
    }
}
