//! Candidate-to-branch matching and branch merging.
//!
//! As in Tailwind, every definition of a name contributes: a project `@utility` adds to
//! a `@stillwind-utility` of the same name instead of replacing it. The definitions are
//! evaluated in Tailwind's registration order, `@stillwind-utility` definitions first,
//! then plugin registrations, then the stylesheet's `@utility` definitions, and each
//! one that resolves is emitted in that order. Within one registry, every matching
//! definition is a typed declaration branch, and a candidate selects the branches whose
//! value domains accept it.
//!
//! Ordering is by branch specificity: an exact name outranks a wildcard, and a wildcard
//! with typed arbitrary or theme domains outranks a generic one. When an exact
//! definition exists, wildcard overloads are alternative grammar branches rather than
//! extra declarations for the same spelling.

use std::collections::HashSet;

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{ValueType, declaration_ranges, split_top_level};
use stillwind_syntax::{Value, decode_arbitrary_value, utility_registration_key};

use super::super::context::ArbitraryValueCheck;
use super::super::values::{expand_placeholder, theme_domain_pattern};
use super::body::{
    UtilityResolution, UtilityResolutionDiagnosticData, UtilityResolutionError,
    resolve_css_utility_body,
};
use super::nested::NestedUtilityRule;
use super::resolve::ResolvedUtilityTarget;
use super::variables::{single_variable, variable_mismatch};
use crate::{CustomUtility, ThemeIndex};

/// Returns the named spelling that the bracket utility name `name` resolves as, such as
/// `icon-mdi--close` for `icon-[mdi--close]`.
///
/// Returns `None` when `name` is not the bracket form of a candidate of a dynamic
/// family that accepts both forms, or when a definition accepts the bracket value by
/// grammar.
///
/// # Errors
///
/// Returns a malformed requirement from a matching utility definition.
pub fn dynamic_named_spelling(
    name: &str,
    theme: &ThemeIndex,
) -> Result<Option<LeanString>, UtilityResolutionError> {
    let Some(named) = theme.dynamic_named_utility(name) else {
        return Ok(None);
    };
    Ok(
        resolve_registry_layers(name, None, false, theme, ArbitraryValueCheck::Grammar)?
            .is_none()
            .then_some(named),
    )
}

/// Resolves a named candidate through the registry layers.
///
/// Project CSS is consulted first; a successfully resolved project branch wins. If
/// no project branch resolves, `@stillwind-utility` definitions are tried. There is
/// deliberately no property-prefix fallback: a definition that rejects the candidate's
/// value type is a non-match, not an invitation to reinterpret the candidate.
///
/// An arbitrary value without a type hint that no definition accepts by grammar is
/// emitted as written, as Tailwind emits it, and the resolution records it in
/// `invalid_value`. When the matching definitions accept values of several types, the
/// resolution also records their type hints, because the receiving property is then a
/// fallback rather than the type of the value. Branch selection then follows the rules
/// for an unhinted value whose type cannot be inferred. This fallback does not apply to
/// a `var()` reference, to a value that is not well formed, or when the candidate
/// resolves without its modifier, because then the modifier, not the value, is
/// rejected.
///
/// A definition that accepts the bracket value of a dynamic family's bracket candidate
/// by grammar resolves that candidate. `icon-[2rem]` then selects an `icon-*`
/// definition that accepts `[length]`. Otherwise, the candidate belongs to the family
/// and skips the fallback for unhinted values. When the family accepts both forms, the
/// candidate resolves as its named spelling: `icon-[mdi--close]` selects
/// `icon-mdi--close`.
pub(in crate::evaluation) fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let check = ArbitraryValueCheck::Grammar;
    let lookup = negative.then(|| format_args!("-{name}").to_lean_string());
    if let Some(mut resolution) = resolve_registry_layers(name, modifier, negative, theme, check)? {
        check_variable(
            &mut resolution.diagnostic_data,
            lookup.as_deref().unwrap_or(name),
            theme,
        );
        return Ok(Some(resolution));
    }
    if let Some((_, true)) = theme.dynamic_family(name) {
        return match theme.dynamic_named_utility(name) {
            Some(named) => resolve_named_utility(&named, modifier, negative, theme),
            None => Ok(None),
        };
    }
    let Some(value) = unhinted_arbitrary_value(lookup.as_deref().unwrap_or(name)) else {
        return Ok(None);
    };
    if stands_for_any_type(&value)
        || (modifier.is_some()
            && resolve_registry_layers(name, None, negative, theme, check)?.is_some())
    {
        return Ok(None);
    }
    let check = ArbitraryValueCheck::WellFormed;
    let Some(mut resolution) = resolve_registry_layers(name, modifier, negative, theme, check)?
    else {
        return Ok(None);
    };
    resolution.diagnostic_data.invalid_value = resolution
        .diagnostic_data
        .arbitrary_uses
        .iter()
        .find(|usage| usage.base)
        .or_else(|| resolution.diagnostic_data.arbitrary_uses.first())
        .cloned();
    resolution.diagnostic_data.fallback_type_hints =
        fallback_type_hints(lookup.as_deref().unwrap_or(name), theme);
    Ok(Some(resolution))
}

/// Records the type hint of the selected definition and a type mismatch when the value
/// of `lookup` is one `var()` reference without a type hint.
///
/// A mismatch is recorded only when no other invalid value is recorded. The hint is
/// recorded only when the utility accepts values of several types and no mismatch is
/// recorded, because the hint of the selected definition would state the type that the
/// property rejects.
fn check_variable(
    diagnostic_data: &mut UtilityResolutionDiagnosticData,
    lookup: &str,
    theme: &ThemeIndex,
) {
    let Some(value) = unhinted_arbitrary_value(lookup) else {
        return;
    };
    let Some(variable) = single_variable(&value) else {
        return;
    };
    let hints = fallback_type_hints(lookup, theme);
    if diagnostic_data.invalid_value.is_none()
        && let Some((usage, mismatch)) =
            variable_mismatch(variable, &diagnostic_data.token_uses, &hints, theme)
    {
        diagnostic_data.invalid_value = Some(usage);
        diagnostic_data.variable_mismatch = Some(mismatch);
        return;
    }
    if !hints.is_empty() {
        diagnostic_data.untyped_variable_hint = diagnostic_data
            .variable_value_types
            .first()
            .map(|value_type| value_type.hint().to_lean_str());
    }
}

/// Resolves a named candidate with every definition of its name and combines the ones
/// that resolve, in Tailwind's registration order.
///
/// The `@stillwind-utility` definitions form one group of typed branches, the other
/// definitions from `@plugin` statements form a second group, and the stylesheet's
/// `@utility` definitions form a third, as [`resolve_css_utility_branches`] describes.
/// The `(functional)` definitions of the complete candidate also stand for plugin
/// registrations. They are evaluated after the `@stillwind-utility` definitions and
/// before the other plugin definitions. Tailwind registers every plugin before the
/// stylesheet's `@utility` rules, regardless of where `@plugin` appears, and the later
/// definition wins in the cascade.
///
/// A candidate is static when an exact definition in either registry has its complete
/// name, including a leading `-` and the modifier, because Tailwind parses a candidate
/// with a static utility only as that utility. Then only the exact definitions apply,
/// and a name such as `@utility foo-1/2` consumes the modifier. Otherwise the candidate
/// is functional, and an exact definition of the name without the modifier remains one
/// branch of its registry: the package defines `bg-linear-to-r` exactly but accept
/// `bg-linear-to-r/oklch` through `bg-linear-to-*`.
///
/// A `(stop)` definition ends the evaluation and keeps what earlier definitions
/// produced. `(fallback)` definitions apply only when nothing else resolves.
fn resolve_registry_layers(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
    arbitrary_check: ArbitraryValueCheck,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let lookup = negative.then(|| format_args!("-{name}").to_lean_string());
    let lookup = lookup.as_deref().unwrap_or(name);
    let key = utility_registration_key(name, modifier, negative);
    if theme.has_exact_utility(&key) || theme.registry().has_exact_utility(&key) {
        let resolve_exact = |mut definitions: Vec<&CustomUtility>| {
            definitions.retain(|definition| definition.name == key);
            resolve_css_utility_branches(
                definitions.into_iter(),
                &key,
                None,
                theme,
                arbitrary_check,
            )
        };
        let mut resolution = resolve_exact(theme.registry().matching_utilities(&key))?;
        let (plugins, stylesheet): (Vec<_>, Vec<_>) = theme
            .matching_utilities(&key)
            .into_iter()
            .partition(|definition| definition.plugin);
        for group in [plugins, stylesheet] {
            if let Some(next) = resolve_exact(group)? {
                append_resolution(&mut resolution, next);
            }
        }
        return Ok(resolution);
    }
    let resolve_group = |definitions: Vec<&CustomUtility>| {
        resolve_css_utility_branches(
            definitions.into_iter(),
            lookup,
            modifier,
            theme,
            arbitrary_check,
        )
    };
    let mut resolution = resolve_group(theme.registry().matching_utilities(lookup))?;
    let mut fallbacks = Vec::new();
    for definition in theme.functional_utilities(&key) {
        if definition.role.fallback {
            fallbacks.push(definition);
            continue;
        }
        if definition.role.stop {
            return Ok(resolution);
        }
        if let Some(plugin) =
            resolve_css_utility_body(definition, &key, None, theme, arbitrary_check)?
        {
            append_resolution(&mut resolution, plugin);
        }
    }
    let (plugins, stylesheet): (Vec<_>, Vec<_>) = theme
        .matching_utilities(lookup)
        .into_iter()
        .partition(|definition| definition.plugin);
    for group in [plugins, stylesheet] {
        if let Some(next) = resolve_group(group)? {
            append_resolution(&mut resolution, next);
        }
    }
    if resolution.is_some() {
        return Ok(resolution);
    }
    for definition in fallbacks {
        if definition.role.stop {
            break;
        }
        if let Some(plugin) =
            resolve_css_utility_body(definition, &key, None, theme, arbitrary_check)?
        {
            append_resolution(&mut resolution, plugin);
        }
    }
    Ok(resolution)
}

/// Adds the CSS of a later definition to the resolution of earlier ones.
///
/// Declarations for the same selector are appended, so a later definition wins in the
/// cascade, as it does in Tailwind's output. A different selector becomes an additional
/// target.
fn append_resolution(resolution: &mut Option<UtilityResolution>, next: UtilityResolution) {
    let Some(base) = resolution else {
        *resolution = Some(next);
        return;
    };
    base.position = base.position.max(next.position);
    if base.tw_sort.is_none() {
        base.tw_sort = next.tw_sort;
    }
    if base.selector == next.selector {
        base.declaration = format_args!("{}{}", base.declaration, next.declaration).to_lean_str();
        base.nested.extend(next.nested);
    } else {
        base.additional.push(ResolvedUtilityTarget {
            declaration: next.declaration,
            selector: next.selector,
            nested: next.nested,
        });
    }
    base.additional.extend(next.additional);
    base.required_properties.extend(next.required_properties);
    base.diagnostic_data
        .token_uses
        .extend(next.diagnostic_data.token_uses);
    base.diagnostic_data
        .arbitrary_uses
        .extend(next.diagnostic_data.arbitrary_uses);
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
/// for any type, so only the earliest registered branch that accepts it is kept. The
/// same rule applies when `arbitrary_check` accepts every well-formed value, because
/// such a value has no type to infer.
pub(in crate::evaluation) fn resolve_css_utility_branches<'a>(
    utilities: impl Iterator<Item = &'a CustomUtility>,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    arbitrary_check: ArbitraryValueCheck,
) -> Result<Option<UtilityResolution>, UtilityResolutionError> {
    let mut branches = Vec::new();
    let variable = unhinted_arbitrary_value(name).is_some_and(|value| value.contains("var("));
    // Registries list matching definitions newest first, so a project definition
    // without a directive position registers earlier the later it appears here.
    for (index, utility) in utilities.enumerate() {
        if let Some(mut resolution) =
            resolve_css_utility_body(utility, name, modifier, theme, arbitrary_check)?
        {
            if variable {
                resolution.diagnostic_data.variable_value_types =
                    declared_value_types(utility, theme);
            }
            branches.push((
                utility_branch_specificity(utility, name),
                utility.name == name,
                utility_value_types(utility),
                resolution,
                utility.position.unwrap_or(usize::MAX - index),
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
        if stands_for_any_type(&arbitrary) || arbitrary_check == ArbitraryValueCheck::WellFormed {
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
    let mut branches = branches.into_iter();
    let Some((_, _, _, mut resolution, base_registration)) = branches.next() else {
        return Ok(None);
    };
    // Registration of each target in `resolution.additional`, for ordering below.
    let mut registrations = vec![base_registration; resolution.additional.len()];
    for (_, _, _, branch, registration) in branches {
        // Overloads with a shared selector form one declaration list. CSS fallbacks
        // authored within one overload intentionally retain their order, while
        // competing overloads cannot overwrite a more-specific property branch.
        if resolution.tw_sort.is_none() {
            resolution.tw_sort.clone_from(&branch.tw_sort);
        }
        merge_out_of_range_alpha(&mut resolution, &branch);
        let diagnostic_data = &mut resolution.diagnostic_data;
        let branch_data = branch.diagnostic_data;
        resolution
            .required_properties
            .extend(branch.required_properties);
        if branch.selector == resolution.selector {
            let existing = declaration_property_names(&resolution.declaration);
            diagnostic_data.arbitrary_uses.extend(
                branch_data
                    .arbitrary_uses
                    .into_iter()
                    .filter(|usage| !usage.base || !existing.contains(usage.property.as_str())),
            );
            diagnostic_data.token_uses.extend(
                branch_data
                    .token_uses
                    .into_iter()
                    .filter(|usage| !usage.base || !existing.contains(usage.property.as_str())),
            );
            resolution.declaration =
                (merge_branch_declarations(&resolution.declaration, &branch.declaration)).into();
            merge_nested_rules(&mut resolution.nested, branch.nested);
        } else {
            diagnostic_data.extend_non_base_uses(branch_data);
            registrations.push(registration);
            resolution.additional.push(ResolvedUtilityTarget {
                declaration: branch.declaration,
                selector: branch.selector,
                nested: branch.nested,
            });
        }
        registrations.extend(std::iter::repeat_n(registration, branch.additional.len()));
        resolution.additional.extend(branch.additional);
    }
    order_targets_by_registration(&mut resolution, base_registration, registrations);
    Ok(Some(resolution))
}

/// Reorders the targets of `resolution` so that targets with distinct selectors follow
/// the registration order of their definitions, as Tailwind emits the rules of several
/// definitions of one name.
///
/// Merging visits definitions newest first, so a later definition wins for a shared
/// selector. Without this step, rules with distinct selectors would also be emitted
/// newest first: with the typography plugin, `.prose > :first-child { margin-top: 0 }`
/// would precede `.prose p { margin-top: 1.25em }` and lose to it.
///
/// `base_registration` is the registration of the primary target, and `registrations`
/// holds one registration for each target in `resolution.additional`. Smaller values
/// were registered earlier. The sort is stable, so targets of one definition keep
/// their order.
fn order_targets_by_registration(
    resolution: &mut UtilityResolution,
    base_registration: usize,
    registrations: Vec<usize>,
) {
    if registrations
        .iter()
        .all(|registration| *registration >= base_registration)
        && registrations.is_sorted()
    {
        return;
    }
    let primary = ResolvedUtilityTarget {
        declaration: std::mem::take(&mut resolution.declaration),
        selector: std::mem::take(&mut resolution.selector),
        nested: std::mem::take(&mut resolution.nested),
    };
    let mut targets: Vec<_> = std::iter::once((base_registration, primary))
        .chain(
            registrations
                .into_iter()
                .zip(resolution.additional.drain(..)),
        )
        .collect();
    targets.sort_by_key(|(registration, _)| *registration);
    let mut targets = targets.into_iter().map(|(_, target)| target);
    if let Some(primary) = targets.next() {
        resolution.declaration = primary.declaration;
        resolution.selector = primary.selector;
        resolution.nested = primary.nested;
    }
    resolution.additional = targets.collect();
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

/// Takes the out-of-range alpha of `branch` when `resolution` has none. The use stays
/// in the base list only when the branch shares the selector of `resolution`.
fn merge_out_of_range_alpha(resolution: &mut UtilityResolution, branch: &UtilityResolution) {
    if resolution.diagnostic_data.out_of_range_alpha.is_none() {
        resolution.diagnostic_data.out_of_range_alpha = branch
            .diagnostic_data
            .out_of_range_alpha
            .clone()
            .map(|mut usage| {
                usage.base &= branch.selector == resolution.selector;
                usage
            });
    }
}

/// The value types that `utility` names in `[type]` hints or, without hints, the types
/// declared for the theme namespaces of its `--value(--namespace-*)` domains.
fn declared_value_types(utility: &CustomUtility, theme: &ThemeIndex) -> Vec<ValueType> {
    let types = utility_value_types(utility);
    if !types.is_empty() {
        return types;
    }
    let mut types = Vec::new();
    let mut body = utility.body.to_lean_string();
    let _ = expand_placeholder(&mut body, "--value", |domains| {
        for (namespace, _) in split_top_level(domains, |token| matches!(token, Token::Comma))
            .into_iter()
            .filter_map(|domain| theme_domain_pattern(domain.trim(), theme))
        {
            for &value_type in namespace.value_types() {
                if !types.contains(&value_type) {
                    types.push(value_type);
                }
            }
        }
        Some(LeanStr::new())
    });
    types
}

/// Returns the type hints of the value types that the definitions matching `lookup`
/// accept in brackets, when there are at least two of them. Project definitions come
/// first in authored order, followed by `@stillwind-utility` definitions in registry
/// order. The `[type]` hints anywhere in the definitions come first. A type declared
/// for the theme namespace of a `--value(--namespace-*)` domain follows when no hinted
/// definition sets any property of its definition: the color namespace of `shadow-*`
/// adds `color`, but the font-size namespace of `text-*` adds nothing beside
/// `[length]`. An empty list means the utility accepts values of one type, so the
/// property that receives an untyped value is not a fallback between types.
fn fallback_type_hints(lookup: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    // Registries list matching definitions newest first.
    let mut definitions = theme.matching_utilities(lookup);
    definitions.reverse();
    let mut stillwind = theme.registry().matching_utilities(lookup);
    stillwind.sort_by_key(|definition| definition.position);
    definitions.extend(stillwind);
    let mut hinted = Vec::new();
    let mut hinted_properties = HashSet::new();
    let mut namespaced = Vec::new();
    for definition in definitions {
        let properties = declaration_property_names(&definition.body);
        let value_types = utility_value_types(definition);
        if !value_types.is_empty() {
            hinted.extend(value_types);
            hinted_properties.extend(properties.iter().copied());
        }
        let mut body = definition.body.to_lean_string();
        let _ = expand_placeholder(&mut body, "--value", |domains| {
            for (namespace, _) in split_top_level(domains, |token| matches!(token, Token::Comma))
                .into_iter()
                .filter_map(|domain| theme_domain_pattern(domain.trim(), theme))
            {
                for &value_type in namespace.value_types() {
                    namespaced.push((value_type, properties.clone()));
                }
            }
            Some(LeanStr::new())
        });
    }
    let mut types = Vec::new();
    for value_type in hinted {
        if value_type != ValueType::Any && !types.contains(&value_type) {
            types.push(value_type);
        }
    }
    for (value_type, properties) in namespaced {
        if !types.contains(&value_type)
            && !properties
                .iter()
                .any(|property| hinted_properties.contains(property))
        {
            types.push(value_type);
        }
    }
    if types.len() < 2 {
        return Vec::new();
    }
    types
        .into_iter()
        .map(|value_type| value_type.hint().to_lean_str())
        .collect()
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
        return hint.is_empty().then(|| decode_arbitrary_value(value));
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
                && existing.parent == rule.parent
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
    use crate::UtilityRole;

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
            position: None,
            tw_sort: None,
            role: UtilityRole::default(),
            plugin: false,
        };
        let wildcard = CustomUtility {
            name: "w-*".into(),
            tw_sort: None,
            role: UtilityRole::default(),
            plugin: false,
            ..exact.clone()
        };
        assert_eq!(utility_value_types(&exact), vec![ValueType::Length]);
        assert!(
            utility_branch_specificity(&exact, "w-1")
                > utility_branch_specificity(&wildcard, "w-1")
        );
    }
}
