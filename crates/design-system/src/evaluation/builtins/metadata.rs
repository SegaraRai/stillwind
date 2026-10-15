//! Flat views over the selected built-in definitions.
//!
//! Accessors borrow the immutable owner's authored collections. Property emission and
//! completion use them without copying definitions or rebuilding candidate lookup.
//!
//! The compatibility color constants below are registry facts, not user settings: they
//! name the color space and capability query the Tailwind compatibility path emits.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CustomPropertyDefinition, ValueType};

use crate::{ThemeIndex, ThemeNamespace};

/// Every `@stillwind-property` definition selected by the theme, in authored order.
pub fn builtin_property_definitions(
    theme: &ThemeIndex,
) -> &[std::sync::Arc<CustomPropertyDefinition>] {
    theme.registry().properties()
}

/// Pairs of utility-name prefix and theme-token prefix for wildcard utilities whose
/// body reads a theme namespace.
///
/// Completion uses these to offer token names without hard-coding which utility family
/// owns which namespace. A definition whose body does not name a `--value(--token-*)`
/// domain contributes nothing.
pub fn builtin_utility_token_prefixes(theme: &ThemeIndex) -> Vec<(LeanStr, LeanStr)> {
    let mut entries = Vec::new();
    for utility in theme.registry().utilities() {
        let Some(prefix) = utility.name.strip_suffix('*') else {
            continue;
        };
        let prefix = prefix.trim_end_matches('-');
        let Some(start) = utility.body.find("--value(--") else {
            continue;
        };
        let rest = &utility.body[start + "--value(--".len()..];
        let Some(end) = rest.find("-*") else { continue };
        entries.push((prefix.to_lean_str(), rest[..end].to_lean_str()));
    }
    entries
}

/// Names of utilities that emit a compatibility marker class.
pub(in crate::evaluation) fn builtin_marker_utilities(theme: &ThemeIndex) -> &BTreeSet<LeanStr> {
    theme.registry().marker_utilities()
}

/// Declaration-format metadata declared by the selected built-ins.
pub fn builtin_declaration_formats(theme: &ThemeIndex) -> &BTreeMap<LeanStr, LeanStr> {
    theme.registry().declaration_formats()
}

/// Token-pruning rules declared by the selected built-ins.
pub fn builtin_omit_token_prefixes(theme: &ThemeIndex) -> &[(LeanStr, LeanStr)] {
    theme.registry().omit_token_prefixes()
}

/// The authored theme-namespace order used when serializing the theme layer.
pub fn builtin_theme_order(theme: &ThemeIndex) -> &[LeanStr] {
    theme.registry().theme_order()
}

/// Value types accepted per theme namespace, as declared by the built-ins.
pub(in crate::evaluation) fn builtin_theme_namespace_types(
    theme: &ThemeIndex,
) -> &BTreeMap<ThemeNamespace, Vec<ValueType>> {
    theme.registry().theme_namespace_types()
}

/// Color space of the compatibility fallback branch.
pub(in crate::evaluation) const COMPAT_COLOR_LEGACY_SPACE: &str = "srgb";

/// Color space of the modern branch, which the capability query guards.
pub(in crate::evaluation) const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";

/// Capability query that decides whether the modern color branch is usable.
pub(in crate::evaluation) const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

/// Relationship patterns declared by the selected built-ins.
pub(in crate::evaluation) fn builtin_relationship_patterns(
    theme: &ThemeIndex,
) -> &BTreeSet<LeanStr> {
    theme.registry().relationships()
}
