//! Candidate resolution into compiled utilities.
//!
//! One candidate can produce several targets, for example a utility that expands to more than one
//! rule, and each target becomes its own [`CompiledUtility`] sharing the candidate's parsed
//! variants. The distinction between a candidate that is not valid syntax and a name the registry
//! does not know is preserved here, because the lint code a user sees depends on it.

use stillwind_design_system::{
    NestedUtilityRule, ResolvedUtilityTarget, ThemeIndex, VariantApplication,
};
use stillwind_diagnostics::UtilityFailure;
use stillwind_syntax::{Variant, parse_candidate};

use crate::candidate::CandidateResolver;

/// The rule that carries a view transition's name and class custom properties.
//
// Both properties start as `none` and are then referenced, which is what allows a static call to
// leave them alone while a runtime use overwrites them through the style payload.
pub(in crate::compiler) fn view_transition_utility() -> CompiledUtility {
    CompiledUtility {
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration: "--sw-vt-name:none;--sw-vt-class:none;view-transition-name:var(--sw-vt-name);view-transition-class:var(--sw-vt-class);".to_owned(),
        selector: None,
        container: None,
        nested: Vec::new(),
        builtin_order: None,
        candidate: String::new(),
    }
}

/// One resolved utility rule, ready to be ordered and rendered.
#[derive(Clone, Debug)]
pub(crate) struct CompiledUtility {
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
    /// Global built-in definition position used as an ordering key; None for
    /// unpositioned rules.
    pub(in crate::compiler) builtin_order: Option<usize>,
    /// Original candidate spelling for ordering and diagnostics; empty for synthetic
    /// utility rules.
    pub(in crate::compiler) candidate: String,
}

/// Resolves a candidate and returns its first target, for callers that render a single rule.
pub(in crate::compiler) fn resolve_utility(
    utility: &str,
    theme: &ThemeIndex,
) -> Option<CompiledUtility> {
    resolve_utility_targets(utility, theme)?.into_iter().next()
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
/// Returns `None` when the candidate is unknown or unavailable in this theme; the caller decides
/// whether that is a lint finding, an external class name, or a diagnostic.
pub(crate) fn resolve_utility_targets(
    utility: &str,
    theme: &ThemeIndex,
) -> Option<Vec<CompiledUtility>> {
    let resolved = CandidateResolver::resolve(utility, theme)?;
    let mut targets = vec![ResolvedUtilityTarget {
        declaration: resolved.declaration,
        selector: resolved.selector,
        nested: resolved.nested,
    }];
    targets.extend(resolved.additional);
    Some(
        targets
            .into_iter()
            .map(|target| CompiledUtility {
                variants: resolved.candidate.variants.clone(),
                variant_applications: Vec::new(),
                declaration: target.declaration,
                required_properties: resolved.required_properties.clone(),
                selector: target.selector,
                container: None,
                nested: target.nested,
                builtin_order: resolved.builtin_order,
                candidate: utility.to_owned(),
            })
            .collect(),
    )
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
