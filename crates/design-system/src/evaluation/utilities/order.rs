//! Tailwind 4.3.3 utility ordering before CSS optimization and variant application.
//!
//! [`UtilityOrder`] is the sort key of one resolved rule: authored registration position
//! by default, or Tailwind property metadata when the theme selects it.
//! Property metadata follows Tailwind's `src/property-order.ts`; comparison follows
//! `compile.ts` and `utils/compare.ts`. The adjacent data file retains upstream order.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use cssparser::Parser;
use lean_string::LeanString;

use stillwind_css::{CssNode, CssNodeKind, parse_structure};

use super::tailwind_property_order::PROPERTIES;

/// Utility precedence within a variant context, pinned to the snapshot's ordering mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UtilityOrder {
    /// First registration offset; synthetic utilities have no registration.
    Authored(Option<usize>),
    /// Resolved declaration metadata before optimization.
    Tailwind(UtilityPropertySort),
}

impl UtilityOrder {
    /// Compares two rules' precedence; `left` and `right` are their candidate spellings.
    ///
    /// Authored keys compare by registration position, Tailwind keys by property
    /// metadata, and a tie falls back to the names: byte order in authored mode,
    /// Tailwind's numeric-aware comparison in Tailwind mode. Keys of different modes
    /// compare by mode, so a synthetic rule, which has no registration position, sorts
    /// ahead of a resolved utility in either mode.
    pub fn compare(&self, left: &str, other: &Self, right: &str) -> Ordering {
        match (self, other) {
            (Self::Authored(a), Self::Authored(b)) => a.cmp(b).then_with(|| left.cmp(right)),
            (Self::Tailwind(a), Self::Tailwind(b)) => {
                a.compare(b).then_with(|| compare_candidates(left, right))
            }
            (Self::Authored(_), Self::Tailwind(_)) => Ordering::Less,
            (Self::Tailwind(_), Self::Authored(_)) => Ordering::Greater,
        }
    }
}

/// Tailwind's sorted unique property indices and total declaration count.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UtilityPropertySort {
    /// Ascending indices in Tailwind's global property table; unknown properties add no index.
    pub properties: Vec<usize>,
    /// Tailwind AST-equivalent count, including duplicates, empty values, and the header sort marker.
    pub count: usize,
}

impl UtilityPropertySort {
    /// Builds one utility's sort key from the declaration groups it authors.
    ///
    /// `groups` holds the utility's own declaration lists (base, conditional, and
    /// additional targets) with the compatibility fallback branches left out by the
    /// caller. Declarations in nested rules and conditions also count; keyframe and
    /// property definitions are excluded. Every declaration counts, including duplicates
    /// and empty values. A `tw_sort` property counts once and replaces the collected
    /// indices when the property table knows it.
    pub(in crate::evaluation) fn from_groups<'a>(
        groups: impl IntoIterator<Item = &'a str>,
        tw_sort: Option<&str>,
    ) -> Self {
        let mut properties = BTreeSet::new();
        let mut count = usize::from(tw_sort.is_some());
        let override_index =
            tw_sort.and_then(|value| PROPERTIES.iter().position(|name| *name == value));
        for group in groups {
            collect_properties(group, &parse_structure(group), &mut properties, &mut count);
        }
        if let Some(index) = override_index {
            properties = BTreeSet::from([index]);
        }
        Self {
            properties: properties.into_iter().collect(),
            count,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        for index in 0..self.properties.len().max(other.properties.len()) {
            let comparison = self
                .properties
                .get(index)
                .unwrap_or(&usize::MAX)
                .cmp(other.properties.get(index).unwrap_or(&usize::MAX));
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
        other.count.cmp(&self.count)
    }
}

/// Counts declarations of utility rules and conditions, excluding definitions that
/// emission moves outside those rules.
///
/// All ranges in `nodes` and their descendants must address `css`. The function adds
/// known property indices to `properties` and increments `count` for each declaration
/// with a property identifier, including duplicates, unknown properties, and empty
/// values. It retains existing accumulator contents and skips keyframe and `@property`
/// definitions with their descendants.
fn collect_properties(
    css: &str,
    nodes: &[CssNode],
    properties: &mut BTreeSet<usize>,
    count: &mut usize,
) {
    for node in nodes {
        match &node.kind {
            CssNodeKind::Declaration { property, .. } => {
                let property = &css[property.clone()];
                let mut parser = Parser::new(property);
                if parser.expect_ident().is_err() {
                    continue;
                }
                *count += 1;
                if let Some(index) = PROPERTIES.iter().position(|name| *name == property) {
                    properties.insert(index);
                }
            }
            CssNodeKind::AtRule { name, .. }
                if name.ends_with("keyframes") || name == "property" => {}
            _ => collect_properties(css, node.children(), properties, count),
        }
    }
}

/// Tailwind's candidate comparison: digit runs compare as numbers, numerically equal
/// runs fall back to their text, other characters compare by UTF-16 code unit, and a
/// strict prefix sorts first.
fn compare_candidates(left: &str, right: &str) -> Ordering {
    let left: Vec<_> = left.encode_utf16().collect();
    let right: Vec<_> = right.encode_utf16().collect();
    let digit = |value: &u16| (48..=57).contains(value);
    for index in 0..left.len().min(right.len()) {
        if digit(&left[index]) && digit(&right[index]) {
            let a = &left[index
                ..index
                    + left[index..]
                        .iter()
                        .take_while(|value| digit(value))
                        .count()];
            let b = &right[index
                ..index
                    + right[index..]
                        .iter()
                        .take_while(|value| digit(value))
                        .count()];
            let number = |digits: &[u16]| {
                LeanString::from_utf16_lossy(digits)
                    .parse::<f64>()
                    .unwrap_or(f64::INFINITY)
            };
            let difference = number(a) - number(b);
            if difference < 0.0 {
                return Ordering::Less;
            }
            if difference > 0.0 {
                return Ordering::Greater;
            }
            let comparison = a.cmp(b);
            if comparison != Ordering::Equal {
                return comparison;
            }
        } else {
            let comparison = left[index].cmp(&right[index]);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
    }
    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_sequences_use_infinity_and_counts_include_unknown_empty_and_repeated_declarations()
    {
        let key = |css| UtilityPropertySort::from_groups([css], None);
        assert_eq!(key("color:red;--x:;color:blue").count, 3);
        assert_eq!(
            key("color:red;display:block").compare(&key("display:block")),
            Ordering::Less
        );
        assert_eq!(
            key("color:red;--x:;").compare(&key("color:red")),
            Ordering::Less
        );
        assert_eq!(key("unregistered:1").properties, Vec::<usize>::new());
        assert_eq!(
            key("--tw-sort:margin;color:red").properties,
            key("color:red").properties
        );
        let overridden =
            UtilityPropertySort::from_groups(["color:red;display:block;"], Some("margin"));
        assert_eq!(overridden.properties, key("margin:0").properties);
        assert_eq!(overridden.count, 3);
        let unknown = UtilityPropertySort::from_groups(["color:red"], Some("unknown"));
        assert_eq!(unknown.properties, key("color:red").properties);
        assert_eq!(unknown.count, 2);
    }

    #[test]
    fn nested_rules_contribute_to_property_order() {
        assert_eq!(
            UtilityPropertySort::from_groups(
                ["[data-icon]{opacity:.7;}@media print{color:red;}"],
                None
            ),
            UtilityPropertySort::from_groups(["opacity:.7;color:red;"], None)
        );
    }

    #[test]
    fn names_follow_upstream_numeric_and_utf16_comparison() {
        for (a, b) in [
            ("x-2", "x-10"),
            ("x-02", "x-2"),
            ("x-2", "x-2/3"),
            ("x-𐀀", "x-\u{e000}"),
        ] {
            assert_eq!(compare_candidates(a, b), Ordering::Less);
            assert_eq!(compare_candidates(b, a), Ordering::Greater);
        }
    }
}
