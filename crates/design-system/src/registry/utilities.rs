//! Utility definition parsing.
//!
//! User-authored `@utility` blocks and compiler-owned `@stillwind-utility` blocks use
//! the same body language but carry different identity: built-ins record the byte
//! offset they were authored at, while user definitions have none. That offset supplies
//! the candidate precedence of authored utility ordering, so it must stay the authored
//! position rather than a counter over parsed definitions. A definition header may also
//! carry a descriptor list such as `(tw-sort: property)`, which supplies the property
//! indices of Tailwind utility ordering, or the [`UtilityRole`] flags of a definition
//! that a Tailwind plugin adapter registers. CSS escapes in a name decode as in
//! Tailwind, so `@utility foo-1\/2` defines `foo-1/2`.
//!
//! Trailing-`*` names are ordinary here; wildcard matching happens when a candidate is
//! resolved. Duplicate names are preserved in source order because they are separate
//! declaration branches, not a replacement.

use cssparser::Parser;
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{extract_named_blocks_with_offsets, strip_css_comments};

/// One utility definition: its name, its body, and the ordering metadata of its
/// registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    /// Authored name; a trailing `*` marks a prefix pattern.
    pub name: LeanStr,
    /// Declaration body, excluding the selector block or `@slot`.
    pub body: LeanStr,
    /// Global registration order for built-in utilities (`@stillwind-utility`).
    pub builtin_order: Option<usize>,
    /// Property name from the header's `(tw-sort: property)` descriptor, which replaces
    /// the resolved property indices in Tailwind ordering mode.
    pub tw_sort: Option<LeanStr>,
    /// How the definition takes part in resolving its name.
    pub role: UtilityRole,
    /// Whether the answer to a `@plugin` statement contains the definition. Plugin
    /// definitions of a name are evaluated after built-ins and before the stylesheet's
    /// definitions, as Tailwind registers plugins before the stylesheet's `@utility`
    /// rules.
    pub plugin: bool,
}

/// Header flags of a definition that stands for one candidate of a functional
/// utility. A Tailwind plugin adapter registers the CSS that a `matchUtilities`
/// callback returned for one candidate this way, because the callback cannot be
/// expressed as a `--value()` body.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UtilityRole {
    /// `(functional)`: the name is one complete candidate, including a leading `-` and
    /// a `/modifier`, and the definition belongs to the functional utility of its root.
    /// An exact definition of the same name hides it, as it hides wildcard definitions.
    pub functional: bool,
    /// `(fallback)`: the definition applies only when no other definition of the
    /// candidate resolves. Tailwind treats a `matchUtilities` registration whose types
    /// include `any` and at least one other type this way.
    pub fallback: bool,
    /// `(stop)`: the definition contributes nothing and ends the evaluation of the
    /// definitions that Tailwind registers after it. Tailwind stops this way when a
    /// typed `matchUtilities` callback rejects an arbitrary value.
    pub stop: bool,
}

/// Parsed definition header: the decoded name and its descriptors.
struct UtilityHeader {
    name: LeanStr,
    tw_sort: Option<LeanStr>,
    role: UtilityRole,
}

/// Splits a definition header into its decoded name and its optional descriptor list.
///
/// The name is the first whitespace-delimited token. The descriptor list, when present,
/// is one parenthesized, comma-separated list of `tw-sort: <identifier>`, `functional`,
/// `fallback`, and `stop`. `fallback` and `stop` require `functional`. Any other header
/// is rejected, which drops the definition from the registry.
fn parse_utility_header(header: &str) -> Option<UtilityHeader> {
    let end = header.find(char::is_whitespace).unwrap_or(header.len());
    let mut parsed = UtilityHeader {
        name: decode_utility_name(&header[..end])?,
        tw_sort: None,
        role: UtilityRole::default(),
    };
    let descriptors = header[end..].trim();
    if descriptors.is_empty() {
        return Some(parsed);
    }
    for descriptor in descriptors.strip_prefix('(')?.strip_suffix(')')?.split(',') {
        match descriptor.split_once(':') {
            Some((key, value)) if key.trim() == "tw-sort" && parsed.tw_sort.is_none() => {
                let value = value.trim();
                let mut parser = Parser::new(value);
                parser.expect_ident().ok()?;
                parser.is_exhausted().then_some(())?;
                parsed.tw_sort = Some(value.to_lean_str());
            }
            None if descriptor.trim() == "functional" => parsed.role.functional = true,
            None if descriptor.trim() == "fallback" => parsed.role.fallback = true,
            None if descriptor.trim() == "stop" => parsed.role.stop = true,
            _ => return None,
        }
    }
    ((!parsed.role.fallback && !parsed.role.stop) || parsed.role.functional).then_some(parsed)
}

/// Decodes the CSS escapes of a definition name.
///
/// A name without a backslash is kept as written, so unescaped names such as
/// `icon-[mdi--close]` keep their meaning. An escape is a backslash followed by up to
/// six hexadecimal digits and one optional whitespace character, or by any other
/// character. Returns `None` for an empty name, a trailing backslash, or a code point
/// that is not a character.
pub(crate) fn decode_utility_name(raw: &str) -> Option<LeanStr> {
    if !raw.contains('\\') {
        return (!raw.is_empty()).then(|| raw.into());
    }
    let mut name = String::with_capacity(raw.len());
    let mut characters = raw.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            name.push(character);
            continue;
        }
        let mut digits = String::new();
        while digits.len() < 6
            && let Some(digit) = characters.next_if(char::is_ascii_hexdigit)
        {
            digits.push(digit);
        }
        if digits.is_empty() {
            name.push(characters.next()?);
        } else {
            name.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
            characters.next_if(|character| character.is_whitespace());
        }
    }
    (!name.is_empty()).then(|| name.into())
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
        .into_iter()
        .map(|(_, utility)| utility)
        .collect()
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
fn parse_ordered_utility_definitions(css: &str, directive: &str) -> Vec<CustomUtility> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .filter_map(|(builtin_order, header, body)| {
            let UtilityHeader {
                name,
                tw_sort,
                role,
            } = parse_utility_header(&header)?;
            Some(CustomUtility {
                name,
                body,
                builtin_order: Some(builtin_order),
                tw_sort,
                role,
                plugin: false,
            })
        })
        .collect()
}

/// Extracts user `@utility` definitions with the byte offset of each directive in
/// `css`. Commented example CSS cannot register a definition, and comments inside a
/// header or body are removed.
pub(crate) fn extract_custom_utilities(css: &str) -> Vec<(usize, CustomUtility)> {
    extract_named_blocks_with_offsets(css, "@utility")
        .into_iter()
        .filter_map(|(offset, header, body)| {
            let UtilityHeader {
                name,
                tw_sort,
                role,
            } = parse_utility_header(strip_css_comments(&header).trim())?;
            Some((
                offset,
                CustomUtility {
                    name,
                    body: strip_css_comments(&body).trim().into(),
                    builtin_order: None,
                    tw_sort,
                    role,
                    plugin: false,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::*;

    #[test]
    fn parses_sort_metadata_without_adding_css_declarations() {
        for directive in ["@utility", "@stillwind-utility"] {
            let css = format_args!("{directive} space-x-* (tw-sort: row-gap) {{ margin:0; }}")
                .to_lean_string();
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
    }
}
