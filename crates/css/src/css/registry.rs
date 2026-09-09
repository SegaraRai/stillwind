use super::{
    BTreeMap, BTreeSet, BuiltinArbitraryPropertyOrder, BuiltinVariantModifierType,
    BuiltinVariantSort, BuiltinVariantSortDirection, BuiltinVariantValueType,
    CustomPropertyDefinition, CustomUtility, CustomVariant, ThemeNamespace, ValueType,
    extract_custom_utilities, extract_custom_variants, extract_declaration_formats,
    extract_marker_utilities, extract_named_blocks, extract_named_blocks_with_offsets,
    extract_order_statements, extract_property_definitions, strip_css_comments,
};

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

pub(super) fn parse_ordered_utility_definitions(css: &str, directive: &str) -> Vec<CustomUtility> {
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

/// Parses the CSS-authored type set for each Tailwind theme namespace.
#[must_use]
pub fn parse_theme_namespace_types(css: &str) -> BTreeMap<ThemeNamespace, Vec<ValueType>> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-theme-namespace"))
        .filter_map(|segment| {
            segment
                .split_once(';')
                .map(|(definition, _)| definition.trim())
        })
        .filter_map(|definition| {
            let mut parts = definition.split_whitespace();
            let namespace = parts
                .next()?
                .strip_prefix("--")?
                .strip_suffix("-*")
                .and_then(ThemeNamespace::parse)?;
            let types = parts
                .filter_map(|value| value.strip_prefix('[')?.strip_suffix(']'))
                .filter_map(ValueType::from_hint)
                .collect::<Vec<_>>();
            (!types.is_empty()).then_some((namespace, types))
        })
        .collect()
}

/// Parses explicit cascade positions for arbitrary-property candidates.
/// Patterns use an exact property name or a trailing `*` prefix wildcard.
#[must_use]
pub fn parse_builtin_arbitrary_property_order(css: &str) -> Vec<BuiltinArbitraryPropertyOrder> {
    extract_order_statements(css, "@stillwind-arbitrary-property")
        .into_iter()
        .map(|(builtin_order, pattern)| BuiltinArbitraryPropertyOrder {
            pattern,
            builtin_order,
        })
        .collect()
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

pub(super) struct ParsedBuiltinVariantHeader {
    name: String,
    wildcard: bool,
    sort: Option<BuiltinVariantSort>,
    value_type: Option<BuiltinVariantValueType>,
    modifier_type: Option<BuiltinVariantModifierType>,
}

pub(super) fn parse_variant_name(value: &str) -> Option<(String, bool)> {
    if value.is_empty() {
        return None;
    }
    let wildcard = value.ends_with('*') && !value.ends_with(r"\*");
    Some((value.replace(r"\*", "*"), wildcard))
}

pub(super) fn parse_builtin_variant_header(header: &str) -> Option<ParsedBuiltinVariantHeader> {
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

/// Parses compiler-owned custom-property registrations (`@stillwind-property`).
#[must_use]
pub fn parse_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_property_definitions(css)
}

/// Extracts compatibility marker utility names declared by CSS metadata.
pub fn parse_marker_utilities(css: &str) -> BTreeSet<String> {
    extract_marker_utilities(css)
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

/// Canonicalizes serializer-only transparent color spellings emitted by CSS
/// color conversion. This is independent of any utility or framework name.
pub fn normalize_serialized_declaration(value: &str) -> String {
    value
        .replace(
            "oklab(0% none none/0) oklab(0% none none/0) oklab(0% none none/0) oklab(0% none none/0)",
            "#0000",
        )
        .replace(":oklab(0% none none/0);", ":#0000;")
}
