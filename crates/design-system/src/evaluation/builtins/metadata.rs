use std::collections::{BTreeMap, BTreeSet};

use stillwind_css::{CustomPropertyDefinition, ValueType};

use crate::{ThemeIndex, ThemeNamespace};

pub fn builtin_property_definitions(theme: &ThemeIndex) -> &[CustomPropertyDefinition] {
    &theme.registry.properties
}

pub fn builtin_utility_names(theme: &ThemeIndex) -> Vec<String> {
    theme
        .registry
        .utilities
        .iter()
        .map(|utility| utility.name.clone())
        .collect()
}

pub fn builtin_utility_token_prefixes(theme: &ThemeIndex) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for utility in &theme.registry.utilities {
        let Some(prefix) = utility.name.strip_suffix('*') else {
            continue;
        };
        let prefix = prefix.trim_end_matches('-');
        let Some(start) = utility.body.find("--value(--") else {
            continue;
        };
        let rest = &utility.body[start + "--value(--".len()..];
        let Some(end) = rest.find("-*") else { continue };
        entries.push((prefix.to_owned(), rest[..end].to_owned()));
    }
    entries
}

pub fn builtin_variant_names(theme: &ThemeIndex) -> Vec<String> {
    theme
        .registry
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect()
}

pub(crate) fn builtin_marker_utilities(theme: &ThemeIndex) -> &BTreeSet<String> {
    &theme.registry.marker_utilities
}

pub fn builtin_declaration_formats(theme: &ThemeIndex) -> &BTreeMap<String, String> {
    &theme.registry.declaration_formats
}

pub fn builtin_omit_token_prefixes(theme: &ThemeIndex) -> &Vec<(String, String)> {
    &theme.registry.omit_token_prefixes
}

pub fn builtin_theme_order(theme: &ThemeIndex) -> &Vec<String> {
    &theme.registry.theme_order
}

pub(crate) fn builtin_theme_namespace_types(
    theme: &ThemeIndex,
) -> &BTreeMap<ThemeNamespace, Vec<ValueType>> {
    &theme.registry.theme_namespace_types
}

pub(crate) const COMPAT_COLOR_LEGACY_SPACE: &str = "srgb";
pub(crate) const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";
pub(crate) const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

pub(crate) fn builtin_relationship_patterns(theme: &ThemeIndex) -> &BTreeSet<String> {
    &theme.registry.relationships
}
