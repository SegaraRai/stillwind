//! Utility definition parsing.
//!
//! User-authored `@utility` blocks and compiler-owned `@stillwind-utility` blocks use
//! the same body language but carry different identity: built-ins record the byte
//! offset they were authored at, while user definitions have none. That offset supplies
//! the candidate precedence of authored utility ordering, so it must stay the authored
//! position rather than a counter over parsed definitions. A definition header may also
//! carry a `(tw-sort: property)` override, which supplies the property indices of
//! Tailwind utility ordering.
//!
//! Trailing-`*` names are ordinary here; wildcard matching happens when a candidate is
//! resolved. Duplicate names are preserved in source order because they are separate
//! declaration branches, not a replacement.

use std::collections::BTreeSet;

use cssparser::Parser;

use stillwind_css::{extract_named_blocks, extract_named_blocks_with_offsets, strip_css_comments};

/// One utility definition: its name, its body, and the ordering metadata of its
/// registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    /// Authored name; a trailing `*` marks a prefix pattern.
    pub name: String,
    /// Declaration body, excluding the selector block or `@slot`.
    pub body: String,
    /// Global registration order for built-in utilities (`@stillwind-utility`).
    pub builtin_order: Option<usize>,
    /// Property name from the header's `(tw-sort: property)` descriptor, which replaces
    /// the resolved property indices in Tailwind ordering mode.
    pub tw_sort: Option<String>,
}

/// Splits a definition header into its name and its optional `(tw-sort: property)`
/// descriptor.
///
/// The name is the first whitespace-delimited token. The descriptor, when present, must
/// hold exactly one `tw-sort` key whose value is a single identifier; any other header
/// is rejected, which drops the definition from the registry.
fn parse_utility_header(header: &str) -> Option<(String, Option<String>)> {
    let end = header.find(char::is_whitespace).unwrap_or(header.len());
    let name = header[..end].to_owned();
    if name.is_empty() {
        return None;
    }
    let descriptor = header[end..].trim();
    if descriptor.is_empty() {
        return Some((name, None));
    }
    let (key, value) = descriptor
        .strip_prefix('(')?
        .strip_suffix(')')?
        .split_once(':')?;
    if key.trim() != "tw-sort" {
        return None;
    }
    let value = value.trim();
    let mut parser = Parser::new(value);
    parser.expect_ident().ok()?;
    parser
        .is_exhausted()
        .then(|| (name, Some(value.to_owned())))
}

/// Parses user-authored `@utility` definitions without requiring a complete theme
/// stylesheet.
///
/// Returned definitions carry no `builtin_order`: authored ordering reads a
/// definition's position from [`crate::ThemeIndex::utility_positions`], and Tailwind
/// ordering derives its key from the resolved declarations instead.
#[must_use]
pub fn parse_utility_definitions(css: &str) -> Vec<CustomUtility> {
    extract_custom_utilities(css)
}

/// Parses compiler-owned `@stillwind-utility` definitions and attaches their global
/// source order.
///
/// The order is the directive's byte offset, which positions built-ins relative to one
/// another and to arbitrary-property positions in authored utility mode.
#[must_use]
pub fn parse_builtin_utility_definitions(css: &str) -> Vec<CustomUtility> {
    parse_ordered_utility_definitions(css, "@stillwind-utility")
}

/// Parses compiler-owned animation option definitions. These use the same value-domain
/// language as utilities without making the core know utility spellings or CSS property
/// projections.
///
/// Options are held apart from utilities so an option spelling never becomes a class
/// candidate of its own.
#[must_use]
pub fn parse_builtin_animation_option_definitions(css: &str) -> Vec<CustomUtility> {
    parse_ordered_utility_definitions(css, "@stillwind-animation-option")
}

/// Shared body of the ordered parsers: matches `directive` exactly and keeps the
/// authored offset.
///
/// A name that is missing from a block header is skipped rather than stored under an
/// empty name.
pub(crate) fn parse_ordered_utility_definitions(css: &str, directive: &str) -> Vec<CustomUtility> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .filter_map(|(builtin_order, header, body)| {
            let (name, tw_sort) = parse_utility_header(&header)?;
            Some(CustomUtility {
                name,
                body,
                builtin_order: Some(builtin_order),
                tw_sort,
            })
        })
        .collect()
}

/// Extracts compatibility marker utility names declared by CSS metadata.
///
/// A marker utility emits a literal class that relational selectors can find across
/// independent style calls, so the set is data about the registry, not a list of
/// utilities to resolve.
pub fn parse_marker_utilities(css: &str) -> BTreeSet<String> {
    extract_marker_utilities(css)
}

/// Extracts user `@utility` definitions after comment stripping, so commented example
/// CSS cannot register a definition.
pub(crate) fn extract_custom_utilities(css: &str) -> Vec<CustomUtility> {
    let css = strip_css_comments(css);
    extract_named_utility_blocks(&css, "@utility")
}

/// Shared body of the unordered parsers: matches `directive` exactly and leaves
/// [`CustomUtility::builtin_order`] unset.
pub(crate) fn extract_named_utility_blocks(css: &str, directive: &str) -> Vec<CustomUtility> {
    extract_named_blocks(css, directive)
        .into_iter()
        .filter_map(|(header, body)| {
            let (name, tw_sort) = parse_utility_header(&header)?;
            Some(CustomUtility {
                name,
                body,
                builtin_order: None,
                tw_sort,
            })
        })
        .collect()
}

/// Reads `@stillwind-marker` statements into a deduplicated set.
///
/// The set only answers membership, so authored order and repeats carry no meaning
/// here.
pub(crate) fn extract_marker_utilities(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-marker"))
        .filter_map(|segment| segment.split_once(';').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sort_metadata_without_adding_css_declarations() {
        for directive in ["@utility", "@stillwind-utility"] {
            let css = format!("{directive} space-x-* (tw-sort: row-gap) {{ margin:0; }}");
            let utilities = parse_ordered_utility_definitions(&css, directive);
            assert_eq!(utilities.len(), 1);
            assert_eq!(utilities[0].name, "space-x-*");
            assert_eq!(utilities[0].tw_sort.as_deref(), Some("row-gap"));
            assert_eq!(utilities[0].body.trim(), "margin:0;");
        }
        for header in [
            "",
            "x (tw-sort:)",
            "x (tw-sort: row-gap color)",
            "x (other: color)",
        ] {
            assert!(parse_utility_header(header).is_none());
        }
    }

    #[test]
    fn parses_animation_option_definitions() {
        let css = "@stillwind-animation-option duration-* { animation-duration: --value(--duration-*); }\
                       @stillwind-animation-option-property transition-duration: animation-duration;\
                       @stillwind-theme-namespace --duration-* [time];\
                       @stillwind-theme-namespace --leading-* [number] [length];";
        let options = parse_builtin_animation_option_definitions(css);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].name, "duration-*");
    }

    #[test]
    fn separates_builtin_utilities_from_userland() {
        let css = "@utility user-* { color:red; }\
                       @stillwind-utility builtin-* { color:blue; }\
                       @variant user-state { &:hover { @slot; } }\
                       @stillwind-variant builtin-state { &:focus { @slot; } }";

        let user_utilities = parse_utility_definitions(css);
        let builtin_utilities = parse_builtin_utility_definitions(css);
        assert_eq!(user_utilities.len(), 1);
        assert_eq!(user_utilities[0].name, "user-*");
        assert_eq!(user_utilities[0].builtin_order, None);
        assert_eq!(builtin_utilities.len(), 1);
        assert_eq!(builtin_utilities[0].name, "builtin-*");
        assert_eq!(
            builtin_utilities[0].builtin_order,
            css.find("@stillwind-utility")
        );
    }

    #[test]
    fn preserves_explicit_color_modifier_bodies() {
        let utilities = parse_utility_definitions(
            "@utility border { border-color:color-mix(in oklab,currentcolor --alpha(--modifier()),transparent); } @utility outline { outline-color:color-mix(in oklab,currentcolor --alpha(--modifier()),transparent); }",
        );

        assert_eq!(utilities.len(), 2);
        assert_eq!(utilities[0].name, "border");
        assert!(utilities[0].body.contains("border-color:color-mix"));
        assert_eq!(utilities[1].name, "outline");
        assert!(utilities[1].body.contains("outline-color:color-mix"));
    }

    #[test]
    fn preserves_repeated_mask_utility_definitions_in_source_order() {
        let utilities = parse_utility_definitions(
            "@utility mask-x-from-* { --tw-mask-left-from-position:--value(--spacing-*); } @utility mask-x-from-* { --tw-mask-right-from-color:--value(--color-*); } @utility mask-x-from-* { --alpha(--modifier()); }",
        );

        assert_eq!(utilities.len(), 3);
        assert!(
            utilities
                .iter()
                .all(|utility| utility.name == "mask-x-from-*")
        );
        assert!(
            utilities[0]
                .body
                .contains("--tw-mask-left-from-position:--value(--spacing-*")
        );
        assert!(
            utilities[1]
                .body
                .contains("--tw-mask-right-from-color:--value(--color-*")
        );
        assert!(utilities[2].body.contains("--alpha(--modifier())"));
    }

    #[test]
    fn named_and_ordered_utilities_preserve_bodies_and_exclude_prefix_collisions() {
        let css = "@utility user { color:red; } @stillwind-utility builtin {color:blue;}";
        let users = parse_utility_definitions(css);
        let builtins = parse_builtin_utility_definitions(css);
        assert_eq!(users[0].name, "user");
        assert_eq!(users[0].builtin_order, None);
        assert_eq!(builtins[0].body, "color:blue;");
        assert_eq!(builtins[0].builtin_order, css.find("@stillwind-utility"));
        assert_eq!(
            parse_ordered_utility_definitions(
                "@stillwind-utility-other x {x:y;}",
                "@stillwind-utility"
            ),
            [] as [CustomUtility; 0]
        );
        assert_eq!(
            parse_marker_utilities("@stillwind-marker group; @stillwind-marker group;"),
            BTreeSet::from(["group".into()])
        );
    }
}
