//! Registry metadata for declaration composition, value typing and dependencies.
//!
//! `compose-space` specifies declaration composition and `zero-length` supplies
//! the length type required by arithmetic consumers. Neither requests formatting.
//! Whitespace, color spelling and arithmetic optimization belong to the host.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

/// Parses generic token-pruning rules authored by the registry CSS.
///
/// Each rule pairs a token-name prefix with a condition string; when generated CSS
/// contains the condition, the prefixed tokens are dropped after dependency closure.
/// Rules keep authored order and incomplete entries are skipped.
pub fn parse_omit_token_prefixes(css: &str) -> Vec<(LeanStr, LeanStr)> {
    css.split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-omit-token-prefix"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let (prefix, condition) = value.split_once(char::is_whitespace)?;
            let prefix = prefix.trim();
            let condition = condition.trim();
            (!prefix.is_empty() && !condition.is_empty())
                .then(|| (prefix.to_lean_str(), condition.to_lean_str()))
        })
        .collect()
}

/// Parses the authored order of theme namespaces used when serializing the generated
/// theme layer.
pub fn parse_theme_order(css: &str) -> Vec<LeanStr> {
    css.split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-theme-order"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .flat_map(str::split_whitespace)
        .filter(|value| !value.is_empty())
        .map(LeanStr::from)
        .collect()
}

/// Parses the utility-name patterns whose presence marks a relationship candidate, for
/// example `group-*`, as a set.
pub fn parse_relationship_patterns(css: &str) -> BTreeSet<LeanStr> {
    css.split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-relationship"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter(|value| !value.is_empty())
        .map(LeanStr::from)
        .collect()
}

/// Parses `@stillwind-legacy-utility old new;` statements into pairs of the old and the
/// new utility name, in authored order. A trailing `*` in both names keeps the rest of
/// the name. Statements without two names, or with a `*` in only one name, are skipped.
pub(super) fn parse_legacy_utilities(css: &str) -> Vec<(LeanStr, LeanStr)> {
    css.split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-legacy-utility"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let mut parts = value.split_whitespace();
            let (old, new) = (parts.next()?, parts.next()?);
            (old.ends_with('*') == new.ends_with('*')).then(|| (old.into(), new.into()))
        })
        .collect()
}

/// The theme namespace that one Tailwind v3 config key reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyThemeKey {
    /// The namespace name without `--` and `-*`, such as `text` for `fontSize`.
    pub namespace: LeanStr,
    /// How plugin configuration sees the CSS values of the namespace under the key.
    pub exposure: LegacyThemeExposure,
}

/// How plugin configuration sees the CSS values of a namespace under a v3 config key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyThemeExposure {
    /// Plugin configuration sees only the v3 default values of the key.
    None,
    /// The CSS values are added to the v3 default values of the key.
    Extend,
    /// The CSS values replace the v3 default values of the key.
    Replace,
}

/// Parses `@stillwind-legacy-theme-key key --namespace-* [exposure];` statements into a
/// map from each Tailwind v3 config key to its namespace. The optional exposure is
/// `extend` or `replace`. A later statement for the same key replaces an earlier one.
/// Statements whose namespace is not written as `--name-*`, or whose exposure is
/// unknown, are skipped.
pub(super) fn parse_legacy_theme_keys(css: &str) -> BTreeMap<LeanStr, LegacyThemeKey> {
    css.split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-legacy-theme-key"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let mut parts = value.split_whitespace();
            let key = parts.next()?;
            let namespace = parts.next()?.strip_prefix("--")?.strip_suffix("-*")?;
            let exposure = match parts.next() {
                None => LegacyThemeExposure::None,
                Some("extend") => LegacyThemeExposure::Extend,
                Some("replace") => LegacyThemeExposure::Replace,
                Some(_) => return None,
            };
            (!namespace.is_empty()).then(|| {
                (
                    key.into(),
                    LegacyThemeKey {
                        namespace: namespace.into(),
                        exposure,
                    },
                )
            })
        })
        .collect()
}

/// Parses declaration-format metadata, keyed by property name or by a pattern with a
/// single `*` wildcard.
///
/// Repeated entries for one key accumulate into one space-separated list and keep their
/// authored order.
pub fn parse_declaration_formats(css: &str) -> BTreeMap<LeanStr, LeanStr> {
    let mut formats: BTreeMap<LeanStr, LeanString> = BTreeMap::new();
    for (property, format) in css
        .split('@')
        .filter_map(|segment| strip_directive(segment, "stillwind-format"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter_map(|value| {
            let mut parts = value.split_whitespace();
            Some((parts.next()?.to_lean_str(), parts.next()?.to_lean_string()))
        })
    {
        formats
            .entry(property)
            .and_modify(|existing: &mut LeanString| {
                if !existing.is_empty() {
                    existing.push(' ');
                }
                existing.push_str(&format);
            })
            .or_insert(format);
    }
    formats
        .into_iter()
        .map(|(property, value)| (property, value.into()))
        .collect()
}

/// Resolves declaration metadata by exact property name first, then by an authored
/// single-wildcard property pattern.
#[must_use]
pub fn declaration_format<'a>(
    formats: &'a BTreeMap<LeanStr, LeanStr>,
    property: &str,
) -> Option<&'a str> {
    formats.get(property).map(LeanStr::as_str).or_else(|| {
        formats.iter().find_map(|(pattern, format)| {
            let (prefix, suffix) = pattern.split_once('*')?;
            (property.starts_with(prefix) && property.ends_with(suffix)).then_some(format.as_str())
        })
    })
}

/// Returns the rest of `segment` after the at-rule name `name`, or `None` when
/// `segment` does not start with `name` or the name continues, as in
/// `stillwind-format-x`.
///
/// `segment` is the text after one `@`. The name ends at whitespace, `;`, or the end of
/// `segment`.
fn strip_directive<'a>(segment: &'a str, name: &str) -> Option<&'a str> {
    let rest = segment.strip_prefix(name)?;
    (rest.is_empty()
        || rest.starts_with(|character: char| character.is_whitespace() || character == ';'))
    .then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_match_only_complete_names() {
        let css = "@stillwind-format-x margin zero; @stillwind-formatx margin one;                    @stillwind-format	margin two; @stillwind-theme-order;";
        assert_eq!(
            parse_declaration_formats(css),
            BTreeMap::from([("margin".into(), "two".into())])
        );
        assert_eq!(parse_theme_order(css), [] as [LeanStr; 0]);
    }

    #[test]
    fn merges_declaration_metadata() {
        let formats = parse_declaration_formats(
            "@stillwind-format transform zero-length; @stillwind-format transform compose-space;",
        );
        assert_eq!(
            formats.get("transform").map(LeanStr::as_str),
            Some("zero-length compose-space")
        );
        assert_eq!(
            parse_omit_token_prefixes("@stillwind-omit-token-prefix --shadow- --tw-shadow:;"),
            vec![("--shadow-".to_lean_str(), "--tw-shadow:".to_lean_str())]
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

    #[test]
    fn legacy_metadata_requires_matching_wildcards_and_known_exposures() {
        let css = "@stillwind-legacy-utility start-* inset-s-*; @stillwind-legacy-utility order-none order-0; @stillwind-legacy-utility bad-* bad; @stillwind-legacy-theme-key fontSize --text-* extend; @stillwind-legacy-theme-key fontFamily --font-*; @stillwind-legacy-theme-key colors --color-* merge; @stillwind-legacy-theme-key screens breakpoint;";
        assert_eq!(
            parse_legacy_utilities(css),
            [
                ("start-*".into(), "inset-s-*".into()),
                ("order-none".into(), "order-0".into())
            ]
        );
        assert_eq!(
            parse_legacy_theme_keys(css),
            BTreeMap::from([
                (
                    "fontFamily".into(),
                    LegacyThemeKey {
                        namespace: "font".into(),
                        exposure: LegacyThemeExposure::None,
                    }
                ),
                (
                    "fontSize".into(),
                    LegacyThemeKey {
                        namespace: "text".into(),
                        exposure: LegacyThemeExposure::Extend,
                    }
                ),
            ])
        );
    }
}
