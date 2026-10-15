//! Variant ordering, as declared by the registry rather than by markup.
//!
//! Each built-in variant carries an authored position, and variants sharing a sort
//! group are ordered by the group's first position so that related variants (for
//! example a set of breakpoints) sort as one unit. A variant that no definition matches
//! sorts last, which keeps an unknown spelling from silently acquiring the position of
//! something it resembles.
//!
//! Within a group, a variant's sort key is the value of a theme token; numeric values
//! compare by unit and then magnitude, with direction applied to magnitude. Anything
//! that is not a number falls back to text order so the comparison stays total and
//! deterministic.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, LeanString};

use stillwind_syntax::{Variant, VariantKind};

use super::super::builtins::builtin_variants;
use super::super::values::decode_arbitrary;
use super::super::values::theme_arbitrary_types;
use super::resolve::{parse_nested_variant, split_variant_modifier};
use super::substitution::registered_variant_candidates;
use crate::{
    BuiltinVariantSortDirection, BuiltinVariantValueType, CustomVariant, ThemeIndex, ThemeNamespace,
};

/// The single built-in definition that captures this variant text, if any.
///
/// An exact name wins outright; otherwise the matching pattern with the most literal
/// prefix-plus-suffix bytes wins. Equal specificity selects the last definition.
fn direct_builtin_variant<'a>(value: &str, theme: &'a ThemeIndex) -> Option<&'a CustomVariant> {
    builtin_variants(theme)
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

/// The theme namespace named by a sort key such as `--breakpoint-*`.
pub(in crate::evaluation) fn variant_sort_namespace(key: &str) -> Option<ThemeNamespace> {
    ThemeNamespace::parse(key.strip_prefix("--")?.strip_suffix("-*")?)
}

/// A variant that names a theme token without its prefix, as a bare breakpoint variant
/// does.
fn bare_builtin_variant<'a>(value: &str, theme: &'a ThemeIndex) -> Option<&'a CustomVariant> {
    builtin_variants(theme)
        .iter()
        .map(Arc::as_ref)
        .find(|variant| {
            let Some(sort) = variant.sort.as_ref().filter(|sort| sort.bare) else {
                return false;
            };
            let Some(namespace) = sort.key.as_deref().and_then(variant_sort_namespace) else {
                return false;
            };
            theme.token_in(namespace, value).is_some()
        })
}

/// Resolves a parsed variant to the definition that orders it.
///
/// A definition without sort metadata is still a definition; the sort is only consulted
/// when two variants share a group.
fn builtin_variant_for_order<'a>(
    variant: &Variant,
    theme: &'a ThemeIndex,
) -> Option<&'a CustomVariant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    direct_builtin_variant(value, theme).or_else(|| bare_builtin_variant(value, theme))
}

/// The ordering group of a variant definition, which defaults to its own name.
fn variant_group(variant: &CustomVariant) -> &str {
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
fn variant_order(variant: &Variant, theme: &ThemeIndex) -> usize {
    let custom = if let VariantKind::Named(value) = &variant.kind {
        registered_variant_candidates(theme.variants(), value)
            .first()
            .map(|(definition, _)| *definition)
    } else {
        None
    };
    let Some(variant) = builtin_variant_for_order(variant, theme).or(custom) else {
        return usize::MAX;
    };
    let group = variant_group(variant);
    builtin_variants(theme)
        .iter()
        .chain(theme.variants())
        .filter(|candidate| variant_group(candidate) == group)
        .filter_map(|candidate| {
            theme
                .variant_positions()
                .get(candidate.name.as_str())
                .copied()
                .or(candidate.builtin_order)
        })
        .min()
        .unwrap_or(usize::MAX)
}

/// Splits a numeric CSS value into its number and unit.
///
/// [`None`] means the value does not start with a number, which routes the comparison
/// to text ordering.
fn numeric_dimension(value: &str) -> Option<(f64, &str)> {
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
fn compare_sort_values(
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
fn variant_sort_value(variant: &Variant, theme: &ThemeIndex) -> Option<LeanString> {
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
fn compare_variants_in_group(left: &Variant, right: &Variant, theme: &ThemeIndex) -> Ordering {
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

/// The inner variant of a compound variant definition, when it has one.
fn compound_inner_variant(variant: &Variant, theme: &ThemeIndex) -> Option<Variant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let capture = compound_variant_capture(theme.variants(), value)
        .or_else(|| compound_variant_capture(builtin_variants(theme), value))?;
    Some(parse_nested_variant(split_variant_modifier(&capture).0))
}

/// The capture of the first definition whose declared value type is a variant stack, so
/// the capture can be parsed as an inner variant.
fn compound_variant_capture(variants: &[Arc<CustomVariant>], value: &str) -> Option<LeanString> {
    registered_variant_candidates(variants, value)
        .into_iter()
        .find_map(|(variant, capture)| {
            (variant.value_type == Some(BuiltinVariantValueType::Variant))
                .then(|| capture.map(LeanString::from))?
        })
}

/// Compares variant stacks using first registration positions in the expanded CSS.
/// This uses an order-bit comparison while leaving
/// utility/property ordering independent.
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
    /// Fallback spelling for groups without comparable sort values.
    raw: LeanStr,
    inner: Option<Box<VariantSequenceOrder>>,
    direction: Option<BuiltinVariantSortDirection>,
    /// Theme token text or a decoded arbitrary capture.
    value: Option<LeanStr>,
}

impl VariantGroupOrder {
    fn new(variant: &Variant, theme: &ThemeIndex) -> Self {
        Self {
            raw: variant.raw.clone(),
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
        self.raw.cmp(&other.raw)
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
