use std::collections::BTreeSet;

use super::utility_definitions::extract_relationship_patterns;

pub fn parse_function_formats(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-format-functions"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .flat_map(str::split_whitespace)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parses generic token-pruning rules authored by the registry CSS.
pub fn parse_omit_token_prefixes(css: &str) -> Vec<(String, String)> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-omit-token-prefix"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let (prefix, condition) = value.split_once(char::is_whitespace)?;
            let prefix = prefix.trim();
            let condition = condition.trim();
            (!prefix.is_empty() && !condition.is_empty())
                .then(|| (prefix.to_owned(), condition.to_owned()))
        })
        .collect()
}

/// Parses the authored order of theme namespaces used when serializing the
/// generated theme layer.
pub fn parse_theme_order(css: &str) -> Vec<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-theme-order"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .flat_map(str::split_whitespace)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn parse_relationship_patterns(css: &str) -> BTreeSet<String> {
    extract_relationship_patterns(css)
}
