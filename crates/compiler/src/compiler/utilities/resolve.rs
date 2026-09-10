use stillwind_design_system::{NestedUtilityRule, ResolvedUtilityTarget, ThemeIndex};
use stillwind_syntax::{Variant, parse_candidate};

use crate::candidate::CandidateResolver;

pub(in crate::compiler) fn view_transition_utility() -> CompiledUtility {
    CompiledUtility {
        variants: Vec::new(),
        declaration: "--sw-vt-name:none;--sw-vt-class:none;view-transition-name:var(--sw-vt-name);view-transition-class:var(--sw-vt-class);".to_owned(),
        selector: None,
        container: None,
        nested: Vec::new(),
        builtin_order: None,
        candidate: String::new(),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledUtility {
    pub(in crate::compiler) variants: Vec<Variant>,
    pub(in crate::compiler) declaration: String,
    pub(in crate::compiler) selector: Option<String>,
    pub(in crate::compiler) container: Option<String>,
    pub(in crate::compiler) nested: Vec<NestedUtilityRule>,
    pub(in crate::compiler) builtin_order: Option<usize>,
    pub(in crate::compiler) candidate: String,
}

pub(in crate::compiler) fn resolve_utility(
    utility: &str,
    theme: &ThemeIndex,
) -> Option<CompiledUtility> {
    resolve_utility_targets(utility, theme)?.into_iter().next()
}

pub(crate) fn unresolved_utility_rule(raw: &str) -> &'static str {
    if parse_candidate(raw).is_some() {
        "unknown-utility"
    } else {
        "invalid-utility"
    }
}

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
                declaration: target.declaration,
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
        assert_eq!(unresolved_utility_rule(""), "invalid-utility");
        assert_eq!(unresolved_utility_rule("hover:["), "invalid-utility");
        assert_eq!(
            unresolved_utility_rule("unknown-but-valid"),
            "unknown-utility"
        );
    }
}
