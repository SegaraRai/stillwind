//! Order of variants within one sort group, from the theme token values they select.

use std::cmp::Ordering;

use lean_string::LeanString;

use stillwind_syntax::{Variant, VariantKind};

use super::super::super::values::{decode_arbitrary, theme_arbitrary_types};
use super::super::resolve::split_variant_modifier;
use super::compare_variant_sequences;
use super::position::{builtin_variant_for_order, compound_inner_variant, variant_sort_namespace};
use crate::{BuiltinVariantSortDirection, ThemeIndex};

/// Splits a numeric CSS value into its number and unit.
///
/// [`None`] means the value does not start with a number, which routes the comparison
/// to text ordering.
pub(super) fn numeric_dimension(value: &str) -> Option<(f64, &str)> {
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    Some((number.parse().ok()?, unit))
}

/// Compares two sort values of one variant group.
///
/// Two numbers compare by unit first and magnitude second, with the authored direction
/// applied only to the magnitude; any other pair compares as text. Applying the
/// direction to the unit as well would make `desc` reorder breakpoints that use
/// different units relative to each other.
pub(super) fn compare_sort_values(
    left: &str,
    right: &str,
    direction: BuiltinVariantSortDirection,
) -> Ordering {
    match (numeric_dimension(left), numeric_dimension(right)) {
        (Some((left_number, left_unit)), Some((right_number, right_unit))) => {
            left_unit.cmp(right_unit).then_with(|| {
                let order = left_number.total_cmp(&right_number);
                match direction {
                    BuiltinVariantSortDirection::Ascending => order,
                    BuiltinVariantSortDirection::Descending => order.reverse(),
                }
            })
        }
        _ => left.cmp(right),
    }
}

/// The sort value of a variant: the theme token value named by its capture.
///
/// A wildcard capture is the text between the definition's literal boundaries, a bare variant is the
/// whole name, and an arbitrary capture is accepted only when the group's namespace
/// value types accept it. A capture's `/modifier` part takes no part in ordering.
pub(super) fn variant_sort_value(variant: &Variant, theme: &ThemeIndex) -> Option<LeanString> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let definition = builtin_variant_for_order(variant, theme)?;
    let sort = definition.sort.as_ref()?;
    let namespace = sort.key.as_deref().and_then(variant_sort_namespace)?;
    let capture = if definition.wildcard.is_some() {
        definition.capture(value).or(sort.bare.then_some(value))?
    } else if sort.bare {
        value
    } else {
        return None;
    };
    let capture = split_variant_modifier(capture).0;
    if let Some(arbitrary) = capture
        .strip_prefix('[')
        .and_then(|capture| capture.strip_suffix(']'))
    {
        let arbitrary = decode_arbitrary(arbitrary);
        return theme_arbitrary_types(namespace, theme)
            .iter()
            .any(|value_type| value_type.accepts(&arbitrary))
            .then_some(arbitrary);
    }
    (theme
        .token_in(namespace, capture)
        .map(|token| token.value.value.clone()))
    .map(Into::into)
}

/// Compares two variants that share an ordering position.
///
/// A compound variant (one whose capture is itself a variant stack) is compared by its
/// inner sequence first, so `group-hover` and `group-focus` order by their inner
/// variants rather than by their raw text. The final tie-breaker is the authored text,
/// which keeps the comparison deterministic.
pub(super) fn compare_variants_in_group(
    left: &Variant,
    right: &Variant,
    theme: &ThemeIndex,
) -> Ordering {
    if let (Some(left_inner), Some(right_inner)) = (
        compound_inner_variant(left, theme),
        compound_inner_variant(right, theme),
    ) {
        let comparison = compare_variant_sequences(&[left_inner], &[right_inner], theme);
        if comparison != Ordering::Equal {
            return comparison;
        }
    }
    let direction = builtin_variant_for_order(left, theme)
        .and_then(|variant| variant.sort.as_ref())
        .and_then(|sort| sort.direction);
    if let Some(direction) = direction
        && let (Some(left_value), Some(right_value)) = (
            variant_sort_value(left, theme),
            variant_sort_value(right, theme),
        )
    {
        return compare_sort_values(&left_value, &right_value, direction)
            .then_with(|| left_value.cmp(&right_value));
    }
    left.raw.cmp(&right.raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorting_compares_magnitudes_only_within_the_same_unit() {
        assert_eq!(numeric_dimension("12.5rem"), Some((12.5, "rem")));
        assert_eq!(numeric_dimension("auto"), None);
        assert_eq!(
            compare_sort_values("2px", "10px", BuiltinVariantSortDirection::Ascending),
            Ordering::Less
        );
        assert_eq!(
            compare_sort_values("2px", "10px", BuiltinVariantSortDirection::Descending),
            Ordering::Greater
        );
        assert_eq!(
            compare_sort_values("2rem", "10px", BuiltinVariantSortDirection::Descending),
            Ordering::Greater
        );
    }
}
