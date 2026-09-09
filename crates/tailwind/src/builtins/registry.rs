use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use stillwind_css::{
    BuiltinArbitraryPropertyOrder, CustomPropertyDefinition, CustomUtility, CustomVariant,
    parse_animation_option_property_mappings, parse_builtin_animation_option_definitions,
    parse_builtin_arbitrary_property_order, parse_builtin_utility_definitions,
    parse_builtin_variant_definitions,
};

use super::metadata::{
    builtin_declaration_formats, builtin_property_definitions, builtin_relationship_patterns,
};
use super::normalization::canonical_builtin_body;

pub(crate) const BUILTIN_REGISTRY_CSS: &str = concat!(
    include_str!("../../../../css/builtins/registry-metadata.css"),
    include_str!("../../../../css/builtins/relationship-metadata.css"),
    include_str!("../../../../css/builtins/layout-utilities.css"),
    include_str!("../../../../css/builtins/spacing-sizing-utilities.css"),
    include_str!("../../../../css/builtins/flex-grid-utilities.css"),
    include_str!("../../../../css/builtins/typography-utilities.css"),
    include_str!("../../../../css/builtins/backgrounds-utilities.css"),
    include_str!("../../../../css/builtins/borders-utilities.css"),
    include_str!("../../../../css/builtins/effects-utilities.css"),
    include_str!("../../../../css/builtins/masks-utilities.css"),
    include_str!("../../../../css/builtins/transforms-utilities.css"),
    include_str!("../../../../css/builtins/motion-utilities.css"),
    include_str!("../../../../css/builtins/interactivity-utilities.css"),
    include_str!("../../../../css/builtins/svg-utilities.css"),
    include_str!("../../../../css/builtins/relationship-variants.css"),
    include_str!("../../../../css/builtins/structural-variants.css"),
    include_str!("../../../../css/builtins/state-variants.css"),
    include_str!("../../../../css/builtins/dynamic-selector-variants.css"),
    include_str!("../../../../css/builtins/conditional-variants.css"),
    include_str!("../../../../css/builtins/properties.css"),
    include_str!("../../../../css/builtins.css"),
);

/// Process-wide immutable definitions retained by each selected design system.
#[derive(Debug)]
pub struct BuiltinDefinitions {
    pub utilities: Vec<CustomUtility>,
    pub variants: Vec<CustomVariant>,
    pub properties: Vec<CustomPropertyDefinition>,
    pub source: &'static str,
    pub declaration_formats: BTreeMap<String, String>,
    pub relationships: BTreeSet<String>,
}

pub fn builtin_definitions() -> Arc<BuiltinDefinitions> {
    static DEFINITIONS: OnceLock<Arc<BuiltinDefinitions>> = OnceLock::new();
    Arc::clone(DEFINITIONS.get_or_init(|| {
        Arc::new(BuiltinDefinitions {
            utilities: parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS),
            variants: parse_builtin_variant_definitions(BUILTIN_REGISTRY_CSS),
            properties: builtin_property_definitions().to_vec(),
            source: BUILTIN_REGISTRY_CSS,
            declaration_formats: builtin_declaration_formats().clone(),
            relationships: builtin_relationship_patterns().clone(),
        })
    }))
}

pub(crate) struct BuiltinUtilityRegistry {
    pub(crate) builtin_exact: HashMap<String, Vec<CustomUtility>>,
    pub(crate) wildcards: Vec<CustomUtility>,
    pub(crate) arbitrary_property_order: Vec<BuiltinArbitraryPropertyOrder>,
}

pub(crate) fn builtin_utilities() -> &'static BuiltinUtilityRegistry {
    static UTILITIES: OnceLock<BuiltinUtilityRegistry> = OnceLock::new();
    UTILITIES.get_or_init(|| {
        let normalize = |utility: CustomUtility| {
            let mut utility = utility;
            // The registry asset is intentionally formatted for humans. The generated
            // output must nevertheless be byte-stable across formatter versions.
            utility.body = canonical_builtin_body(&utility.body);
            utility
        };
        let builtins = parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS)
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let arbitrary_property_order = parse_builtin_arbitrary_property_order(BUILTIN_REGISTRY_CSS);
        let mut builtin_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut wildcards = Vec::new();
        for utility in builtins {
            if utility.name.ends_with('*') {
                wildcards.push(utility);
            } else {
                builtin_exact
                    .entry(utility.name.clone())
                    .or_default()
                    .push(utility);
            }
        }
        BuiltinUtilityRegistry {
            builtin_exact,
            wildcards,
            arbitrary_property_order,
        }
    })
}

pub(crate) fn arbitrary_property_order(property: &str) -> usize {
    builtin_utilities()
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

pub(crate) fn builtin_variants() -> &'static [CustomVariant] {
    static VARIANTS: OnceLock<Vec<CustomVariant>> = OnceLock::new();
    VARIANTS
        .get_or_init(|| parse_builtin_variant_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

pub(crate) fn builtin_animation_options() -> &'static [CustomUtility] {
    static OPTIONS: OnceLock<Vec<CustomUtility>> = OnceLock::new();
    OPTIONS
        .get_or_init(|| {
            parse_builtin_animation_option_definitions(BUILTIN_REGISTRY_CSS)
                .into_iter()
                .map(|mut option| {
                    option.body = canonical_builtin_body(&option.body);
                    option
                })
                .collect()
        })
        .as_slice()
}

pub(crate) fn animation_option_property_mappings() -> &'static BTreeMap<String, String> {
    static MAPPINGS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MAPPINGS.get_or_init(|| parse_animation_option_property_mappings(BUILTIN_REGISTRY_CSS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_property_order_prefers_exact_patterns_and_sorts_unknowns_last() {
        assert_eq!(
            arbitrary_property_order("unregistered-property"),
            arbitrary_property_order("*")
        );
        assert!(
            arbitrary_property_order("background-color")
                < arbitrary_property_order("background-image")
        );
        assert!(
            arbitrary_property_order("background-image")
                < arbitrary_property_order("unregistered-property")
        );
        let wildcard = arbitrary_property_order("--tw-gradient-custom");
        assert_ne!(wildcard, usize::MAX);
        assert!(arbitrary_property_order("--tw-gradient-position") > wildcard);
        assert_eq!(arbitrary_property_order("--tw-gradient-other"), wildcard);
    }
}
