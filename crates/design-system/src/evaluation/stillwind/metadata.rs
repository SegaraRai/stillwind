//! Flat views over the selected Stillwind definitions.
//!
//! Accessors borrow the immutable owner's authored collections. Property emission and
//! completion use them without copying definitions or rebuilding candidate lookup.
//!
//! The compatibility color constants below are registry facts, not user settings: they
//! name the color space and capability query the Tailwind compatibility path emits.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::CustomPropertyDefinition;

use crate::{MarkerClasses, ThemeIndex};

/// Every `@stillwind-property` definition selected by the theme, in authored order.
pub fn stillwind_property_definitions(
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
pub fn stillwind_utility_token_prefixes(theme: &ThemeIndex) -> Vec<(LeanStr, LeanStr)> {
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

/// Marker classes declared or selected by the Stillwind definitions.
pub(in crate::evaluation) fn stillwind_markers(theme: &ThemeIndex) -> &MarkerClasses {
    theme.registry().markers()
}

/// Declaration-format metadata declared by `@stillwind-format`.
pub fn stillwind_declaration_formats(theme: &ThemeIndex) -> &BTreeMap<LeanStr, LeanStr> {
    theme.registry().declaration_formats()
}

/// Token-pruning rules declared by `@stillwind-omit-token-prefix`.
pub fn stillwind_omit_token_prefixes(theme: &ThemeIndex) -> &[(LeanStr, LeanStr)] {
    theme.registry().omit_token_prefixes()
}

/// The authored theme-namespace order used when serializing the theme layer.
pub fn stillwind_theme_order(theme: &ThemeIndex) -> &[LeanStr] {
    theme.registry().theme_order()
}

/// Color space used for generated opacity expressions.
pub(in crate::evaluation) const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";

/// Capability query that decides whether the modern color branch is usable.
pub(in crate::evaluation) const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

/// Relationship patterns declared by `@stillwind-relationship`.
pub(in crate::evaluation) fn stillwind_relationship_patterns(
    theme: &ThemeIndex,
) -> &BTreeSet<LeanStr> {
    theme.registry().relationships()
}
