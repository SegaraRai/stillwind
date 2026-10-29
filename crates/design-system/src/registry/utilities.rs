//! Utility definition parsing.
//!
//! User-authored `@utility` blocks and compiler-owned `@stillwind-utility` blocks use
//! the same body language but carry different identity: `@stillwind-utility` records
//! the byte offset they were authored at, while user definitions have none. That offset
//! supplies the candidate precedence of authored utility ordering, so it must stay the
//! authored position rather than a counter over parsed definitions. A definition header
//! may also carry a descriptor list such as `(tw-sort: property)`, which supplies the
//! property indices of Tailwind utility ordering, or the [`UtilityRole`] flags of a
//! definition that a Tailwind plugin adapter registers. CSS escapes in a name decode as
//! in Tailwind, so `@utility foo-1\/2` defines `foo-1/2`.
//!
//! Trailing-`*` names are ordinary here; wildcard matching happens when a candidate is
//! resolved. Duplicate names are preserved in source order because they are separate
//! declaration branches, not a replacement.

use cssparser::Parser;
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{
    CssBanner, banner_blocks, decode_css_escapes, extract_named_blocks, strip_css_comments,
    whitespace_separated_ranges, without_banner_blocks,
};

/// Where a definition comes from. Lint checks the declarations that a definition
/// emits when the definition is the user's own, or when the lint settings select its
/// package or plugin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionOrigin {
    /// The user's own CSS, including the answer to a `@plugin` statement that the
    /// user's CSS loads by a relative or absolute path.
    Own,
    /// Package CSS, such as the `stillwind` package and files under `node_modules`.
    Library {
        /// Package name, such as `stillwind` or `@scope/name`, or `None` when the host
        /// does not know it.
        package: Option<LeanStr>,
    },
    /// The answer to a `@plugin` statement that loads a package or a plugin configured
    /// for the project.
    Plugin {
        /// Specifier of the `@plugin` statement, which names the package or the
        /// configured plugin.
        name: LeanStr,
    },
}

impl Default for DefinitionOrigin {
    /// Returns a library of an unknown package, the origin of CSS that the host does
    /// not classify.
    fn default() -> Self {
        Self::Library { package: None }
    }
}

/// One registered utility definition with its CSS body, matching role, ordering, and
/// classification for lint policies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    /// Notices required when the definition contributes declarations.
    pub banners: Vec<CssBanner>,
    /// Decoded utility name. A trailing `*` marks a prefix pattern.
    pub name: LeanStr,
    /// CSS inside the definition's outer braces, including nested rules and placeholder
    /// functions.
    pub body: LeanStr,
    /// Byte offset of a `@stillwind-utility` directive in the parsed CSS, which orders
    /// these definitions. `None` for `@utility`, whose order comes from
    /// [`crate::ThemeIndex::utility_positions`].
    pub position: Option<usize>,
    /// Property name from the header's `(tw-sort: property)` descriptor, which replaces
    /// the resolved property indices in Tailwind ordering mode.
    pub tw_sort: Option<LeanStr>,
    /// How the definition takes part in resolving its name.
    pub role: UtilityRole,
    /// True for an `(implementation)` header. Evaluators classify fixed declarations
    /// and their token uses as implementation CSS. Declarations with executable
    /// placeholders retain ordinary classifications.
    pub implementation: bool,
    /// Whether the answer to a `@plugin` statement contains the definition. Plugin
    /// definitions of a name are evaluated after `@stillwind-utility` definitions and
    /// before the stylesheet's definitions, as Tailwind registers plugins before the
    /// stylesheet's `@utility` rules.
    pub plugin: bool,
    /// Source of the definition, which decides whether lint checks the declarations
    /// that it emits.
    pub origin: DefinitionOrigin,
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
    implementation: bool,
}

/// Splits a definition header into its decoded name and its optional descriptor list.
///
/// The name is the first whitespace-delimited token. The descriptor list, when present,
/// is one parenthesized, comma-separated list of `tw-sort: <identifier>`, `functional`,
/// `fallback`, `stop`, and `implementation`. `tw-sort` can occur only once. `fallback`
/// and `stop` require `functional`. Returns `None` when the header is empty, its name
/// cannot be decoded, or its descriptor list is invalid or unsupported. Callers omit
/// rejected definitions from the registry.
fn parse_utility_header(header: &str) -> Option<UtilityHeader> {
    let end = whitespace_separated_ranges(header).first()?.end;
    let mut parsed = UtilityHeader {
        name: decode_css_escapes(&header[..end])?,
        tw_sort: None,
        role: UtilityRole::default(),
        implementation: false,
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
            None if descriptor.trim() == "functional" => {
                parsed.role.functional = true;
            }
            None if descriptor.trim() == "fallback" => {
                parsed.role.fallback = true;
            }
            None if descriptor.trim() == "stop" => {
                parsed.role.stop = true;
            }
            None if descriptor.trim() == "implementation" => {
                parsed.implementation = true;
            }
            _ => {
                return None;
            }
        }
    }
    ((!parsed.role.fallback && !parsed.role.stop) || parsed.role.functional).then_some(parsed)
}

/// Parses compiler-owned `@stillwind-utility` definitions in source order.
///
/// `position` is the byte offset of each directive in `css`, which positions these
/// definitions relative to one another and to arbitrary-property positions in authored
/// utility mode. Definitions whose headers cannot be parsed are skipped.
#[must_use]
pub fn parse_stillwind_utility_definitions(css: &str) -> Vec<CustomUtility> {
    extract_named_blocks(css, "@stillwind-utility")
        .into_iter()
        .filter_map(|(position, header, body)| {
            let UtilityHeader {
                name,
                tw_sort,
                role,
                implementation,
            } = parse_utility_header(&header)?;
            let banners = banner_blocks(&body)
                .ok()?
                .into_iter()
                .map(|block| block.banner)
                .collect();
            let body = without_banner_blocks(&body).ok()?.into();
            Some(CustomUtility {
                banners,
                name,
                body,
                position: Some(position),
                tw_sort,
                role,
                implementation,
                plugin: false,
                origin: DefinitionOrigin::default(),
            })
        })
        .collect()
}

/// Parses user-authored `@utility` definitions without requiring a complete theme
/// stylesheet. Returns each definition with the byte offset of its directive in `css`.
/// Commented example CSS cannot register a definition, and comments inside a header or
/// body are removed.
///
/// Returned definitions carry no `position`: authored ordering reads a definition's
/// position from [`crate::ThemeIndex::utility_positions`], and Tailwind ordering
/// derives its key from the resolved declarations instead.
pub(crate) fn parse_utility_definitions(css: &str) -> Vec<(usize, CustomUtility)> {
    extract_named_blocks(css, "@utility")
        .into_iter()
        .filter_map(|(offset, header, body)| {
            let UtilityHeader {
                name,
                tw_sort,
                role,
                implementation,
            } = parse_utility_header(strip_css_comments(&header).trim())?;
            let banners = banner_blocks(&body)
                .ok()?
                .into_iter()
                .map(|block| block.banner)
                .collect();
            let body = without_banner_blocks(&body).ok()?;
            Some((
                offset,
                CustomUtility {
                    banners,
                    name,
                    body: strip_css_comments(&body).trim().into(),
                    position: None,
                    tw_sort,
                    role,
                    implementation,
                    plugin: false,
                    origin: DefinitionOrigin::default(),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::*;
    use crate::test_support::utility_definitions;

    #[test]
    fn parses_sort_metadata_without_adding_css_declarations() {
        for directive in ["@utility", "@stillwind-utility"] {
            let css = format_args!("{directive} space-x-* (tw-sort: row-gap) {{ margin:0; }}")
                .to_lean_string();
            let utilities = if directive == "@utility" {
                utility_definitions(&css)
            } else {
                parse_stillwind_utility_definitions(&css)
            };
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
    fn parses_implementation_with_other_header_descriptors() {
        for directive in ["@utility", "@stillwind-utility"] {
            let css = format!("{directive} icon (implementation, tw-sort: width) {{ width:1em; }}");
            let utilities = if directive == "@utility" {
                utility_definitions(&css)
            } else {
                parse_stillwind_utility_definitions(&css)
            };
            assert_eq!(utilities.len(), 1);
            assert!(utilities[0].implementation);
            assert_eq!(utilities[0].tw_sort.as_deref(), Some("width"));
        }
    }

    #[test]
    fn separates_stillwind_utilities_from_tailwind_style_utilities() {
        let css = "@utility user-* { color:red; }\
                       @stillwind-utility builtin-* { color:blue; }\
                       @variant user-state { &:hover { @slot; } }\
                       @stillwind-variant builtin-state { &:focus { @slot; } }";

        let user_utilities = utility_definitions(css);
        let stillwind_utilities = parse_stillwind_utility_definitions(css);
        assert_eq!(user_utilities.len(), 1);
        assert_eq!(user_utilities[0].name, "user-*");
        assert_eq!(user_utilities[0].position, None);
        assert_eq!(stillwind_utilities.len(), 1);
        assert_eq!(stillwind_utilities[0].name, "builtin-*");
        assert_eq!(
            stillwind_utilities[0].position,
            css.find("@stillwind-utility")
        );
    }

    #[test]
    fn preserves_explicit_color_modifier_bodies() {
        let utilities = utility_definitions(
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
        let utilities = utility_definitions(
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
        let users = utility_definitions(css);
        let stillwind = parse_stillwind_utility_definitions(css);
        assert_eq!(users[0].name, "user");
        assert_eq!(users[0].position, None);
        assert_eq!(stillwind[0].body, "color:blue;");
        assert_eq!(stillwind[0].position, css.find("@stillwind-utility"));
        assert_eq!(
            parse_stillwind_utility_definitions("@stillwind-utility-other x {x:y;}"),
            [] as [CustomUtility; 0]
        );
    }
}
