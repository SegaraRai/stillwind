//! Lookup helpers for the selected built-in registry.
//!
//! The registry is shared with the snapshot through an [`Arc`], so these helpers borrow
//! rather than copy. Ordering is preserved everywhere: exact-name groups keep authored
//! order, and this module compares positions in exactly one place,
//! [`arbitrary_property_order`]. Utility emission and variant ordering compare
//! positions in their own modules.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{BuiltinDefinitions, CustomUtility, CustomVariant, ThemeIndex};

/// The built-in definition set selected by this theme.
pub fn builtin_definitions(theme: &ThemeIndex) -> Arc<BuiltinDefinitions> {
    Arc::clone(&theme.registry)
}

/// Borrows the same definition set without cloning the `Arc`.
pub(crate) fn builtin_utilities(theme: &ThemeIndex) -> &BuiltinDefinitions {
    &theme.registry
}

/// Cascade position of an arbitrary property, or [`usize::MAX`] when no pattern
/// matches.
///
/// A pattern matches by exact name or by a trailing-`*` prefix; among matching patterns
/// an exact name wins over wildcards, otherwise the longest prefix wins. Equal
/// specificity selects the last matching entry. [`usize::MAX`] sorts an unregistered
/// property last.
pub(crate) fn arbitrary_property_order(property: &str, theme: &ThemeIndex) -> usize {
    theme
        .registry
        .arbitrary_property_order
        .iter()
        .filter_map(|entry| {
            let prefix = entry.pattern.strip_suffix('*');
            let matches = prefix.map_or(entry.pattern == property, |prefix| {
                property.starts_with(prefix)
            });
            matches.then_some((prefix.map_or(usize::MAX, str::len), entry.builtin_order))
        })
        .max_by_key(|(specificity, _)| *specificity)
        .map_or(usize::MAX, |(_, order)| order)
}

/// Built-in variant definitions in authored order.
pub(crate) fn builtin_variants(theme: &ThemeIndex) -> &[CustomVariant] {
    &theme.registry.variants
}

/// Built-in animation option definitions, kept apart from ordinary utilities.
pub(crate) fn builtin_animation_options(theme: &ThemeIndex) -> &[CustomUtility] {
    &theme.registry.animation_options
}

/// CSS-authored declaration projections applied when a utility is used as an animation
/// option.
pub(crate) fn animation_option_property_mappings(theme: &ThemeIndex) -> &BTreeMap<String, String> {
    &theme.registry.animation_property_mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_defaults::BUILTIN_REGISTRY_CSS;

    #[test]
    fn arbitrary_property_order_prefers_exact_patterns_and_sorts_unknowns_last() {
        let theme = ThemeIndex {
            registry: Arc::new(BuiltinDefinitions::from_css(BUILTIN_REGISTRY_CSS)),
            ..ThemeIndex::default()
        };
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
