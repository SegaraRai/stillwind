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

use stillwind_syntax::Value;

use super::super::builtins::metadata::{COMPAT_COLOR_SUPPORT, builtin_property_definitions};
use super::super::syntax::closing_parenthesis;
use super::declarations::resolve_css_declarations;
use super::nested::{
    NestedUtilityAtRule, NestedUtilityRule, ParsedNestedUtilityRule, split_nested_conditions,
    split_utility_selector,
};
use super::resolve::ResolvedUtilityTarget;
use crate::{CustomUtility, ThemeIndex, ThemeNamespace};

/// The resolved outcome of one matched utility definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UtilityResolution {
    /// Declarations for the matched selector, or for the utility itself.
    pub(crate) declaration: String,
    /// Selector template from the body, when the body wrapped its declarations.
    pub(crate) selector: Option<String>,
    /// Conditional declarations to emit next to the base declaration.
    pub(crate) nested: Vec<NestedUtilityRule>,
    /// Extra selector targets produced by further overloads.
    pub(crate) additional: Vec<ResolvedUtilityTarget>,
    /// Authored position of the matched built-in, used for ordering.
    pub(crate) builtin_order: Option<usize>,
}

impl UtilityResolution {
    /// A declaration with no selector, conditionals, or ordering.
    pub(crate) fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
            builtin_order: None,
        }
    }
}

/// Resolves one definition body against a candidate name and modifier.
///
/// Returns [`None`] when the body uses `@slot` (that is a variant, not a utility), when
/// the candidate carries a modifier no branch consumes, or when no declaration group
/// matches.
pub(crate) fn resolve_css_utility_body(
    utility: &CustomUtility,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if utility.body.contains("@slot") {
        return None;
    }
    let (selector, body, parent_body) = split_utility_selector(&utility.body)?;
    let (body, nested) = split_nested_conditions(body)?;
    // A slash modifier is meaningful only when a branch explicitly consumes it. This
    // prevents a broad base branch (for example `bg-*`'s color declaration) from
    // accepting `bg-red-500/not-an-opacity` merely because another overload happened to
    // define modifiers. Fractions are the one exception: their slash is deliberately
    // reassembled by `--fraction()`.
    let consumes_modifier = body.contains("--modifier")
        || body.contains("--fraction()")
        || nested
            .iter()
            .any(|rule| rule.body.contains("--modifier") || rule.body.contains("--fraction()"));
    if modifier.is_some() && !consumes_modifier {
        return None;
    }
    let declarations =
        resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
    let declaration = normalize_registered_var_fallbacks(declarations.base, theme);
    let mut modifier_resolved = body.contains("--modifier(")
        || body.contains("--modifier-map(")
        || body.contains("--fraction()");
    let mut resolved_nested = Vec::new();
    for modern in declarations.compat_modern {
        resolved_nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: COMPAT_COLOR_SUPPORT.to_owned(),
            declaration: normalize_registered_var_fallbacks(modern, theme),
            inside: true,
        });
    }
    for ParsedNestedUtilityRule {
        at_rule,
        condition,
        body,
    } in nested
    {
        for (condition, body) in expand_theme_wildcard_nested(&condition, &body, theme) {
            let uses_modifier = body.contains("--modifier(")
                || body.contains("--modifier-map(")
                || body.contains("--fraction()");
            let Some(declarations) =
                resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)
            else {
                continue;
            };
            let declaration = normalize_registered_var_fallbacks(declarations.base, theme);
            if !declaration.is_empty() {
                modifier_resolved |= uses_modifier;
                resolved_nested.push(NestedUtilityRule {
                    at_rule,
                    condition,
                    declaration,
                    inside: true,
                });
                for modern in declarations.compat_modern {
                    resolved_nested.push(NestedUtilityRule {
                        at_rule: NestedUtilityAtRule::Supports,
                        condition: COMPAT_COLOR_SUPPORT.to_owned(),
                        declaration: normalize_registered_var_fallbacks(modern, theme),
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
        let parent_declaration =
            resolve_css_declarations(parent_body, utility.name.as_str(), name, modifier, theme)?
                .base;
        let additional = (!declaration.is_empty() || !resolved_nested.is_empty()).then(|| {
            vec![ResolvedUtilityTarget {
                declaration,
                selector,
                nested: resolved_nested,
            }]
        })?;
        (!parent_declaration.is_empty() || !additional.is_empty()).then_some(UtilityResolution {
            declaration: parent_declaration,
            selector: None,
            nested: Vec::new(),
            additional,
            builtin_order: utility.builtin_order,
        })
    } else {
        (!declaration.is_empty() || !resolved_nested.is_empty()).then_some(UtilityResolution {
            declaration,
            selector,
            nested: resolved_nested,
            additional: Vec::new(),
            builtin_order: utility.builtin_order,
        })
    }
}

/// Removes a `var()` fallback when it is exactly the registered initial value
/// of that custom property. The registry metadata is the source of truth;
/// no Tailwind property family is encoded here.
pub(crate) fn normalize_registered_var_fallbacks(
    mut declaration: String,
    theme: &ThemeIndex,
) -> String {
    let mut cursor = 0;
    while let Some(relative) = declaration[cursor..].find("var(--") {
        let start = cursor + relative;
        let Some(open) = declaration[start..].find('(').map(|offset| start + offset) else {
            break;
        };
        let Some(close) = closing_parenthesis(&declaration, open) else {
            break;
        };
        let expression = &declaration[open + 1..close];
        let Some((name, fallback)) = expression.split_once(',') else {
            cursor = close + 1;
            continue;
        };
        let name = name.trim().to_owned();
        let fallback = fallback.trim();
        let property = theme
            .properties
            .iter()
            .find(|property| property.name == name)
            .or_else(|| {
                builtin_property_definitions(theme)
                    .iter()
                    .find(|property| property.name == name)
            });
        let matches_initial = property.is_some_and(|property| {
            property.body.split(';').map(str::trim).any(|entry| {
                entry
                    .strip_prefix("initial-value:")
                    .is_some_and(|value| value.trim() == fallback)
            })
        });
        if matches_initial {
            declaration.replace_range(open + 1..close, &name);
            cursor = open + 1 + name.len();
        } else {
            cursor = close + 1;
        }
    }
    declaration
}

/// Expands a conditional whose prelude names a theme namespace pattern into one rule
/// per active token, ordered by numeric value.
///
/// This is how a wildcard definition such as `max-w-*` learns its variants from the
/// theme. Tokens that are not numeric lengths sort last, so an unparsable value cannot
/// displace a real scale.
pub(crate) fn expand_theme_wildcard_nested(
    condition: &str,
    body: &str,
    theme: &ThemeIndex,
) -> Vec<(String, String)> {
    let marker_start = condition.find("--value(--");
    let Some(marker_start) = marker_start else {
        return vec![(condition.to_owned(), body.to_owned())];
    };
    let Some(marker_end) = condition[marker_start..].find(')') else {
        return vec![(condition.to_owned(), body.to_owned())];
    };
    let marker_end = marker_start + marker_end + 1;
    let marker = &condition[marker_start..marker_end];
    let Some(namespace) = marker
        .strip_prefix("--value(--")
        .and_then(|value| value.strip_suffix("-*)"))
        .and_then(ThemeNamespace::parse)
    else {
        return vec![(condition.to_owned(), body.to_owned())];
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
                condition.replace(marker, &value),
                body.replace(marker, &value),
            )
        })
        .collect()
}

/// Sort key for a CSS length, normalizing `rem` and `em` to pixels.
///
/// A value that is not a plain length is infinite, which keeps it after every
/// measurable length regardless of the author's unit choice.
pub(crate) fn css_length_sort_key(value: &str) -> f64 {
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
