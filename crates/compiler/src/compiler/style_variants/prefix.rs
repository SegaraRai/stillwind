//! Parsing and resolution of the ordinary variant prefixes of object keys and ref
//! conditions.

use lean_string::{LeanStr, ToLeanString};

use stillwind_design_system::{
    ThemeIndex, UtilityFailure, UtilityFailureReason, VariantApplication, resolve_variant,
};
use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_source::Spanned;
use stillwind_syntax::{Variant, parse_candidate};

use super::super::utilities::unresolved_utility_finding;

/// Parses a wrapper prefix and resolves each of its variants.
///
/// Returns `None` after pushing the diagnostic for a prefix that is not a complete
/// `name:` list, or for the first variant that does not resolve. The second finding
/// covers the failing part of that variant.
pub(super) fn collect_variant_and_applications(
    theme: &ThemeIndex,
    prefix: &Spanned<LeanStr>,
    lint_findings: &mut Vec<LintFinding>,
) -> Option<(Vec<Variant>, Vec<VariantApplication>)> {
    let Some(variants) = parse_prefix(prefix) else {
        lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidVariantPrefix,
            prefix.span,
        ));
        return None;
    };
    let mut applications = Vec::with_capacity(variants.len());
    let mut start = 0;
    for variant in &variants {
        match resolve_variant(variant, theme) {
            Ok(application) => applications.push(application),
            Err(failure) => {
                let failure = failure.shifted(start);
                let failure = UtilityFailure {
                    range: failure.range,
                    reason: UtilityFailureReason::Variant(failure.reason),
                };
                lint_findings.push(unresolved_utility_finding(
                    &prefix.value,
                    prefix.value.len(),
                    &failure,
                    prefix.span,
                    theme,
                ));
                return None;
            }
        }
        start += variant.raw.len() + 1;
    }
    Some((variants, applications))
}

/// Parses a wrapper prefix into its variants, requiring complete `name:` segments.
///
/// The prefix is parsed as a candidate followed by a placeholder utility, which reuses
/// the candidate grammar instead of duplicating it here. A prefix that does not end in
/// `:` or that yields an empty variant is rejected, so a malformed wrapper cannot
/// silently shift the meaning of the utility it wraps. An empty prefix returns an
/// empty variant list. The utility suffix is discarded before resolution and needs no
/// registry definition.
pub(in crate::compiler) fn parse_prefix(prefix: &Spanned<LeanStr>) -> Option<Vec<Variant>> {
    if !prefix.value.is_empty() && !prefix.value.ends_with(':') {
        return None;
    }
    let candidate = parse_candidate(&format_args!("{}utility", prefix.value).to_lean_string())?;
    if candidate
        .variants
        .iter()
        .any(|variant| variant.raw.is_empty())
    {
        return None;
    }
    Some(candidate.variants)
}
