//! Flat views over the selected built-in definitions.
//!
//! The registry stores definitions grouped for lookup; evaluation often needs a whole
//! collection instead, for example every property definition or the utility names used
//! by completion. These accessors expose the registry's collections directly, so no
//! caller builds a second copy that could drift from the snapshot it came from.
//!
//! The compatibility color constants below are registry facts, not user settings: they
//! name the color space and capability query the Tailwind compatibility path emits.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_css::{CustomPropertyDefinition, ValueType};

use crate::{ThemeIndex, ThemeNamespace};

/// Every `@stillwind-property` definition selected by the theme, in authored order.
pub fn builtin_property_definitions(theme: &ThemeIndex) -> &[CustomPropertyDefinition] {
    &theme.registry.properties
}

/// Names of all selected utilities, in authored order and including repeats.
pub fn builtin_utility_names(theme: &ThemeIndex) -> Vec<String> {
    theme
        .registry
        .utilities
        .iter()
        .map(|utility| utility.name.clone())
        .collect()
}

/// Pairs of utility-name prefix and theme-token prefix for wildcard utilities whose
/// body reads a theme namespace.
///
/// Completion uses these to offer token names without hard-coding which utility family
/// owns which namespace. A definition whose body does not name a `--value(--token-*)`
/// domain contributes nothing.
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

/// Names of all selected variants, in authored order and including repeats.
pub fn builtin_variant_names(theme: &ThemeIndex) -> Vec<String> {
    theme
        .registry
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect()
}

/// Names of utilities that emit a compatibility marker class.
pub(crate) fn builtin_marker_utilities(theme: &ThemeIndex) -> &BTreeSet<String> {
    &theme.registry.marker_utilities
}

/// Declaration-format metadata declared by the selected built-ins.
pub fn builtin_declaration_formats(theme: &ThemeIndex) -> &BTreeMap<String, String> {
    &theme.registry.declaration_formats
}

/// Token-pruning rules declared by the selected built-ins.
pub fn builtin_omit_token_prefixes(theme: &ThemeIndex) -> &Vec<(String, String)> {
    &theme.registry.omit_token_prefixes
}

/// The authored theme-namespace order used when serializing the theme layer.
pub fn builtin_theme_order(theme: &ThemeIndex) -> &Vec<String> {
    &theme.registry.theme_order
}

/// Value types accepted per theme namespace, as declared by the built-ins.
pub(crate) fn builtin_theme_namespace_types(
    theme: &ThemeIndex,
) -> &BTreeMap<ThemeNamespace, Vec<ValueType>> {
    &theme.registry.theme_namespace_types
}

/// Color space of the compatibility fallback branch.
pub(crate) const COMPAT_COLOR_LEGACY_SPACE: &str = "srgb";

/// Color space of the modern branch, which the capability query guards.
pub(crate) const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";

/// Capability query that decides whether the modern color branch is usable.
pub(crate) const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

/// Relationship patterns declared by the selected built-ins.
pub(crate) fn builtin_relationship_patterns(theme: &ThemeIndex) -> &BTreeSet<String> {
    &theme.registry.relationships
}
