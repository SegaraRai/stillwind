use std::collections::{BTreeMap, BTreeSet, HashMap};

use stillwind_css::{CustomPropertyDefinition, ValueType, parse_property_definitions};

use super::normalization::canonical_builtin_body;
use crate::{
    BuiltinArbitraryPropertyOrder, CustomUtility, CustomVariant, ThemeNamespace,
    parse_animation_option_property_mappings, parse_builtin_animation_option_definitions,
    parse_builtin_arbitrary_property_order, parse_builtin_utility_definitions,
    parse_builtin_variant_definitions, parse_declaration_formats, parse_function_formats,
    parse_marker_utilities, parse_omit_token_prefixes, parse_relationship_patterns,
    parse_theme_namespace_types, parse_theme_order,
};

/// Immutable registries parsed from the CSS supplied by a host.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuiltinDefinitions {
    pub source: String,
    pub utilities: Vec<CustomUtility>,
    pub variants: Vec<CustomVariant>,
    pub properties: Vec<CustomPropertyDefinition>,
    pub builtin_exact: HashMap<String, Vec<CustomUtility>>,
    pub wildcards: Vec<CustomUtility>,
    pub arbitrary_property_order: Vec<BuiltinArbitraryPropertyOrder>,
    pub animation_options: Vec<CustomUtility>,
    pub animation_property_mappings: BTreeMap<String, String>,
    pub declaration_formats: BTreeMap<String, String>,
    pub function_formats: BTreeSet<String>,
    pub relationships: BTreeSet<String>,
    pub marker_utilities: BTreeSet<String>,
    pub omit_token_prefixes: Vec<(String, String)>,
    pub theme_order: Vec<String>,
    pub theme_namespace_types: BTreeMap<ThemeNamespace, Vec<ValueType>>,
}

impl BuiltinDefinitions {
    pub fn from_css(css: &str) -> Self {
        let function_formats = parse_function_formats(css);
        let normalize = |mut utility: CustomUtility| {
            utility.body = canonical_builtin_body(&utility.body, &function_formats);
            utility
        };
        let utilities: Vec<_> = parse_builtin_utility_definitions(css)
            .into_iter()
            .map(normalize)
            .collect();
        let mut builtin_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut wildcards = Vec::new();
        for utility in &utilities {
            if utility.name.ends_with('*') {
                wildcards.push(utility.clone());
            } else {
                builtin_exact
                    .entry(utility.name.clone())
                    .or_default()
                    .push(utility.clone());
            }
        }
        Self {
            source: css.into(),
            animation_options: parse_builtin_animation_option_definitions(css)
                .into_iter()
                .map(normalize)
                .collect(),
            utilities,
            builtin_exact,
            wildcards,
            variants: parse_builtin_variant_definitions(css),
            properties: parse_property_definitions(css),
            arbitrary_property_order: parse_builtin_arbitrary_property_order(css),
            animation_property_mappings: parse_animation_option_property_mappings(css),
            declaration_formats: parse_declaration_formats(css),
            function_formats,
            relationships: parse_relationship_patterns(css),
            marker_utilities: parse_marker_utilities(css),
            omit_token_prefixes: parse_omit_token_prefixes(css),
            theme_order: parse_theme_order(css),
            theme_namespace_types: parse_theme_namespace_types(css),
        }
    }
}
