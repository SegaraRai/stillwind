use std::collections::BTreeSet;
use std::fmt::Write;

use crate::blocks::{extract_named_blocks, extract_named_blocks_with_offsets, strip_css_comments};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    pub name: String,
    pub body: String,
    /// Global registration order for built-in utilities (`@stillwind-utility`).
    pub builtin_order: Option<usize>,
}

/// Parses user-authored `@utility` definitions without requiring a complete
/// theme stylesheet.
#[must_use]
pub fn parse_utility_definitions(css: &str) -> Vec<CustomUtility> {
    extract_custom_utilities(css)
}

/// Parses compiler-owned `@stillwind-utility` definitions and attaches their
/// global source order.
#[must_use]
pub fn parse_builtin_utility_definitions(css: &str) -> Vec<CustomUtility> {
    parse_ordered_utility_definitions(css, "@stillwind-utility")
}

/// Parses compiler-owned animation option definitions. These use the same
/// value-domain language as utilities without making the core know utility
/// spellings or CSS property projections.
#[must_use]
pub fn parse_builtin_animation_option_definitions(css: &str) -> Vec<CustomUtility> {
    parse_ordered_utility_definitions(css, "@stillwind-animation-option")
}

pub(crate) fn parse_ordered_utility_definitions(css: &str, directive: &str) -> Vec<CustomUtility> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .filter_map(|(builtin_order, header, body)| {
            if header.starts_with("-family") {
                return None;
            }
            let name = header.split_whitespace().next()?.to_owned();
            Some(CustomUtility {
                name,
                body,
                builtin_order: Some(builtin_order),
            })
        })
        .collect()
}

/// Extracts compatibility marker utility names declared by CSS metadata.
pub fn parse_marker_utilities(css: &str) -> BTreeSet<String> {
    extract_marker_utilities(css)
}

pub(crate) fn extract_custom_utilities(css: &str) -> Vec<CustomUtility> {
    let css = strip_css_comments(css);
    let mut utilities = extract_named_utility_blocks(&css, "@utility");
    utilities.extend(extract_utility_families(&css));
    utilities
}

pub(crate) fn extract_named_utility_blocks(css: &str, directive: &str) -> Vec<CustomUtility> {
    extract_named_blocks(css, directive)
        .into_iter()
        .filter_map(|(header, body)| {
            if header.starts_with("-family") {
                return None;
            }
            let name = header.split_whitespace().next()?.to_owned();
            Some(CustomUtility {
                name,
                body,
                builtin_order: None,
            })
        })
        .collect()
}

pub(crate) fn extract_marker_utilities(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-marker"))
        .filter_map(|segment| segment.split_once(';').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Expands the generic value-bearing utility-family syntax.
///
/// ```css
/// @utility-family value=--value(--spacing-*, --spacing(), [length]) {
///   inset: inset;
///   inset-x: inset-inline;
/// }
/// ```
///
/// The value expression is authored in the CSS header, so this parser does
/// not need a table of framework namespaces or property families. A family
/// definition remains reviewable without enumerating concrete token values.
pub(crate) fn extract_utility_families(css: &str) -> Vec<CustomUtility> {
    extract_named_blocks(css, "@utility-family")
        .into_iter()
        .flat_map(|(header, body)| {
            let Some(value) = parse_utility_family_header(&header) else {
                return Vec::new();
            };
            body.split(';')
                .filter_map(|entry| {
                    let (names, properties) = entry.split_once(':')?;
                    let names = names
                        .split(',')
                        .map(str::trim)
                        .filter(|name| !name.is_empty());
                    let properties = properties
                        .split(',')
                        .map(str::trim)
                        .filter(|property| !property.is_empty())
                        .collect::<Vec<_>>();
                    (!properties.is_empty()).then_some((names, properties))
                })
                .flat_map(move |(names, properties)| {
                    names.flat_map({
                        let value = value.clone();
                        move |name| {
                            let mut body = String::new();
                            for property in &properties {
                                write!(body, "{property}:{value};")
                                    .expect("writing to a String cannot fail");
                            }
                            vec![CustomUtility {
                                name: format!("{name}-*"),
                                body,
                                builtin_order: None,
                            }]
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Reads the generic family header. Value expressions are authored in CSS as
/// `value=--value(...)`. Headers without an authored value expression are
/// ignored, keeping framework vocabulary and sign handling out of the parser.
/// Negative utilities must be authored explicitly with their own `* -1`.
pub(crate) fn parse_utility_family_header(header: &str) -> Option<String> {
    let start = header.find("value=")?;
    let value = &header[start + "value=".len()..];
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_ordered_utilities_preserve_bodies_and_exclude_family_headers() {
        let css = "@utility user { color:red; } @stillwind-utility builtin {color:blue;}";
        let users = parse_utility_definitions(css);
        let builtins = parse_builtin_utility_definitions(css);
        assert_eq!(users[0].name, "user");
        assert_eq!(users[0].builtin_order, None);
        assert_eq!(builtins[0].body, "color:blue;");
        assert_eq!(builtins[0].builtin_order, css.find("@stillwind-utility"));
        assert!(
            parse_ordered_utility_definitions(
                "@stillwind-utility-family x {x:y;}",
                "@stillwind-utility"
            )
            .is_empty()
        );
        assert_eq!(
            parse_marker_utilities("@stillwind-marker group; @stillwind-marker group;"),
            BTreeSet::from(["group".into()])
        );
    }

    #[test]
    fn family_expansion_preserves_alias_and_property_order_and_skips_empty_entries() {
        let utilities = extract_utility_families(
            "@utility-family value=--value(number) { x,y: width,height; bad; empty: ; }",
        );
        assert_eq!(utilities.len(), 2);
        assert_eq!(
            utilities
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>(),
            ["x-*", "y-*"]
        );
        assert!(
            utilities
                .iter()
                .all(|u| u.body == "width:--value(number);height:--value(number);")
        );
        for header in ["", "value=   ", "number"] {
            assert!(parse_utility_family_header(header).is_none());
        }
        assert_eq!(
            parse_utility_family_header("value=  calc(--value(number) * -1) ").as_deref(),
            Some("calc(--value(number) * -1)")
        );
    }
}
