//! Candidate-to-branch matching and branch merging.
//!
//! Because user CSS is the authoritative registry layer, a project definition must be
//! able to replace any built-in name when its branch resolves. If all user branches
//! fail, resolution still tries the built-in registry. Within one layer, every matching
//! definition is a typed declaration branch, and a candidate selects the branches whose
//! value domains accept it.
//!
//! Ordering is by branch specificity: an exact name outranks a wildcard, and a wildcard
//! with typed arbitrary or theme domains outranks a generic one. When an exact
//! definition exists, wildcard overloads are alternative grammar branches rather than
//! extra declarations for the same spelling.

use std::collections::HashSet;

use lean_string::{LeanString, ToLeanString};

use stillwind_css::{ValueType, declaration_ranges};
use stillwind_syntax::Value;

use super::super::values::decode_arbitrary;
use super::body::{UtilityResolution, UtilityResolutionError, resolve_css_utility_body};
use super::nested::NestedUtilityRule;
use super::resolve::ResolvedUtilityTarget;
use crate::{CustomUtility, ThemeIndex};

/// Resolves a named candidate through the registry layers.
///
/// Project CSS is consulted first; a successfully resolved project branch wins. If
/// no project branch resolves, built-in definitions are tried. There is deliberately no
/// property-prefix fallback: a definition that rejects the candidate's value type is a
/// non-match, not an invitation to reinterpret the candidate.
pub(in crate::evaluation) fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    // Application CSS is the authoritative registry layer. It must be able to replace
    // every built-in name.
    if let Some(declaration) = resolve_user_css_utility(name, modifier, negative, theme)? {
        return Ok(Some(declaration));
    }
    if let Some(declaration) = resolve_builtin_css_utility(name, modifier, negative, theme)? {
        return Ok(Some(declaration));
    }
    // A failed `@utility` branch is a type error, not an invitation to use a legacy
    // property-prefix fallback. Keeping that fallback here made `p-[color:red]` and
    // `border-[time:1ms]` appear valid after the typed registry correctly rejected
    // them. Arbitrary properties (`[x:y]`) are handled explicitly by the candidate
    // grammar above; named utilities are defined solely by CSS registry entries.
    Ok(None)
}

/// Resolves the same `@utility` body language for user CSS and vendored built-ins.
/// `--value(...)` tries its comma-separated value domains in order.
fn resolve_user_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let lookup = negative.then(|| format_args!("-{name}").to_lean_string());
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        theme.matching_utilities(lookup).into_iter(),
        lookup,
        modifier,
        theme,
    )
}

/// Resolves a candidate against the selected built-ins.
///
/// Wildcard families are searched before exact-name groups, and both iterate in reverse
/// authored order so the most recently registered definition is considered first;
/// branch specificity then decides which one wins.
fn resolve_builtin_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let lookup = negative.then(|| format_args!("-{name}").to_lean_string());
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        theme.registry().matching_utilities(lookup).into_iter(),
        lookup,
        modifier,
        theme,
    )
}

/// Every matching `@utility` block is a typed declaration branch. A candidate such as
/// `bg-[url(...)]` may therefore select the image branch while a color candidate
/// selects the color branch, without giving the root an implementation-specific special
/// case.
///
/// Branches are ordered by specificity, then merged: branches sharing a selector
/// contribute one declaration list, and branches with distinct selectors become
/// additional targets. A property already claimed by a more-specific branch is not
/// overwritten by a broader one. The most specific branch that carries a `tw_sort`
/// override supplies the retained one.
///
/// Without an exact definition, an unhinted arbitrary value keeps only the branches
/// that accept its inferred type. A `var()` reference or CSS-wide keyword can stand
/// for any type, so only the earliest registered branch that accepts it is kept.
pub(in crate::evaluation) fn resolve_css_utility_branches<'a>(
    utilities: impl Iterator<Item = &'a CustomUtility>,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let mut branches = Vec::new();
    // Registries list matching definitions newest first, so a project definition
    // without a built-in position registers earlier the later it appears here.
    for (index, utility) in utilities.enumerate() {
        if let Some(resolution) = resolve_css_utility_body(utility, name, modifier, theme)? {
            branches.push((
                utility_branch_specificity(utility, name),
                utility.name == name,
                utility_value_types(utility),
                resolution,
                utility.builtin_order.unwrap_or(usize::MAX - index),
            ));
        }
    }
    branches.sort_by_key(|(specificity, _, _, _, _)| std::cmp::Reverse(*specificity));
    let has_exact = branches.iter().any(|(_, exact, _, _, _)| *exact);
    if has_exact {
        // An exact registration defines the complete utility. Wildcard overloads are
        // alternative candidate grammar branches, not a source of extra declarations
        // for a spelling that has an exact definition.
        branches.retain(|(_, exact, _, _, _)| *exact);
    } else if let Some(arbitrary) = unhinted_arbitrary_value(name) {
        let available = branches
            .iter()
            .flat_map(|(_, _, domains, _, _)| domains.iter().copied())
            .collect::<Vec<_>>();
        if stands_for_any_type(&arbitrary) {
            // A `var()` reference or CSS-wide keyword selects one definition instead
            // of setting every property that accepts it: `border-(--x)` must not set
            // both border-width and border-color. Emission order can give every
            // overload of a name the same position, so the individual registrations
            // decide.
            let first = branches
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, _, _, _, registration))| *registration)
                .map(|(index, _)| index);
            branches = first
                .map(|index| branches.swap_remove(index))
                .into_iter()
                .collect();
        } else if let Some(selected) =
            infer_arbitrary_value_type(&arbitrary, &available).or_else(|| {
                branches
                    .iter()
                    .min_by_key(|(_, _, _, _, registration)| *registration)
                    .and_then(|(_, _, domains, _, _)| domains.first().copied())
            })
        {
            branches.retain(|(_, _, domains, _, _)| domains.contains(&selected));
        }
    }
    let Some((_, _, _, mut resolution, _)) = branches.first().cloned() else {
        return Ok(None);
    };
    for (_, _, _, branch, _) in branches.into_iter().skip(1) {
        // Overloads with a shared selector form one declaration list. CSS fallbacks
        // authored within one overload intentionally retain their order, while
        // competing overloads cannot overwrite a more-specific property branch.
        if resolution.tw_sort.is_none() {
            resolution.tw_sort.clone_from(&branch.tw_sort);
        }
        resolution
            .required_properties
            .extend(branch.required_properties);
        if branch.selector == resolution.selector {
            let existing = declaration_property_names(&resolution.declaration);
            resolution.arbitrary_uses.extend(
                branch
                    .arbitrary_uses
                    .into_iter()
                    .filter(|usage| !usage.base || !existing.contains(usage.property.as_str())),
            );
            resolution.token_uses.extend(
                branch
                    .token_uses
                    .into_iter()
                    .filter(|usage| !usage.base || !existing.contains(usage.property.as_str())),
            );
            resolution.declaration =
                (merge_branch_declarations(&resolution.declaration, &branch.declaration)).into();
            merge_nested_rules(&mut resolution.nested, branch.nested);
            resolution.additional.extend(branch.additional);
        } else {
            resolution
                .arbitrary_uses
                .extend(branch.arbitrary_uses.into_iter().map(|mut usage| {
                    usage.base = false;
                    usage
                }));
            resolution
                .token_uses
                .extend(branch.token_uses.into_iter().map(|mut usage| {
                    usage.base = false;
                    usage
                }));
            resolution.additional.push(ResolvedUtilityTarget {
                declaration: branch.declaration,
                selector: branch.selector,
                nested: branch.nested,
            });
            resolution.additional.extend(branch.additional);
        }
    }
    Ok(Some(resolution))
}

/// The value types a definition mentions in its `[type]` hints, in authored order and
/// without duplicates.
///
/// Used to pick between wildcard branches when an arbitrary value carries no hint of
/// its own.
fn utility_value_types(utility: &CustomUtility) -> Vec<ValueType> {
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

/// The value of an arbitrary capture that carries no type hint, decoded, or the `var()`
/// text of a variable capture.
///
/// [`None`] means the candidate named a hinted arbitrary value or is not an arbitrary
/// capture at all; hinted values are matched by domain instead.
fn unhinted_arbitrary_value(name: &str) -> Option<LeanString> {
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
        .then(|| format_args!("var({variable})").to_lean_string())
}

/// True for CSS-wide keywords and values that contain a `var()` reference, whose
/// type cannot be inferred from their text.
fn stands_for_any_type(value: &str) -> bool {
    value.contains("var(")
        || value.starts_with("--")
        || matches!(
            value.trim(),
            "initial" | "inherit" | "unset" | "revert" | "revert-layer"
        )
}

/// Chooses the value type an unhinted arbitrary value most likely means.
///
/// Only types the branch actually accepts are considered, the image and color families
/// are tried before geometry, and values for which [`stands_for_any_type`] holds are
/// never inferred.
fn infer_arbitrary_value_type(value: &str, available: &[ValueType]) -> Option<ValueType> {
    if stands_for_any_type(value) {
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

/// Adds conditional rules that do not conflict with an existing rule.
///
/// A conflict is the same conditional and placement declaring one of the same
/// properties; the primary rule keeps its position and value.
fn merge_nested_rules(primary: &mut Vec<NestedUtilityRule>, secondary: Vec<NestedUtilityRule>) {
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

/// The property names declared by a declaration list.
fn declaration_property_names(declaration: &str) -> HashSet<&str> {
    declaration_ranges(declaration)
        .into_iter()
        .map(|range| &declaration[range])
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(property, _)| property.trim())
        .collect()
}

/// Merges independent `@utility` overloads without erasing repeated fallback
/// declarations inside either body. For example `opacity-0` keeps its static
/// declaration instead of also gaining the generic percentage overload.
fn merge_branch_declarations(primary: &str, secondary: &str) -> LeanString {
    let mut properties = declaration_ranges(primary)
        .into_iter()
        .map(|range| &primary[range])
        .filter_map(|declaration| {
            declaration
                .split_once(':')
                .map(|(property, _)| property.trim())
        })
        .collect::<HashSet<_>>();
    let mut merged = primary.to_lean_string();
    for range in declaration_ranges(secondary) {
        let declaration = &secondary[range];
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

/// How strongly a branch claims a candidate spelling.
///
/// An exact name contributes a large constant so it outranks any combination of
/// wildcard metadata, and within wildcards the typed arbitrary, theme, and modifier
/// domains add weight. The exact-registration term keeps canonical spellings such as
/// `opacity-60` on their dedicated definition.
fn utility_branch_specificity(utility: &CustomUtility, name: &str) -> usize {
    let body = utility.body.as_str();
    let typed_arbitrary = body
        .matches('[')
        .count()
        .saturating_sub(body.matches("[*]").count());
    let theme_domain = body.matches("--value(--").count();
    let modifier = body.matches("--modifier(").count();
    // An exact registration is a semantic override of a wildcard family. This keeps the
    // canonical Tailwind spelling (`opacity-60`, `bg-none`, …) intact while still
    // letting typed wildcard branches discriminate arbitrary values among themselves.
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
            tw_sort: None,
        };
        let wildcard = CustomUtility {
            name: "w-*".into(),
            tw_sort: None,
            ..exact.clone()
        };
        assert_eq!(utility_value_types(&exact), vec![ValueType::Length]);
        assert!(
            utility_branch_specificity(&exact, "w-1")
                > utility_branch_specificity(&wildcard, "w-1")
        );
    }
}
