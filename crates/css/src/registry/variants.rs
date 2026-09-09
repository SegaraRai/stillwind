use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::{Parse, ToCss};
use lightningcss::values::ident::Ident;

use crate::blocks::{extract_named_blocks, strip_css_comments};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantSortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuiltinVariantSort {
    pub group: Option<String>,
    pub key: Option<String>,
    pub direction: Option<BuiltinVariantSortDirection>,
    pub bare: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomVariant {
    pub name: String,
    /// Whether the final `*` in `name` captures a value. Escaped `\*` in a
    /// definition header is decoded as a literal asterisk instead.
    pub wildcard: bool,
    pub selector: String,
    /// Full `@variant` body. Empty for the `@custom-variant` selector shorthand.
    pub body: String,
    /// Global registration order for built-in variants (`@stillwind-variant`).
    pub builtin_order: Option<usize>,
    /// Optional ordering behavior authored in the built-in definition header.
    pub sort: Option<BuiltinVariantSort>,
    /// Grammar and semantic kind of the wildcard capture.
    pub value_type: Option<BuiltinVariantValueType>,
    /// Grammar required when the capture contains a `/modifier` suffix.
    pub modifier_type: Option<BuiltinVariantModifierType>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantValueType {
    Variant,
    Aria,
    Data,
    Nth,
    SupportsCondition,
}

impl BuiltinVariantValueType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "variant" => Some(Self::Variant),
            "aria" => Some(Self::Aria),
            "data" => Some(Self::Data),
            "nth" => Some(Self::Nth),
            "supports-condition" => Some(Self::SupportsCondition),
            _ => None,
        }
    }

    /// Validates CSS-defined captures whose grammar can be delegated to
    /// Lightning CSS. Compound variants and attribute shorthands are resolved
    /// structurally by the Tailwind registry instead.
    #[must_use]
    pub fn accepts_css(self, value: &str) -> bool {
        let stylesheet = match self {
            Self::Nth => format!(".x:nth-child({value}) {{ color: red; }}"),
            Self::SupportsCondition => {
                format!("@supports ({value}) {{ .x {{ color: red; }} }}")
            }
            Self::Variant | Self::Aria | Self::Data => return false,
        };
        StyleSheet::parse(&stylesheet, ParserOptions::default()).is_ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantModifierType {
    ClassIdentifier,
}

impl BuiltinVariantModifierType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "class-identifier" => Some(Self::ClassIdentifier),
            _ => None,
        }
    }

    /// Parses the logical class identifier and serializes it for insertion
    /// into a CSS selector, including any escapes required by CSS syntax.
    #[must_use]
    pub fn serialize(self, value: &str) -> Option<String> {
        match self {
            Self::ClassIdentifier => Ident::parse_string(value)
                .ok()?
                .to_css_string(PrinterOptions::default())
                .ok(),
        }
    }
}

/// Parses `@custom-variant` and block `@variant` definitions without
/// requiring a complete theme stylesheet. This mirrors the utility registry
/// entry point so tools can build a registry once and share it at runtime.
#[must_use]
pub fn parse_variant_definitions(css: &str) -> Vec<CustomVariant> {
    extract_custom_variants(css)
}

/// Parses compiler-owned `@stillwind-variant` definitions and attaches their
/// global source order.
#[must_use]
pub fn parse_builtin_variant_definitions(css: &str) -> Vec<CustomVariant> {
    extract_named_blocks(&strip_css_comments(css), "@stillwind-variant")
        .into_iter()
        .enumerate()
        .filter_map(|(builtin_order, (header, body))| {
            let header = parse_builtin_variant_header(&header)?;
            Some(CustomVariant {
                name: header.name,
                wildcard: header.wildcard,
                selector: String::new(),
                body,
                builtin_order: Some(builtin_order),
                sort: header.sort,
                value_type: header.value_type,
                modifier_type: header.modifier_type,
            })
        })
        .collect()
}

pub(crate) struct ParsedBuiltinVariantHeader {
    name: String,
    wildcard: bool,
    sort: Option<BuiltinVariantSort>,
    value_type: Option<BuiltinVariantValueType>,
    modifier_type: Option<BuiltinVariantModifierType>,
}

pub(crate) fn parse_variant_name(value: &str) -> Option<(String, bool)> {
    if value.is_empty() {
        return None;
    }
    let wildcard = value.ends_with('*') && !value.ends_with(r"\*");
    Some((value.replace(r"\*", "*"), wildcard))
}

pub(crate) fn parse_builtin_variant_header(header: &str) -> Option<ParsedBuiltinVariantHeader> {
    let name_end = header.find(char::is_whitespace).unwrap_or(header.len());
    let (name, wildcard) = parse_variant_name(&header[..name_end])?;
    let arguments = header[name_end..].trim();
    if arguments.is_empty() {
        return Some(ParsedBuiltinVariantHeader {
            name,
            wildcard,
            sort: None,
            value_type: None,
            modifier_type: None,
        });
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let mut sort = BuiltinVariantSort::default();
    let mut has_sort = false;
    let mut value_type = None;
    let mut modifier_type = None;
    for descriptor in arguments.split(',') {
        let (key, value) = descriptor.trim().split_once(':')?;
        let value = value.trim();
        match key.trim() {
            "value" => value_type = Some(BuiltinVariantValueType::parse(value)?),
            "modifier" => modifier_type = Some(BuiltinVariantModifierType::parse(value)?),
            "group" => {
                sort.group = Some(value.to_owned());
                has_sort = true;
            }
            "sort" => {
                sort.key = Some(value.to_owned());
                has_sort = true;
            }
            "direction" => {
                sort.direction = Some(match value {
                    "asc" => BuiltinVariantSortDirection::Ascending,
                    "desc" => BuiltinVariantSortDirection::Descending,
                    _ => return None,
                });
                has_sort = true;
            }
            "bare" => {
                sort.bare = value.parse().ok()?;
                has_sort = true;
            }
            _ => return None,
        }
    }
    Some(ParsedBuiltinVariantHeader {
        name,
        wildcard,
        sort: has_sort.then_some(sort),
        value_type,
        modifier_type,
    })
}

pub(crate) fn extract_custom_variants(css: &str) -> Vec<CustomVariant> {
    let css = strip_css_comments(css);
    let mut variants = css
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.contains('{') {
                return None;
            }
            let value = line.strip_prefix("@custom-variant ")?.trim_end_matches(';');
            let (name, selector) = value.split_once(' ')?;
            let (name, wildcard) = parse_variant_name(name)?;
            Some(CustomVariant {
                name,
                wildcard,
                selector: selector
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .to_owned(),
                body: String::new(),
                builtin_order: None,
                sort: None,
                value_type: None,
                modifier_type: None,
            })
        })
        .collect::<Vec<_>>();
    variants.extend(
        extract_named_blocks(&css, "@custom-variant")
            .into_iter()
            .filter_map(|(header, body)| {
                let (name, wildcard) = parse_variant_name(header.split_whitespace().next()?)?;
                Some(CustomVariant {
                    name,
                    wildcard,
                    selector: String::new(),
                    body,
                    builtin_order: None,
                    sort: None,
                    value_type: None,
                    modifier_type: None,
                })
            }),
    );
    variants.extend(
        extract_named_blocks(&css, "@variant")
            .into_iter()
            .filter_map(|(header, body)| {
                if !body.contains("@slot") {
                    return None;
                }
                let (name, wildcard) = parse_variant_name(header.split_whitespace().next()?)?;
                Some(CustomVariant {
                    name,
                    wildcard,
                    selector: String::new(),
                    body,
                    builtin_order: None,
                    sort: None,
                    value_type: None,
                    modifier_type: None,
                })
            }),
    );
    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_class_identifier_modifiers_for_css_selectors() {
        let kind = BuiltinVariantModifierType::ClassIdentifier;
        assert_eq!(kind.serialize("menu").as_deref(), Some("menu"));
        assert_eq!(
            kind.serialize(r"menu\:open").as_deref(),
            Some(r"menu\:open")
        );
        assert_eq!(kind.serialize("not an identifier"), None);
    }

    #[test]
    fn validates_nth_and_supports_conditions_through_lightning_css() {
        assert!(BuiltinVariantValueType::Nth.accepts_css("3n + 1"));
        assert!(BuiltinVariantValueType::Nth.accepts_css("odd"));
        assert!(!BuiltinVariantValueType::Nth.accepts_css("not-an-nth"));
        assert!(BuiltinVariantValueType::SupportsCondition.accepts_css("display: grid"));
    }

    #[test]
    fn parses_typed_variant_values_modifiers_and_literal_asterisks() {
        let variants = parse_builtin_variant_definitions(
            r"@stillwind-variant \* { :is(& > *) { @slot; } }
               @stillwind-variant group-* (value: [variant], modifier: [class-identifier]) { @slot; }
               @stillwind-variant supports-* (value: [supports-condition]) { @slot; }",
        );

        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name, "*");
        assert!(!variants[0].wildcard);
        assert!(variants[1].wildcard);
        assert_eq!(
            variants[1].value_type,
            Some(BuiltinVariantValueType::Variant)
        );
        assert_eq!(
            variants[1].modifier_type,
            Some(BuiltinVariantModifierType::ClassIdentifier)
        );
        assert_eq!(
            variants[2].value_type,
            Some(BuiltinVariantValueType::SupportsCondition)
        );
    }

    #[test]
    fn parses_builtin_variant_sort_options() {
        let variants = parse_builtin_variant_definitions(
            "@stillwind-variant max-* (sort: --breakpoint-*, direction: desc) { @slot; }\
             @stillwind-variant min-* (group: responsive-min, sort: --breakpoint-*, direction: asc, bare: true) { @slot; }",
        );

        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants[0].sort.as_ref().unwrap().key.as_deref(),
            Some("--breakpoint-*")
        );
        assert_eq!(
            variants[0].sort.as_ref().unwrap().direction,
            Some(BuiltinVariantSortDirection::Descending)
        );
        assert_eq!(
            variants[1].sort.as_ref().unwrap().group.as_deref(),
            Some("responsive-min")
        );
        assert!(variants[1].sort.as_ref().unwrap().bare);
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn variant_names_and_sort_headers_reject_invalid_metadata() {
        assert_eq!(
            parse_variant_name(r"literal\*"),
            Some(("literal*".into(), false))
        );
        assert_eq!(parse_variant_name("min-*"), Some(("min-*".into(), true)));
        assert_eq!(parse_variant_name(""), None);
        assert!(parse_builtin_variant_header("min-* (direction: sideways)").is_none());
        assert!(parse_builtin_variant_header("min-* (unknown: x)").is_none());
        assert!(parse_builtin_variant_header("min-* (bare: maybe)").is_none());
    }
}
