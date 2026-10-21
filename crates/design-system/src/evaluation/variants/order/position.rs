//! Registry positions of variants and the groups that sort as one unit.

use std::sync::Arc;

use lean_string::LeanString;

use stillwind_syntax::{Variant, VariantKind};

use super::super::super::stillwind::stillwind_variants;
use super::super::resolve::{parse_nested_variant, split_typed_capture};
use super::super::substitution::registered_variant_candidates;
use crate::{CustomVariant, StillwindVariantValueType, ThemeIndex, ThemeNamespace};

/// The single `@stillwind-variant` definition that captures this variant text, if any.
///
/// An exact name wins outright; otherwise the matching pattern with the most literal
/// prefix-plus-suffix bytes wins. Equal specificity selects the last definition.
pub(super) fn direct_stillwind_variant<'a>(
    value: &str,
    theme: &'a ThemeIndex,
) -> Option<&'a CustomVariant> {
    stillwind_variants(theme)
        .iter()
        .filter_map(|variant| {
            if variant.wildcard.is_none() && variant.name == value {
                return Some((usize::MAX, variant));
            }
            variant
                .capture(value)
                .map(|_| (variant.name.len() - 1, variant))
        })
        .max_by_key(|(specificity, _)| *specificity)
        .map(|(_, variant)| variant.as_ref())
}

/// The declared theme namespace named by a sort key such as `--breakpoint-*`.
pub(in crate::evaluation) fn variant_sort_namespace<'a>(
    key: &str,
    theme: &'a ThemeIndex,
) -> Option<&'a ThemeNamespace> {
    theme
        .namespaces()
        .get(key.strip_prefix("--")?.strip_suffix("-*")?)
}

/// A variant that names a theme token without its prefix, as a bare breakpoint variant
/// does.
pub(super) fn bare_stillwind_variant<'a>(
    value: &str,
    theme: &'a ThemeIndex,
) -> Option<&'a CustomVariant> {
    stillwind_variants(theme)
        .iter()
        .map(Arc::as_ref)
        .find(|variant| {
            let Some(sort) = variant.sort.as_ref().filter(|sort| sort.bare) else {
                return false;
            };
            let Some(namespace) = sort
                .key
                .as_deref()
                .and_then(|key| variant_sort_namespace(key, theme))
            else {
                return false;
            };
            theme.token_in(&namespace.name, value).is_some()
        })
}

/// Resolves a parsed variant to the definition that orders it.
///
/// A definition without sort metadata is still a definition; the sort is only consulted
/// when two variants share a group.
pub(super) fn stillwind_variant_for_order<'a>(
    variant: &Variant,
    theme: &'a ThemeIndex,
) -> Option<&'a CustomVariant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    direct_stillwind_variant(value, theme).or_else(|| bare_stillwind_variant(value, theme))
}

/// The ordering group of a variant definition, which defaults to its own name.
pub(super) fn variant_group(variant: &CustomVariant) -> &str {
    variant
        .sort
        .as_ref()
        .and_then(|sort| sort.group.as_deref())
        .unwrap_or(&variant.name)
}

/// The position of a variant in the registry's ordering space.
///
/// Every variant of one group reports the same position, so a group moves as a unit.
/// [`usize::MAX`] means the variant is not registered and therefore sorts after
/// everything registered.
pub(super) fn variant_order(variant: &Variant, theme: &ThemeIndex) -> usize {
    let custom = if let VariantKind::Named(value) = &variant.kind {
        registered_variant_candidates(theme.variants(), value)
            .first()
            .map(|(definition, _)| *definition)
    } else {
        None
    };
    let Some(variant) = stillwind_variant_for_order(variant, theme).or(custom) else {
        return usize::MAX;
    };
    let group = variant_group(variant);
    stillwind_variants(theme)
        .iter()
        .chain(theme.variants())
        .filter(|candidate| variant_group(candidate) == group)
        .filter_map(|candidate| {
            theme
                .variant_positions()
                .get(candidate.name.as_str())
                .copied()
                .or(candidate.position)
        })
        .min()
        .unwrap_or(usize::MAX)
}

/// The inner variant of a compound variant definition, when it has one.
pub(super) fn compound_inner_variant(variant: &Variant, theme: &ThemeIndex) -> Option<Variant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let capture = compound_variant_capture(theme.variants(), value)
        .or_else(|| compound_variant_capture(stillwind_variants(theme), value))?;
    Some(parse_nested_variant(&capture))
}

/// Returns the inner variant of a compound variant, such as `hover` for `group-hover`,
/// from the project's definitions and then the `@stillwind-variant` definitions whose
/// declared value type is a
/// variant stack. The compound variant's own modifier, such as `/name`, is excluded.
/// Returns `None` when no such definition matches `value`.
pub fn compound_variant_inner(value: &str, theme: &ThemeIndex) -> Option<LeanString> {
    compound_variant_capture(theme.variants(), value)
        .or_else(|| compound_variant_capture(stillwind_variants(theme), value))
}

/// The inner variant text of the first definition whose declared value type is a variant
/// stack, without that definition's own modifier.
pub(super) fn compound_variant_capture(
    variants: &[Arc<CustomVariant>],
    value: &str,
) -> Option<LeanString> {
    registered_variant_candidates(variants, value)
        .into_iter()
        .find_map(|(variant, capture)| {
            (variant.value_type == Some(StillwindVariantValueType::Variant))
                .then(|| capture.map(|capture| split_typed_capture(variant, capture).0.into()))?
        })
}
