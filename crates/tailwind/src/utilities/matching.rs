use std::collections::HashSet;

use stillwind_css::{CustomUtility, ThemeIndex, ValueType};
use stillwind_grammar::Value;

use crate::builtins::registry::builtin_utilities;
use crate::values::arbitrary::decode_arbitrary;

use super::body::{UtilityResolution, resolve_css_utility_body};
use super::nested::NestedUtilityRule;
use super::resolve::ResolvedUtilityTarget;

pub(crate) fn matching_utilities<'a>(
    utilities: &'a [CustomUtility],
    name: &'a str,
) -> impl Iterator<Item = &'a CustomUtility> {
    utilities.iter().rev().filter(move |utility| {
        utility.name == name
            || utility
                .name
                .strip_suffix('*')
                .is_some_and(|prefix| name.starts_with(prefix))
    })
}

pub(crate) fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    // Application CSS is the authoritative registry layer. It must be able to
    // replace every built-in name.
    if let Some(declaration) = resolve_user_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    if let Some(declaration) = resolve_builtin_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    // A failed `@utility` branch is a type error, not an invitation to use a
    // legacy property-prefix fallback. Keeping that fallback here made
    // `p-[color:red]` and `border-[time:1ms]` appear valid after the typed
    // registry correctly rejected them. Arbitrary properties (`[x:y]`) are
    // handled explicitly by the candidate grammar above; named utilities are
    // defined solely by CSS registry entries.
    None
}

/// Resolves the same `@utility` body language for user CSS and vendored
/// built-ins. `--value(...)` tries its comma-separated value domains in order.
pub(crate) fn resolve_user_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&theme.utilities, lookup),
        lookup,
        modifier,
        theme,
    )
}

pub(crate) fn resolve_builtin_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if !theme.imports.builtins {
        return None;
    }
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&builtin_utilities(theme).wildcards, lookup).chain(
            builtin_utilities(theme)
                .builtin_exact
                .get(lookup)
                .into_iter()
                .flat_map(|utilities| utilities.iter().rev()),
        ),
        lookup,
        modifier,
        theme,
    )
}

/// Every matching `@utility` block is a typed declaration branch. A candidate
/// such as `bg-[url(...)]` may therefore select the image branch while a
/// colour candidate selects the colour branch, without giving the root an
/// implementation-specific special case.
pub(crate) fn resolve_css_utility_branches<'a>(
    utilities: impl Iterator<Item = &'a CustomUtility>,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let mut branches = utilities
        .filter_map(|utility| {
            resolve_css_utility_body(utility, name, modifier, theme).map(|resolution| {
                (
                    utility_branch_specificity(utility, name),
                    utility.name == name,
                    utility_value_types(utility),
                    resolution,
                )
            })
        })
        .collect::<Vec<_>>();
    branches.sort_by_key(|(specificity, _, _, _)| std::cmp::Reverse(*specificity));
    let has_exact = branches.iter().any(|(_, exact, _, _)| *exact);
    if has_exact {
        // An exact registration defines the complete utility. Wildcard
        // overloads are alternative candidate grammar branches, not a source
        // of extra declarations for a spelling that has an exact definition.
        branches.retain(|(_, exact, _, _)| *exact);
    } else if let Some(arbitrary) = unhinted_arbitrary_value(name) {
        let available = branches
            .iter()
            .flat_map(|(_, _, domains, _)| domains.iter().copied())
            .collect::<Vec<_>>();
        let selected = infer_arbitrary_value_type(&arbitrary, &available).or_else(|| {
            branches
                .iter()
                .min_by_key(|(_, _, _, resolution)| resolution.builtin_order)
                .and_then(|(_, _, domains, _)| domains.first().copied())
        });
        if let Some(selected) = selected {
            branches.retain(|(_, _, domains, _)| domains.contains(&selected));
        }
    }
    let (_, _, _, mut resolution) = branches.first()?.clone();
    for (_, _, _, branch) in branches.into_iter().skip(1) {
        // Overloads with a shared selector form one declaration list. CSS
        // fallbacks authored within one overload intentionally retain their
        // order, while competing overloads cannot overwrite a more-specific
        // property branch.
        if branch.selector == resolution.selector {
            resolution.declaration =
                merge_branch_declarations(&resolution.declaration, &branch.declaration);
            merge_nested_rules(&mut resolution.nested, branch.nested);
            resolution.additional.extend(branch.additional);
        } else {
            resolution.additional.push(ResolvedUtilityTarget {
                declaration: branch.declaration,
                selector: branch.selector,
                nested: branch.nested,
            });
            resolution.additional.extend(branch.additional);
        }
    }
    Some(resolution)
}

pub(crate) fn utility_value_types(utility: &CustomUtility) -> Vec<ValueType> {
    let mut types = Vec::new();
    let mut rest = utility.body.as_str();
    while let Some(start) = rest.find('[') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find(']') else {
            break;
        };
        if let Some(value_type) = ValueType::from_hint(&rest[..end])
            && !types.contains(&value_type)
        {
            types.push(value_type);
        }
        rest = &rest[end + 1..];
    }
    types
}

pub(crate) fn unhinted_arbitrary_value(name: &str) -> Option<String> {
    if let Some(start) = name.rfind("-[")
        && let Some(value) = name[start + 2..].strip_suffix(']')
    {
        let (hint, value) = value.split_once(':').unwrap_or(("", value));
        return hint.is_empty().then(|| decode_arbitrary(value));
    }
    let start = name.rfind("-(")?;
    let variable = name[start + 2..].strip_suffix(')')?;
    variable
        .starts_with("--")
        .then(|| format!("var({variable})"))
}

pub(crate) fn infer_arbitrary_value_type(
    value: &str,
    available: &[ValueType],
) -> Option<ValueType> {
    if value.contains("var(")
        || value.starts_with("--")
        || matches!(
            value.trim(),
            "initial" | "inherit" | "unset" | "revert" | "revert-layer"
        )
    {
        return None;
    }
    [
        ValueType::Image,
        ValueType::Color,
        ValueType::Percentage,
        ValueType::Position,
        ValueType::BackgroundSize,
        ValueType::Length,
        ValueType::Url,
    ]
    .into_iter()
    .filter(|value_type| available.contains(value_type))
    .find(|value_type| value_type.accepts(value))
    .or_else(|| {
        available
            .iter()
            .copied()
            .find(|value_type| value_type.accepts(value))
    })
}

pub(crate) fn merge_nested_rules(
    primary: &mut Vec<NestedUtilityRule>,
    secondary: Vec<NestedUtilityRule>,
) {
    for rule in secondary {
        let properties = declaration_property_names(&rule.declaration);
        let conflicts = primary.iter().any(|existing| {
            existing.at_rule == rule.at_rule
                && existing.condition == rule.condition
                && existing.inside == rule.inside
                && declaration_property_names(&existing.declaration)
                    .iter()
                    .any(|property| properties.contains(property))
        });
        if !conflicts {
            primary.push(rule);
        }
    }
}

pub(crate) fn declaration_property_names(declaration: &str) -> HashSet<&str> {
    declaration
        .split_terminator(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(property, _)| property.trim())
        .collect()
}

/// Merges independent `@utility` overloads without erasing repeated fallback
/// declarations inside either body. For example `opacity-0` keeps its static
/// declaration instead of also gaining the generic percentage overload.
pub(crate) fn merge_branch_declarations(primary: &str, secondary: &str) -> String {
    let mut properties = primary
        .split_terminator(';')
        .filter_map(|declaration| {
            declaration
                .split_once(':')
                .map(|(property, _)| property.trim())
        })
        .collect::<HashSet<_>>();
    let mut merged = primary.to_owned();
    for declaration in secondary.split_terminator(';') {
        let Some((property, _)) = declaration.split_once(':') else {
            continue;
        };
        if properties.insert(property.trim()) {
            merged.push_str(declaration);
            merged.push(';');
        }
    }
    merged
}

pub(crate) fn utility_branch_specificity(utility: &CustomUtility, name: &str) -> usize {
    let body = utility.body.as_str();
    let typed_arbitrary = body
        .matches('[')
        .count()
        .saturating_sub(body.matches("[*]").count());
    let theme_domain = body.matches("--value(--").count();
    let modifier = body.matches("--modifier(").count();
    // An exact registration is a semantic override of a wildcard family.
    // This keeps the canonical Tailwind spelling (`opacity-60`, `bg-none`,
    // …) intact while still letting typed wildcard branches discriminate
    // arbitrary values among themselves.
    usize::from(utility.name == name) * 32 + typed_arbitrary * 4 + theme_domain * 2 + modifier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_merging_keeps_primary_values_and_adds_only_missing_properties() {
        assert_eq!(
            merge_branch_declarations("color:red;display:block;", "color:blue;width:1px;"),
            "color:red;display:block;width:1px;"
        );
        assert_eq!(
            declaration_property_names(" color:red; color:blue; width:1px;"),
            HashSet::from(["color", "width"])
        );
        assert_eq!(unhinted_arbitrary_value("w-[2_px]"), Some("2 px".into()));
        assert_eq!(unhinted_arbitrary_value("w-[length:2px]"), None);
        assert_eq!(
            unhinted_arbitrary_value("w-(--size)"),
            Some("var(--size)".into())
        );
        assert_eq!(
            infer_arbitrary_value_type("var(--x)", &[ValueType::Length]),
            None
        );
        assert_eq!(
            infer_arbitrary_value_type("red", &[ValueType::Length, ValueType::Color]),
            Some(ValueType::Color)
        );
    }
    #[test]
    fn exact_branches_outrank_wildcards_and_value_types_are_deduplicated() {
        let exact = CustomUtility {
            name: "w-1".into(),
            body: "width:--value([length]);x:--value([length]);".into(),
            builtin_order: None,
        };
        let wildcard = CustomUtility {
            name: "w-*".into(),
            ..exact.clone()
        };
        assert_eq!(utility_value_types(&exact), vec![ValueType::Length]);
        assert!(
            utility_branch_specificity(&exact, "w-1")
                > utility_branch_specificity(&wildcard, "w-1")
        );
        assert_eq!(matching_utilities(&[exact, wildcard], "missing").count(), 0);
    }
}
