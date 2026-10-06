//! Utility literal resolution into canonical class inputs and CSS contributions.
//!
//! [`LiteralContribution::resolve`] retains unresolved external names and reports lint
//! findings while applying semantic wrappers to resolved utilities.

use stillwind_design_system::{
    ThemeIndex, compatibility_marker_class, is_condition_variant,
    is_tailwind_relationship_candidate, resolve_variant,
};
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintDiagnostic, LintFinding, LintKind,
};
use stillwind_source::Spanned;
use stillwind_syntax::parse_candidate;

use super::lexer::lex_utilities;
use super::style_variants::StyleVariantPlan;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_kind,
};
use super::variants::document_scoped_selector;

/// Rules and class names collected before they are assigned to a class unit.
///
/// [`Self::canonical`] contributes to the class hash assembled by `ClassUnits`.
/// Separate lists for rules and names allow unresolved candidates to appear as
/// class names without adding CSS. Transition uses keep their CSS context until
/// the owning unit chooses whether to write tokens directly or through slots.
#[derive(Default)]
pub(super) struct LiteralContribution {
    /// Hash fragment for rule-bearing candidates and their wrappers, in source order.
    pub(super) canonical: String,
    /// Utilities to emit as this leaf's rule.
    pub(super) utilities: Vec<CompiledUtility>,
    /// Compatibility markers, emitted instead of a rule.
    pub(super) markers: Vec<String>,
    /// Class names Stillwind does not own, passed through unchanged.
    pub(super) external: Vec<String>,
    /// Names with no rule of their own, such as asset markers.
    pub(super) plain: Vec<String>,
    /// Transition uses with their tokens, CSS contexts, and names, in encounter order.
    pub(super) view_transitions: Vec<TransitionContribution>,
}

/// A transition token with its CSS application context and name supply.
pub(super) struct TransitionContribution {
    /// Canonical generated token, escaped when serialized into CSS.
    pub token: String,
    /// Escaped static participant name.
    pub name: Option<String>,
    /// Custom property that holds a dynamic name controlled by a CSS variant.
    /// Requires `plan` and a runtime value for this property from the same use.
    pub dynamic_name_property: Option<String>,
    /// CSS application context; absence permits a base token to be emitted literally.
    pub plan: Option<StyleVariantPlan>,
}

impl LiteralContribution {
    /// Appends the canonical input, utilities, and transition uses from `other` when
    /// it contains rules. Their order is preserved.
    ///
    /// Marker, external, and plain class names are discarded. The caller must extract
    /// any names it needs before passing `other` to this function.
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
    /// marks a Tailwind relationship, and an unresolved candidate becomes an external
    /// class name plus a lint finding, which keeps the user's class in the output
    /// rather than dropping it. A resolved candidate whose own variants would apply to
    /// the document is reported without changing its output, because the host decides
    /// that policy's severity. A malformed CSS-registered runtime condition appends a
    /// compile diagnostic and skips that candidate; callers must discard the
    /// compilation if diagnostics are present.
    pub(super) fn resolve(
        value: &Spanned<String>,
        plan: &StyleVariantPlan,
        theme: &ThemeIndex,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> LiteralContribution {
        let mut contribution = LiteralContribution::default();
        for utility in lex_utilities(value, diagnostics) {
            let canonical_candidate = format!("{}{}", plan.canonical, utility.value.raw);
            let mut raw = utility.value.raw.clone();
            raw.insert_str(0, &plan.prefix);
            if parse_candidate(&raw).is_some_and(|candidate| {
                candidate.variants.iter().any(|variant| {
                    resolve_variant(variant, theme).is_none()
                        && is_condition_variant(&variant.raw, theme)
                })
            }) {
                diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidRuntimeConditional {
                        detail: "invalid CSS-registered if-condition capture".to_owned(),
                    },
                    utility.span,
                ));
                continue;
            }
            if theme
                .config
                .lint_settings
                .enabled_for_detection(LintKind::PreferTypedRelationships)
                && is_tailwind_relationship_candidate(&raw, theme)
            {
                lint_findings.push(LintFinding::new(
                    LintDiagnostic::PreferTypedRelationships {
                        utility: raw.clone(),
                    },
                    utility.span,
                ));
            }
            if !plan.has_refs
                && let Some(marker) = compatibility_marker_class(&raw, theme)
            {
                contribution.markers.push(marker);
                continue;
            }
            contribution.canonical.push_str(&canonical_candidate);
            contribution.canonical.push('\0');
            let resolved_input = raw.strip_prefix("tw:").unwrap_or(&raw);
            match resolve_utility_targets(resolved_input, theme) {
                Ok(Some(mut resolved)) => {
                    if theme
                        .config
                        .lint_settings
                        .enabled_for_detection(LintKind::RequireElementScope)
                        && let Some(candidate) = parse_candidate(&utility.value.raw)
                        && let Some(selector) = candidate
                            .variants
                            .iter()
                            .find_map(|variant| document_scoped_selector(variant, theme))
                    {
                        lint_findings.push(LintFinding::new(
                            LintDiagnostic::DocumentScopedSelector { selector },
                            utility.span,
                        ));
                    }
                    for resolved_utility in &mut resolved {
                        plan.apply(resolved_utility, theme);
                    }
                    contribution.utilities.append(&mut resolved);
                }
                Ok(None) => {
                    lint_findings.push(LintFinding::new(
                        LintDiagnostic::UnresolvedUtility {
                            utility: utility.value.raw.clone(),
                            classification: unresolved_utility_kind(&utility.value.raw),
                        },
                        utility.span,
                    ));
                    contribution.external.push(utility.value.raw.clone());
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
