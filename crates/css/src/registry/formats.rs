use std::collections::{BTreeMap, BTreeSet};

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

/// Parses source-to-target declaration mappings used when an ordinary utility
/// is accepted as an animation option.
#[must_use]
pub fn parse_animation_option_property_mappings(css: &str) -> BTreeMap<String, String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-animation-option-property"))
        .filter_map(|segment| segment.split_once(';').map(|(mapping, _)| mapping.trim()))
        .filter_map(|mapping| mapping.split_once(':'))
        .map(|(source, target)| (source.trim().to_owned(), target.trim().to_owned()))
        .filter(|(source, target)| !source.is_empty() && !target.is_empty())
        .collect()
}

pub fn parse_declaration_formats(css: &str) -> BTreeMap<String, String> {
    extract_declaration_formats(css)
}

/// Resolves declaration metadata by exact property name first, then by an
/// authored single-wildcard property pattern.
#[must_use]
pub fn declaration_format<'a>(
    formats: &'a BTreeMap<String, String>,
    property: &str,
) -> Option<&'a str> {
    formats.get(property).map(String::as_str).or_else(|| {
        formats.iter().find_map(|(pattern, format)| {
            let (prefix, suffix) = pattern.split_once('*')?;
            (property.starts_with(prefix) && property.ends_with(suffix)).then_some(format.as_str())
        })
    })
}

pub(crate) fn extract_declaration_formats(css: &str) -> BTreeMap<String, String> {
    let mut formats = BTreeMap::new();
    for (property, format) in css
        .split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-format"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let mut parts = value.split_whitespace();
            Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
        })
    {
        formats
            .entry(property)
            .and_modify(|existing: &mut String| {
                if !existing.is_empty() {
                    existing.push(' ');
                }
                existing.push_str(&format);
            })
            .or_insert(format);
    }
    formats
}

pub(crate) fn extract_relationship_patterns(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-relationship"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
