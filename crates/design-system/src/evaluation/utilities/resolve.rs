//! The public utility-resolution entry points.
//!
//! Resolution is stateless: the same candidate and theme always produce the same
//! result. An immutable provider can reuse token lookups and parsed programs between
//! calls. Candidates that fail to parse, match
//! no definition, or match a branch that rejects their value all produce [`None`]; a
//! caller that needs to distinguish "unknown utility" from "wrong value type" should
//! read the diagnostics of the layer above.

use std::fmt::Write;
use std::sync::Arc;

use stillwind_css::inspect_css_value;
use stillwind_syntax::{Candidate, Utility, parse_candidate};

use super::super::builtins::registry::{
    animation_option_property_mappings, arbitrary_property_order, builtin_animation_options,
};
use super::body::{UtilityResolution, UtilityResolutionError};
use super::formatting::finalize_declaration;
use super::matching::{resolve_css_utility_branches, resolve_named_utility};
use super::nested::NestedUtilityRule;
use super::order::{UtilityOrder, UtilityPropertySort};
use crate::{ThemeIndex, UtilityOrderMode};

/// One candidate resolved to the declarations and selectors it emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    /// Direct token selections before inline values and compatibility fallbacks expand.
    pub token_uses: Vec<UtilityTokenUse>,
    /// Accepted arbitrary captures associated with their receiving declarations.
    pub arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// Ordering key of this resolution in the snapshot's configured mode, computed
    /// before variant application.
    pub order: UtilityOrder,
    /// The parsed candidate, including its variants, flags, and modifier.
    pub candidate: Candidate,
    /// Registrations explicitly required by the selected utility branches.
    pub required_properties: Vec<String>,
    /// Authored position of the matched definition, from the import-expanded stylesheet
    /// when the snapshot orders by authored position and from the built-in registry
    /// otherwise.
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

/// Theme tokens selected by one emitted declaration before their values expand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityTokenUse {
    /// True when the declaration belongs to the utility's base list; false for nested or additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the selected tokens.
    pub property: String,
    /// Complete custom-property names; aliases retain the directly selected name.
    pub tokens: Vec<String>,
}

/// One arbitrary source capture accepted by a utility declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityArbitraryUse {
    /// True for the base declaration list; false for nested and additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the capture.
    pub property: String,
    /// Decoded CSS text before token expansion and declaration formatting.
    pub value: String,
    /// The capture was accepted by a color value domain.
    pub color: bool,
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
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
#[allow(clippy::too_many_lines)]
pub fn resolve_utility(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Option<ResolvedUtility>, UtilityResolutionError> {
    let Some(candidate) = parse_candidate(raw) else {
        return Ok(None);
    };
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            let mut resolution = UtilityResolution::plain(format!("{property}:{value};"));
            resolution.arbitrary_uses.push(UtilityArbitraryUse {
                base: true,
                property: property.to_ascii_lowercase(),
                value: value.clone(),
                color: false,
            });
            let tokens = inspect_css_value(value).1;
            if !tokens.is_empty() {
                resolution.token_uses.push(UtilityTokenUse {
                    base: true,
                    property: property.to_ascii_lowercase(),
                    tokens: tokens.into_iter().collect(),
                });
            }
            resolution.builtin_order = Some(arbitrary_property_order(property, theme));
            resolution
        }
        Utility::Named { name, modifier } => {
            let Some(resolution) =
                resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
            else {
                return Ok(None);
            };
            resolution
        }
    };
    let order = match theme.config().utility_order {
        UtilityOrderMode::Authored => UtilityOrder::Authored(resolution.builtin_order),
        UtilityOrderMode::Tailwind => {
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
    Ok(Some(ResolvedUtility {
        token_uses: resolution.token_uses,
        arbitrary_uses: resolution.arbitrary_uses,
        order,
        required_properties: resolution.required_properties,
        candidate,
        builtin_order: resolution.builtin_order,
        declaration,
        selector,
        nested,
        additional,
    }))
}

/// Resolves one animation option from the CSS-authored option registry. As a
/// compatibility path, ordinary utilities may project declarations through CSS-authored
/// property mappings without exposing their names to Rust.
///
/// Variants, negation, and importance are rejected: an animation option describes a
/// value, not a qualified utility. A plain declaration list is accepted; selectors,
/// conditionals and additional targets are rejected. The declaration list can contain
/// several properties.
///
/// # Errors
///
/// Returns a malformed requirement from a matching option or utility definition.
pub fn resolve_animation_option(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Option<String>, UtilityResolutionError> {
    let Some(candidate) = parse_candidate(raw) else {
        return Ok(None);
    };
    if candidate.important || candidate.negative || !candidate.variants.is_empty() {
        return Ok(None);
    }
    let Utility::Named { name, modifier } = &candidate.utility else {
        return project_animation_option_utility(raw, theme);
    };
    if let Some(resolution) = resolve_css_utility_branches(
        builtin_animation_options(theme)
            .iter()
            .rev()
            .map(Arc::as_ref)
            .filter(|utility| {
                utility.name == *name
                    || utility
                        .name
                        .strip_suffix('*')
                        .is_some_and(|prefix| name.starts_with(prefix))
            }),
        name,
        modifier.as_ref(),
        theme,
    )? {
        return Ok((resolution.selector.is_none()
            && resolution.nested.is_empty()
            && resolution.additional.is_empty())
        .then(|| finalize_declaration(resolution.declaration, false, theme)));
    }
    project_animation_option_utility(raw, theme)
}

/// Projects an ordinary utility's declarations through the registry's animation-option
/// property mappings.
///
/// Properties without a mapping are dropped, and a utility that produced selectors,
/// conditionals, or extra targets is rejected rather than projected partially.
pub(crate) fn project_animation_option_utility(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Option<String>, UtilityResolutionError> {
    let Some(utility) = resolve_utility(raw, theme)? else {
        return Ok(None);
    };
    if !utility.candidate.variants.is_empty()
        || utility.selector.is_some()
        || !utility.nested.is_empty()
        || !utility.additional.is_empty()
    {
        return Ok(None);
    }
    let mappings = animation_option_property_mappings(theme);
    let mut projected = String::new();
    for declaration in utility
        .declaration
        .split(';')
        .filter(|part| !part.is_empty())
    {
        let Some((property, value)) = declaration.split_once(':') else {
            return Ok(None);
        };
        let Some(target) = mappings.get(property) else {
            continue;
        };
        let _ = write!(&mut projected, "{target}:{value};");
    }
    Ok((!projected.is_empty()).then_some(projected))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::BuiltinDefinitions;
    use crate::theme::builder::ThemeBuilder;

    fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
        super::resolve_utility(raw, theme).unwrap()
    }

    fn resolve_animation_option(raw: &str, theme: &ThemeIndex) -> Option<String> {
        super::resolve_animation_option(raw, theme).unwrap()
    }

    fn project_animation_option_utility(raw: &str, theme: &ThemeIndex) -> Option<String> {
        super::project_animation_option_utility(raw, theme).unwrap()
    }

    #[test]
    fn css_registries_are_isolated_without_process_defaults() {
        let first = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-utility sample { color: red; }",
            )),
            ..Default::default()
        }
        .finish();
        let second = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-utility sample { color: blue; }",
            )),
            ..Default::default()
        }
        .finish();
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
        let theme = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-animation-option-property transition-duration: animation-duration;\n@stillwind-animation-option-property transition-timing-function: animation-timing-function;",
            )),
            ..Default::default()
        }.finish();
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

    /// A user branch that fails its value domain falls through to the built-in
    /// definition instead of resolving partially.
    #[test]
    fn failed_user_branch_can_resolve_through_the_builtin_registry() {
        let theme = crate::parse_theme(
            &SourceFile::new(
                FileId(1), None, None,
                "@stillwind-utility x-* { color: --value([color]); } @utility x-* { width: --value([length]); }",
            )
            .unwrap(),
            &CssSourceName::from("test.css"),
        )
        .unwrap();
        let resolved = resolve_utility("x-[red]", &theme).unwrap();
        assert!(
            resolved.declaration.contains("color:red"),
            "{}",
            resolved.declaration
        );
        assert!(!resolved.declaration.contains("width:"));
    }
}
