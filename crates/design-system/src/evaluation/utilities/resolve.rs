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
use stillwind_syntax::{Candidate, Utility, Value, parse_candidate};

use super::super::stillwind::{COMPAT_COLOR_SUPPORT, arbitrary_property_order};
use super::super::values::color_fallback;
use super::body::{UtilityResolution, UtilityResolutionError};
use super::diagnosis::{UtilityFailure, diagnose_utility};
use super::formatting::finalize_declaration;
use super::matching::resolve_named_utility;
use super::nested::{NestedUtilityAtRule, NestedUtilityRule};
use super::order::{UtilityOrder, UtilityPropertySort};
use super::ranges::{arbitrary_value_range, modifier_value_range};
use super::variables::VariableMismatch;
use crate::{ThemeIndex, UtilityOrderMode};

/// One candidate resolved to the declarations and selectors it emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    /// Classifications of declarations retained in the primary target. An empty list
    /// requires consumers to inspect `declaration` as ordinary CSS.
    pub policy_declarations: Vec<UtilityDeclaration>,
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
    /// when the snapshot orders by authored position and the directive offset of a
    /// `@stillwind-utility` definition otherwise.
    pub position: Option<usize>,
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

/// A resolved declaration with the classification used by lint policies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityDeclaration {
    /// Decoded property name; ordinary properties use lowercase.
    pub property: LeanStr,
    /// Serialized CSS value without its semicolon. Definition records retain authored
    /// `!important` before candidate priority and runtime guards are added.
    pub value: LeanStr,
    /// True for fixed CSS from an `(implementation)` definition. Declarations with
    /// executable placeholders remain false, including those that select theme values.
    pub implementation: bool,
    /// True for a direct declaration of its recorded group. Collectors that flatten
    /// CSS mark every recovered declaration as base.
    pub base: bool,
}

/// A value that a utility emits although its property rejects it, such as `1/2` for
/// `width` in `w-[1/2]` or `[width:1/2]`, or although no value type of the utility
/// accepts it and no property grammar can confirm it.
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
    /// Why the value is invalid.
    pub reason: InvalidValueReason,
}

/// Why the value of an [`InvalidPropertyValue`] is invalid.
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
    /// No value type of a named utility accepts an arbitrary value without a type hint,
    /// and no property grammar decides the value: only custom properties receive it, or
    /// its declaration contains `var()`, `env()`, or `attr()`. `property` names the
    /// property that receives the value, which need not reject it.
    ValueType {
        /// Type hints as [`InvalidValueReason::Grammar`] records them.
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

/// Direct token selections and custom-property references of one retained declaration
/// before inline expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityTokenUse {
    /// True when these uses come from fixed CSS in an `(implementation)` definition.
    /// Uses from declarations with executable placeholders remain false.
    pub implementation: bool,
    /// True when the declaration belongs to the utility's base list. False for nested
    /// or additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the selected tokens.
    /// Custom property names retain case; ordinary names are lowercase.
    pub property: LeanStr,
    /// Complete custom-property names, including unresolved runtime references.
    /// Consumers must resolve names against the theme before treating them as tokens.
    /// Aliases retain the directly selected name.
    pub tokens: Vec<LeanStr>,
}

/// One arbitrary source capture accepted by a utility declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityArbitraryUse {
    /// True for the base declaration list. False for nested and additional targets.
    pub base: bool,
    /// Decoded CSS property name receiving the capture.
    /// Custom property names retain case; ordinary names are lowercase.
    pub property: LeanStr,
    /// Decoded CSS text before token expansion and declaration formatting.
    pub value: LeanStr,
    /// The capture was accepted by a color value domain.
    pub color: bool,
}

/// An extra selector target a resolved utility must also be emitted under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    /// Classifications of declarations retained in this target. An empty list requires
    /// consumers to inspect `declaration` as ordinary CSS.
    pub policy_declarations: Vec<UtilityDeclaration>,
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
/// registry and then the Stillwind definitions. Importance is applied to every emitted
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
        Utility::ArbitraryProperty {
            property,
            value,
            modifier,
        } => {
            if !theme.accepts_arbitrary_property(&property.as_str().to_ascii_lowercase()) {
                return Ok(None);
            }
            let Some(value) = theme.inline_theme_functions(value) else {
                return Ok(None);
            };
            let Some(value) = theme.resolve_theme_references(&value).ok().flatten() else {
                return Ok(None);
            };
            let Some(value) = theme.resolve_css_functions(&value) else {
                return Ok(None);
            };
            let value = if let Some(modifier) = modifier {
                let opacity = match modifier {
                    Value::Named(value) => format!("{value}%"),
                    Value::CssVariable(value) => format!("var({value})"),
                    Value::Arbitrary(value) => value
                        .parse::<f64>()
                        .map_or_else(|_| value.to_string(), |value| format!("{}%", value * 100.0)),
                };
                if opacity == "100%" {
                    value
                } else {
                    format!("color-mix(in oklab,{value} {opacity},transparent)").into()
                }
            } else {
                value
            };
            let value = value.as_str();
            let mut resolution =
                UtilityResolution::plain(format_args!("{property}:{value};").to_lean_string());
            if let Some(fallback) = color_fallback(&resolution.declaration, theme) {
                resolution.nested.push(NestedUtilityRule {
                    policy_declarations: Vec::new(),
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
                    property: diagnostic_property(property),
                    value: value.to_lean_str(),
                    color: false,
                });
            let tokens = inspect_css_value(value).1;
            if !tokens.is_empty() {
                resolution.diagnostic_data.token_uses.push(UtilityTokenUse {
                    implementation: false,
                    base: true,
                    property: diagnostic_property(property),
                    tokens: tokens.into_iter().collect(),
                });
            }
            resolution.position = Some(match theme.utility_order() {
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
        UtilityOrderMode::Authored => UtilityOrder::Authored(resolution.position),
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
        None if resolution.diagnostic_data.invalid_value_unchecked => {
            InvalidValueReason::ValueType {
                fallback_type_hints: resolution.diagnostic_data.fallback_type_hints,
            }
        }
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
        policy_declarations: resolution.policy_declarations,
        diagnostic_data: UtilityDiagnosticData {
            token_uses: resolution.diagnostic_data.token_uses,
            arbitrary_uses: resolution.diagnostic_data.arbitrary_uses,
            invalid_value,
            untyped_variable,
        },
        order,
        required_properties: resolution.required_properties,
        candidate,
        position: resolution.position,
        declaration: (declaration),
        selector,
        nested,
        additional,
    }))
}

/// Trims `property` and lowercases ordinary property names. Custom property names
/// retain case because they identify distinct declarations.
pub(super) fn diagnostic_property(property: &str) -> LeanStr {
    let property = property.trim();
    if property.starts_with("--") {
        property.to_lean_str()
    } else {
        property.to_lean_string().to_ascii_lowercase().into()
    }
}

/// Whether the grammar of `property` rejects the arbitrary property value `value`.
///
/// Custom properties accept every value. A value that [`property_accepts`] cannot
/// check counts as accepted.
fn property_rejects(property: &str, value: &str) -> bool {
    property_accepts(property, value) == Some(false)
}

/// Checks `value` against the grammar of `property`.
///
/// Returns `None` when no grammar decides the value: for a custom property, for a
/// property that the Webref data does not define, and for a value that contains
/// `var()`, `env()`, or `attr()`, because substitution happens at computed-value time.
pub(super) fn property_accepts(property: &str, value: &str) -> Option<bool> {
    if property.starts_with("--") {
        return None;
    }
    let lowercase = value.to_ascii_lowercase();
    if ["var(", "env(", "attr("]
        .iter()
        .any(|function| lowercase.contains(function))
    {
        return None;
    }
    CssProperty::from_name(&property.to_ascii_lowercase()).map(|property| property.accepts(value))
}

/// Resolves a candidate through the project registry and then the Stillwind definitions.
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
    use crate::{StillwindDefinitions, parse_theme};

    fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
        super::resolve_utility(raw, theme).unwrap().ok()
    }

    #[test]
    fn css_registries_are_isolated_without_process_defaults() {
        let first = ThemeBuilder {
            registry: Arc::new(StillwindDefinitions::from_css(
                "@stillwind-utility sample { color: red; }",
            )),
            ..Default::default()
        }
        .finish();
        let second = ThemeBuilder {
            registry: Arc::new(StillwindDefinitions::from_css(
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

    /// A user branch that fails its value domain falls through to the
    /// `@stillwind-utility` definition instead of resolving partially.
    #[test]
    fn failed_user_branch_can_resolve_through_stillwind_definitions() {
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
