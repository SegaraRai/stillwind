//! The complete built-in definition set parsed from one CSS buffer.
//!
//! A snapshot keeps the selected built-ins as one immutable value so callers never
//! consult a process-global registry: two snapshots built from different CSS stay
//! independent, and no load can leak definitions into another.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use stillwind_css::{CustomPropertyDefinition, ValueType, parse_property_definitions};

use crate::{
    BuiltinArbitraryPropertyOrder, CustomUtility, CustomVariant, ThemeNamespace,
    parse_animation_option_property_mappings, parse_builtin_animation_option_definitions,
    parse_builtin_arbitrary_property_order, parse_builtin_utility_definitions,
    parse_builtin_variant_definitions, parse_declaration_formats, parse_marker_utilities,
    parse_omit_token_prefixes, parse_relationship_patterns, parse_theme_namespace_types,
    parse_theme_order,
};

/// Immutable registries parsed from the CSS supplied by a host.
///
/// `utilities`, `variants`, and the metadata collections stay in authored order,
/// including repeated names; consumers resolve duplicates by their own documented
/// precedence rules and may not sort these vectors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuiltinDefinitions {
    /// The exact CSS this value was parsed from. It participates in snapshot
    /// fingerprints, so an upstream definition change invalidates artifacts even when
    /// the parsed structures happen to compare equal.
    pub source: String,
    /// All `@stillwind-utility` definitions in authored order.
    pub utilities: Vec<CustomUtility>,
    /// All `@stillwind-variant` definitions in authored order.
    pub variants: Vec<CustomVariant>,
    /// All `@stillwind-property` definitions in authored order.
    pub properties: Vec<CustomPropertyDefinition>,
    /// Exact-name utilities, grouped without losing authored order.
    pub builtin_exact: HashMap<String, Vec<CustomUtility>>,
    /// Wildcard utilities, which are matched by longest name prefix.
    pub wildcards: Vec<CustomUtility>,
    /// Source order for arbitrary properties, used when a utility body sets several of
    /// them and declaration order is observable.
    pub arbitrary_property_order: Vec<BuiltinArbitraryPropertyOrder>,
    /// Animation option definitions, kept apart from ordinary utilities so option
    /// spellings never become utilities.
    pub animation_options: Vec<CustomUtility>,
    /// Animation option name to the property it rewrites.
    pub animation_property_mappings: BTreeMap<String, String>,
    /// Declaration format name to its serialized value grammar.
    pub declaration_formats: BTreeMap<String, String>,
    /// Candidate patterns identifying compatibility relationships.
    pub relationships: BTreeSet<String>,
    /// Utilities that emit only a marker class and no declarations.
    pub marker_utilities: BTreeSet<String>,
    /// Namespace to the prefix a token name omits when it is referenced.
    pub omit_token_prefixes: Vec<(String, String)>,
    /// Theme namespace names in the order utilities consume them.
    pub theme_order: Vec<String>,
    /// CSS-authored value types per namespace.
    pub theme_namespace_types: BTreeMap<ThemeNamespace, Vec<ValueType>>,
}

impl BuiltinDefinitions {
    /// Parses every registry that can be expressed as CSS from `css`.
    ///
    /// Utility bodies retain their CSS token spelling, including meaningful whitespace.
    pub fn from_css(css: &str) -> Self {
        let utilities = parse_builtin_utility_definitions(css);
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
            animation_options: parse_builtin_animation_option_definitions(css),
            utilities,
            builtin_exact,
            wildcards,
            variants: parse_builtin_variant_definitions(css),
            properties: parse_property_definitions(css),
            arbitrary_property_order: parse_builtin_arbitrary_property_order(css),
            animation_property_mappings: parse_animation_option_property_mappings(css),
            declaration_formats: parse_declaration_formats(css),
            relationships: parse_relationship_patterns(css),
            marker_utilities: parse_marker_utilities(css),
            omit_token_prefixes: parse_omit_token_prefixes(css),
            theme_order: parse_theme_order(css),
            theme_namespace_types: parse_theme_namespace_types(css),
        }
    }
}
