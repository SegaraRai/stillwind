//! Expansion of one CSS utility body into resolved declarations.
//!
//! A body may carry a selector, base declarations, conditional declarations, and a
//! parent selector wrapper. This module splits those apart and resolves each
//! declaration group independently, which is what lets one broad definition serve
//! several candidate spellings: a declaration whose typed placeholder does not match
//! the candidate is dropped while its siblings survive.
//!
//! A slash modifier is only accepted when a branch actually consumes it, so a color
//! branch cannot silently accept an opacity modifier by accident. Conditional rules are
//! emitted as authored, including support queries that the compatibility color path
//! adds as a second branch of one declaration.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{PropertyRequirementError, ValueType, take_property_requirements};
use stillwind_source::ReplaceText;
use stillwind_syntax::Value;

use super::super::ArbitraryValueCheck;
use super::super::builtins::COMPAT_COLOR_SUPPORT;
use super::declarations::resolve_css_declarations;
use super::nested::{
    NestedUtilityAtRule, NestedUtilityRule, ParsedNestedUtilityRule, split_nested_conditions,
    split_utility_selector,
};
use super::resolve::{ResolvedUtilityTarget, UtilityArbitraryUse, UtilityTokenUse};
use super::variables::VariableMismatch;
use crate::{CustomUtility, ThemeIndex, ThemeNamespace, UtilityOrderMode};

/// A failed utility resolution caused by a malformed requirement in the matched definition.
#[derive(Clone, Debug, thiserror::Error)]
#[error("utility `{name}`: {source}")]
pub struct UtilityResolutionError {
    /// Registered utility definition containing the directive.
    pub name: LeanStr,
    /// Syntax failure and its offset within the definition body.
    #[source]
    pub source: PropertyRequirementError,
}

/// The resolved outcome of one matched utility definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::evaluation) struct UtilityResolution {
    pub(in crate::evaluation) token_uses: Vec<UtilityTokenUse>,
    pub(in crate::evaluation) arbitrary_uses: Vec<UtilityArbitraryUse>,
    /// Declarations for the matched selector, or for the utility itself.
    pub(in crate::evaluation) declaration: LeanStr,
    /// Explicit custom-property registration dependencies collected from requirement
    /// directives, independent of textual var references.
    pub(in crate::evaluation) required_properties: Vec<LeanStr>,
    /// Selector template from the body, when the body wrapped its declarations.
    pub(in crate::evaluation) selector: Option<LeanStr>,
    /// Conditional declarations to emit next to the base declaration.
    pub(in crate::evaluation) nested: Vec<NestedUtilityRule>,
    /// Extra selector targets produced by further overloads.
    pub(in crate::evaluation) additional: Vec<ResolvedUtilityTarget>,
    /// Authored position of the matched definition: its registration offset when the
    /// snapshot orders by authored position, otherwise the built-in registry offset.
    /// `None` when the definition has no position, as for synthetic rules.
    pub(in crate::evaluation) builtin_order: Option<usize>,
    /// `(tw-sort: property)` value authored on the definition header, which replaces
    /// the resolved property indices when the snapshot selects Tailwind ordering.
    pub(in crate::evaluation) tw_sort: Option<LeanStr>,
    /// An unhinted arbitrary value that no definition accepted by grammar, emitted as
    /// written.
    pub(in crate::evaluation) invalid_value: Option<UtilityArbitraryUse>,
    /// Type hints of the value types that the utility accepts, when `invalid_value`
    /// received a property chosen as a fallback between them; otherwise empty.
    pub(in crate::evaluation) fallback_type_hints: Vec<LeanStr>,
    /// A declaration that mixes a color with an alpha percentage outside `0%` to
    /// `100%`, with that percentage as its value.
    pub(in crate::evaluation) out_of_range_alpha: Option<UtilityArbitraryUse>,
    /// Value types that a definition selected for a `var()` value names in `[type]`
    /// hints or, without hints, declares for its theme namespaces.
    pub(in crate::evaluation) variable_value_types: Vec<ValueType>,
    /// Type hint of the selected definition, when the value is one `var()` reference
    /// without a type hint and the utility accepts values of several types.
    pub(in crate::evaluation) untyped_variable_hint: Option<LeanStr>,
    /// The known type of the `var()` value that `invalid_value` records, when the
    /// receiving property rejects that type.
    pub(in crate::evaluation) variable_mismatch: Option<VariableMismatch>,
}

impl UtilityResolution {
    /// A declaration with no selector, conditionals, or ordering.
    pub(in crate::evaluation) fn plain(declaration: LeanString) -> Self {
        Self {
            token_uses: Vec::new(),
            arbitrary_uses: Vec::new(),
            declaration: (declaration).into(),
            required_properties: Vec::new(),
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
            builtin_order: None,
            tw_sort: None,
            invalid_value: None,
            fallback_type_hints: Vec::new(),
            out_of_range_alpha: None,
            variable_value_types: Vec::new(),
            untyped_variable_hint: None,
            variable_mismatch: None,
        }
    }
}

/// Resolves one definition body against a candidate name and modifier.
/// `utility` must belong to `theme` or its selected built-in registry.
///
/// `arbitrary_check` selects how typed domains check arbitrary values without a type
/// hint. Returns [`None`] when the body uses `@slot` (that is a variant, not a utility), when
/// the candidate carries a modifier no branch consumes, or when no declaration group
/// matches.
///
/// # Errors
///
/// Returns a malformed property requirement with its definition name and byte offset.
#[allow(clippy::too_many_lines)]
pub(in crate::evaluation) fn resolve_css_utility_body(
    utility: &CustomUtility,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    arbitrary_check: ArbitraryValueCheck,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let Some(prepared) = theme.programs().body(&utility.body) else {
        return Ok(None);
    };
    let body = prepared.as_ref().map_err(|source| UtilityResolutionError {
        name: utility.name.to_lean_str(),
        source: *source,
    })?;
    Ok(body.as_ref().and_then(|body| {
        resolve_valid_utility_body(utility, name, modifier, theme, body, arbitrary_check)
    }))
}

/// Structural utility syntax retained by the immutable evaluator.
#[derive(Debug)]
pub(in crate::evaluation) struct PreparedUtilityBody {
    body: LeanStr,
    selector: Option<LeanStr>,
    parent_body: Option<LeanStr>,
    nested: Vec<ParsedNestedUtilityRule>,
    required_properties: Vec<LeanStr>,
    consumes_modifier: bool,
}

impl PreparedUtilityBody {
    pub(in crate::evaluation) fn parse(
        text: &str,
    ) -> Result<Option<Self>, PropertyRequirementError> {
        if text.contains("@slot") {
            return Ok(None);
        }
        let (text, required_properties) = take_property_requirements(text)?;
        let Some((selector, direct, parent_body)) = split_utility_selector(&text) else {
            return Ok(None);
        };
        let Some((body, nested)) = split_nested_conditions(direct) else {
            return Ok(None);
        };
        let consumes_modifier = body.contains("--modifier")
            || body.contains("--fraction()")
            || nested
                .iter()
                .any(|rule| rule.body.contains("--modifier") || rule.body.contains("--fraction()"));
        Ok(Some(Self {
            body: (body).into(),
            selector: selector.map(Into::into),
            parent_body: (parent_body.map(LeanString::from)).map(Into::into),
            nested,
            required_properties,
            consumes_modifier,
        }))
    }

    pub(in crate::evaluation) fn declaration_groups(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.body.as_str())
            .chain(self.parent_body.as_deref())
            .chain(self.nested.iter().map(|rule| rule.body.as_str()))
    }
}

#[allow(clippy::too_many_lines)] // One definition body resolves base, nested, and parent branches.
fn resolve_valid_utility_body(
    utility: &CustomUtility,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    prepared: &PreparedUtilityBody,
    arbitrary_check: ArbitraryValueCheck,
) -> Option<UtilityResolution> {
    let body = &prepared.body;
    let nested = &prepared.nested;
    let selector = prepared.selector.clone();
    let parent_body = prepared.parent_body.as_deref();
    let required_properties = prepared.required_properties.clone();
    let resolve = |body: &str, color_compatibility| {
        resolve_css_declarations(
            body,
            utility.name.as_str(),
            name,
            modifier,
            theme,
            utility.builtin_order.is_some(),
            color_compatibility,
            arbitrary_check,
        )
    };
    // A slash modifier is meaningful only when a branch explicitly consumes it. This
    // prevents a broad base branch (for example `bg-*`'s color declaration) from
    // accepting `bg-red-500/not-an-opacity` merely because another overload happened to
    // define modifiers. Fractions are the one exception: their slash is deliberately
    // reassembled by `--fraction()`.
    let consumes_modifier = prepared.consumes_modifier;
    if modifier.is_some() && !consumes_modifier {
        return None;
    }
    let declarations = resolve(body, true)?;
    let mut out_of_range_alpha = declarations.out_of_range_alpha;
    let mut token_uses = declarations.token_uses;
    let mut arbitrary_uses = declarations.arbitrary_uses;
    let declaration = declarations.base;
    let mut modifier_resolved = body.contains("--modifier(")
        || body.contains("--modifier-map(")
        || body.contains("--fraction()");
    let mut resolved_nested = Vec::new();
    for modern in declarations.compat_modern {
        resolved_nested.push(NestedUtilityRule {
            compatibility: true,
            at_rule: NestedUtilityAtRule::Supports,
            condition: COMPAT_COLOR_SUPPORT.to_lean_str(),
            parent: None,
            declaration: modern,
            inside: true,
        });
    }
    for ParsedNestedUtilityRule {
        at_rule,
        condition,
        body,
    } in nested
    {
        for (condition, body) in expand_theme_wildcard_nested(condition, body, theme) {
            let uses_modifier = body.contains("--modifier(")
                || body.contains("--modifier-map(")
                || body.contains("--fraction()");
            let color_compatibility =
                *at_rule != NestedUtilityAtRule::Supports || !condition.contains("color-mix(");
            let Some(declarations) = resolve(&body, color_compatibility) else {
                continue;
            };
            let declaration = declarations.base;
            if out_of_range_alpha.is_none() {
                out_of_range_alpha = declarations.out_of_range_alpha.map(|mut usage| {
                    usage.base = false;
                    usage
                });
            }
            arbitrary_uses.extend(declarations.arbitrary_uses.into_iter().map(|mut usage| {
                usage.base = false;
                usage
            }));
            token_uses.extend(declarations.token_uses.into_iter().map(|mut usage| {
                usage.base = false;
                usage
            }));
            if !declaration.is_empty() {
                modifier_resolved |= uses_modifier;
                resolved_nested.push(NestedUtilityRule {
                    compatibility: false,
                    at_rule: *at_rule,
                    condition: condition.to_lean_str(),
                    parent: None,
                    declaration: (declaration).into(),
                    inside: true,
                });
                for modern in declarations.compat_modern {
                    resolved_nested.push(NestedUtilityRule {
                        compatibility: true,
                        at_rule: NestedUtilityAtRule::Supports,
                        condition: COMPAT_COLOR_SUPPORT.to_lean_str(),
                        parent: Some((*at_rule, condition.to_lean_str())),
                        declaration: modern,
                        inside: true,
                    });
                }
            }
        }
    }
    if modifier.is_some() && consumes_modifier && !modifier_resolved {
        return None;
    }
    if let Some(parent_body) = parent_body {
        let parent = resolve(parent_body, true)?;
        for usage in &mut token_uses {
            usage.base = false;
        }
        for usage in &mut arbitrary_uses {
            usage.base = false;
        }
        arbitrary_uses.extend(parent.arbitrary_uses);
        if let Some(usage) = &mut out_of_range_alpha {
            usage.base = false;
        }
        out_of_range_alpha = parent.out_of_range_alpha.or(out_of_range_alpha);
        token_uses.extend(parent.token_uses);
        let parent_declaration = parent.base;
        let parent_nested = parent
            .compat_modern
            .into_iter()
            .map(|modern| NestedUtilityRule {
                compatibility: true,
                at_rule: NestedUtilityAtRule::Supports,
                condition: COMPAT_COLOR_SUPPORT.to_lean_str(),
                parent: None,
                declaration: modern,
                inside: true,
            })
            .collect();
        let additional = (!declaration.is_empty() || !resolved_nested.is_empty()).then(|| {
            vec![ResolvedUtilityTarget {
                declaration: (declaration).into(),
                selector,
                nested: resolved_nested,
            }]
        })?;
        (!parent_declaration.is_empty() || !additional.is_empty()).then_some(UtilityResolution {
            token_uses,
            arbitrary_uses,
            required_properties,
            declaration: (parent_declaration).into(),
            selector: None,
            nested: parent_nested,
            additional,
            builtin_order: match theme.utility_order() {
                UtilityOrderMode::Authored => theme
                    .utility_positions()
                    .get(utility.name.as_str())
                    .copied(),
                UtilityOrderMode::Tailwind => utility.builtin_order,
            },
            tw_sort: utility.tw_sort.clone(),
            invalid_value: None,
            fallback_type_hints: Vec::new(),
            out_of_range_alpha: out_of_range_alpha.clone(),
            variable_value_types: Vec::new(),
            untyped_variable_hint: None,
            variable_mismatch: None,
        })
    } else {
        (!declaration.is_empty() || !resolved_nested.is_empty()).then_some(UtilityResolution {
            token_uses,
            arbitrary_uses,
            required_properties,
            declaration: (declaration).into(),
            selector,
            nested: resolved_nested,
            additional: Vec::new(),
            builtin_order: match theme.utility_order() {
                UtilityOrderMode::Authored => theme
                    .utility_positions()
                    .get(utility.name.as_str())
                    .copied(),
                UtilityOrderMode::Tailwind => utility.builtin_order,
            },
            tw_sort: utility.tw_sort.clone(),
            invalid_value: None,
            fallback_type_hints: Vec::new(),
            out_of_range_alpha: out_of_range_alpha.clone(),
            variable_value_types: Vec::new(),
            untyped_variable_hint: None,
            variable_mismatch: None,
        })
    }
}

/// Expands a conditional whose prelude names a theme namespace pattern into one rule
/// per active token, ordered by numeric value.
///
/// This is how a wildcard definition such as `max-w-*` learns its variants from the
/// theme. Tokens that are not numeric lengths sort last, so an unparsable value cannot
/// displace a real scale.
fn expand_theme_wildcard_nested(
    condition: &str,
    body: &str,
    theme: &ThemeIndex,
) -> Vec<(LeanString, LeanString)> {
    let marker_start = condition.find("--value(--");
    let Some(marker_start) = marker_start else {
        return vec![(condition.to_lean_string(), body.to_lean_string())];
    };
    let Some(marker_end) = condition[marker_start..].find(')') else {
        return vec![(condition.to_lean_string(), body.to_lean_string())];
    };
    let marker_end = marker_start + marker_end + 1;
    let marker = &condition[marker_start..marker_end];
    let Some(namespace) = marker
        .strip_prefix("--value(--")
        .and_then(|value| value.strip_suffix("-*)"))
        .and_then(ThemeNamespace::parse)
    else {
        return vec![(condition.to_lean_string(), body.to_lean_string())];
    };
    let mut tokens = theme.tokens_in(namespace);
    tokens.sort_by(|left, right| {
        css_length_sort_key(&left.value.value)
            .partial_cmp(&css_length_sort_key(&right.value.value))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tokens
        .into_iter()
        .map(|token| {
            let value = token.value.value.clone();
            (
                condition.replace_text(marker, &value),
                body.replace_text(marker, &value),
            )
        })
        .collect()
}

/// Sort key for a CSS length, normalizing `rem` and `em` to pixels.
///
/// A value that is not a plain length is infinite, which keeps it after every
/// measurable length regardless of the author's unit choice.
fn css_length_sort_key(value: &str) -> f64 {
    let value = value.trim();
    for (unit, scale) in [("rem", 16.0), ("em", 16.0), ("px", 1.0)] {
        if let Some(number) = value.strip_suffix(unit)
            && let Ok(number) = number.trim().parse::<f64>()
        {
            return number * scale;
        }
    }
    f64::INFINITY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_sort_keys_normalize_rem_to_pixels() {
        assert!((css_length_sort_key("1rem") - 16.0).abs() < f64::EPSILON);
        assert!((css_length_sort_key("8px") - 8.0).abs() < f64::EPSILON);
    }
}
