//! The public utility-resolution entry points.
//!
//! Resolution is stateless: the same candidate and theme always produce the same
//! result. An immutable provider can reuse token lookups and parsed programs between
//! calls. [`ResolvedUtility`] carries the declarations and selectors needed for
//! emission. Its [`UtilityDiagnosticData`] retains source selections and value checks
//! for diagnostics and fixes. Resolution can retain a well-formed arbitrary value that
//! its property rejects, and it records that mismatch in the diagnostic data.
//!
//! [`resolve_utility`] explains candidates that fail to parse or resolve through
//! [`UtilityFailure`]. Malformed property requirements return
//! [`UtilityResolutionError`].

use std::ops::Range;

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{CssProperty, inspect_css_value};
use stillwind_syntax::{Candidate, Utility, parse_candidate};

use super::super::builtins::{COMPAT_COLOR_SUPPORT, arbitrary_property_order};
use super::super::values::color_fallback;
use super::body::{UtilityResolution, UtilityResolutionError};
use super::diagnosis::{
    UtilityFailure, arbitrary_value_range, diagnose_utility, modifier_value_range,
};
use super::formatting::finalize_declaration;
use super::matching::resolve_named_utility;
use super::nested::{NestedUtilityAtRule, NestedUtilityRule};
use super::order::{UtilityOrder, UtilityPropertySort};
use super::variables::VariableMismatch;
use crate::{ThemeIndex, UtilityOrderMode};

/// One candidate resolved to the declarations and selectors it emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    /// Source selections and value checks retained for diagnostics and fixes.
    pub diagnostic_data: UtilityDiagnosticData,
    /// Ordering key of this resolution in the snapshot's configured mode, computed
    /// before variant application.
    pub order: UtilityOrder,
    /// The parsed candidate, including its variants, flags, and modifier.
    pub candidate: Candidate,
    /// Registrations explicitly required by the selected utility branches.
    pub required_properties: Vec<LeanStr>,
    /// Authored position of the matched definition, from the import-expanded stylesheet
    /// when the snapshot orders by authored position and from the built-in registry
    /// otherwise.
    pub builtin_order: Option<usize>,
    /// Finalized declarations for the utility itself.
    pub declaration: LeanStr,
    /// Selector the declarations must be nested under, when the branch has one.
    pub selector: Option<LeanStr>,
    /// Conditional rules emitted next to the base declaration.
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads. CSS
    /// fallbacks and selector-bearing branches must never be discarded merely because
    /// they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

/// Source selections and value checks used to diagnose a resolved utility.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UtilityDiagnosticData {
    /// Direct token selections before inline values and compatibility fallbacks expand.
    pub token_uses: Vec<UtilityTokenUse>,
    /// Accepted arbitrary captures associated with their receiving declarations.
    pub arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// A value that its property rejects. The declarations contain it as written. See
    /// [`InvalidValueReason`] for the kinds of values.
    pub invalid_value: Option<InvalidPropertyValue>,
    /// A `var()` value without a type hint for a utility that accepts values of several
    /// types. `None` when `invalid_value` records the variable's type as rejected.
    pub untyped_variable: Option<UntypedVariable>,
}

/// A value that a utility emits although its property rejects it, such as `1/2` for
/// `width` in `w-[1/2]` or `[width:1/2]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidPropertyValue {
    /// Lowercase CSS property name receiving the value.
    pub property: LeanStr,
    /// Decoded value text as the declaration emits it.
    pub value: LeanStr,
    /// Byte range of the value within the candidate text: the bracketed or
    /// parenthesized value of a named utility, the text after the colon of an arbitrary
    /// property, or the text after the `/` of a modifier.
    pub range: Range<usize>,
    /// Why the property rejects the value.
    pub reason: InvalidValueReason,
}

/// Why the property of an [`InvalidPropertyValue`] rejects its value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidValueReason {
    /// The grammar of the property rejects an arbitrary value: a value without a type
    /// hint that no definition accepts, or the value of an arbitrary property.
    Grammar {
        /// Type hints of the value types that a named utility accepts, in registration
        /// order, when the utility accepts several types and the property was chosen
        /// because the value's type could not be inferred. Empty otherwise.
        fallback_type_hints: Vec<LeanStr>,
    },
    /// The value is one `var()` reference without a type hint, and the property
    /// rejects the variable's known type.
    VariableType(VariableMismatch),
    /// An opacity modifier gives `color-mix()` a percentage outside `0%` to `100%`.
    /// The value is that percentage, and the range covers the modifier.
    OpacityModifier,
}

/// A `var()` value written without a type hint for a utility whose definitions accept
/// values of several types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntypedVariable {
    /// Byte range of the value within the candidate text, such as `(--x)` or
    /// `[var(--x)]`.
    pub range: Range<usize>,
    /// Type hint of the definition that receives the value, such as `color`.
    pub hint: LeanStr,
}

/// Theme tokens selected by one emitted declaration before their values expand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityTokenUse {
    /// True when the declaration belongs to the utility's base list. False for nested
    /// or additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the selected tokens.
    pub property: LeanStr,
    /// Complete custom-property names. Aliases retain the directly selected name.
    pub tokens: Vec<LeanStr>,
}

/// One arbitrary source capture accepted by a utility declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityArbitraryUse {
    /// True for the base declaration list. False for nested and additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the capture.
    pub property: LeanStr,
    /// Decoded CSS text before token expansion and declaration formatting.
    pub value: LeanStr,
    /// The capture was accepted by a color value domain.
    pub color: bool,
}

/// An extra selector target a resolved utility must also be emitted under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    /// Finalized declarations for this target.
    pub declaration: LeanStr,
    /// Selector this target is nested under, when it has one.
    pub selector: Option<LeanStr>,
    /// Nested rules emitted inside this target, in authored order.
    pub nested: Vec<NestedUtilityRule>,
}

/// Resolves the utility part of a candidate, or returns `None` when no definition
/// accepts it.
///
/// An arbitrary property (`[color:red]`) becomes a plain declaration positioned by the
/// registry's arbitrary-property order; a named utility is resolved through the project
/// registry and then the selected built-ins. Importance is applied to every emitted
/// declaration, including conditional and additional targets, so the flag cannot be
/// lost by a branch that emits several rules.
///
/// # Errors
///
/// Returns a malformed property requirement from a matching utility definition.
#[allow(clippy::too_many_lines)]
fn resolve_candidate(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Option<ResolvedUtility>, UtilityResolutionError> {
    let Some(candidate) = parse_candidate(raw) else {
        return Ok(None);
    };
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            if !theme.accepts_arbitrary_property(&property.as_str().to_ascii_lowercase()) {
                return Ok(None);
            }
            let Some(value) = theme.inline_theme_functions(value) else {
                return Ok(None);
            };
            let value = value.as_str();
            let mut resolution =
                UtilityResolution::plain(format_args!("{property}:{value};").to_lean_string());
            if let Some(fallback) = color_fallback(&resolution.declaration, theme) {
                resolution.nested.push(NestedUtilityRule {
                    compatibility: true,
                    at_rule: NestedUtilityAtRule::Supports,
                    condition: COMPAT_COLOR_SUPPORT.to_lean_str(),
                    parent: None,
                    declaration: resolution.declaration.clone(),
                    inside: true,
                });
                resolution.declaration = fallback.into();
            }
            resolution
                .diagnostic_data
                .arbitrary_uses
                .push(UtilityArbitraryUse {
                    base: true,
                    property: (property.as_str().to_lean_string().to_ascii_lowercase()).into(),
                    value: value.to_lean_str(),
                    color: false,
                });
            let tokens = inspect_css_value(value).1;
            if !tokens.is_empty() {
                resolution.diagnostic_data.token_uses.push(UtilityTokenUse {
                    base: true,
                    property: (property.as_str().to_lean_string().to_ascii_lowercase()).into(),
                    tokens: tokens.into_iter().collect(),
                });
            }
            resolution.builtin_order = Some(match theme.utility_order() {
                UtilityOrderMode::Authored => arbitrary_property_order(property, theme)
                    .saturating_add(theme.utility_position_base()),
                UtilityOrderMode::Tailwind => arbitrary_property_order(property, theme),
            });
            if property_rejects(property, value) {
                resolution.diagnostic_data.invalid_value =
                    resolution.diagnostic_data.arbitrary_uses.first().cloned();
            }
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
    let order = match theme.utility_order() {
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
    let value_range = arbitrary_value_range(raw, &candidate);
    let reason = match resolution.diagnostic_data.variable_mismatch {
        Some(mismatch) => InvalidValueReason::VariableType(mismatch),
        None => InvalidValueReason::Grammar {
            fallback_type_hints: resolution.diagnostic_data.fallback_type_hints,
        },
    };
    let invalid_value = resolution
        .diagnostic_data
        .invalid_value
        .map(|usage| InvalidPropertyValue {
            property: usage.property,
            value: usage.value,
            range: value_range.clone(),
            reason,
        })
        .or_else(|| {
            let usage = resolution.diagnostic_data.out_of_range_alpha?;
            Some(InvalidPropertyValue {
                property: usage.property,
                value: usage.value,
                range: modifier_value_range(raw, &candidate)?,
                reason: InvalidValueReason::OpacityModifier,
            })
        });
    let untyped_variable = resolution
        .diagnostic_data
        .untyped_variable_hint
        .map(|hint| UntypedVariable {
            range: value_range,
            hint,
        });
    Ok(Some(ResolvedUtility {
        diagnostic_data: UtilityDiagnosticData {
            token_uses: resolution.diagnostic_data.token_uses,
            arbitrary_uses: resolution.diagnostic_data.arbitrary_uses,
            invalid_value,
            untyped_variable,
        },
        order,
        required_properties: resolution.required_properties,
        candidate,
        builtin_order: resolution.builtin_order,
        declaration: (declaration),
        selector,
        nested,
        additional,
    }))
}

/// Whether the grammar of `property` rejects the arbitrary property value `value`.
///
/// Custom properties accept every value. A property that the Webref data does not
/// define, and a value that contains `var()`, `env()`, or `attr()`, cannot be checked
/// and count as accepted.
fn property_rejects(property: &str, value: &str) -> bool {
    if property.starts_with("--") {
        return false;
    }
    let lowercase = value.to_ascii_lowercase();
    if ["var(", "env(", "attr("]
        .iter()
        .any(|function| lowercase.contains(function))
    {
        return false;
    }
    CssProperty::from_name(&property.to_ascii_lowercase())
        .is_some_and(|property| !property.accepts(value))
}

/// Resolves a candidate through the project registry and then the selected built-ins.
/// Explains a candidate that cannot be resolved.
///
/// [`super::check_candidate_variants`] checks the parsed variants separately. The inner
/// result is the resolution, or a [`UtilityFailure`] whose range lies within `raw`.
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
pub fn resolve_utility(
    raw: &str,
    theme: &ThemeIndex,
) -> Result<Result<ResolvedUtility, UtilityFailure>, UtilityResolutionError> {
    match resolve_candidate(raw, theme)? {
        Some(resolved) => Ok(Ok(resolved)),
        None => Ok(Err(diagnose_utility(
            raw,
            parse_candidate(raw).as_ref(),
            theme,
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::theme::ThemeBuilder;
    use crate::{BuiltinDefinitions, parse_theme};

    fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
        super::resolve_utility(raw, theme).unwrap().ok()
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

    /// A user branch that fails its value domain falls through to the built-in
    /// definition instead of resolving partially.
    #[test]
    fn failed_user_branch_can_resolve_through_the_builtin_registry() {
        let theme = parse_theme(
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
