use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
pub(crate) use stillwind_css::BUILTIN_REGISTRY_CSS;
pub use stillwind_css::BuiltinDefinitions;
use stillwind_css::{CustomUtility, CustomVariant, ThemeIndex};

pub fn builtin_definitions(theme: &ThemeIndex) -> Arc<BuiltinDefinitions> {
    Arc::clone(&theme.registry)
}

pub(crate) fn builtin_utilities(theme: &ThemeIndex) -> &BuiltinDefinitions {
    &theme.registry
}

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

pub(crate) fn builtin_variants(theme: &ThemeIndex) -> &[CustomVariant] {
    &theme.registry.variants
}

pub(crate) fn builtin_animation_options(theme: &ThemeIndex) -> &[CustomUtility] {
    &theme.registry.animation_options
}

pub(crate) fn animation_option_property_mappings(theme: &ThemeIndex) -> &BTreeMap<String, String> {
    &theme.registry.animation_property_mappings
}

#[cfg(test)]
mod tests {
    use super::*;

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
