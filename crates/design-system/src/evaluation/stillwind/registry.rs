//! Lookup helpers for the Stillwind definitions of a theme.
//!
//! The registry is shared with the snapshot through an [`Arc`], so these helpers borrow
//! rather than copy. Ordering is preserved everywhere: exact-name groups keep authored
//! order, and this module compares positions in exactly one place,
//! [`arbitrary_property_order`]. Utility emission and variant ordering compare
//! positions in their own modules.

use std::sync::Arc;

use crate::{CustomVariant, StillwindDefinitions, ThemeIndex};

/// The Stillwind definitions of this theme.
pub fn stillwind_definitions(theme: &ThemeIndex) -> Arc<StillwindDefinitions> {
    Arc::clone(theme.registry())
}

/// Cascade position of an arbitrary property, or [`usize::MAX`] when no pattern
/// matches.
///
/// A pattern matches by exact name or by a trailing-`*` prefix; among matching patterns
/// an exact name wins over wildcards, otherwise the longest prefix wins. Equal
/// specificity selects the last matching entry. [`usize::MAX`] sorts an unregistered
/// property last.
pub(in crate::evaluation) fn arbitrary_property_order(property: &str, theme: &ThemeIndex) -> usize {
    theme
        .registry()
        .arbitrary_property_order()
        .iter()
        .filter_map(|entry| {
            let prefix = entry.pattern.strip_suffix('*');
            let matches = prefix.map_or(entry.pattern == property, |prefix| {
                property.starts_with(prefix)
            });
            matches.then_some((prefix.map_or(usize::MAX, str::len), entry.position))
        })
        .max_by_key(|(specificity, _)| *specificity)
        .map_or(usize::MAX, |(_, order)| order)
}

/// `@stillwind-variant` definitions in authored order.
pub(in crate::evaluation) fn stillwind_variants(theme: &ThemeIndex) -> &[Arc<CustomVariant>] {
    theme.registry().variants()
}

#[cfg(test)]
mod tests {
    use stillwind_test_support::PACKAGE_CSS;

    use super::*;
    use crate::theme::ThemeBuilder;

    #[test]
    fn arbitrary_property_order_prefers_exact_patterns_and_sorts_unknowns_last() {
        let theme = ThemeBuilder {
            registry: Arc::new(StillwindDefinitions::from_css(PACKAGE_CSS)),
            ..Default::default()
        }
        .finish();
        let order = |property| arbitrary_property_order(property, &theme);
        assert_eq!(order("unregistered-property"), order("*"));
        assert!(order("background-color") < order("background-image"));
        assert!(order("background-image") < order("unregistered-property"));
        let wildcard = order("--tw-gradient-custom");
        assert_ne!(wildcard, usize::MAX);
        assert!(order("--tw-gradient-position") > wildcard);
        assert_eq!(order("--tw-gradient-other"), wildcard);
    }
}
