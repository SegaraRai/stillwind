use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use stillwind_css::{
    CustomPropertyDefinition, ThemeNamespace, ValueType, parse_builtin_utility_definitions,
    parse_declaration_formats, parse_function_formats, parse_marker_utilities,
    parse_omit_token_prefixes, parse_property_definitions, parse_relationship_patterns,
    parse_theme_namespace_types, parse_theme_order,
};

use super::registry::{BUILTIN_REGISTRY_CSS, builtin_definitions};

/// Compiler-owned property definitions exposed by the built-in CSS entrypoint.
#[must_use]
pub fn builtin_property_definitions() -> &'static [CustomPropertyDefinition] {
    static PROPERTIES: OnceLock<Vec<CustomPropertyDefinition>> = OnceLock::new();
    PROPERTIES
        .get_or_init(|| parse_property_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

#[must_use]
pub fn builtin_utility_names() -> Vec<String> {
    builtin_definitions()
        .utilities
        .iter()
        .map(|utility| utility.name.clone())
        .collect()
}

#[must_use]
pub fn builtin_utility_token_prefixes() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for utility in parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS) {
        let Some(prefix) = utility.name.strip_suffix('*') else {
            continue;
        };
        let prefix = prefix.trim_end_matches('-');
        let Some(start) = utility.body.find("--value(--") else {
            continue;
        };
        let rest = &utility.body[start + "--value(--".len()..];
        let Some(end) = rest.find("-*") else {
            continue;
        };
        entries.push((prefix.to_owned(), rest[..end].to_owned()));
    }
    entries
}

#[must_use]
pub fn builtin_variant_names() -> Vec<String> {
    builtin_definitions()
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect()
}

#[must_use]
pub(crate) fn builtin_marker_utilities() -> &'static BTreeSet<String> {
    static MARKERS: OnceLock<BTreeSet<String>> = OnceLock::new();
    MARKERS.get_or_init(|| parse_marker_utilities(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_declaration_formats() -> &'static BTreeMap<String, String> {
    static FORMATS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    FORMATS.get_or_init(|| parse_declaration_formats(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_omit_token_prefixes() -> &'static Vec<(String, String)> {
    static RULES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    RULES.get_or_init(|| parse_omit_token_prefixes(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_theme_order() -> &'static Vec<String> {
    static ORDER: OnceLock<Vec<String>> = OnceLock::new();
    ORDER.get_or_init(|| parse_theme_order(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub(crate) fn builtin_function_formats() -> &'static BTreeSet<String> {
    static FORMATS: OnceLock<BTreeSet<String>> = OnceLock::new();
    FORMATS.get_or_init(|| parse_function_formats(BUILTIN_REGISTRY_CSS))
}

pub(crate) fn builtin_theme_namespace_types() -> &'static BTreeMap<ThemeNamespace, Vec<ValueType>> {
    static TYPES: OnceLock<BTreeMap<ThemeNamespace, Vec<ValueType>>> = OnceLock::new();
    TYPES.get_or_init(|| parse_theme_namespace_types(BUILTIN_REGISTRY_CSS))
}

pub(crate) const COMPAT_COLOR_LEGACY_SPACE: &str = "srgb";

pub(crate) const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";

pub(crate) const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

#[must_use]
pub(crate) fn builtin_relationship_patterns() -> &'static BTreeSet<String> {
    static PATTERNS: OnceLock<BTreeSet<String>> = OnceLock::new();
    PATTERNS.get_or_init(|| parse_relationship_patterns(BUILTIN_REGISTRY_CSS))
}
