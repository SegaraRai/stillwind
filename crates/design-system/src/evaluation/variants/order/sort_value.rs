//! Order of variants within one sort group, from the theme token values they select.

use std::cmp::Ordering;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::selector_uses_subject;
use stillwind_syntax::{Variant, VariantKind, decode_arbitrary_value};

use super::super::resolve::{decode_variant_template, split_variant_modifier};
use super::compare_variant_sequences;
use super::position::{
    compound_inner_variant, stillwind_variant_for_order, variant_sort_namespace,
};
use crate::{StillwindVariantSortDirection, ThemeIndex};

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
    direction: StillwindVariantSortDirection,
) -> Ordering {
    match (numeric_dimension(left), numeric_dimension(right)) {
        (Some((left_number, left_unit)), Some((right_number, right_unit))) => {
            left_unit.cmp(right_unit).then_with(|| {
                let order = left_number.total_cmp(&right_number);
                match direction {
                    StillwindVariantSortDirection::Ascending => order,
                    StillwindVariantSortDirection::Descending => order.reverse(),
                }
            })
        }
        _ => left.cmp(right),
    }
}

/// The sort value of a variant: the theme token value named by its capture.
///
/// A wildcard capture is the text between the definition's literal boundaries, a bare
/// variant is the whole name, and an arbitrary capture is accepted only when the
/// group's namespace value types accept it. A capture's `/modifier` part takes no part
/// in ordering.
pub(super) fn variant_sort_value(variant: &Variant, theme: &ThemeIndex) -> Option<LeanString> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let definition = stillwind_variant_for_order(variant, theme)?;
    let sort = definition.sort.as_ref()?;
    let namespace = sort
        .key
        .as_deref()
        .and_then(|key| variant_sort_namespace(key, theme))?;
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
        let arbitrary = decode_arbitrary_value(arbitrary);
        return namespace
            .types
            .iter()
            .any(|value_type| value_type.accepts(&arbitrary))
            .then_some(arbitrary);
    }
    (theme
        .token_in(&namespace.name, capture)
        .map(|token| token.value.value.clone()))
    .map(Into::into)
}

/// Returns the text that breaks a tie between two variants of one ordering position.
///
/// An arbitrary variant uses its selector after `&` insertion, as Tailwind 4.3.3 does:
/// `[:hover]` uses `&:is(:hover)`, and `[.dark_&]` uses `.dark &`. An at-rule such as
/// `[@media_print]` and a selector that contains `&` use their decoded text. The text
/// does not depend on whether an anchor exists, so a document-level style orders its
/// variants as `style()` does, although it emits a selector without `&` as written.
/// Any other variant uses its authored text.
pub(super) fn variant_tie_text(variant: &Variant) -> LeanStr {
    let VariantKind::Arbitrary(value) = &variant.kind else {
        return variant.raw.clone();
    };
    let value = decode_variant_template(value);
    if value.starts_with('@') || selector_uses_subject(&value) {
        value.into()
    } else {
        format_args!("&:is({value})").to_lean_str()
    }
}

/// Compares two variants that share an ordering position.
///
/// A compound variant (one whose capture is itself a variant stack) is compared by its
/// inner sequence first, so `group-hover` and `group-focus` order by their inner
/// variants rather than by their raw text. The final tie-breaker is
/// [`variant_tie_text`], which keeps the comparison deterministic.
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
    let direction = stillwind_variant_for_order(left, theme)
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
    variant_tie_text(left).cmp(&variant_tie_text(right))
}

#[cfg(test)]
mod tests {
    use stillwind_syntax::parse_candidate;

    use super::super::VariantSequenceOrder;
    use super::*;
    use crate::theme::ThemeBuilder;

    /// Arbitrary variants of one position order by their selector after `&` insertion,
    /// as in Tailwind 4.3.3, in both comparisons.
    #[test]
    fn arbitrary_variants_order_by_the_selector_after_nesting_insertion() {
        let theme = ThemeBuilder::default().finish();
        let variants = |raw: &str| parse_candidate(raw).unwrap().variants;
        let expected = [
            "[:hover]:flex",
            "[[data-x]]:flex",
            "[&>svg]:flex",
            "[.a_&]:flex",
            "[.dark_&]:flex",
            "[@media_print]:flex",
        ];
        let mut sorted = expected;
        sorted.reverse();
        sorted.sort_by(|left, right| {
            compare_variant_sequences(&variants(left), &variants(right), &theme)
        });
        assert_eq!(sorted, expected);
        sorted.reverse();
        sorted.sort_by(|left, right| {
            VariantSequenceOrder::new(&variants(left), &theme)
                .compare(&VariantSequenceOrder::new(&variants(right), &theme))
        });
        assert_eq!(sorted, expected);
        assert_eq!(variant_tie_text(&variants("[.dark_&]:flex")[0]), ".dark &");
        assert_eq!(
            variant_tie_text(&variants("[:root]:flex")[0]),
            "&:is(:root)"
        );
        assert_eq!(variant_tie_text(&variants("hover:flex")[0]), "hover");
    }

    #[test]
    fn sorting_compares_magnitudes_only_within_the_same_unit() {
        assert_eq!(numeric_dimension("12.5rem"), Some((12.5, "rem")));
        assert_eq!(numeric_dimension("auto"), None);
        assert_eq!(
            compare_sort_values("2px", "10px", StillwindVariantSortDirection::Ascending),
            Ordering::Less
        );
        assert_eq!(
            compare_sort_values("2px", "10px", StillwindVariantSortDirection::Descending),
            Ordering::Greater
        );
        assert_eq!(
            compare_sort_values("2rem", "10px", StillwindVariantSortDirection::Descending),
            Ordering::Greater
        );
    }
}
