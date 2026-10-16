//! Comparison of variant stacks by registry position, with prepared keys for repeated
//! comparisons.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use lean_string::LeanStr;

use stillwind_syntax::Variant;

use super::position::{builtin_variant_for_order, compound_inner_variant, variant_order};
use super::sort_value::{
    compare_sort_values, compare_variants_in_group, variant_sort_value, variant_tie_text,
};
use crate::{BuiltinVariantSortDirection, ThemeIndex};

/// Compares variant stacks using first registration positions in the expanded CSS.
/// This uses an order-bit comparison while leaving utility/property ordering
/// independent.
///
/// Each stack contributes the distinct positions of its variants, and the positions are
/// compared in descending order, so the highest registry position decides first. Ties
/// are broken variant by variant within a shared position, which makes the comparison
/// total for any two stacks.
#[must_use]
pub fn compare_variant_sequences(
    left: &[Variant],
    right: &[Variant],
    theme: &ThemeIndex,
) -> Ordering {
    let mut left_orders = left
        .iter()
        .map(|variant| variant_order(variant, theme))
        .collect::<Vec<_>>();
    let mut right_orders = right
        .iter()
        .map(|variant| variant_order(variant, theme))
        .collect::<Vec<_>>();
    left_orders.sort_unstable_by(|left, right| right.cmp(left));
    right_orders.sort_unstable_by(|left, right| right.cmp(left));
    left_orders.dedup();
    right_orders.dedup();
    left_orders.cmp(&right_orders).then_with(|| {
        for order in left_orders {
            let left_variant = left
                .iter()
                .find(|variant| variant_order(variant, theme) == order);
            let right_variant = right
                .iter()
                .find(|variant| variant_order(variant, theme) == order);
            if let (Some(left_variant), Some(right_variant)) = (left_variant, right_variant) {
                let comparison = compare_variants_in_group(left_variant, right_variant, theme);
                if comparison != Ordering::Equal {
                    return comparison;
                }
            }
        }
        Ordering::Equal
    })
}

/// Prepared registry positions and theme values for a variant stack in one snapshot.
/// Keys can be compared repeatedly without consulting the theme. Callers must prepare
/// both keys from the same theme revision.
#[derive(Clone, Debug)]
pub struct VariantSequenceOrder {
    /// Groups appear in descending registry order; each retains its first source variant.
    groups: Vec<(usize, VariantGroupOrder)>,
}

impl VariantSequenceOrder {
    /// Resolves ordering metadata for `variants` against immutable `theme`.
    /// Repeated positions use the first variant, matching [`compare_variant_sequences`].
    #[must_use]
    pub fn new(variants: &[Variant], theme: &ThemeIndex) -> Self {
        let mut groups = BTreeMap::new();
        for variant in variants {
            groups
                .entry(variant_order(variant, theme))
                .or_insert_with(|| VariantGroupOrder::new(variant, theme));
        }
        Self {
            groups: groups.into_iter().rev().collect(),
        }
    }

    /// Compares this key with `other`, which must use the same theme revision.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.groups
            .iter()
            .map(|(position, _)| position)
            .cmp(other.groups.iter().map(|(position, _)| position))
            .then_with(|| {
                self.groups
                    .iter()
                    .zip(&other.groups)
                    .map(|((_, left), (_, right))| left.compare(right))
                    .find(|order| *order != Ordering::Equal)
                    .unwrap_or(Ordering::Equal)
            })
    }
}

/// Stores the resolved inputs for a tie within one registry group.
#[derive(Clone, Debug)]
struct VariantGroupOrder {
    /// [`variant_tie_text`] for groups without comparable sort values.
    tie: LeanStr,
    inner: Option<Box<VariantSequenceOrder>>,
    direction: Option<BuiltinVariantSortDirection>,
    /// Theme token text or a decoded arbitrary capture.
    value: Option<LeanStr>,
}

impl VariantGroupOrder {
    fn new(variant: &Variant, theme: &ThemeIndex) -> Self {
        Self {
            tie: variant_tie_text(variant),
            inner: compound_inner_variant(variant, theme)
                .map(|inner| Box::new(VariantSequenceOrder::new(&[inner], theme))),
            direction: builtin_variant_for_order(variant, theme)
                .and_then(|definition| definition.sort.as_ref())
                .and_then(|sort| sort.direction),
            value: (variant_sort_value(variant, theme)).map(Into::into),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        if let (Some(left), Some(right)) = (&self.inner, &other.inner) {
            let order = left.compare(right);
            if order != Ordering::Equal {
                return order;
            }
        }
        if let Some(direction) = self.direction
            && let (Some(left), Some(right)) = (&self.value, &other.value)
        {
            return compare_sort_values(left, right, direction).then_with(|| left.cmp(right));
        }
        self.tie.cmp(&other.tie)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lean_string::ToLeanString;

    use stillwind_syntax::parse_candidate;

    use super::*;
    use crate::BuiltinDefinitions;
    use crate::theme::ThemeBuilder;

    /// Stacks are ordered by the highest registration position first, and the
    /// comparison is total: equal stacks compare equal and every pair is antisymmetric.
    #[test]
    fn compare_variant_sequences_orders_by_registry_position() {
        let theme = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-variant early { &:hover { @slot; } }\n@stillwind-variant late { &:focus { @slot; } }",
            )),
            ..Default::default()
        }.finish();
        let variant = |text: &str| parse_candidate(text).unwrap().variants[0].clone();
        let early = variant("early:flex");
        let late = variant("late:flex");
        assert_eq!(
            compare_variant_sequences(
                std::slice::from_ref(&early),
                std::slice::from_ref(&early),
                &theme
            ),
            Ordering::Equal
        );
        assert_eq!(
            compare_variant_sequences(
                std::slice::from_ref(&late),
                std::slice::from_ref(&early),
                &theme
            ),
            Ordering::Greater
        );
        assert_eq!(
            compare_variant_sequences(
                std::slice::from_ref(&early),
                std::slice::from_ref(&late),
                &theme
            ),
            Ordering::Less
        );
        assert_eq!(
            compare_variant_sequences(
                &[early.clone(), late.clone()],
                std::slice::from_ref(&early),
                &theme
            ),
            Ordering::Greater
        );
        assert_eq!(
            compare_variant_sequences(
                std::slice::from_ref(&late),
                &[late.clone(), early.clone()],
                &theme
            ),
            Ordering::Less
        );
        for (left, right) in [
            (vec![early.clone()], vec![late.clone()]),
            (vec![late.clone()], vec![late.clone(), early.clone()]),
            (vec![], vec![early.clone()]),
        ] {
            assert_eq!(
                compare_variant_sequences(&left, &right, &theme),
                compare_variant_sequences(&right, &left, &theme).reverse(),
                "{left:?} vs {right:?}"
            );
        }
        assert_eq!(compare_variant_sequences(&[], &[], &theme), Ordering::Equal);
    }

    #[test]
    fn prepared_keys_preserve_numeric_compound_and_repeated_group_comparisons() {
        let theme = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-theme-namespace --breakpoint-* [length];
                @stillwind-variant hover { &:hover { @slot; } }
                @stillwind-variant focus { &:focus { @slot; } }
                @stillwind-variant min-* (sort: --breakpoint-*, direction: asc) { @media (width >= --value(--breakpoint-*, [length])) { @slot; } }
                @stillwind-variant max-* (sort: --breakpoint-*, direction: desc) { @media (width < --value(--breakpoint-*, [length])) { @slot; } }
                @stillwind-variant group-* (value: [variant]) { &:is(:where(.group):--value() *) { @slot; } }",
            )),
            ..Default::default()
        }.finish();
        let key = |raw| VariantSequenceOrder::new(&parse_candidate(raw).unwrap().variants, &theme);
        assert_eq!(
            key("min-[2px]:flex").compare(&key("min-[10px]:flex")),
            Ordering::Less
        );
        assert_eq!(
            key("max-[2px]:flex").compare(&key("max-[10px]:flex")),
            Ordering::Greater
        );
        assert_eq!(
            key("min-[1rem]:flex").compare(&key("min-[2px]:flex")),
            Ordering::Greater
        );
        let values = [
            "flex",
            "hover:flex",
            "focus:hover:flex",
            "hover:focus:flex",
            "hover:hover:flex",
            "min-[10px]:flex",
            "min-[2px]:flex",
            "min-[1rem]:flex",
            "max-[2px]:flex",
            "max-[10px]:flex",
            "group-hover:flex",
            "group-focus:flex",
            "group-group-hover:flex",
            "unknown:flex",
        ];
        for left in values {
            for right in values {
                let a = parse_candidate(left).unwrap().variants;
                let b = parse_candidate(right).unwrap().variants;
                assert_eq!(
                    VariantSequenceOrder::new(&a, &theme)
                        .compare(&VariantSequenceOrder::new(&b, &theme)),
                    compare_variant_sequences(&a, &b, &theme),
                    "{left} vs {right}"
                );
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn same_unit_numeric_comparison_is_antisymmetric(a in 0u16..1000, b in 0u16..1000) {
            let left = format_args!("{a}px").to_lean_string();
            let right = format_args!("{b}px").to_lean_string();
            proptest::prop_assert_eq!(
                compare_sort_values(&left, &right, BuiltinVariantSortDirection::Ascending),
                a.cmp(&b)
            );
            proptest::prop_assert_eq!(
                compare_sort_values(&left, &right, BuiltinVariantSortDirection::Ascending),
                compare_sort_values(&right, &left, BuiltinVariantSortDirection::Ascending).reverse()
            );
        }
    }
}

#[cfg(test)]
mod sequence_property_tests {
    use std::sync::Arc;

    use stillwind_syntax::parse_candidate;

    use super::*;
    use crate::BuiltinDefinitions;
    use crate::theme::ThemeBuilder;

    fn theme() -> ThemeIndex {
        ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(
                "@stillwind-variant early { &:hover { @slot; } }\n@stillwind-variant late { &:focus { @slot; } }",
            )),
            ..Default::default()
        }.finish()
    }

    fn registered_variants() -> Vec<Variant> {
        ["early:flex", "late:flex", "missing:flex"]
            .iter()
            .map(|text| parse_candidate(text).unwrap().variants[0].clone())
            .collect()
    }

    /// A repeated variant contributes one position, so the tie-break compares equal
    /// variants and the stacks stay equal.
    #[test]
    fn repeated_variants_share_one_position_and_compare_equal() {
        let theme = theme();
        let early = registered_variants()[0].clone();
        let repeated = [early.clone(), early.clone()];
        assert_eq!(
            compare_variant_sequences(&repeated, std::slice::from_ref(&early), &theme),
            Ordering::Equal
        );
        assert_eq!(
            compare_variant_sequences(&repeated, &repeated, &theme),
            Ordering::Equal
        );
    }

    proptest::proptest! {
        /// The comparison is a total preorder over generated stacks of the same
        /// registered variants: reflexive, antisymmetric, and transitive.
        #[test]
        fn comparison_is_a_total_preorder(
            left in proptest::collection::vec(0usize..3, 0..5),
            middle in proptest::collection::vec(0usize..3, 0..5),
            right in proptest::collection::vec(0usize..3, 0..5),
        ) {
            let theme = theme();
            let variants = registered_variants();
            let stack = |indices: &Vec<usize>| {
                indices
                    .iter()
                    .map(|index| variants[*index].clone())
                    .collect::<Vec<_>>()
            };
            let (a, b, c) = (stack(&left), stack(&middle), stack(&right));
            let prepared_a = VariantSequenceOrder::new(&a, &theme);
            let prepared_b = VariantSequenceOrder::new(&b, &theme);
            proptest::prop_assert_eq!(prepared_a.compare(&prepared_b), compare_variant_sequences(&a, &b, &theme));
            let ab = compare_variant_sequences(&a, &b, &theme);
            let ba = compare_variant_sequences(&b, &a, &theme);
            proptest::prop_assert_eq!(compare_variant_sequences(&a, &a, &theme), Ordering::Equal);
            proptest::prop_assert_eq!(ab, ba.reverse());
            if ab != Ordering::Greater
                && compare_variant_sequences(&b, &c, &theme) != Ordering::Greater
            {
                proptest::prop_assert_ne!(compare_variant_sequences(&a, &c, &theme), Ordering::Greater);
            }
        }
    }
}
