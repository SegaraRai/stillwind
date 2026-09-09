use stillwind_css::ThemeIndex;
use stillwind_grammar::{Variant, parse_candidate};
use stillwind_tailwind::{NestedUtilityRule, ResolvedUtilityTarget};

use crate::query::CandidateResolver;

pub(super) fn view_transition_utility() -> CompiledUtility {
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
    pub(super) variants: Vec<Variant>,
    pub(super) declaration: String,
    pub(super) selector: Option<String>,
    pub(super) container: Option<String>,
    pub(super) nested: Vec<NestedUtilityRule>,
    pub(super) builtin_order: Option<usize>,
    pub(super) candidate: String,
}

pub(super) fn resolve_utility(utility: &str, theme: &ThemeIndex) -> Option<CompiledUtility> {
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
