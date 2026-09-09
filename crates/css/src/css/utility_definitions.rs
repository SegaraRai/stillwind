use super::{BTreeMap, BTreeSet, CustomUtility, Write, extract_named_blocks, strip_css_comments};

pub(super) fn extract_custom_utilities(css: &str) -> Vec<CustomUtility> {
    let css = strip_css_comments(css);
    let mut utilities = extract_named_utility_blocks(&css, "@utility");
    utilities.extend(extract_utility_families(&css));
    utilities
}

pub(super) fn extract_named_utility_blocks(css: &str, directive: &str) -> Vec<CustomUtility> {
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

pub(super) fn extract_marker_utilities(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-marker"))
        .filter_map(|segment| segment.split_once(';').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn extract_declaration_formats(css: &str) -> BTreeMap<String, String> {
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

pub(super) fn extract_relationship_patterns(css: &str) -> BTreeSet<String> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-relationship"))
        .filter_map(|segment| segment.split_once(';').map(|(value, _)| value.trim()))
        .filter(|value| !value.is_empty())
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
pub(super) fn extract_utility_families(css: &str) -> Vec<CustomUtility> {
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
pub(super) fn parse_utility_family_header(header: &str) -> Option<String> {
    let start = header.find("value=")?;
    let value = &header[start + "value=".len()..];
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}
