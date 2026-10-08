//! Candidate resolution into compiled utilities.
//!
//! One candidate can produce several targets, for example a utility that expands to more than one
//! rule, and each target becomes its own [`CompiledUtility`] sharing the candidate's parsed
//! variants. The distinction between a candidate that is not valid syntax and a name the registry
//! does not know is preserved here, because the lint code a user sees depends on it.

use stillwind_design_system::{
    NestedUtilityRule, ResolvedUtilityTarget, ThemeIndex, UtilityArbitraryUse, UtilityOrder,
    UtilityResolutionError, UtilityTokenUse, VariantApplication, resolve_variant,
};
use stillwind_diagnostics::UtilityFailure;
use stillwind_source::Span;
use stillwind_syntax::{Variant, parse_candidate};

use crate::candidate::CandidateResolver;

/// One resolved utility rule, ready to be ordered and rendered.
#[derive(Clone, Debug)]
pub(crate) struct CompiledUtility {
    /// Candidate source occurrence and token selections before values expand.
    pub(in crate::compiler) source: UtilitySource,
    /// Parsed variants in candidate order, resolved through the registry unless lowered
    /// applications are present.
    pub(in crate::compiler) variants: Vec<Variant>,
    /// Prelowered alternative paths, used instead of registry resolution when nonempty;
    /// steps retain application order.
    pub(in crate::compiler) variant_applications: Vec<Vec<VariantApplication>>,
    /// Serialized declaration-list content without the outer rule braces.
    pub(in crate::compiler) declaration: String,
    /// Custom-property registration names required by this utility, independent of CSS
    /// references.
    pub(in crate::compiler) required_properties: Vec<String>,
    /// Optional selector template whose ampersands denote the generated class selector.
    pub(in crate::compiler) selector: Option<String>,
    /// Optional container query prelude without @container or braces.
    pub(in crate::compiler) container: Option<String>,
    /// Conditional utility rules in resolved emission order.
    pub(in crate::compiler) nested: Vec<NestedUtilityRule>,
    /// Snapshot-selected declaration metadata or authored registration position.
    pub(in crate::compiler) order: UtilityOrder,
    /// Original candidate spelling for ordering and diagnostics; empty for synthetic
    /// utility rules.
    pub(in crate::compiler) candidate: String,
}

/// Origin shared by all rules that expand from one utility candidate.
#[derive(Clone, Debug, Default)]
pub(in crate::compiler) struct UtilitySource {
    /// Candidate range with decoded offsets until the emitter maps it to original source.
    pub(in crate::compiler) span: Option<Span>,
    /// Direct selections preserved before inline theme values expand.
    pub(in crate::compiler) token_uses: Vec<UtilityTokenUse>,
    /// Accepted captures before source values expand into declaration text.
    pub(in crate::compiler) arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// Ranges of variants written in the utility literal, in candidate order.
    pub(in crate::compiler) variant_spans: Vec<Span>,
    /// Leading variants already checked at their wrapper source locations.
    pub(in crate::compiler) wrapper_variant_count: usize,
}

/// Resolves a candidate and returns its first target, for callers that render a single rule.
pub(in crate::compiler) fn resolve_utility(
    utility: &str,
    theme: &ThemeIndex,
) -> Result<Option<CompiledUtility>, UtilityResolutionError> {
    Ok(resolve_utility_targets(utility, theme)?.and_then(|targets| targets.into_iter().next()))
}

/// Classifies an unresolved candidate so its typed finding selects the right lint rule.
///
/// A candidate that parses but does not resolve is an unknown name, which is a different user
/// mistake from text that is not candidate syntax at all.
pub(crate) fn unresolved_utility_kind(raw: &str) -> UtilityFailure {
    if parse_candidate(raw).is_some() {
        UtilityFailure::Unknown
    } else {
        UtilityFailure::Invalid
    }
}

/// Resolves a candidate into every rule it expands to, in emission order.
///
/// Returns `Ok(None)` when the candidate is unknown or unavailable in this theme; the caller decides
/// whether that is a lint finding, an external class name, or a diagnostic. Every
/// variant must also resolve; an absent CSS registration never drops its guard and
/// turns the candidate into an unconditional utility.
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
pub(crate) fn resolve_utility_targets(
    utility: &str,
    theme: &ThemeIndex,
) -> Result<Option<Vec<CompiledUtility>>, UtilityResolutionError> {
    let Some(resolved) = CandidateResolver::resolve(utility, theme)? else {
        return Ok(None);
    };
    for variant in &resolved.candidate.variants {
        if resolve_variant(variant, theme).is_none() {
            return Ok(None);
        }
    }
    let mut targets = vec![ResolvedUtilityTarget {
        declaration: resolved.declaration,
        selector: resolved.selector,
        nested: resolved.nested,
    }];
    targets.extend(resolved.additional);
    Ok(Some(
        targets
            .into_iter()
            .map(|target| CompiledUtility {
                source: UtilitySource {
                    span: None,
                    token_uses: resolved.token_uses.clone(),
                    arbitrary_uses: resolved.arbitrary_uses.clone(),
                    ..UtilitySource::default()
                },
                variants: resolved.candidate.variants.clone(),
                variant_applications: Vec::new(),
                declaration: target.declaration,
                required_properties: resolved.required_properties.clone(),
                selector: target.selector,
                container: None,
                nested: target.nested,
                order: resolved.order.clone(),
                candidate: utility.to_owned(),
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_diagnostics_distinguish_syntax_from_unknown_names() {
        assert_eq!(unresolved_utility_kind(""), UtilityFailure::Invalid);
        assert_eq!(unresolved_utility_kind("hover:["), UtilityFailure::Invalid);
        assert_eq!(
            unresolved_utility_kind("unknown-but-valid"),
            UtilityFailure::Unknown
        );
    }
}
