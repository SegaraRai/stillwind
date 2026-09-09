use std::fmt::Write;
use stillwind_grammar::Candidate;

use stillwind_css::ThemeIndex;
use stillwind_grammar::{Utility, parse_candidate};

use super::formatting::finalize_declaration;
use super::nested::NestedUtilityRule;
use super::nested::matching_utilities;
use super::registry::{
    animation_option_property_mappings, arbitrary_property_order, builtin_animation_options,
};
use super::utility_body::UtilityResolution;
use super::utility_matching::{resolve_css_utility_branches, resolve_named_utility};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    /// Source order of the matched `@stillwind-utility`.
    pub builtin_order: Option<usize>,
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads.
    /// CSS fallbacks and selector-bearing branches must never be discarded
    /// merely because they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
}

/// Resolves a candidate using only the Tailwind-compatible registry.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            let mut resolution = UtilityResolution::plain(format!("{property}:{value};"));
            resolution.builtin_order = Some(arbitrary_property_order(property));
            resolution
        }
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    let declaration = finalize_declaration(resolution.declaration, candidate.important, theme);
    let nested: Vec<NestedUtilityRule> = resolution
        .nested
        .into_iter()
        .map(|mut nested| {
            nested.declaration =
                finalize_declaration(nested.declaration, candidate.important, theme);
            nested
        })
        .collect();
    let additional = resolution
        .additional
        .into_iter()
        .map(|mut target| {
            target.declaration =
                finalize_declaration(target.declaration, candidate.important, theme);
            for nested in &mut target.nested {
                nested.declaration = finalize_declaration(
                    std::mem::take(&mut nested.declaration),
                    candidate.important,
                    theme,
                );
            }
            target
        })
        .collect();
    let selector = resolution.selector;
    Some(ResolvedUtility {
        candidate,
        builtin_order: resolution.builtin_order,
        declaration,
        selector,
        nested,
        additional,
    })
}

/// Resolves one animation option from the CSS-authored option registry. As a
/// compatibility path, ordinary utilities may project declarations through
/// CSS-authored property mappings without exposing their names to Rust.
#[must_use]
pub fn resolve_animation_option(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let candidate = parse_candidate(raw)?;
    if candidate.important || candidate.negative || !candidate.variants.is_empty() {
        return None;
    }
    let Utility::Named { name, modifier } = &candidate.utility else {
        return project_animation_option_utility(raw, theme);
    };
    if let Some(resolution) = resolve_css_utility_branches(
        matching_utilities(builtin_animation_options(), name),
        name,
        modifier.as_ref(),
        theme,
    ) {
        return (resolution.selector.is_none()
            && resolution.nested.is_empty()
            && resolution.additional.is_empty())
        .then(|| finalize_declaration(resolution.declaration, false, theme));
    }
    project_animation_option_utility(raw, theme)
}

pub(super) fn project_animation_option_utility(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let utility = resolve_utility(raw, theme)?;
    if !utility.candidate.variants.is_empty()
        || utility.selector.is_some()
        || !utility.nested.is_empty()
        || !utility.additional.is_empty()
    {
        return None;
    }
    let mappings = animation_option_property_mappings();
    let mut projected = String::new();
    for declaration in utility
        .declaration
        .split(';')
        .filter(|part| !part.is_empty())
    {
        let (property, value) = declaration.split_once(':')?;
        let Some(target) = mappings.get(property) else {
            continue;
        };
        write!(&mut projected, "{target}:{value};").ok()?;
    }
    (!projected.is_empty()).then_some(projected)
}
