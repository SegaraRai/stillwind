//! Utility literal resolution into canonical class inputs and CSS contributions.
//!
//! [`LiteralContribution::resolve`] retains unresolved external names and reports lint
//! findings while applying semantic wrappers to resolved utilities.

use stillwind_design_system::{
    ThemeIndex, compatibility_marker_class, is_condition_variant,
    is_tailwind_relationship_candidate, resolve_variant,
};
use stillwind_diagnostics::{CompileError, CompileErrorKind, LintFinding, LintKind};
use stillwind_source::Spanned;
use stillwind_syntax::parse_candidate;

use super::lexer::lex_utilities;
use super::style_variants::StyleVariantPlan;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_kind,
};
use super::variants::document_scoped_selector;
use crate::CompilerOptions;

/// The pieces one utility literal contributes before it becomes a leaf or a base-unit entry.
///
/// [`Self::canonical`] is the text appended to the class-hash input. Keeping markers,
/// external names, and resolved utilities in separate lists is what lets an unresolved
/// candidate still reach the output as a class name while contributing no rule.
/// A view transition contributes one class token and at most one static name, and they
/// travel beside the utilities because a class unit declares them rather than a utility.
#[derive(Default)]
pub(super) struct LiteralContribution {
    /// Canonical candidate and wrapper spelling of the literal, in source order.
    pub(super) canonical: String,
    /// Utilities to emit as this leaf's rule.
    pub(super) utilities: Vec<CompiledUtility>,
    /// Compatibility markers, emitted instead of a rule.
    pub(super) markers: Vec<String>,
    /// Class names Stillwind does not own, passed through unchanged.
    pub(super) external: Vec<String>,
    /// Names with no rule of their own, such as handle markers.
    pub(super) plain: Vec<String>,
    /// View-transition class token this contribution declares, when it declares one.
    pub(super) view_transition_token: Option<String>,
    /// Escaped view-transition name this contribution declares statically, if any.
    pub(super) view_transition_name: Option<String>,
}

impl LiteralContribution {
    /// Lowers one utility literal into its canonical input, resolved utilities, and classes.
    ///
    /// A compatibility marker is emitted instead of utilities when the candidate only marks a
    /// Tailwind relationship, and an unresolved candidate becomes an external class name plus a
    /// lint finding, which keeps the user's class in the output rather than dropping it.
    /// A resolved candidate whose own variants would apply to the document is reported without
    /// changing its output, because the host decides that policy's severity.
    /// A malformed CSS-registered runtime condition appends a compile diagnostic and skips
    /// that candidate; callers must discard the compilation if diagnostics are present.
    pub(super) fn resolve(
        value: &Spanned<String>,
        plan: &StyleVariantPlan,
        options: &CompilerOptions,
        theme: &ThemeIndex,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
        name_supplies: &mut usize,
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
                    CompileErrorKind::InvalidRuntimeConditional {
                        detail: "invalid CSS-registered if-condition capture".to_owned(),
                    },
                    utility.span,
                ));
                continue;
            }
            if (options.forbid_tailwind_relationships
                || theme.config.lint_enabled("prefer-typed-relationships"))
                && is_tailwind_relationship_candidate(&raw, theme)
            {
                lint_findings.push(LintFinding::new(
                    LintKind::PreferTypedRelationships {
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
            contribution.canonical.push('\u{0}');
            let resolved_input = raw.strip_prefix("tw:").unwrap_or(&raw);
            if let Some(mut resolved) = resolve_utility_targets(resolved_input, theme) {
                *name_supplies += resolved
                    .iter()
                    .filter(|utility| utility.declaration.contains("view-transition-name"))
                    .count();
                if theme.config.lint_enabled("require-element-scope")
                    && let Some(candidate) = parse_candidate(&utility.value.raw)
                    && let Some(selector) = candidate
                        .variants
                        .iter()
                        .find_map(|variant| document_scoped_selector(variant, theme))
                {
                    lint_findings.push(LintFinding::new(
                        LintKind::DocumentScopedSelector { selector },
                        utility.span,
                    ));
                }
                for resolved_utility in &mut resolved {
                    plan.apply(resolved_utility, theme);
                }
                contribution.utilities.append(&mut resolved);
            } else {
                lint_findings.push(LintFinding::new(
                    LintKind::UnresolvedUtility {
                        utility: utility.value.raw.clone(),
                        classification: unresolved_utility_kind(&utility.value.raw),
                    },
                    utility.span,
                ));
                contribution.external.push(utility.value.raw.clone());
            }
        }
        contribution
    }
}
