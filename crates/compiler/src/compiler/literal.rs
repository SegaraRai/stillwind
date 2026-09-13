//! Utility literal resolution into canonical class inputs and CSS contributions.
//!
//! [`LiteralContribution::resolve`] retains unresolved external names and reports lint
//! findings while applying semantic wrappers to resolved utilities.

use stillwind_design_system::{
    ThemeIndex, compatibility_marker_class, is_tailwind_relationship_candidate,
};
use stillwind_diagnostics::{CompileError, LintFinding, LintKind};

use super::lexer::lex_utilities;
use super::style_variants::StyleVariantPlan;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_kind,
};
use crate::CompilerOptions;

/// The pieces one utility literal contributes before it becomes a leaf or a base-unit entry.
///
/// [`Self::canonical`] is the text appended to the class-hash input. Keeping markers,
/// external names, and resolved utilities in separate lists is what lets an unresolved
/// candidate still reach the output as a class name while contributing no rule.
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
}

impl LiteralContribution {
    /// Lowers one utility literal into its canonical input, resolved utilities, and classes.
    ///
    /// A compatibility marker is emitted instead of utilities when the candidate only marks a
    /// Tailwind relationship, and an unresolved candidate becomes an external class name plus a
    /// lint finding, which keeps the user's class in the output rather than dropping it.
    pub(super) fn resolve(
        value: &stillwind_source::Spanned<String>,
        plan: &StyleVariantPlan,
        options: &CompilerOptions,
        theme: &ThemeIndex,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> LiteralContribution {
        let mut contribution = LiteralContribution::default();
        for utility in lex_utilities(value, diagnostics) {
            let canonical_candidate = format!("{}{}", plan.canonical, utility.value.raw);
            let mut raw = utility.value.raw.clone();
            raw.insert_str(0, &plan.prefix);
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
