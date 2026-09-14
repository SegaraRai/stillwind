//! The public utility-resolution entry points.
//!
//! Resolution is stateless: the same candidate and theme always produce the same
//! result, and nothing is memoized between calls. Candidates that fail to parse, match
//! no definition, or match a branch that rejects their value all produce [`None`]; a
//! caller that needs to distinguish "unknown utility" from "wrong value type" should
//! read the diagnostics of the layer above.

use std::fmt::Write;

use stillwind_syntax::{Candidate, Utility, parse_candidate};

use super::super::builtins::registry::{
    animation_option_property_mappings, arbitrary_property_order, builtin_animation_options,
};
use super::body::UtilityResolution;
use super::formatting::finalize_declaration;
use super::matching::{matching_utilities, resolve_css_utility_branches, resolve_named_utility};
use super::nested::NestedUtilityRule;
use super::order::{UtilityOrder, UtilityPropertySort};
use crate::ThemeIndex;

/// One candidate resolved to the declarations and selectors it emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    /// Snapshot-selected utility ordering before variant application.
    pub order: UtilityOrder,
    /// The parsed candidate, including its variants, flags, and modifier.
    pub candidate: Candidate,
    /// Registrations explicitly required by the selected utility branches.
    pub required_properties: Vec<String>,
    /// Source order of the matched `@stillwind-utility`.
    pub builtin_order: Option<usize>,
    /// Finalized declarations for the utility itself.
    pub declaration: String,
    /// Selector the declarations must be nested under, when the branch has one.
    pub selector: Option<String>,
    /// Conditional rules emitted next to the base declaration.
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads. CSS
    /// fallbacks and selector-bearing branches must never be discarded merely because
    /// they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

/// An extra selector target a resolved utility must also be emitted under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    /// Finalized declarations for this target.
    pub declaration: String,
    /// Selector this target is nested under, when it has one.
    pub selector: Option<String>,
    /// Nested rules emitted inside this target, in authored order.
    pub nested: Vec<NestedUtilityRule>,
}

/// Resolves a candidate using only the Tailwind-compatible registry.
///
/// An arbitrary property (`[color:red]`) becomes a plain declaration positioned by the
/// registry's arbitrary-property order; a named utility is resolved through the project
/// registry and then the selected built-ins. Importance is applied to every emitted
/// declaration, including conditional and additional targets, so the flag cannot be
/// lost by a branch that emits several rules.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            let mut resolution = UtilityResolution::plain(format!("{property}:{value};"));
            resolution.builtin_order = Some(arbitrary_property_order(property, theme));
            resolution
        }
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    let order = if theme.config.authored_utility_order {
        UtilityOrder::Authored(resolution.builtin_order)
    } else {
        let groups = std::iter::once(resolution.declaration.as_str())
            .chain(
                resolution
                    .nested
                    .iter()
                    .filter(|rule| !rule.compatibility)
                    .map(|rule| rule.declaration.as_str()),
            )
            .chain(resolution.additional.iter().flat_map(|target| {
                std::iter::once(target.declaration.as_str()).chain(
                    target
                        .nested
                        .iter()
                        .filter(|rule| !rule.compatibility)
                        .map(|rule| rule.declaration.as_str()),
                )
            }));
        UtilityOrder::Tailwind(UtilityPropertySort::from_groups(
            groups,
            resolution.tw_sort.as_deref(),
        ))
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
        order,
        required_properties: resolution.required_properties,
        candidate,
        builtin_order: resolution.builtin_order,
        declaration,
        selector,
        nested,
        additional,
    })
}

/// Resolves one animation option from the CSS-authored option registry. As a
/// compatibility path, ordinary utilities may project declarations through CSS-authored
/// property mappings without exposing their names to Rust.
///
/// Variants, negation, and importance are rejected: an animation option describes a
/// value, not a qualified utility. A plain declaration list is accepted; selectors,
/// conditionals and additional targets are rejected. The declaration list can contain
/// several properties.
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
        matching_utilities(builtin_animation_options(theme), name),
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

/// Projects an ordinary utility's declarations through the registry's animation-option
/// property mappings.
///
/// Properties without a mapping are dropped, and a utility that produced selectors,
/// conditionals, or extra targets is rejected rather than projected partially.
pub(crate) fn project_animation_option_utility(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let utility = resolve_utility(raw, theme)?;
    if !utility.candidate.variants.is_empty()
        || utility.selector.is_some()
        || !utility.nested.is_empty()
        || !utility.additional.is_empty()
    {
        return None;
    }
    let mappings = animation_option_property_mappings(theme);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::BuiltinDefinitions;

    #[test]
    fn css_registries_are_isolated_without_process_defaults() {
        let first = ThemeIndex {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-utility sample { color: red; }",
            )),
            ..ThemeIndex::default()
        };
        let second = ThemeIndex {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-utility sample { color: blue; }",
            )),
            ..ThemeIndex::default()
        };
        assert_eq!(
            resolve_utility("sample", &first).unwrap().declaration,
            "color:red;"
        );
        assert_eq!(
            resolve_utility("sample", &second).unwrap().declaration,
            "color:blue;"
        );
        assert_eq!(
            resolve_utility("sample", &first).unwrap().declaration,
            "color:red;"
        );
        assert!(resolve_utility("sample", &ThemeIndex::default()).is_none());
        assert!(resolve_utility("flex", &ThemeIndex::default()).is_none());
    }

    #[test]
    fn animation_projection_keeps_only_mapped_properties_and_rejects_variants() {
        let theme = ThemeIndex {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-animation-option-property transition-duration: animation-duration;\n@stillwind-animation-option-property transition-timing-function: animation-timing-function;",
            )),
            ..ThemeIndex::default()
        };
        assert_eq!(
            project_animation_option_utility("[transition-duration:120ms]", &theme).as_deref(),
            Some("animation-duration:120ms;")
        );
        assert_eq!(
            resolve_animation_option("[transition-timing-function:linear]", &theme).as_deref(),
            Some("animation-timing-function:linear;")
        );
        for candidate in [
            "hover:[transition-duration:120ms]",
            "![transition-duration:120ms]",
            "-[transition-duration:120ms]",
            "[color:red]",
            "missing",
        ] {
            assert!(
                resolve_animation_option(candidate, &theme).is_none(),
                "{candidate}"
            );
        }
    }
}
