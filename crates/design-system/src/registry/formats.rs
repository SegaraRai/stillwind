//! Registry metadata for declaration composition, value typing and dependencies.
//!
//! `compose-space` specifies declaration composition and `zero-length` supplies
//! the length type required by arithmetic consumers. Neither requests formatting.
//! Whitespace, color spelling and arithmetic optimization belong to the host.

use std::collections::{BTreeMap, BTreeSet};

/// Parses generic token-pruning rules authored by the registry CSS.
///
/// Each rule pairs a token-name prefix with a condition string; when generated CSS
/// contains the condition, the prefixed tokens are dropped after dependency closure.
/// Rules keep authored order and incomplete entries are skipped.
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

/// Parses the authored order of theme namespaces used when serializing the generated
/// theme layer.
pub fn parse_theme_order(css: &str) -> Vec<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-theme-order"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .flat_map(str::split_whitespace)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parses the utility-name patterns whose presence marks a relationship candidate, for
/// example `group-*`, as a set.
pub fn parse_relationship_patterns(css: &str) -> BTreeSet<String> {
    extract_relationship_patterns(css)
}

/// Parses source-to-target declaration mappings used when an ordinary utility is
/// accepted as an animation option.
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

/// Parses declaration-format metadata, keyed by property name or by a pattern with a
/// single `*` wildcard.
///
/// Repeated entries for one key accumulate into one space-separated list and keep their
/// authored order.
pub fn parse_declaration_formats(css: &str) -> BTreeMap<String, String> {
    extract_declaration_formats(css)
}

/// Resolves declaration metadata by exact property name first, then by an authored
/// single-wildcard property pattern.
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

/// Reads `@stillwind-format` statements, merging repeats for one property into a single
/// space-separated format list.
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

/// Reads `@stillwind-relationship` statements into a deduplicated set.
pub(crate) fn extract_relationship_patterns(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-relationship"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_animation_option_property_mapping() {
        let css = "@stillwind-animation-option duration-* { animation-duration: --value(--duration-*); }\
                       @stillwind-animation-option-property transition-duration: animation-duration;\
                       @stillwind-theme-namespace --duration-* [time];\
                       @stillwind-theme-namespace --leading-* [number] [length];";
        assert_eq!(
            parse_animation_option_property_mappings(css).get("transition-duration"),
            Some(&"animation-duration".to_owned())
        );
    }

    #[test]
    fn merges_declaration_metadata() {
        let formats = parse_declaration_formats(
            "@stillwind-format transform zero-length; @stillwind-format transform compose-space;",
        );
        assert_eq!(
            formats.get("transform").map(String::as_str),
            Some("zero-length compose-space")
        );
        assert_eq!(
            parse_omit_token_prefixes("@stillwind-omit-token-prefix --shadow- --tw-shadow:;"),
            vec![("--shadow-".to_owned(), "--tw-shadow:".to_owned())]
        );
    }

    #[test]
    fn metadata_preserves_order_deduplicates_sets_and_rejects_incomplete_entries() {
        assert_eq!(
            parse_theme_order("@stillwind-theme-order color spacing;"),
            vec!["color", "spacing"]
        );
        assert_eq!(
            parse_omit_token_prefixes(
                "@stillwind-omit-token-prefix --x- used; @stillwind-omit-token-prefix broken;"
            ),
            vec![("--x-".into(), "used".into())]
        );
        assert_eq!(
            parse_relationship_patterns(
                "@stillwind-relationship group-*;@stillwind-relationship group-*;"
            ),
            BTreeSet::from(["group-*".into()])
        );
        assert_eq!(parse_animation_option_property_mappings("@stillwind-animation-option-property duration: animation-duration; @stillwind-animation-option-property : bad;").len(),1);
    }

    #[test]
    fn exact_formats_override_wildcards_and_repeated_formats_accumulate() {
        let formats = parse_declaration_formats(
            "@stillwind-format margin-* zero; @stillwind-format margin-top first; @stillwind-format margin-top second;",
        );
        assert_eq!(
            declaration_format(&formats, "margin-top"),
            Some("first second")
        );
        assert_eq!(declaration_format(&formats, "margin-left"), Some("zero"));
        assert_eq!(declaration_format(&formats, "padding"), None);
    }
}
