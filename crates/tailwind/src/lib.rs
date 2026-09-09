//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap},
    fmt::Write,
    sync::OnceLock,
};

use stillwind_css::{
    BuiltinVariantSortDirection, CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace,
    ValueDomain, ValueType, shadow_with_color_fallback, shadow_with_color_override,
};
use stillwind_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    /// Source order of the matched `@stillwind-utility`.
    pub builtin_order: Option<usize>,
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads.
    /// CSS fallbacks and selector-bearing branches must never be discarded
    /// merely because they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
}

/// A conditional declaration authored inside a CSS `@utility` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedUtilityRule {
    pub at_rule: NestedUtilityAtRule,
    pub condition: String,
    pub declaration: String,
    /// Whether the conditional was authored inside the utility's selector
    /// block (or as a CSS `@supports` declaration, which is nested by CSS).
    /// This preserves placement from the CSS source instead of classifying
    /// declarations by Tailwind property names.
    pub inside: bool,
}

/// The CSS conditional wrapper associated with a nested utility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedUtilityAtRule {
    Media,
    Supports,
}

type ParsedNestedUtilityRule = (NestedUtilityAtRule, String, String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct UtilityResolution {
    declaration: String,
    selector: Option<String>,
    nested: Vec<NestedUtilityRule>,
    additional: Vec<ResolvedUtilityTarget>,
    builtin_order: Option<usize>,
}

#[derive(Clone, Debug, Default)]
struct ResolvedCssDeclarations {
    base: String,
    compat_modern: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompatColorMode {
    Legacy,
    Modern,
}

impl UtilityResolution {
    fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
            builtin_order: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    Compound(Vec<VariantApplication>),
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
    },
    SelectorMedia {
        selector: String,
        condition: String,
    },
    SelectorMediaRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
        condition: String,
    },
    Media(String),
    Container(String),
    Supports(String),
    StartingStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    pub property: String,
    pub value: String,
}

const BUILTIN_REGISTRY_CSS: &str = concat!(
    include_str!("../../../css/builtins.css"),
    include_str!("../../../css/builtins/registry-metadata.css"),
    include_str!("../../../css/builtins/relationship-metadata.css"),
    include_str!("../../../css/builtins/layout-utilities.css"),
    include_str!("../../../css/builtins/spacing-sizing-utilities.css"),
    include_str!("../../../css/builtins/flex-grid-utilities.css"),
    include_str!("../../../css/builtins/typography-utilities.css"),
    include_str!("../../../css/builtins/backgrounds-utilities.css"),
    include_str!("../../../css/builtins/borders-utilities.css"),
    include_str!("../../../css/builtins/effects-utilities.css"),
    include_str!("../../../css/builtins/masks-utilities.css"),
    include_str!("../../../css/builtins/transforms-utilities.css"),
    include_str!("../../../css/builtins/motion-utilities.css"),
    include_str!("../../../css/builtins/interactivity-utilities.css"),
    include_str!("../../../css/builtins/svg-utilities.css"),
    include_str!("../../../css/builtins/relationship-variants.css"),
    include_str!("../../../css/builtins/structural-variants.css"),
    include_str!("../../../css/builtins/state-variants.css"),
    include_str!("../../../css/builtins/dynamic-selector-variants.css"),
    include_str!("../../../css/builtins/conditional-variants.css"),
    include_str!("../../../css/builtins/properties.css"),
);

struct BuiltinUtilityRegistry {
    builtin_exact: HashMap<String, Vec<CustomUtility>>,
    wildcards: Vec<CustomUtility>,
    arbitrary_property_order: Vec<stillwind_css::BuiltinArbitraryPropertyOrder>,
}

fn builtin_utilities() -> &'static BuiltinUtilityRegistry {
    static UTILITIES: OnceLock<BuiltinUtilityRegistry> = OnceLock::new();
    UTILITIES.get_or_init(|| {
        let normalize = |utility: CustomUtility| {
            let mut utility = utility;
            // The registry asset is intentionally formatted for humans. The generated
            // output must nevertheless be byte-stable across formatter versions.
            utility.body = canonical_builtin_body(&utility.body);
            utility
        };
        let builtins = stillwind_css::parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS)
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let arbitrary_property_order =
            stillwind_css::parse_builtin_arbitrary_property_order(BUILTIN_REGISTRY_CSS);
        let mut builtin_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut wildcards = Vec::new();
        for utility in builtins {
            if utility.name.ends_with('*') {
                wildcards.push(utility);
            } else {
                builtin_exact
                    .entry(utility.name.clone())
                    .or_default()
                    .push(utility);
            }
        }
        BuiltinUtilityRegistry {
            builtin_exact,
            wildcards,
            arbitrary_property_order,
        }
    })
}

fn arbitrary_property_order(property: &str) -> usize {
    builtin_utilities()
        .arbitrary_property_order
        .iter()
        .filter_map(|entry| {
            let prefix = entry.pattern.strip_suffix('*');
            let matches = prefix.map_or(entry.pattern == property, |prefix| {
                property.starts_with(prefix)
            });
            matches.then_some((prefix.map_or(usize::MAX, str::len), entry.builtin_order))
        })
        .max_by_key(|(specificity, _)| *specificity)
        .map_or(usize::MAX, |(_, order)| order)
}

fn builtin_variants() -> &'static [CustomVariant] {
    static VARIANTS: OnceLock<Vec<CustomVariant>> = OnceLock::new();
    VARIANTS
        .get_or_init(|| stillwind_css::parse_builtin_variant_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

fn direct_builtin_variant(value: &str) -> Option<&'static CustomVariant> {
    builtin_variants()
        .iter()
        .filter_map(|variant| {
            if variant.name == value {
                return Some((usize::MAX, variant));
            }
            variant
                .name
                .strip_suffix('*')
                .filter(|prefix| !prefix.is_empty() && value.starts_with(prefix))
                .map(|prefix| (prefix.len(), variant))
        })
        .max_by_key(|(specificity, _)| *specificity)
        .map(|(_, variant)| variant)
}

fn variant_sort_namespace(key: &str) -> Option<ThemeNamespace> {
    ThemeNamespace::parse(key.strip_prefix("--")?.strip_suffix("-*")?)
}

fn bare_builtin_variant(value: &str, theme: &ThemeIndex) -> Option<&'static CustomVariant> {
    builtin_variants().iter().find(|variant| {
        let Some(sort) = variant.sort.as_ref().filter(|sort| sort.bare) else {
            return false;
        };
        let Some(namespace) = sort.key.as_deref().and_then(variant_sort_namespace) else {
            return false;
        };
        let name = match namespace {
            ThemeNamespace::Container => value.strip_prefix('@').unwrap_or(value),
            _ => value,
        };
        theme.token_in(namespace, name).is_some()
    })
}

fn builtin_variant_for_order(
    variant: &Variant,
    theme: &ThemeIndex,
) -> Option<&'static CustomVariant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    direct_builtin_variant(value).or_else(|| bare_builtin_variant(value, theme))
}

fn variant_group(variant: &CustomVariant) -> &str {
    variant
        .sort
        .as_ref()
        .and_then(|sort| sort.group.as_deref())
        .unwrap_or(&variant.name)
}

fn variant_order(variant: &Variant, theme: &ThemeIndex) -> usize {
    let Some(variant) = builtin_variant_for_order(variant, theme) else {
        return usize::MAX;
    };
    let group = variant_group(variant);
    builtin_variants()
        .iter()
        .filter(|candidate| variant_group(candidate) == group)
        .filter_map(|candidate| candidate.builtin_order)
        .min()
        .unwrap_or(usize::MAX)
}

fn width_condition(application: &VariantApplication) -> Option<&str> {
    let condition = match application {
        VariantApplication::Media(condition)
        | VariantApplication::Container(condition)
        | VariantApplication::SelectorMedia { condition, .. }
        | VariantApplication::SelectorMediaRule { condition, .. } => condition.as_str(),
        _ => return None,
    };
    let condition = condition.strip_prefix("(width ")?.strip_suffix(')')?;
    if let Some(value) = condition.strip_prefix(">= ") {
        Some(value)
    } else {
        condition.strip_prefix("< ")
    }
}

fn numeric_dimension(value: &str) -> Option<(f64, &str)> {
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    Some((number.parse().ok()?, unit))
}

fn compare_width_values(
    left: &str,
    right: &str,
    direction: BuiltinVariantSortDirection,
) -> Ordering {
    match (numeric_dimension(left), numeric_dimension(right)) {
        (Some((left_number, left_unit)), Some((right_number, right_unit))) => {
            left_unit.cmp(right_unit).then_with(|| {
                let order = left_number.total_cmp(&right_number);
                match direction {
                    BuiltinVariantSortDirection::Ascending => order,
                    BuiltinVariantSortDirection::Descending => order.reverse(),
                }
            })
        }
        _ => left.cmp(right),
    }
}

fn compare_variants_in_group(left: &Variant, right: &Variant, theme: &ThemeIndex) -> Ordering {
    if let (Some(left_inner), Some(right_inner)) =
        (compound_inner_variant(left), compound_inner_variant(right))
    {
        let comparison = compare_variant_sequences(&[left_inner], &[right_inner], theme);
        if comparison != Ordering::Equal {
            return comparison;
        }
    }
    let direction = builtin_variant_for_order(left, theme)
        .and_then(|variant| variant.sort.as_ref())
        .and_then(|sort| sort.direction);
    let left_application = resolve_variant(left, theme);
    let right_application = resolve_variant(right, theme);
    if let Some(direction) = direction
        && let (Some(left), Some(right)) = (left_application.as_ref(), right_application.as_ref())
        && let (Some(left_value), Some(right_value)) =
            (width_condition(left), width_condition(right))
    {
        return compare_width_values(left_value, right_value, direction)
            .then_with(|| left_value.cmp(right_value));
    }
    left.raw.cmp(&right.raw)
}

fn compound_inner_variant(variant: &Variant) -> Option<Variant> {
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let inner = ["not-", "group-", "peer-", "in-", "has-"]
        .into_iter()
        .find_map(|prefix| value.strip_prefix(prefix))?;
    parse_candidate(&format!("{inner}:__variant_order"))?
        .variants
        .into_iter()
        .next()
}

/// Compares variant stacks using the canonical ordering declared by the
/// built-in CSS registry. This mirrors Tailwind's order-bit comparison while
/// leaving utility/property ordering independent.
#[must_use]
pub fn compare_variant_sequences(
    left: &[Variant],
    right: &[Variant],
    theme: &ThemeIndex,
) -> Ordering {
    let mut left_orders = left
        .iter()
        .map(|variant| variant_order(variant, theme))
        .collect::<Vec<_>>();
    let mut right_orders = right
        .iter()
        .map(|variant| variant_order(variant, theme))
        .collect::<Vec<_>>();
    left_orders.sort_unstable_by(|left, right| right.cmp(left));
    right_orders.sort_unstable_by(|left, right| right.cmp(left));
    left_orders.dedup();
    right_orders.dedup();
    left_orders.cmp(&right_orders).then_with(|| {
        for order in left_orders {
            let left_variant = left
                .iter()
                .find(|variant| variant_order(variant, theme) == order);
            let right_variant = right
                .iter()
                .find(|variant| variant_order(variant, theme) == order);
            if let (Some(left_variant), Some(right_variant)) = (left_variant, right_variant) {
                let comparison = compare_variants_in_group(left_variant, right_variant, theme);
                if comparison != Ordering::Equal {
                    return comparison;
                }
            }
        }
        Ordering::Equal
    })
}

/// Compiler-owned property definitions exposed by the built-in CSS entrypoint.
#[must_use]
pub fn builtin_property_definitions() -> &'static [stillwind_css::CustomPropertyDefinition] {
    static PROPERTIES: OnceLock<Vec<stillwind_css::CustomPropertyDefinition>> = OnceLock::new();
    PROPERTIES
        .get_or_init(|| stillwind_css::parse_property_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

#[must_use]
pub fn builtin_utility_names() -> Vec<String> {
    stillwind_css::parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS)
        .into_iter()
        .map(|utility| utility.name)
        .collect()
}

#[must_use]
pub fn builtin_utility_token_prefixes() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for utility in stillwind_css::parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS) {
        let Some(prefix) = utility.name.strip_suffix('*') else {
            continue;
        };
        let prefix = prefix.trim_end_matches('-');
        let Some(start) = utility.body.find("--value(--") else {
            continue;
        };
        let rest = &utility.body[start + "--value(--".len()..];
        let Some(end) = rest.find("-*") else {
            continue;
        };
        entries.push((prefix.to_owned(), rest[..end].to_owned()));
    }
    entries
}

#[must_use]
pub fn builtin_variant_names() -> Vec<String> {
    stillwind_css::parse_builtin_variant_definitions(BUILTIN_REGISTRY_CSS)
        .into_iter()
        .map(|variant| variant.name)
        .collect()
}

#[must_use]
fn builtin_marker_utilities() -> &'static BTreeSet<String> {
    static MARKERS: OnceLock<BTreeSet<String>> = OnceLock::new();
    MARKERS.get_or_init(|| stillwind_css::parse_marker_utilities(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_declaration_formats() -> &'static std::collections::BTreeMap<String, String> {
    static FORMATS: OnceLock<std::collections::BTreeMap<String, String>> = OnceLock::new();
    FORMATS.get_or_init(|| stillwind_css::parse_declaration_formats(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_omit_token_prefixes() -> &'static Vec<(String, String)> {
    static RULES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    RULES.get_or_init(|| stillwind_css::parse_omit_token_prefixes(BUILTIN_REGISTRY_CSS))
}

#[must_use]
pub fn builtin_theme_order() -> &'static Vec<String> {
    static ORDER: OnceLock<Vec<String>> = OnceLock::new();
    ORDER.get_or_init(|| stillwind_css::parse_theme_order(BUILTIN_REGISTRY_CSS))
}

#[must_use]
fn builtin_function_formats() -> &'static BTreeSet<String> {
    static FORMATS: OnceLock<BTreeSet<String>> = OnceLock::new();
    FORMATS.get_or_init(|| stillwind_css::parse_function_formats(BUILTIN_REGISTRY_CSS))
}

const COMPAT_COLOR_LEGACY_SPACE: &str = "srgb";
const COMPAT_COLOR_MODERN_SPACE: &str = "oklab";
const COMPAT_COLOR_SUPPORT: &str = "color:color-mix(in lab, red, red)";

#[must_use]
fn builtin_relationship_patterns() -> &'static BTreeSet<String> {
    static PATTERNS: OnceLock<BTreeSet<String>> = OnceLock::new();
    PATTERNS.get_or_init(|| stillwind_css::parse_relationship_patterns(BUILTIN_REGISTRY_CSS))
}

/// Removes non-semantic whitespace from vendored declarations.
///
/// This applies only to Stillwind-maintained CSS; user `@utility` bodies keep
/// their authored serialization. Quoted values are preserved byte-for-byte.
#[allow(clippy::too_many_lines)]
fn canonical_builtin_body(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut quote = None;
    let mut escaped = false;
    let mut whitespace = false;
    let mut functions = Vec::<String>::new();
    let mut custom_property_declaration = false;

    for character in value.chars() {
        if let Some(current_quote) = quote {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }

        match character {
            '\'' | '"' => {
                if whitespace && output.chars().last().is_some_and(is_word_character) {
                    output.push(' ');
                }
                whitespace = false;
                quote = Some(character);
                output.push(character);
            }
            ';' => {
                whitespace = false;
                custom_property_declaration = false;
                functions.clear();
                output.push(character);
            }
            '(' => {
                let name = output
                    .chars()
                    .rev()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                    })
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                functions.push(name);
                output.push(character);
            }
            ')' => {
                functions.pop();
                output.push(character);
            }
            ',' => {
                output.push(character);
                let preserve_function_spacing = functions
                    .iter()
                    .any(|name| builtin_function_formats().contains(name));
                if custom_property_declaration || preserve_function_spacing {
                    output.push(' ');
                }
                whitespace = false;
            }
            ':' if output
                .rsplit_once(';')
                .map_or(output.as_str(), |(_, declaration)| declaration)
                .trim_start()
                .starts_with("--") =>
            {
                custom_property_declaration = true;
                output.push(character);
            }
            character if character.is_whitespace() => whitespace = true,
            character => {
                if whitespace && let Some(previous) = output.chars().last() {
                    let adjacent_words = is_word_character(previous)
                        && (is_word_character(character) || character == '#');
                    let function_followed_by_word = previous == ')' && is_word_character(character);
                    // Keep a separator after commas. Tailwind's vendored
                    // utility serializer retains this spelling in function
                    // arguments (notably color-mix/gradient stop lists), and
                    // LightningCSS does not normalize it away in its output.
                    let preserve_function_spacing = functions
                        .iter()
                        .any(|name| builtin_function_formats().contains(name));
                    if adjacent_words
                        || function_followed_by_word
                        || (previous == ','
                            && (custom_property_declaration || preserve_function_spacing))
                    {
                        output.push(' ');
                    }
                }
                whitespace = false;
                output.push(character);
            }
        }
    }

    // Fallback lists retain a separator before `var()` regardless of the
    // surrounding property; this is a value-level serialization rule.
    output.replace(",var(", ", var(")
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '%')
}

fn normalize_var_fallbacks(value: &str, add_missing: bool) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative) = value[cursor..].find("var(--") {
        let start = cursor + relative;
        output.push_str(&value[cursor..start]);
        let Some(open) = value[start..].find('(').map(|offset| start + offset) else {
            output.push_str(&value[start..]);
            return output;
        };
        let Some(close) = closing_parenthesis(value, open) else {
            output.push_str(&value[start..]);
            return output;
        };
        let expression = &value[open + 1..close];
        output.push_str(&value[start..=open]);
        if let Some((name, fallback)) = expression.split_once(',') {
            output.push_str(name.trim());
            output.push(',');
            if !fallback.trim().is_empty() {
                output.push_str(fallback);
            }
        } else if add_missing {
            output.push_str(expression);
            output.push(',');
        } else {
            output.push_str(expression);
        }
        output.push(')');
        cursor = close + 1;
    }
    output.push_str(&value[cursor..]);
    output
}

/// Resolves a candidate using only the Tailwind-compatible registry.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            let mut resolution = UtilityResolution::plain(format!("{property}:{value};"));
            resolution.builtin_order = Some(arbitrary_property_order(property));
            resolution
        }
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    let declaration = finalize_declaration(resolution.declaration, candidate.important, theme);
    let nested: Vec<NestedUtilityRule> = resolution
        .nested
        .into_iter()
        .map(|mut nested| {
            nested.declaration =
                finalize_declaration(nested.declaration, candidate.important, theme);
            nested
        })
        .collect();
    let additional = resolution
        .additional
        .into_iter()
        .map(|mut target| {
            target.declaration =
                finalize_declaration(target.declaration, candidate.important, theme);
            for nested in &mut target.nested {
                nested.declaration = finalize_declaration(
                    std::mem::take(&mut nested.declaration),
                    candidate.important,
                    theme,
                );
            }
            target
        })
        .collect();
    let selector = resolution.selector;
    Some(ResolvedUtility {
        candidate,
        builtin_order: resolution.builtin_order,
        declaration,
        selector,
        nested,
        additional,
    })
}

fn finalize_declaration(declaration: String, is_important: bool, theme: &ThemeIndex) -> String {
    let declaration = if is_important {
        important(&declaration)
    } else {
        declaration
    };
    let declaration = normalize_format_var_fallbacks(declaration, theme);
    let declaration = apply_declaration_formats(declaration, theme);
    let declaration = apply_zero_length_format(declaration, theme);
    stillwind_css::fold_calc_expressions(&declaration)
}

fn apply_declaration_formats(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
    } else {
        &theme.declaration_formats
    };
    for (property, format) in formats {
        if !format
            .split_whitespace()
            .any(|item| item == "slash-space-span")
        {
            continue;
        }
        let prefix = format!("{property}:");
        if let Some(value) = declaration.strip_prefix(&prefix) {
            if !value.contains("span") {
                continue;
            }
            let value = value.replace('/', " / ");
            declaration = format!("{prefix}{value}");
        }
    }
    declaration
}

fn apply_zero_length_format(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
    } else {
        &theme.declaration_formats
    };
    let ranges = declaration
        .split_terminator(';')
        .scan(0, |cursor, segment| {
            let start = *cursor;
            *cursor += segment.len() + 1;
            Some((start, start + segment.len(), segment))
        })
        .filter_map(|(start, end, segment)| {
            let property = segment.split_once(':')?.0;
            stillwind_css::declaration_format(formats, property)
                .is_some_and(|format| format.split_whitespace().any(|item| item == "zero-length"))
                .then_some((start, end))
        })
        .collect::<Vec<_>>();
    for (start, end) in ranges.into_iter().rev() {
        let segment = declaration[start..end].split_once(':').map_or_else(
            || declaration[start..end].to_owned(),
            |(property, value)| {
                format!(
                    "{property}:{}",
                    stillwind_css::format_numeric_zero(value, "px")
                )
            },
        );
        declaration.replace_range(start..end, &segment);
    }
    declaration
}

fn normalize_format_var_fallbacks(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
    } else {
        &theme.declaration_formats
    };
    for (property, format) in formats {
        let add_missing = format.split_whitespace().any(|item| item == "var-fallback");
        let normalize_empty = format
            .split_whitespace()
            .any(|item| item == "empty-var-fallback");
        if !add_missing && !normalize_empty {
            continue;
        }
        let prefix = format!("{property}:");
        let mut cursor = 0;
        while let Some(relative) = declaration[cursor..].find(&prefix) {
            let start = cursor + relative;
            let end = declaration[start..]
                .find(';')
                .map_or(declaration.len(), |offset| start + offset);
            let value = normalize_var_fallbacks(&declaration[start..end], add_missing);
            declaration.replace_range(start..end, &value);
            cursor = start + value.len();
            if end == declaration.len() {
                break;
            }
        }
    }
    declaration
}

#[must_use]
pub fn resolve_variant(variant: &Variant, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let VariantKind::Arbitrary(value) = &variant.kind {
        if let Some(condition) = value
            .strip_prefix("@supports(")
            .and_then(|value| value.strip_suffix(')'))
        {
            return Some(VariantApplication::Supports(condition.to_owned()));
        }
        return Some(VariantApplication::SelectorTemplate(value.clone()));
    }
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    if let Some(application) = resolve_registered_variant(value, theme) {
        return Some(application);
    }
    for (prefix, attribute) in [("aria-[", "aria"), ("data-[", "data")] {
        if let Some(value) = value
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(']'))
            && let Some((name, expected)) = value.split_once('=')
        {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&[{attribute}-{name}=\"{expected}\"]"
            )));
        }
    }
    theme
        .token_in(ThemeNamespace::Breakpoint, value)
        .map(|token| VariantApplication::Media(format!("(width >= {})", token.value.value)))
}

fn with_hover_media(application: VariantApplication) -> VariantApplication {
    match application {
        VariantApplication::Compound(applications) => {
            VariantApplication::Compound(applications.into_iter().map(with_hover_media).collect())
        }
        VariantApplication::SelectorTemplate(selector) if selector.contains(":hover") => {
            VariantApplication::SelectorMedia {
                selector,
                condition: "(hover: hover)".to_owned(),
            }
        }
        VariantApplication::SelectorRule {
            selector,
            declarations,
        } if selector.contains(":hover") => VariantApplication::SelectorMediaRule {
            selector,
            declarations,
            condition: "(hover: hover)".to_owned(),
        },
        application => application,
    }
}

fn with_builtin_hover_media(value: &str, application: VariantApplication) -> VariantApplication {
    if value == "hover" || value.starts_with("group-hover") || value.starts_with("peer-hover") {
        with_hover_media(application)
    } else {
        application
    }
}

/// Whether a Tailwind candidate uses a relationship primitive for which
/// projects may prefer Stillwind's typed handle API.
#[must_use]
pub fn is_tailwind_relationship_candidate(raw: &str, theme: &ThemeIndex) -> bool {
    let Some(candidate) = parse_candidate(raw) else {
        return false;
    };
    let matches_pattern = |value: &str| {
        theme
            .relationship_patterns
            .iter()
            .chain(builtin_relationship_patterns().iter())
            .any(|pattern| {
                pattern
                    .strip_suffix('*')
                    .map_or(value == pattern, |prefix| value.starts_with(prefix))
            })
    };
    matches!(
        &candidate.utility,
        Utility::Named { name, .. } if matches_pattern(name)
        || name.starts_with('@'))
        || raw.starts_with('@')
        || candidate.variants.iter().any(|variant| {
            matches!(&variant.kind, VariantKind::Named(name) if matches_pattern(name))
                || variant.raw.starts_with('@')
        })
}

/// Returns the literal marker class carried by Tailwind's declaration-only
/// `group` and `peer` utilities. These classes intentionally bypass hashing so
/// relational selectors can find them across independent `style()` calls.
#[must_use]
pub fn compatibility_marker_class(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let candidate = parse_candidate(raw)?;
    if candidate.negative || candidate.important || !candidate.variants.is_empty() {
        return None;
    }
    let Utility::Named { name, modifier } = candidate.utility else {
        return None;
    };
    if !theme.marker_utilities.contains(name.as_str())
        && !builtin_marker_utilities().contains(name.as_str())
    {
        return None;
    }
    match modifier {
        None => Some(name),
        Some(Value::Named(modifier)) => Some(format!("{name}/{modifier}")),
        Some(Value::Arbitrary(_) | Value::CssVariable(_)) => None,
    }
}

fn resolve_registered_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let Some(application) = resolve_registered_variant_from(&theme.variants, value, theme) {
        return Some(application);
    }
    theme.imports.builtins.then(|| {
        resolve_registered_variant_from(builtin_variants(), value, theme)
            .map(|application| with_builtin_hover_media(value, application))
    })?
}

fn resolve_registered_variant_from(
    variants: &[CustomVariant],
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    registered_variant_candidates(variants, value)
        .into_iter()
        .find_map(|(custom, capture)| {
            let body = substitute_variant_capture(&custom.body, capture)?;
            let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
            resolve_variant_body(&body).or_else(|| {
                substitute_variant_capture(&custom.selector, capture)
                    .map(VariantApplication::SelectorTemplate)
            })
        })
}

fn registered_variant_candidates<'a, 'b>(
    variants: &'a [CustomVariant],
    value: &'b str,
) -> Vec<(&'a CustomVariant, Option<&'b str>)> {
    let mut candidates = Vec::new();
    for variant in variants.iter().rev() {
        if variant.name == value {
            candidates.push((variant, None));
        }
    }
    for variant in variants.iter().rev() {
        let Some(prefix) = variant.name.strip_suffix('*') else {
            continue;
        };
        if prefix.is_empty() {
            continue;
        }
        let Some(capture) = value.strip_prefix(prefix) else {
            continue;
        };
        // Attribute variants with bracket payloads use the dedicated
        // `aria-[name=value]` / `data-[name=value]` grammar below; their
        // wildcard CSS bodies are for simple names only.
        if (variant.name == "aria-*" || variant.name == "data-*") && capture.starts_with('[') {
            continue;
        }
        if !capture.is_empty() {
            candidates.push((variant, Some(capture)));
        }
    }
    candidates
}

fn substitute_variant_capture(value: &str, capture: Option<&str>) -> Option<String> {
    if !value.contains("--value()")
        && !value.contains(r"\*")
        && !value.contains("{modifier}")
        && !value.contains(r"\{modifier\}")
        && !value.contains(":root")
        && !value.contains("0n")
    {
        return Some(value.to_owned());
    }
    let capture = capture?;
    let (capture, modifier) = capture
        .split_once('/')
        .map_or((capture, None), |(capture, modifier)| {
            (capture, Some(modifier))
        });
    let capture = if let Some(inner) = capture
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        // Arbitrary selector variants use `[&...]`; relationship CSS expects
        // the selector fragment without the variant's leading `&` wrapper.
        inner.strip_prefix('&').unwrap_or(inner).replace('_', " ")
    } else {
        capture.replace('_', " ")
    };
    let modifier = modifier
        .map(|modifier| modifier.replace('_', " ").replace('/', r"\/"))
        .unwrap_or_default();
    let modifier_suffix = if modifier.is_empty() {
        String::new()
    } else {
        format!(r"\/{modifier}")
    };
    let mut expanded = value
        .replace("--value()", &capture)
        .replace(r"\*", &capture)
        .replace(r"\/\{modifier\}", &modifier_suffix)
        .replace(r"\{modifier\}", &modifier)
        .replace("/{modifier}", &modifier_suffix)
        .replace("{modifier}", &modifier)
        .replace(":root", &format!(":{capture}"))
        .replace("0n", &capture);
    if capture.starts_with('[') {
        expanded = expanded.replace("):[", ")[");
    }
    Some(expanded)
}

fn resolve_variant_css_placeholders(
    body: &str,
    variant_name: &str,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = variant_name
        .strip_suffix('*')
        .and_then(|prefix| value.strip_prefix(prefix))
        .map(|capture| capture.split_once('/').map_or(capture, |(value, _)| value));
    let mut body = body.to_owned();
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_variant_value_domains(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")).then_some(body)
}

fn resolve_variant_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if let Some(namespace) = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)
        {
            return capture.and_then(|name| {
                theme
                    .token_in(namespace, name)
                    .map(|token| token.value.value.clone())
            });
        }
        resolve_value_domains(domain, capture, theme)
    })
}

fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
    }
    let blocks = split_variant_blocks(body)?;
    if blocks.len() > 1 {
        return blocks
            .into_iter()
            .map(resolve_variant_body)
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound);
    }
    if body.contains("@starting-style") {
        return Some(VariantApplication::StartingStyle);
    }
    if let Some(start) = body.find("@media ") {
        let condition_start = start + "@media ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let media_end = closing_brace(body, condition_end)?;
        let condition = body[condition_start..condition_end].trim().to_owned();
        let nested = &body[condition_end + 1..media_end];
        if let Some(rule) = selector_rule_from_variant_body(nested) {
            return if rule.declarations.is_empty() {
                Some(VariantApplication::SelectorMedia {
                    selector: rule.selector,
                    condition,
                })
            } else {
                Some(VariantApplication::SelectorMediaRule {
                    selector: rule.selector,
                    declarations: rule.declarations,
                    condition,
                })
            };
        }
        return Some(VariantApplication::Media(condition));
    }
    if let Some(start) = body.find("@container ") {
        let condition_start = start + "@container ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        return Some(VariantApplication::Container(
            body[condition_start..condition_end].trim().to_owned(),
        ));
    }
    if let Some(start) = body.find("@supports ") {
        let condition_start = start + "@supports ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let condition = body[condition_start..condition_end]
            .trim()
            .strip_prefix('(')
            .and_then(|condition| condition.strip_suffix(')'))?
            .trim()
            .to_owned();
        return Some(VariantApplication::Supports(condition));
    }
    let rule = selector_rule_from_variant_body(body)?;
    if rule.declarations.is_empty() {
        Some(VariantApplication::SelectorTemplate(rule.selector))
    } else {
        Some(VariantApplication::SelectorRule {
            selector: rule.selector,
            declarations: rule.declarations,
        })
    }
}

fn split_variant_blocks(body: &str) -> Option<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        let start = body[cursor..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(body.len(), |(offset, _)| cursor + offset);
        if start == body.len() {
            break;
        }
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        blocks.push(&body[start..=close]);
        cursor = close + 1;
    }
    (!blocks.is_empty()).then_some(blocks)
}

struct VariantSelectorRule {
    selector: String,
    declarations: Vec<VariantDeclaration>,
}

fn selector_rule_from_variant_body(body: &str) -> Option<VariantSelectorRule> {
    let open = body.find('{')?;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[..open].trim();
    if selector.is_empty() || !selector.contains('&') {
        return None;
    }

    let inner = &body[open + 1..close];
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (offset, character) in inner.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.checked_sub(1)?,
            '{' | '}' => return None,
            ';' if depth == 0 => {
                let node = inner[start..offset].trim();
                start = offset + 1;
                if node.is_empty() || node == "@slot" {
                    continue;
                }
                let (property, value) = node.split_once(':')?;
                let property = property.trim();
                let value = value.trim();
                if property.is_empty() || value.is_empty() {
                    return None;
                }
                declarations.push(VariantDeclaration {
                    property: property.to_owned(),
                    value: value.to_owned(),
                });
            }
            _ => {}
        }
    }
    if !inner[start..].trim().is_empty() {
        return None;
    }
    Some(VariantSelectorRule {
        selector: selector.to_owned(),
        declarations,
    })
}

fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    // Application CSS is the authoritative registry layer. It must be able to
    // replace every built-in name.
    if let Some(declaration) = resolve_user_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    if let Some(declaration) = resolve_builtin_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    // A failed `@utility` branch is a type error, not an invitation to use a
    // legacy property-prefix fallback. Keeping that fallback here made
    // `p-[color:red]` and `border-[time:1ms]` appear valid after the typed
    // registry correctly rejected them. Arbitrary properties (`[x:y]`) are
    // handled explicitly by the candidate grammar above; named utilities are
    // defined solely by CSS registry entries.
    None
}

/// Resolves the same `@utility` body language for user CSS and vendored
/// built-ins. `--value(...)` tries its comma-separated value domains in order.
fn resolve_user_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&theme.utilities, lookup),
        lookup,
        modifier,
        theme,
    )
}

fn resolve_builtin_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if !theme.imports.builtins {
        return None;
    }
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&builtin_utilities().wildcards, lookup).chain(
            builtin_utilities()
                .builtin_exact
                .get(lookup)
                .into_iter()
                .flat_map(|utilities| utilities.iter().rev()),
        ),
        lookup,
        modifier,
        theme,
    )
}

/// Every matching `@utility` block is a typed declaration branch. A candidate
/// such as `bg-[url(...)]` may therefore select the image branch while a
/// colour candidate selects the colour branch, without giving the root an
/// implementation-specific special case.
fn resolve_css_utility_branches<'a>(
    utilities: impl Iterator<Item = &'a CustomUtility>,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let mut branches = utilities
        .filter_map(|utility| {
            resolve_css_utility_body(utility, name, modifier, theme).map(|resolution| {
                (
                    utility_branch_specificity(utility, name),
                    utility.name == name,
                    utility_value_types(utility),
                    resolution,
                )
            })
        })
        .collect::<Vec<_>>();
    branches.sort_by_key(|(specificity, _, _, _)| std::cmp::Reverse(*specificity));
    let has_exact = branches.iter().any(|(_, exact, _, _)| *exact);
    if has_exact {
        // An exact registration defines the complete utility. Wildcard
        // overloads are alternative candidate grammar branches, not a source
        // of extra declarations for a spelling that has an exact definition.
        branches.retain(|(_, exact, _, _)| *exact);
    } else if let Some(arbitrary) = unhinted_arbitrary_value(name) {
        let available = branches
            .iter()
            .flat_map(|(_, _, domains, _)| domains.iter().copied())
            .collect::<Vec<_>>();
        let selected = infer_arbitrary_value_type(&arbitrary, &available).or_else(|| {
            branches
                .iter()
                .min_by_key(|(_, _, _, resolution)| resolution.builtin_order)
                .and_then(|(_, _, domains, _)| domains.first().copied())
        });
        if let Some(selected) = selected {
            branches.retain(|(_, _, domains, _)| domains.contains(&selected));
        }
    }
    let (_, _, _, mut resolution) = branches.first()?.clone();
    for (_, _, _, branch) in branches.into_iter().skip(1) {
        // Overloads with a shared selector form one declaration list. CSS
        // fallbacks authored within one overload intentionally retain their
        // order, while competing overloads cannot overwrite a more-specific
        // property branch.
        if branch.selector == resolution.selector {
            resolution.declaration =
                merge_branch_declarations(&resolution.declaration, &branch.declaration);
            merge_nested_rules(&mut resolution.nested, branch.nested);
            resolution.additional.extend(branch.additional);
        } else {
            resolution.additional.push(ResolvedUtilityTarget {
                declaration: branch.declaration,
                selector: branch.selector,
                nested: branch.nested,
            });
            resolution.additional.extend(branch.additional);
        }
    }
    Some(resolution)
}

fn utility_value_types(utility: &CustomUtility) -> Vec<stillwind_css::ValueType> {
    let mut types = Vec::new();
    let mut rest = utility.body.as_str();
    while let Some(start) = rest.find('[') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find(']') else {
            break;
        };
        if let Some(value_type) = stillwind_css::ValueType::from_hint(&rest[..end])
            && !types.contains(&value_type)
        {
            types.push(value_type);
        }
        rest = &rest[end + 1..];
    }
    types
}

fn unhinted_arbitrary_value(name: &str) -> Option<String> {
    if let Some(start) = name.rfind("-[")
        && let Some(value) = name[start + 2..].strip_suffix(']')
    {
        let (hint, value) = value.split_once(':').unwrap_or(("", value));
        return hint.is_empty().then(|| decode_arbitrary(value));
    }
    let start = name.rfind("-(")?;
    let variable = name[start + 2..].strip_suffix(')')?;
    variable
        .starts_with("--")
        .then(|| format!("var({variable})"))
}

fn infer_arbitrary_value_type(
    value: &str,
    available: &[stillwind_css::ValueType],
) -> Option<stillwind_css::ValueType> {
    use stillwind_css::ValueType;

    if value.contains("var(")
        || value.starts_with("--")
        || matches!(
            value.trim(),
            "initial" | "inherit" | "unset" | "revert" | "revert-layer"
        )
    {
        return None;
    }
    [
        ValueType::Image,
        ValueType::Color,
        ValueType::Percentage,
        ValueType::Position,
        ValueType::BackgroundSize,
        ValueType::Length,
        ValueType::Url,
    ]
    .into_iter()
    .filter(|value_type| available.contains(value_type))
    .find(|value_type| value_type.accepts(value))
    .or_else(|| {
        available
            .iter()
            .copied()
            .find(|value_type| value_type.accepts(value))
    })
}

fn merge_nested_rules(primary: &mut Vec<NestedUtilityRule>, secondary: Vec<NestedUtilityRule>) {
    for rule in secondary {
        let properties = declaration_property_names(&rule.declaration);
        let conflicts = primary.iter().any(|existing| {
            existing.at_rule == rule.at_rule
                && existing.condition == rule.condition
                && existing.inside == rule.inside
                && declaration_property_names(&existing.declaration)
                    .iter()
                    .any(|property| properties.contains(property))
        });
        if !conflicts {
            primary.push(rule);
        }
    }
}

fn declaration_property_names(declaration: &str) -> std::collections::HashSet<&str> {
    declaration
        .split_terminator(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(property, _)| property.trim())
        .collect()
}

/// Merges independent `@utility` overloads without erasing repeated fallback
/// declarations inside either body. For example `opacity-0` keeps its static
/// declaration instead of also gaining the generic percentage overload.
fn merge_branch_declarations(primary: &str, secondary: &str) -> String {
    let mut properties = primary
        .split_terminator(';')
        .filter_map(|declaration| {
            declaration
                .split_once(':')
                .map(|(property, _)| property.trim())
        })
        .collect::<std::collections::HashSet<_>>();
    let mut merged = primary.to_owned();
    for declaration in secondary.split_terminator(';') {
        let Some((property, _)) = declaration.split_once(':') else {
            continue;
        };
        if properties.insert(property.trim()) {
            merged.push_str(declaration);
            merged.push(';');
        }
    }
    merged
}

fn utility_branch_specificity(utility: &CustomUtility, name: &str) -> usize {
    let body = utility.body.as_str();
    let typed_arbitrary = body
        .matches('[')
        .count()
        .saturating_sub(body.matches("[*]").count());
    let theme_domain = body.matches("--value(--").count();
    let modifier = body.matches("--modifier(").count();
    // An exact registration is a semantic override of a wildcard family.
    // This keeps the canonical Tailwind spelling (`opacity-60`, `bg-none`,
    // …) intact while still letting typed wildcard branches discriminate
    // arbitrary values among themselves.
    usize::from(utility.name == name) * 32 + typed_arbitrary * 4 + theme_domain * 2 + modifier
}

fn resolve_css_utility_body(
    utility: &CustomUtility,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if utility.body.contains("@slot") {
        return None;
    }
    let (selector, body, parent_body) = split_utility_selector(&utility.body)?;
    let (body, nested) = split_nested_conditions(body)?;
    // A slash modifier is meaningful only when a branch explicitly consumes
    // it. This prevents a broad base branch (for example `bg-*`'s colour
    // declaration) from accepting `bg-red-500/not-an-opacity` merely because
    // another overload happened to define modifiers. Fractions are the one
    // exception: their slash is deliberately reassembled by `--fraction()`.
    let consumes_modifier = body.contains("--modifier")
        || body.contains("--fraction()")
        || nested
            .iter()
            .any(|(_, _, body)| body.contains("--modifier") || body.contains("--fraction()"));
    if modifier.is_some() && !consumes_modifier {
        return None;
    }
    let declarations =
        resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
    let declaration = normalize_registered_var_fallbacks(declarations.base, theme);
    let mut modifier_resolved = body.contains("--modifier(")
        || body.contains("--modifier-map(")
        || body.contains("--fraction()");
    let mut resolved_nested = Vec::new();
    for modern in declarations.compat_modern {
        resolved_nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: COMPAT_COLOR_SUPPORT.to_owned(),
            declaration: normalize_registered_var_fallbacks(modern, theme),
            inside: true,
        });
    }
    for (at_rule, condition, body) in nested {
        for (condition, body) in expand_theme_wildcard_nested(&condition, &body, theme) {
            let uses_modifier = body.contains("--modifier(")
                || body.contains("--modifier-map(")
                || body.contains("--fraction()");
            let Some(declarations) =
                resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)
            else {
                continue;
            };
            let declaration = normalize_registered_var_fallbacks(declarations.base, theme);
            if !declaration.is_empty() {
                modifier_resolved |= uses_modifier;
                resolved_nested.push(NestedUtilityRule {
                    at_rule,
                    condition,
                    declaration,
                    inside: true,
                });
                for modern in declarations.compat_modern {
                    resolved_nested.push(NestedUtilityRule {
                        at_rule: NestedUtilityAtRule::Supports,
                        condition: COMPAT_COLOR_SUPPORT.to_owned(),
                        declaration: normalize_registered_var_fallbacks(modern, theme),
                        inside: true,
                    });
                }
            }
        }
    }
    if modifier.is_some() && consumes_modifier && !modifier_resolved {
        return None;
    }
    if let Some(parent_body) = parent_body {
        let parent_declaration =
            resolve_css_declarations(parent_body, utility.name.as_str(), name, modifier, theme)?
                .base;
        let additional = (!declaration.is_empty() || !resolved_nested.is_empty()).then(|| {
            vec![ResolvedUtilityTarget {
                declaration,
                selector,
                nested: resolved_nested,
            }]
        })?;
        (!parent_declaration.is_empty() || !additional.is_empty()).then_some(UtilityResolution {
            declaration: parent_declaration,
            selector: None,
            nested: Vec::new(),
            additional,
            builtin_order: utility.builtin_order,
        })
    } else {
        (!declaration.is_empty() || !resolved_nested.is_empty()).then_some(UtilityResolution {
            declaration,
            selector,
            nested: resolved_nested,
            additional: Vec::new(),
            builtin_order: utility.builtin_order,
        })
    }
}

/// Removes a `var()` fallback when it is exactly the registered initial value
/// of that custom property. The registry metadata is the source of truth;
/// no Tailwind property family is encoded here.
fn normalize_registered_var_fallbacks(mut declaration: String, theme: &ThemeIndex) -> String {
    let mut cursor = 0;
    while let Some(relative) = declaration[cursor..].find("var(--") {
        let start = cursor + relative;
        let Some(open) = declaration[start..].find('(').map(|offset| start + offset) else {
            break;
        };
        let Some(close) = closing_parenthesis(&declaration, open) else {
            break;
        };
        let expression = &declaration[open + 1..close];
        let Some((name, fallback)) = expression.split_once(',') else {
            cursor = close + 1;
            continue;
        };
        let name = name.trim().to_owned();
        let fallback = fallback.trim();
        let property = theme
            .properties
            .iter()
            .find(|property| property.name == name)
            .or_else(|| {
                builtin_property_definitions()
                    .iter()
                    .find(|property| property.name == name)
            });
        let matches_initial = property.is_some_and(|property| {
            property.body.split(';').map(str::trim).any(|entry| {
                entry
                    .strip_prefix("initial-value:")
                    .is_some_and(|value| value.trim() == fallback)
            })
        });
        if matches_initial {
            declaration.replace_range(open + 1..close, &name);
            cursor = open + 1 + name.len();
        } else {
            cursor = close + 1;
        }
    }
    declaration
}

fn expand_theme_wildcard_nested(
    condition: &str,
    body: &str,
    theme: &ThemeIndex,
) -> Vec<(String, String)> {
    let marker_start = condition.find("--value(--");
    let Some(marker_start) = marker_start else {
        return vec![(condition.to_owned(), body.to_owned())];
    };
    let Some(marker_end) = condition[marker_start..].find(')') else {
        return vec![(condition.to_owned(), body.to_owned())];
    };
    let marker_end = marker_start + marker_end + 1;
    let marker = &condition[marker_start..marker_end];
    let Some(namespace) = marker
        .strip_prefix("--value(--")
        .and_then(|value| value.strip_suffix("-*)"))
        .and_then(ThemeNamespace::parse)
    else {
        return vec![(condition.to_owned(), body.to_owned())];
    };
    let mut tokens = theme.tokens_in(namespace);
    tokens.sort_by(|left, right| {
        css_length_sort_key(&left.value.value)
            .partial_cmp(&css_length_sort_key(&right.value.value))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tokens
        .into_iter()
        .map(|token| {
            let value = token.value.value.clone();
            (
                condition.replace(marker, &value),
                body.replace(marker, &value),
            )
        })
        .collect()
}

fn css_length_sort_key(value: &str) -> f64 {
    let value = value.trim();
    for (unit, scale) in [("rem", 16.0), ("em", 16.0), ("px", 1.0)] {
        if let Some(number) = value.strip_suffix(unit)
            && let Ok(number) = number.trim().parse::<f64>()
        {
            return number * scale;
        }
    }
    f64::INFINITY
}

/// Resolves each declaration independently. `@utility` overloads are often
/// intentionally broad: a single body can expose both colour and length
/// forms. A failed typed placeholder must remove only the incompatible
/// declaration, never the compatible siblings.
fn resolve_css_declarations(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<ResolvedCssDeclarations> {
    let declarations = split_css_declarations(body);
    let uses_placeholder = declarations.iter().any(|declaration| {
        declaration.contains("--value(")
            || declaration.contains("--value-literal(")
            || declaration.contains("--value-map(")
            || declaration.contains("--modifier(")
            || declaration.contains("--modifier-map(")
            || declaration.contains("--modifier-none()")
            || declaration.contains("--alpha(")
            || declaration.contains("--compat-color(")
            || declaration.contains("--relative-color(")
    });
    let uses_value = declarations.iter().any(|declaration| {
        declaration.contains("--value(") || declaration.contains("--value-literal(")
    });
    let requires_modifier = declarations
        .iter()
        .any(|declaration| declaration.contains("--modifier("));
    let required_declarations = declarations
        .iter()
        .filter(|declaration| declaration.contains("--relative-color("))
        .count();
    let mut resolved_placeholder = false;
    let mut resolved_value = false;
    let mut resolved_modifier = false;
    let mut resolved_required_declarations = 0;
    let mut output = ResolvedCssDeclarations::default();

    for authored_declaration in declarations {
        let contains_placeholder = authored_declaration.contains("--value(")
            || authored_declaration.contains("--value-literal(")
            || authored_declaration.contains("--value-map(")
            || authored_declaration.contains("--modifier(")
            || authored_declaration.contains("--modifier-map(")
            || authored_declaration.contains("--modifier-none()")
            || authored_declaration.contains("--alpha(")
            || authored_declaration.contains("--compat-color(")
            || authored_declaration.contains("--relative-color(");
        let contains_modifier = authored_declaration.contains("--modifier(");
        let contains_required = authored_declaration.contains("--relative-color(");
        let contains_value = authored_declaration.contains("--value(")
            || authored_declaration.contains("--value-literal(");
        if let Some(declaration) = resolve_css_placeholders(
            authored_declaration,
            utility_name,
            name,
            modifier,
            theme,
            CompatColorMode::Legacy,
        ) && !declaration.trim().is_empty()
        {
            resolved_placeholder |= contains_placeholder;
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            resolved_required_declarations += usize::from(contains_required);
            if output.base.is_empty() {
                output.base.push_str(declaration.trim_start());
            } else {
                output.base.push_str(&declaration);
            }
            if authored_declaration.contains("--compat-color(")
                && let Some(modern) = resolve_css_placeholders(
                    authored_declaration,
                    utility_name,
                    name,
                    modifier,
                    theme,
                    CompatColorMode::Modern,
                )
                && modern != declaration
            {
                output.compat_modern.push(modern.trim_start().to_owned());
            }
        }
    }

    // Static declarations in a functional body are supporting declarations,
    // not an independent branch. For example the generic `translate-*` body
    // must not emit `translate: …` when none of its typed values match.
    (!uses_placeholder || resolved_placeholder)
        .then_some(())
        // A modifier-only sibling (such as `text-*/leading`) cannot make an
        // unresolved value branch valid. This is what prevents a colour token
        // from being misinterpreted as a spacing value merely because both
        // spellings use a slash.
        .filter(|()| !uses_value || resolved_value)
        .filter(|()| !requires_modifier || resolved_modifier)
        .filter(|()| resolved_required_declarations == required_declarations)
        .map(|()| output)
}

/// Splits a declaration list without mistaking a semicolon in a string or a
/// function argument for a CSS declaration boundary.
fn split_css_declarations(body: &str) -> Vec<&str> {
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in body.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ';' if paren_depth == 0 && bracket_depth == 0 => {
                declarations.push(&body[start..=index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if !body[start..].trim().is_empty() {
        declarations.push(&body[start..]);
    }
    declarations
}

#[allow(clippy::too_many_lines)]
fn resolve_css_placeholders(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    compat_color_mode: CompatColorMode,
) -> Option<String> {
    let capture = utility_name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    resolve_literal_value_placeholders(&mut body, capture, theme, compat_color_mode)?;
    resolve_text_line_height_placeholders(&mut body, capture, theme)?;
    resolve_value_map_placeholders(&mut body, capture)?;
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let domains = &body[open + 1..end];
        // The candidate grammar deliberately represents the slash in `w-1/2`
        // as a modifier. Value domains that explicitly opt into fractions can
        // reassemble that spelling without changing slash semantics for color
        // opacity, fonts, or arbitrary modifiers.
        let fraction_capture = if domains.contains("--fraction()") {
            capture.zip(modifier).and_then(|(capture, modifier)| {
                let modifier = match modifier {
                    Value::Named(value) => value,
                    Value::Arbitrary(_) | Value::CssVariable(_) => return None,
                };
                (capture.parse::<u16>().is_ok() && modifier.parse::<u16>().is_ok())
                    .then(|| format!("{capture}/{modifier}"))
            })
        } else {
            None
        };
        let capture = fraction_capture.as_deref().or(capture);
        let replacement = resolve_value_domains(domains, capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier(") {
        let open = start + "--modifier".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let domains = &body[open + 1..end];
        let replacement = if domains.trim().is_empty() {
            match modifier {
                Value::Named(value) | Value::Arbitrary(value) => value.clone(),
                Value::CssVariable(value) => format!("var({value})"),
            }
        } else {
            let capture = match modifier {
                Value::Named(value) => value.clone(),
                // `Value::Arbitrary` is decoded by the grammar. Re-wrap it
                // so the same typed-domain resolver used by `--value(...)`
                // can check an optional arbitrary-value hint.
                Value::Arbitrary(value) => format!("[{value}]"),
                Value::CssVariable(value) => format!("({value})"),
            };
            resolve_value_domains(domains, Some(&capture), theme)?
        };
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-map(") {
        let open = start + "--modifier-map".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let modifier = match modifier {
            Value::Named(value) | Value::Arbitrary(value) => value,
            Value::CssVariable(_) => return None,
        };
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == modifier).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-none()") {
        if modifier.is_some() {
            return None;
        }
        let declaration_start = body[..start].rfind(';').map_or(0, |end| end + 1);
        let declaration_end = body[start..].find(';').map_or_else(
            || start + "--modifier-none()".len(),
            |offset| start + offset + 1,
        );
        body.replace_range(declaration_start..declaration_end, "");
    }
    while let Some(start) = body.find("--alpha(") {
        let open = start + "--alpha".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = alpha(&body[open + 1..end])?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--compat-color(") {
        let open = start + "--compat-color".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_compat_color(&body[open + 1..end], compat_color_mode)?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--relative-color(") {
        let open = start + "--relative-color".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_shadow_color(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")
        && !body.contains("--value-literal(")
        && !body.contains("--text-line-height()")
        && !body.contains("--value-map(")
        && !body.contains("--modifier(")
        && !body.contains("--modifier-map(")
        && !body.contains("--modifier-none()")
        && !body.contains("--alpha(")
        && !body.contains("--compat-color(")
        && !body.contains("--relative-color("))
    .then_some(body)
}

/// Produces the legacy Color 4 fallback for an authored color and alpha.
/// The caller also emits the modern upgrade guarded by the capability query
/// intrinsic to `--compat-color()`.
fn resolve_compat_color(arguments: &str, mode: CompatColorMode) -> Option<String> {
    let arguments = split_domains(arguments);
    let (color, alpha, modern_alpha) = match arguments.as_slice() {
        [color, alpha] => (*color, *alpha, None),
        [color, alpha, modern_alpha] => (*color, *alpha, Some(modern_alpha.trim())),
        _ => return None,
    };
    let color = color.trim();
    let alpha = alpha.trim();
    if color.is_empty() || alpha.is_empty() {
        return None;
    }
    let color_kind = CompatColorKind::from_css(color);
    if color.eq_ignore_ascii_case("inherit") && alpha == "100%" {
        return Some(color.to_owned());
    }
    let color_space = match (mode, color_kind, modern_alpha.is_none()) {
        // Tailwind does not synthesize an sRGB compatibility branch for CSS
        // wide/transparent keywords. Keeping both passes identical makes the
        // declaration unconditional instead of wrapping the modern form in
        // an unnecessary capability query.
        (_, CompatColorKind::ModernOnlyKeyword, true) | (CompatColorMode::Modern, _, _) => {
            COMPAT_COLOR_MODERN_SPACE
        }
        (CompatColorMode::Legacy, _, _) => COMPAT_COLOR_LEGACY_SPACE,
    };
    let mixed = if alpha == "100%"
        || (mode == CompatColorMode::Legacy && color_kind == CompatColorKind::CurrentColor)
    {
        color.to_owned()
    } else {
        format!("color-mix(in {color_space}, {color} {alpha}, transparent)")
    };
    if mode == CompatColorMode::Modern
        && let Some(modern_alpha) = modern_alpha
    {
        return Some(format!(
            "color-mix(in {COMPAT_COLOR_MODERN_SPACE}, {mixed} {modern_alpha}, transparent)"
        ));
    }
    Some(mixed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompatColorKind {
    CurrentColor,
    ModernOnlyKeyword,
    General,
}

impl CompatColorKind {
    fn from_css(color: &str) -> Self {
        if color.eq_ignore_ascii_case("currentcolor") {
            Self::CurrentColor
        } else if color.eq_ignore_ascii_case("inherit") || color.eq_ignore_ascii_case("transparent")
        {
            Self::ModernOnlyKeyword
        } else {
            Self::General
        }
    }
}

/// CSS-first extension for theme shadow opacity modifiers. `--value()` emits
/// the normal `var(--shadow-*)` reference; this intentionally reads the
/// authored token to construct a CSS Color 5 relative colour function.
fn resolve_shadow_color(
    arguments: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    let arguments = split_domains(arguments);
    let (domain, color_property, alpha) = match arguments.as_slice() {
        [domain, color_property] => (*domain, *color_property, None),
        [domain, color_property, alpha] => (*domain, *color_property, Some(alpha.trim())),
        _ => return None,
    };
    let namespace = domain
        .trim()
        .strip_prefix("--")
        .and_then(|domain| domain.strip_suffix("-*"))
        .and_then(ThemeNamespace::parse)?;
    let value = theme_value_literal(namespace, capture?, theme)?;
    let color_property = color_property.trim();
    color_property.strip_prefix("--").filter(|name| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
    })?;
    match alpha {
        Some(alpha) => shadow_with_color_fallback(&value, color_property, alpha),
        None => shadow_with_color_override(&value, color_property, None),
    }
}

fn resolve_literal_value_placeholders(
    body: &mut String,
    capture: Option<&str>,
    theme: &ThemeIndex,
    compat_color_mode: CompatColorMode,
) -> Option<()> {
    while let Some(start) = body.find("--value-literal(") {
        let open = start + "--value-literal".len();
        let end = closing_parenthesis(body, open)?;
        let replacement = match compat_color_mode {
            CompatColorMode::Legacy => {
                resolve_literal_value_domains(&body[open + 1..end], capture, theme)?
            }
            CompatColorMode::Modern => resolve_value_domains(&body[open + 1..end], capture, theme)?,
        };
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

fn resolve_text_line_height_placeholders(
    body: &mut String,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<()> {
    while let Some(start) = body.find("--text-line-height()") {
        let capture = capture?;
        let token = theme.token_in(ThemeNamespace::Text, &format!("{capture}--line-height"))?;
        body.replace_range(
            start..start + "--text-line-height()".len(),
            &format!("var({})", token.name.value),
        );
    }
    Some(())
}

fn resolve_value_map_placeholders(body: &mut String, capture: Option<&str>) -> Option<()> {
    while let Some(start) = body.find("--value-map(") {
        let open = start + "--value-map".len();
        let end = closing_parenthesis(body, open)?;
        let capture = capture?;
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == capture).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

fn split_nested_conditions(body: &str) -> Option<(String, Vec<ParsedNestedUtilityRule>)> {
    let mut direct = String::with_capacity(body.len());
    let mut nested = Vec::new();
    let mut cursor = 0;
    while let Some((start, at_rule, keyword)) = next_nested_condition(body, cursor) {
        direct.push_str(&body[cursor..start]);
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        let condition = body[start + keyword.len()..open].trim();
        let condition = match at_rule {
            NestedUtilityAtRule::Media => condition.to_owned(),
            NestedUtilityAtRule::Supports => condition
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))?
                .trim()
                .to_owned(),
        };
        nested.push((at_rule, condition, body[open + 1..close].trim().to_owned()));
        cursor = close + 1;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_owned(), nested))
}

fn next_nested_condition(
    body: &str,
    cursor: usize,
) -> Option<(usize, NestedUtilityAtRule, &'static str)> {
    let supports = body[cursor..]
        .find("@supports")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Supports, "@supports"));
    let media = body[cursor..]
        .find("@media")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Media, "@media"));
    match (supports, media) {
        (Some(supports), Some(media)) => Some(if supports.0 < media.0 {
            supports
        } else {
            media
        }),
        (Some(condition), None) | (None, Some(condition)) => Some(condition),
        (None, None) => None,
    }
}

fn split_utility_selector(body: &str) -> Option<(Option<String>, &str, Option<&str>)> {
    if !body.contains('&') {
        return Some((None, body, None));
    }

    let mut selector_start = 0;
    let mut open = None;
    let mut parenthesis_depth = 0_u32;
    for (index, character) in body.char_indices() {
        match character {
            '(' | '[' => parenthesis_depth += 1,
            ')' | ']' => parenthesis_depth = parenthesis_depth.checked_sub(1)?,
            ';' if parenthesis_depth == 0 => selector_start = index + 1,
            '{' if parenthesis_depth == 0 => {
                let prelude = body[selector_start..index].trim();
                if prelude.contains('&') {
                    open = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let open = open?;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[selector_start..open].trim();
    (!selector.is_empty()).then(|| {
        (
            Some(selector.to_owned()),
            body[open + 1..close].trim(),
            (!body[..selector_start].trim().is_empty()).then_some(body[..selector_start].trim()),
        )
    })
}

fn closing_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_utilities<'a>(
    utilities: &'a [CustomUtility],
    name: &'a str,
) -> impl Iterator<Item = &'a CustomUtility> {
    utilities.iter().rev().filter(move |utility| {
        utility.name == name
            || utility
                .name
                .strip_suffix('*')
                .is_some_and(|prefix| name.starts_with(prefix))
    })
}

fn closing_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if domain == "integer" {
            return capture
                .filter(|value| value.parse::<i64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "number" {
            return capture
                .filter(|value| value.parse::<f64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "percentage" {
            return capture
                .and_then(|value| value.strip_suffix('%'))
                .filter(|value| value.parse::<f64>().is_ok())
                .map(|value| format!("{value}%"));
        }
        if domain == "--spacing()" {
            return capture
                .and_then(|capture| theme_value(ThemeNamespace::Spacing, capture, theme));
        }
        if domain == "--fraction()" {
            return capture.and_then(fraction_value);
        }
        if domain == "--width()" {
            return capture.and_then(|capture| axis_size_value(capture, "vw"));
        }
        if domain == "--height()" {
            return capture.and_then(|capture| axis_size_value(capture, "vh"));
        }
        if let Some(namespace) = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)
        {
            return capture.and_then(|capture| theme_value(namespace, capture, theme));
        }
        if !domain.starts_with('[') && !domain.starts_with("--") {
            return capture.filter(|value| *value == domain).map(|value| {
                if value == "current" {
                    "currentcolor".to_owned()
                } else {
                    value.to_owned()
                }
            });
        }
        let expected = ValueDomain::from_hint(domain.strip_prefix('[')?.strip_suffix(']')?)?;
        if let Some(variable) = parse_css_variable(capture?) {
            let hint_is_compatible = variable.hint.is_none_or(|hint| expected.accepts_hint(hint));
            return hint_is_compatible.then(|| format!("var({})", variable.name));
        }
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (hint, value) = arbitrary.split_once(':').unwrap_or(("", arbitrary));
        let value = decode_arbitrary(value);
        let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
        (hint_is_compatible && expected.accepts(&value)).then_some(value)
    })
}

/// Resolves a typed domain to its authored theme token value rather than a
/// `var(--token)` reference. Tailwind uses this form for broad-gamut-safe sRGB
/// fallbacks, while retaining the variable reference in the upgraded branch.
fn resolve_literal_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        let namespace = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse);
        if let Some(namespace) = namespace {
            return capture.and_then(|capture| theme_value_literal(namespace, capture, theme));
        }
        resolve_value_domains(domain, capture, theme)
    })
}

fn split_domains(value: &str) -> Vec<&str> {
    let mut domains = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (index, character) in value.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                domains.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    domains.push(&value[start..]);
    domains
}

fn theme_value(namespace: ThemeNamespace, value: &str, theme: &ThemeIndex) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = typed_css_variable(namespace, value) => format!("var({value})"),
        // Tailwind's candidate grammar accepts ASCII identifier tokens only;
        // Unicode theme names remain valid CSS but are not utility candidates.
        _ if !value.is_ascii() => return None,
        _ if let Some(token) = theme.token_in(namespace, value) => {
            if matches!(token.mode, stillwind_css::ThemeTokenMode::Inline) {
                token.value.value.clone()
            } else {
                let prefix = theme
                    .imports
                    .prefix
                    .as_deref()
                    .map(|prefix| format!("{}-", prefix.trim_end_matches('-')))
                    .unwrap_or_default();
                format!("var(--{prefix}{}-{value})", namespace.css_name())
            }
        }
        _ if namespace == ThemeNamespace::Spacing
            && theme.scale(ThemeNamespace::Spacing).is_some() =>
        {
            if value == "px" {
                "1px".to_owned()
            } else if value.parse::<f64>().is_ok() {
                if value.parse::<f64>().is_ok_and(|number| number == 0.0) {
                    return Some("0".to_owned());
                }
                let scale_name = theme.scale(ThemeNamespace::Spacing)?.name.value.as_str();
                return Some(format!("calc(var({scale_name}) * {value})"));
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(value)
}

fn theme_value_literal(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = typed_css_variable(namespace, value) => format!("var({value})"),
        _ if let Some(token) = theme.token_in(namespace, value) => token.value.value.clone(),
        _ => return theme_value(namespace, value, theme),
    };
    Some(value)
}

fn typed_theme_arbitrary(namespace: ThemeNamespace, value: &str) -> Option<String> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = decode_arbitrary(value);
    let types = theme_arbitrary_types(namespace);
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint)));
    (hint_is_compatible && (types.is_empty() || types.iter().any(|kind| kind.accepts(&value))))
        .then_some(value)
}

fn theme_arbitrary_types(namespace: ThemeNamespace) -> &'static [ValueType] {
    match namespace {
        ThemeNamespace::Color => &[ValueType::Color],
        ThemeNamespace::Spacing
        | ThemeNamespace::Radius
        | ThemeNamespace::Blur
        | ThemeNamespace::Breakpoint
        | ThemeNamespace::Container
        | ThemeNamespace::Perspective
        | ThemeNamespace::Tracking => &[ValueType::Length],
        ThemeNamespace::AspectRatio => &[ValueType::Ratio],
        ThemeNamespace::Animate => &[ValueType::Animation],
        ThemeNamespace::DropShadow
        | ThemeNamespace::InsetShadow
        | ThemeNamespace::Shadow
        | ThemeNamespace::TextShadow => &[ValueType::Shadow],
        ThemeNamespace::Ease => &[ValueType::Easing],
        ThemeNamespace::Font => &[ValueType::FontFamily],
        ThemeNamespace::FontWeight => &[ValueType::FontWeight],
        ThemeNamespace::Leading => &[ValueType::Number, ValueType::Length],
        ThemeNamespace::FontStretch => &[ValueType::Percentage],
        ThemeNamespace::Text => &[ValueType::FontSize],
        ThemeNamespace::Duration => &[ValueType::Time],
        ThemeNamespace::ZIndex => &[ValueType::ZIndex],
    }
}

fn fraction_value(value: &str) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| format!("calc({numerator} / {denominator} * 100%)"))
}

fn axis_size_value(value: &str, viewport_axis: &str) -> Option<String> {
    let value = match value {
        "auto" => "auto".to_owned(),
        "full" => "100%".to_owned(),
        "screen" => format!("100{viewport_axis}"),
        "svw" => "100svw".to_owned(),
        "lvw" => "100lvw".to_owned(),
        "dvw" => "100dvw".to_owned(),
        "svh" => "100svh".to_owned(),
        "lvh" => "100lvh".to_owned(),
        "dvh" => "100dvh".to_owned(),
        "min" => "min-content".to_owned(),
        "max" => "max-content".to_owned(),
        "fit" => "fit-content".to_owned(),
        "lh" => "1lh".to_owned(),
        _ => return None,
    };
    Some(value)
}

fn typed_css_variable(namespace: ThemeNamespace, value: &str) -> Option<&str> {
    let variable = parse_css_variable(value)?;
    let types = theme_arbitrary_types(namespace);
    (variable.hint.is_none()
        || types.is_empty()
        || variable
            .hint
            .and_then(ValueType::from_hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint))))
    .then_some(variable.name)
}

struct CssVariable<'a> {
    hint: Option<&'a str>,
    name: &'a str,
}

fn parse_css_variable(value: &str) -> Option<CssVariable<'_>> {
    let value = value.strip_prefix('(')?.strip_suffix(')')?;
    let (hint, name) = value
        .split_once(':')
        .map_or((None, value), |(hint, name)| (Some(hint), name));
    name.starts_with("--").then_some(CssVariable { hint, name })
}

fn decode_arbitrary(value: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '_' {
            result.push(' ');
        } else {
            result.push(character);
        }
    }
    result
}

fn alpha(value: &str) -> Option<String> {
    if value.starts_with("var(") {
        return Some(value.to_owned());
    }
    let number = value.parse::<f64>().ok()?;
    Some(if number <= 1.0 {
        format!("{}%", number * 100.0)
    } else {
        format!("{number}%")
    })
}

fn important(declaration: &str) -> String {
    declaration
        .split_terminator(';')
        .filter(|declaration| !declaration.is_empty())
        .fold(String::new(), |mut result, declaration| {
            write!(result, "{declaration}!important;").expect("writing to a String cannot fail");
            result
        })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use stillwind_css::parse_tailwind_theme;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    const SIMPLE_CSSWG_TERMS: &[&str] = &[
        "angle",
        "angle-percentage",
        "color",
        "easing-function",
        "image",
        "integer",
        "length",
        "length-percentage",
        "line-width",
        "number",
        "number-percentage",
        "percentage",
        "position",
        "ratio",
        "time",
        "url",
    ];

    #[test]
    fn derives_arbitrary_property_order_from_builtin_css() {
        let transform = resolve_utility("transform-gpu", &ThemeIndex::default())
            .expect("transform utility")
            .builtin_order
            .expect("built-in order");
        let margin = resolve_utility("[margin:0]", &ThemeIndex::default())
            .expect("arbitrary margin")
            .builtin_order
            .expect("built-in order");
        let margin_left = resolve_utility("[margin-left:1px]", &ThemeIndex::default())
            .expect("arbitrary margin-left")
            .builtin_order
            .expect("derived order");
        let all = resolve_utility("[all:unset]", &ThemeIndex::default())
            .expect("arbitrary all")
            .builtin_order
            .expect("derived order");
        let background = resolve_utility("[background:red]", &ThemeIndex::default())
            .expect("arbitrary background")
            .builtin_order
            .expect("derived order");
        let translate_variable = resolve_utility("[--tw-translate-x:1px]", &ThemeIndex::default())
            .expect("positioned Tailwind variable")
            .builtin_order
            .expect("declared order");
        let unknown_tailwind_variable = resolve_utility("[--tw-unknown:1]", &ThemeIndex::default())
            .expect("fallback Tailwind variable")
            .builtin_order
            .expect("declared order");
        assert!(margin < margin_left && margin_left < transform);
        assert!(translate_variable < transform);
        assert!(transform < all, "transform={transform}, all={all}");
        assert!(transform < unknown_tailwind_variable);
        assert!(unknown_tailwind_variable < background);
        assert_eq!(background, all);
    }

    fn property_references(syntax: &str) -> Vec<&str> {
        let mut references = Vec::new();
        let mut remainder = syntax;
        while let Some(start) = remainder.find("<'") {
            let after_start = &remainder[start + 2..];
            let Some(end) = after_start.find("'>") else {
                break;
            };
            references.push(&after_start[..end]);
            remainder = &after_start[end + 2..];
        }
        references
    }

    fn expanded_csswg_syntax(property: &str, visited: &mut BTreeSet<String>) -> Option<String> {
        if !visited.insert(property.to_owned()) {
            return Some(String::new());
        }
        let syntax = stillwind_css::csswg_formal_syntax(property)?;
        let mut expanded = syntax.to_owned();
        for referenced_property in property_references(syntax) {
            expanded.push(' ');
            expanded.push_str(&expanded_csswg_syntax(referenced_property, visited)?);
        }
        Some(expanded)
    }

    fn formal_terms(syntax: &str) -> BTreeSet<&str> {
        let mut terms = BTreeSet::new();
        let mut remainder = syntax;
        while let Some(start) = remainder.find('<') {
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find('>') else {
                break;
            };
            if after_start.starts_with('\'') {
                remainder = &after_start[end + 1..];
                continue;
            }
            let term = after_start[..end]
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
                .next()
                .unwrap_or_default();
            if !term.is_empty() {
                terms.insert(term);
            }
            remainder = &after_start[end + 1..];
        }
        terms
    }

    fn direct_value_domains(value: &str) -> Vec<ValueType> {
        if !value.trim_start().starts_with("--value(") {
            return Vec::new();
        }
        let mut domains = Vec::new();
        let mut remainder = value;
        while let Some(start) = remainder.find('[') {
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find(']') else {
                break;
            };
            if let Some(domain) = ValueType::from_hint(&after_start[..end]) {
                domains.push(domain);
            }
            remainder = &after_start[end + 1..];
        }
        domains
    }

    fn has_complex_csswg_grammar(syntax: &str, terms: &BTreeSet<&str>) -> bool {
        syntax.contains("&&")
            || syntax.contains("||")
            || terms.iter().any(|term| !SIMPLE_CSSWG_TERMS.contains(term))
    }

    fn theme() -> ThemeIndex {
        parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@stillwind-marker group; @stillwind-marker peer; @theme { --color-brand: red; --breakpoint-sm: 40rem; --breakpoint-md: 48rem; --breakpoint-lg: 64rem; --breakpoint-xl: 80rem; --breakpoint-2xl: 96rem; }",
        ))
        .unwrap()
    }

    #[test]
    fn builtins_declare_no_untyped_arbitrary_value_domains() {
        assert!(
            !BUILTIN_REGISTRY_CSS.contains("--value([*])"),
            "built-in utilities must use a typed CSS value domain"
        );
        assert!(
            !BUILTIN_REGISTRY_CSS.contains("[property:"),
            "property names are not CSS value-domain syntax"
        );
    }

    #[test]
    fn simple_utility_domains_follow_csswg_formal_syntax() {
        let mut checked = BTreeSet::new();
        let mut failures = Vec::new();
        for utility in stillwind_css::parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS) {
            for declaration in utility.body.split(';') {
                let Some((property, value)) = declaration.split_once(':') else {
                    continue;
                };
                let property = property.trim();
                if property.starts_with('-') || property.contains('{') {
                    continue;
                }
                for domain in direct_value_domains(value) {
                    let Some(expected_terms) = domain.csswg_formal_terms() else {
                        continue;
                    };
                    let mut visited = BTreeSet::new();
                    let Some(syntax) = expanded_csswg_syntax(property, &mut visited) else {
                        failures.push(format!(
                            "{property} in @utility {} has no Webref formal syntax",
                            utility.name
                        ));
                        continue;
                    };
                    let terms = formal_terms(&syntax);
                    // A named production such as `<grid-line>` needs
                    // property-specific interpretation. Those compound
                    // grammars are detected from their syntax rather than a
                    // per-property exception list.
                    if has_complex_csswg_grammar(&syntax, &terms) {
                        continue;
                    }
                    if expected_terms
                        .iter()
                        .any(|expected| terms.contains(expected))
                    {
                        checked.insert(format!("{property} [{domain:?}]"));
                    } else {
                        failures.push(format!(
                            "{property} in @utility {} uses [{domain:?}], but CSSWG declares {syntax:?}",
                            utility.name
                        ));
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "built-in simple value domains diverge from CSSWG formal syntax:\n{}",
            failures.join("\n")
        );
        assert!(
            checked.len() >= 20,
            "the registry-wide audit unexpectedly covered only {} declarations: {checked:?}",
            checked.len()
        );
    }

    #[test]
    fn every_tailwind_theme_namespace_has_a_typed_arbitrary_value_domain() {
        for namespace in [
            ThemeNamespace::Animate,
            ThemeNamespace::AspectRatio,
            ThemeNamespace::Blur,
            ThemeNamespace::Breakpoint,
            ThemeNamespace::Color,
            ThemeNamespace::Container,
            ThemeNamespace::DropShadow,
            ThemeNamespace::Ease,
            ThemeNamespace::Font,
            ThemeNamespace::FontStretch,
            ThemeNamespace::FontWeight,
            ThemeNamespace::InsetShadow,
            ThemeNamespace::Leading,
            ThemeNamespace::Perspective,
            ThemeNamespace::Radius,
            ThemeNamespace::Shadow,
            ThemeNamespace::Spacing,
            ThemeNamespace::Text,
            ThemeNamespace::TextShadow,
            ThemeNamespace::Tracking,
            ThemeNamespace::ZIndex,
            ThemeNamespace::Duration,
        ] {
            assert!(
                !theme_arbitrary_types(namespace).is_empty(),
                "{namespace:?} must not fall back to an unchecked arbitrary value"
            );
        }
    }

    fn catalog_section<'a>(markdown: &'a str, heading: &str) -> Vec<&'a str> {
        let start = markdown
            .find(heading)
            .and_then(|start| {
                markdown[start..]
                    .find("```text\n")
                    .map(|offset| start + offset + 8)
            })
            .expect("catalog section must contain candidates");
        markdown[start..]
            .split_once("\n```")
            .expect("catalog section must close")
            .0
            .split_whitespace()
            .collect()
    }

    #[test]
    fn resolves_every_upstream_static_utility_from_the_css_registry() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let candidates = catalog_section(catalog, "## Static utilities");
        let theme = theme();

        let unresolved = candidates
            .into_iter()
            .filter(|candidate| resolve_utility(candidate, &theme).is_none())
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved static utilities: {unresolved:?}"
        );
    }

    #[test]
    fn resolves_every_upstream_functional_root_with_an_arbitrary_value() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let roots = catalog_section(catalog, "## Functional utility roots");
        let theme = theme();

        let unresolved = roots
            .into_iter()
            .map(|root| format!("{root}-[initial]"))
            .filter(|candidate| resolve_utility(candidate, &theme).is_none())
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved functional utility candidates: {unresolved:?}"
        );
    }

    #[test]
    fn resolves_every_upstream_static_variant_from_the_css_registry() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let variants = catalog_section(catalog, "## Static variants");
        let theme = theme();

        let unresolved = variants
            .into_iter()
            .filter(|name| {
                let candidate =
                    parse_candidate(&format!("{name}:flex")).expect("catalog variants must parse");
                resolve_variant(&candidate.variants[0], &theme).is_none()
            })
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved static variants: {unresolved:?}"
        );
    }

    #[test]
    fn preserves_a_nested_selector_from_a_css_utility() {
        let resolved = resolve_utility("divide-x-[2px]", &theme()).unwrap();

        assert_eq!(
            resolved.declaration,
            "border-inline-end-style:var(--tw-border-style);border-inline-end-width:2px;"
        );
        assert_eq!(
            resolved.selector.as_deref(),
            Some(":where(&>:not(:last-child))")
        );
    }

    #[test]
    fn resolves_fixture_style_static_theme_and_arbitrary_candidates() {
        let theme = theme();
        assert_eq!(
            resolve_utility("!flex", &theme).unwrap().declaration,
            "display:flex!important;"
        );
        assert_eq!(
            resolve_utility("-mt-4", &theme).unwrap().declaration,
            "margin-top:calc(var(--spacing) * -4);"
        );
        let background = resolve_utility("bg-brand/50", &theme).unwrap();
        assert_eq!(
            background.declaration,
            "background-color:color-mix(in srgb, red 50%, transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule
                    .declaration
                    .contains("background-color:color-mix(in oklab")
                && rule.declaration.contains("color-mix(in oklab")
        }));
        assert_eq!(
            resolve_utility("w-(--sidebar)", &theme)
                .unwrap()
                .declaration,
            "width:var(--sidebar);"
        );
        assert_eq!(
            resolve_utility("[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color:rebeccapurple;"
        );
    }

    #[test]
    fn respects_tailwind_theme_namespace_resets_when_resolving_utilities() {
        let default_theme = theme();
        assert_eq!(
            resolve_utility("bg-red-500", &default_theme)
                .expect("the vendored Tailwind theme provides red-500")
                .declaration,
            "background-color:var(--color-red-500);"
        );

        let reset_theme = parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-*: initial; --color-brand: rebeccapurple; }",
        ))
        .unwrap();
        assert!(resolve_utility("bg-red-500", &reset_theme).is_none());
        assert_eq!(
            resolve_utility("bg-brand", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:var(--color-brand);"
        );
        assert_eq!(
            resolve_utility("bg-current", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:currentcolor;"
        );
        assert_eq!(
            resolve_utility("bg-transparent", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:transparent;"
        );
    }

    #[test]
    fn resolves_native_mask_conic_angles() {
        let resolved = resolve_utility("mask-conic-45", &theme()).expect("mask conic angle");
        assert!(
            resolved
                .declaration
                .contains("--tw-mask-conic-position:45deg;"),
            "{resolved:?}"
        );
        let negative =
            resolve_utility("-mask-conic-45", &theme()).expect("negative mask conic angle");
        assert!(
            negative
                .declaration
                .contains("--tw-mask-conic-position:-45deg;"),
            "{negative:?}"
        );
    }

    #[test]
    fn resolves_gradient_interpolation_modifiers_in_the_css_registry() {
        let default = resolve_utility("bg-conic-45", &theme()).expect("default conic");
        assert!(
            default
                .declaration
                .contains("--tw-gradient-position:from 45deg in oklab;"),
            "{default:?}"
        );
        let interpolated =
            resolve_utility("bg-conic-45/increasing", &theme()).expect("interpolated conic");
        assert!(
            interpolated
                .declaration
                .contains("from 45deg in oklch increasing hue")
        );
    }

    #[test]
    fn resolves_css_defined_color_and_gradient_utilities() {
        let theme = theme();
        assert_eq!(
            resolve_utility("fill-brand/50", &theme)
                .unwrap()
                .declaration,
            "fill:color-mix(in srgb, red 50%, transparent);"
        );
        assert_eq!(
            resolve_utility("from-brand", &theme).unwrap().declaration,
            "--tw-gradient-from:var(--color-brand);--tw-gradient-stops:var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));"
        );
        assert_eq!(
            resolve_utility("bg-linear-to-r", &theme)
                .unwrap()
                .declaration,
            "--tw-gradient-position:to right;background-image:linear-gradient(var(--tw-gradient-stops));"
        );
    }

    #[test]
    fn resolves_ambiguous_backgrounds_through_typed_utility_branches() {
        let theme = theme();
        assert_eq!(
            resolve_utility("bg-[10px]", &theme).unwrap().declaration,
            "background-position:10px;"
        );
        assert_eq!(
            resolve_utility("bg-[20%_30%]", &theme).unwrap().declaration,
            "background-position:20% 30%;"
        );
        assert_eq!(
            resolve_utility("bg-[10px_auto]", &theme)
                .unwrap()
                .declaration,
            "background-size:10px auto;"
        );
        assert_eq!(
            resolve_utility("bg-[var(--surface)]", &theme)
                .unwrap()
                .declaration,
            "background-color:var(--surface);"
        );
        assert_eq!(
            resolve_utility("bg-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "background-color:rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("bg-[image:linear-gradient(red,blue)]", &theme)
                .unwrap()
                .declaration,
            "background-image:linear-gradient(red,blue);"
        );
        assert_eq!(
            resolve_utility("bg-[position:center_20%]", &theme)
                .unwrap()
                .declaration,
            "background-position:center 20%;"
        );
        assert_eq!(
            resolve_utility("bg-[bg-size:24rem_auto]", &theme)
                .unwrap()
                .declaration,
            "background-size:24rem auto;"
        );
    }

    #[test]
    fn prunes_only_incompatible_declarations_from_a_typed_utility_body() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility token-* {
  color: --value([color]);
  width: --value([length]);
}",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("token-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color: rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("token-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "width: 12px;"
        );
    }

    #[test]
    fn rejects_untyped_or_invalid_modifiers_instead_of_ignoring_them() {
        let theme = theme();

        assert!(resolve_utility("bg-red-500/not-an-opacity", &theme).is_none());
        assert!(resolve_utility("bg-[image:url(/hero.svg)]/50", &theme).is_none());
        let background = resolve_utility("bg-red-500/50", &theme).unwrap();
        assert_eq!(
            background.declaration,
            "background-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule
                    .declaration
                    .contains("background-color:color-mix(in oklab")
                && rule.declaration.contains("color-mix(in oklab")
        }));
    }

    #[test]
    fn resolves_typed_modifier_domains_without_accepting_other_types() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility prose-* {
  font-size: --value(--text-*);
  line-height: --modifier(--leading-*, --spacing(), [length]);
}",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("prose-sm/6", &theme).unwrap().declaration,
            "font-size: var(--text-sm);\n  line-height: calc(var(--spacing) * 6);"
        );
        assert!(resolve_utility("prose-sm/[color:rebeccapurple]", &theme).is_none());
    }

    #[test]
    fn preserves_supports_and_media_conditions_from_css_utilities() {
        let theme = theme();
        let outline = resolve_utility("outline-hidden", &theme).unwrap();
        assert!(outline.nested.iter().any(|rule| {
            rule.condition == "(forced-colors:active)"
                && rule.declaration.contains("outline-offset:2px;")
                && rule.declaration.contains("outline:2px solid transparent;")
        }));
    }

    #[test]
    fn resolves_tailwind_relationship_compatibility_primitives() {
        let theme = theme();

        for candidate in [
            "group",
            "peer",
            "group-hover:flex",
            "peer-checked:flex",
            "@sm:flex",
            "@max-sm:flex",
            "@[10rem]:flex",
        ] {
            assert!(
                is_tailwind_relationship_candidate(candidate, &theme),
                "{candidate}"
            );
        }
        assert!(!is_tailwind_relationship_candidate("max-w-lg", &theme));
        assert_eq!(
            compatibility_marker_class("group", &theme).as_deref(),
            Some("group")
        );
        assert_eq!(
            compatibility_marker_class("peer/field", &theme).as_deref(),
            Some("peer/field")
        );
        assert_eq!(
            resolve_utility("@container", &theme).unwrap().declaration,
            "container-type:inline-size;"
        );
        assert_eq!(
            resolve_utility("@container/sidebar", &theme)
                .unwrap()
                .declaration,
            "container-name:sidebar;container-type:inline-size;"
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("group-hover:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorMedia {
                selector: "&:is(:where(.group):hover *)".to_owned(),
                condition: "(hover: hover)".to_owned(),
            })
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("peer-checked:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&:is(:where(.peer):checked ~ *)".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("group-[&[data-open]]:flex")
                    .unwrap()
                    .variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&:is(:where(.group)[data-open] *)".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
    }

    #[test]
    fn resolves_numeric_css_domains_and_default_filter_tokens() {
        let theme = theme();
        let custom_theme = ThemeIndex {
            utilities: stillwind_css::parse_utility_definitions(
                "@utility tab-* { tab-size: --value(integer); }",
            ),
            ..ThemeIndex::default()
        };

        assert_eq!(
            resolve_utility("tab-8", &custom_theme).unwrap().declaration,
            "tab-size: 8;"
        );
        assert_eq!(
            resolve_utility("blur-sm", &theme).unwrap().declaration,
            "--tw-blur:blur(var(--blur-sm));filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
        );
    }

    #[test]
    fn resolves_theme_shadow_opacity_with_a_relative_colour_function() {
        let shadow = resolve_utility("inset-shadow-sm/50", &theme()).unwrap();

        assert!(shadow.declaration.contains("--tw-inset-shadow-alpha:50%;"));
        assert!(
            shadow.declaration.contains(
                "--tw-inset-shadow:inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%));"
            ),
            "{}",
            shadow.declaration
        );
        assert!(shadow.declaration.contains(
            "box-shadow:var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);"
        ));
        assert!(resolve_utility("inset-shadow-sm/not-an-opacity", &theme()).is_none());
    }

    #[test]
    fn keeps_css_wide_color_keywords_in_the_declared_utility_domain() {
        let theme = theme();

        assert!(resolve_utility("divide-inherit/50", &theme).is_some());
        assert!(resolve_utility("shadow-inherit", &theme).is_some());
        assert!(resolve_utility("shadow-inherit/50", &theme).is_none());
        assert!(resolve_utility("inset-shadow-inherit/50", &theme).is_none());
    }

    #[test]
    fn resolves_css_wide_keywords_for_property_typed_built_ins() {
        for candidate in [
            "contain-[initial]",
            "font-features-[initial]",
            "object-[initial]",
            "will-change-[initial]",
            "zoom-[initial]",
        ] {
            assert!(
                resolve_utility(candidate, &theme()).is_some(),
                "{candidate} must resolve through its exact CSS property grammar"
            );
        }
    }

    #[test]
    fn resolves_compact_spacing_families_with_negative_and_fractional_values() {
        let theme = theme();

        assert_eq!(
            resolve_utility("left-auto", &theme).unwrap().declaration,
            "left:auto;"
        );
        assert!(
            resolve_utility("-left-auto", &theme).is_none(),
            "negative keyword admissibility must come from the CSS definition"
        );
        assert_eq!(
            resolve_utility("-left-4", &theme).unwrap().declaration,
            "left:calc(var(--spacing) * -4);"
        );
        assert_eq!(
            resolve_utility("scroll-px-4", &theme).unwrap().declaration,
            "scroll-padding-inline:calc(var(--spacing) * 4);"
        );
        assert_eq!(
            resolve_utility("-left-1/2", &theme).unwrap().declaration,
            "left:calc(1 / 2 * 100% * -1);"
        );
        assert_eq!(
            resolve_utility("-bottom-full", &theme).unwrap().declaration,
            "bottom:-100%;"
        );
    }

    #[test]
    fn resolves_state_attribute_responsive_and_selector_variants() {
        let theme = theme();
        assert_eq!(
            resolve_variant(&parse_candidate("hover:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::SelectorMedia {
                selector: "&:hover".to_owned(),
                condition: "(hover: hover)".to_owned(),
            })
        );
        assert_eq!(
            resolve_variant(&parse_candidate("md:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width >= 48rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("aria-[busy=true]:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-busy=\"true\"]".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("[&>svg]:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate("&>svg".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("motion-reduce:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::Media(
                "(prefers-reduced-motion: reduce)".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_dynamic_media_structural_and_attribute_variants() {
        let theme = theme();
        let resolve = |candidate: &str| {
            let candidate = parse_candidate(candidate).unwrap();
            resolve_variant(&candidate.variants[0], &theme)
        };

        assert_eq!(
            resolve("min-[40rem]:flex"),
            Some(VariantApplication::Media("(width >= 40rem)".to_owned()))
        );
        assert_eq!(
            resolve("@sm:flex"),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
        assert_eq!(
            resolve("nth-[3n+1]:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:nth-child(3n+1)".to_owned()
            ))
        );
        assert_eq!(
            resolve("aria-checked:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-checked=\"true\"]".to_owned()
            ))
        );
        assert_eq!(
            resolve("has-checked:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:has(:checked)".to_owned()
            ))
        );
        assert_eq!(
            resolve("in-range:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:in-range".to_owned()
            ))
        );
        assert_eq!(
            resolve("not-focus:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:not(:focus)".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_user_utilities_with_the_same_value_domains_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@theme { --color-brand: red; }
@utility tone-* { color: --value(--color-*); }
@utility rail-* { inline-size: --value(--spacing()); }
@utility -rail-* { inline-size: calc(--value(--spacing()) * -1); }
@utility offset-* { inset: --value-map(auto: auto, full: 100%); }
@utility -offset-* { inset: calc(--value-map(full: 100%) * -1); }
@utility mask-* { mask-size: --value([length]); }
@utility alpha-* { opacity: --modifier(); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("tone-brand", &theme).unwrap().declaration,
            "color: var(--color-brand);"
        );
        assert_eq!(
            resolve_utility("rail-4", &theme).unwrap().declaration,
            "inline-size: calc(var(--spacing) * 4);"
        );
        assert_eq!(
            resolve_utility("-rail-4", &theme).unwrap().declaration,
            "inline-size: calc(var(--spacing) * -4);"
        );
        assert_eq!(
            resolve_utility("offset-auto", &theme).unwrap().declaration,
            "inset: auto;"
        );
        assert!(resolve_utility("-offset-auto", &theme).is_none());
        assert_eq!(
            resolve_utility("-offset-full", &theme).unwrap().declaration,
            "inset: -100%;"
        );
        assert_eq!(
            resolve_utility("mask-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "mask-size: 12px;"
        );
        assert_eq!(
            resolve_utility("alpha-brand/50", &theme)
                .unwrap()
                .declaration,
            "opacity: 50;"
        );
    }

    #[test]
    fn tries_overloaded_css_utility_definitions_in_reverse_source_order() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility token-* { width: --value([length]); }
@utility token-* { color: --value([color]); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("token-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color: rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("token-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "width: 12px;"
        );
    }

    #[test]
    fn preserves_css_fallbacks_and_multiple_selector_targets() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r#"@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
        );
        let theme = parse_tailwind_theme(&source).unwrap();
        let resolved = resolve_utility("paint-card", &theme).unwrap();

        assert!(
            resolved
                .declaration
                .contains("color: red; color: color(display-p3 1 0 0);")
                || resolved.additional.iter().any(|target| target
                    .declaration
                    .contains("color: red; color: color(display-p3 1 0 0);")),
            "{resolved:#?}"
        );
        assert_eq!(resolved.additional.len(), 1);
        assert_eq!(resolved.selector.as_deref(), Some("&::before"));
        assert_eq!(resolved.declaration, "content: \"fallback\";");
        assert_eq!(
            resolved.additional[0].declaration,
            "color: red; color: color(display-p3 1 0 0);"
        );
    }

    #[test]
    fn color_alpha_family_emits_srgb_fallback_and_oklab_upgrade() {
        let resolved = resolve_utility("text-brand/50", &theme()).unwrap();
        assert_eq!(
            resolved.declaration,
            "color:color-mix(in srgb, red 50%, transparent);"
        );
        assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration
                    == "color:color-mix(in oklab, var(--color-brand) 50%, transparent);"
        }));
        assert!(resolve_utility("text-brand/[length:12px]", &theme()).is_none());
    }

    #[test]
    fn shadow_color_alpha_emits_hex_fallback_and_upgrade() {
        let unmodified = resolve_utility("shadow-red-500", &theme()).unwrap();
        assert_eq!(
            unmodified.declaration,
            "--tw-shadow-color:oklch(63.7% 0.237 25.331);"
        );
        assert!(unmodified.nested.iter().any(|rule| {
            rule.declaration.contains(
                "--tw-shadow-color:color-mix(in oklab, var(--color-red-500) var(--tw-shadow-alpha), transparent);",
            )
        }));

        let resolved = resolve_utility("shadow-red-500/50", &theme()).unwrap();
        assert!(resolved.declaration.contains(
            "--tw-shadow-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
        ));
        assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration.contains(
                    "--tw-shadow-color:color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);",
                )
        }));
    }

    #[test]
    fn resolves_directional_radius_theme_values() {
        let theme = theme();
        for candidate in [
            "rounded-s-sm",
            "rounded-ss-sm",
            "rounded-e-sm",
            "rounded-se-sm",
            "rounded-ee-sm",
            "rounded-es-sm",
            "rounded-t-sm",
            "rounded-l-sm",
            "rounded-tl-sm",
            "rounded-r-sm",
            "rounded-tr-sm",
            "rounded-b-sm",
            "rounded-br-sm",
            "rounded-bl-sm",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_some(),
                "{candidate} must resolve from --radius-sm"
            );
        }
    }

    #[test]
    fn resolves_tailwind_static_keyword_utilities_from_the_css_registry() {
        let theme = theme();
        for (candidate, declaration) in [
            ("cursor-alias", "cursor:alias;"),
            ("overflow-scroll", "overflow:scroll;"),
            ("overscroll-y-none", "overscroll-behavior-y:none;"),
            ("break-after-page", "break-after:page;"),
            ("z-auto", "z-index:auto;"),
            ("z-73", "z-index:73;"),
            ("-z-73", "z-index:-73;"),
            ("border-13", "border-width:13px;"),
            ("col-span-full", "grid-column:1/-1;"),
            ("row-auto", "grid-row:auto;"),
            (
                "rounded-tl-full",
                "border-top-left-radius:calc(infinity * 1px);",
            ),
            ("touch-pan-left", "--tw-pan-x:pan-left;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration)
                    || resolved
                        .additional
                        .iter()
                        .any(|target| target.declaration.contains(declaration)),
                "{candidate}: {resolved:#?}"
            );
        }
    }

    #[test]
    fn resolves_discrete_tailwind_keywords_without_fixture_mappings() {
        let theme = theme();
        for (candidate, declaration, selector) in [
            (
                "divide-y",
                "border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * (1 - var(--tw-divide-y-reverse)));",
                Some(":where(&>:not(:last-child))"),
            ),
            (
                "divide-dashed",
                "border-style:dashed;",
                Some(":where(&>:not(:last-child))"),
            ),
            ("m-auto", "margin:auto;", None),
            ("mix-blend-color-dodge", "mix-blend-mode:color-dodge;", None),
            ("object-bottom-left", "object-position:left bottom;", None),
            (
                "outline",
                "outline-style:var(--tw-outline-style);outline-width:1px;",
                None,
            ),
            (
                "ring",
                "--tw-ring-shadow:var(--tw-ring-inset,) 0 0 0 calc(1px + var(--tw-ring-offset-width))",
                None,
            ),
            (
                "rounded-se-full",
                "border-start-end-radius:calc(infinity * 1px);",
                None,
            ),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration)
                    || resolved
                        .additional
                        .iter()
                        .any(|target| target.declaration.contains(declaration)),
                "{candidate}: {resolved:#?}"
            );
            assert_eq!(
                resolved.selector.as_deref().or_else(|| resolved
                    .additional
                    .first()
                    .and_then(|target| target.selector.as_deref())),
                selector,
                "{candidate}"
            );
        }
    }

    #[test]
    fn resolves_fraction_and_theme_value_families_from_typed_css_domains() {
        let theme = theme();
        for (candidate, declaration) in [
            ("flex-5/12", "flex:calc(5 / 12 * 100%);"),
            ("columns-lg", "columns:var(--container-lg);"),
            ("font-stretch-125%", "font-stretch:125%;"),
            ("bg-radial", "--tw-gradient-position:in oklab;"),
            ("bg-conic", "--tw-gradient-position:in oklab;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
        }
    }

    #[test]
    fn resolves_text_lengths_and_additional_discrete_layout_keywords() {
        let theme = theme();
        for (candidate, declaration) in [
            ("text-[length:1.25rem]", "font-size:1.25rem;"),
            ("grid-cols-subgrid", "grid-template-columns:subgrid;"),
            ("leading-none", "line-height:1;"),
            ("origin-bottom-left", "transform-origin:bottom left;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
        }

        let custom_theme = parse_tailwind_theme(&SourceFile::new(
            FileId(2),
            "theme.css",
            "@theme { --font-stretch-superbold: 150%; }",
        ))
        .unwrap();
        assert!(
            custom_theme
                .token_in(ThemeNamespace::FontStretch, "superbold")
                .is_some()
        );
        assert_eq!(
            resolve_utility("font-stretch-superbold", &custom_theme)
                .unwrap()
                .declaration,
            "font-stretch:var(--font-stretch-superbold);"
        );
    }

    #[test]
    fn validates_unhinted_and_hint_checked_arbitrary_css_values() {
        let theme = theme();
        for candidate in [
            "p-[1rem]",
            "p-[50%]",
            "p-[calc(1rem_+_50%)]",
            "p-[length:1rem]",
            "p-[percentage:50%]",
            "border-[3px]",
            "border-[thin]",
            "z-[12]",
            "z-[auto]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_some(),
                "expected `{candidate}` to resolve"
            );
        }
        assert_eq!(
            resolve_utility("p-[50%]", &theme).unwrap().declaration,
            "padding:50%;"
        );
        for candidate in [
            "p-[length:bogus]",
            "border-[time:1ms]",
            "border-[length:bogus]",
            "border-[50%]",
            "z-[1.5]",
            "z-[integer:1.5]",
            "z-[number:1]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_none(),
                "expected `{candidate}` to be rejected"
            );
        }
    }

    #[test]
    fn validates_arbitrary_utilities_against_property_grammars() {
        let theme = theme();
        for candidate in [
            "opacity-[.5]",
            "delay-[150ms]",
            "aspect-[16/9]",
            "grid-cols-[repeat(2,_minmax(0,_1fr))]",
            "content-['stillwind']",
            "mask-linear-[45deg,_red,_blue]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_some(),
                "expected `{candidate}` to resolve"
            );
        }
        for candidate in [
            "opacity-[red]",
            "delay-[red]",
            "aspect-[red]",
            "grid-cols-[red]",
            "content-[red]",
            "mask-linear-[red_/_blue]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_none(),
                "expected `{candidate}` to be rejected"
            );
        }
    }

    #[test]
    fn matches_tailwind_unbracketed_value_grammar_catalog() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/value-grammar.md");
        let theme = theme();
        for (heading, expected) in [
            ("## Padding: accepted without brackets", true),
            ("## Padding: rejected without brackets", false),
            ("## Border width: accepted without brackets", true),
            ("## Border width: rejected without brackets", false),
            ("## Z index: accepted without brackets", true),
            ("## Z index: rejected without brackets", false),
        ] {
            for candidate in catalog_section(catalog, heading) {
                assert_eq!(
                    resolve_utility(candidate, &theme).is_some(),
                    expected,
                    "{heading}: `{candidate}`"
                );
            }
        }
    }

    #[test]
    fn resolves_placeholder_colours_as_typed_pseudo_element_utilities() {
        let resolved = resolve_utility("placeholder-current/50", &theme()).unwrap();
        assert_eq!(resolved.selector.as_deref(), Some("&::placeholder"));
        assert!(
            resolved.declaration.contains("color:currentcolor;"),
            "{resolved:#?}"
        );
        assert!(resolved.nested.iter().any(|rule| {
            rule.declaration
                .contains("color-mix(in oklab, currentcolor 50%, transparent)")
        }));
        assert!(resolve_utility("placeholder-[length:1rem]", &theme()).is_none());
    }

    #[test]
    fn resolves_gradient_direction_and_interpolation_from_css_maps() {
        let resolved = resolve_utility("bg-linear-to-r/oklch", &theme()).unwrap();
        assert!(
            resolved
                .declaration
                .contains("--tw-gradient-position:to right;")
        );
        assert!(
            resolved
                .nested
                .iter()
                .any(|nested| nested.declaration.contains("to right in oklch"))
        );
        let radial = resolve_utility("bg-radial/hsl", &theme()).unwrap();
        assert!(
            radial
                .declaration
                .contains("--tw-gradient-position:in hsl;")
        );
    }

    #[test]
    fn resolves_size_as_a_functional_fraction_or_keyword() {
        let fractional = resolve_utility("size-7/12", &theme()).unwrap();
        assert!(
            fractional
                .declaration
                .contains("width:calc(7 / 12 * 100%);height:calc(7 / 12 * 100%);")
        );
        assert_eq!(
            resolve_utility("size-fit", &theme()).unwrap().declaration,
            "width:fit-content;height:fit-content;"
        );
    }

    #[test]
    fn resolves_filter_and_perspective_keywords() {
        let theme = theme();
        assert_eq!(
            resolve_utility("blur-none", &theme).unwrap().declaration,
            "--tw-blur:;filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
        );
        assert_eq!(
            resolve_utility("perspective-dramatic", &theme)
                .unwrap()
                .declaration,
            "perspective:var(--perspective-dramatic);"
        );
        assert_eq!(
            resolve_utility("perspective-origin-top-left", &theme)
                .unwrap()
                .declaration,
            "perspective-origin:top left;"
        );
    }

    #[test]
    fn lets_user_css_override_compatibility_fallback_utility_names() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@utility size-* { inline-size: --value(--spacing-*); block-size: --value(--spacing-*); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("size-4", &theme).unwrap().declaration,
            "inline-size: calc(var(--spacing) * 4); block-size: calc(var(--spacing) * 4);"
        );
    }

    #[test]
    fn resolves_animation_size_and_grid_columns_from_css_value_domains() {
        let theme = theme();

        assert_eq!(
            resolve_utility("animate-spin", &theme).unwrap().declaration,
            "animation:var(--animate-spin);"
        );
        assert_eq!(
            resolve_utility("size-[12px]", &theme).unwrap().declaration,
            "width:12px;height:12px;"
        );
        assert_eq!(
            resolve_utility("grid-cols-13", &theme).unwrap().declaration,
            "grid-template-columns:repeat(13,minmax(0,1fr));"
        );
    }

    #[test]
    fn resolves_translate_and_scale_fallbacks_from_css_value_domains() {
        let theme = theme();

        assert_eq!(
            resolve_utility("translate-x-[12px]", &theme)
                .unwrap()
                .declaration,
            "--tw-translate-x:12px;translate:var(--tw-translate-x) var(--tw-translate-y);"
        );
        assert_eq!(
            resolve_utility("-translate-y-[12px]", &theme)
                .unwrap()
                .declaration,
            "--tw-translate-y:-12px;translate:var(--tw-translate-x) var(--tw-translate-y);"
        );
        assert_eq!(
            resolve_utility("scale-[0.8]", &theme).unwrap().declaration,
            "scale:0.8;"
        );
    }

    #[test]
    fn resolves_user_block_variants_with_the_same_slot_syntax_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant pressed { &:active { @slot; } }\n@variant narrow { @media (width < 40rem) { @slot; } }\n@variant decorated { &::before { content: var(--content); @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(
                &parse_candidate("pressed:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate("&:active".to_owned()))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("narrow:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width < 40rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("decorated:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorRule {
                selector: "&::before".to_owned(),
                declarations: vec![VariantDeclaration {
                    property: "content".to_owned(),
                    value: "var(--content)".to_owned(),
                }],
            })
        );
    }

    #[test]
    fn resolves_wildcard_selector_variants_from_application_css() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant state-* { &[data-state=--value()] { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(
                &parse_candidate("state-open:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[data-state=open]".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("aria-expanded:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-expanded=\"true\"]".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_compat_color_value_function() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-brand: red; --breakpoint-narrow: 37rem; --breakpoint-wide: 80rem; }\n@utility tone-* { color: --compat-color(--value(--color-*), --alpha(--modifier())); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();
        assert_eq!(
            resolve_utility("tone-brand/50", &theme)
                .unwrap()
                .declaration,
            "color: color-mix(in srgb, var(--color-brand) 50%, transparent);"
        );
        let utility = resolve_utility("tone-brand/50", &theme).unwrap();
        assert_eq!(utility.nested.len(), 1);
        assert_eq!(utility.nested[0].condition, COMPAT_COLOR_SUPPORT);
        assert!(
            utility.nested[0]
                .declaration
                .contains("color-mix(in oklab, var(--color-brand) 50%, transparent)")
        );

        let plain_source = SourceFile::new(
            FileId(2),
            "plain.css",
            "@utility plain-tone { color: --compat-color(red, 50%); }",
        );
        let plain_theme = parse_tailwind_theme(&plain_source).unwrap();
        assert_eq!(
            resolve_utility("plain-tone", &plain_theme)
                .unwrap()
                .declaration,
            "color: color-mix(in srgb, red 50%, transparent);"
        );

        let nested_source = SourceFile::new(
            FileId(3),
            "nested.css",
            "@utility nested-tone-* { @media (width >= 1px) { color: --compat-color(red, --alpha(--modifier())); } }",
        );
        let nested_theme = parse_tailwind_theme(&nested_source).unwrap();
        let nested = resolve_utility("nested-tone-brand/50", &nested_theme).unwrap();
        assert!(nested.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.declaration.contains("color-mix(in oklab")
        }));
    }

    #[test]
    fn expands_theme_wildcards_in_nested_utility_rules() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --breakpoint-narrow: 37rem; --breakpoint-wide: 80rem; }\n@utility container { width: 100%; @media (width >= --value(--breakpoint-*)) { max-width: --value(--breakpoint-*); } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();
        let utility = resolve_utility("container", &theme).unwrap();
        let conditions = utility
            .nested
            .iter()
            .map(|rule| rule.condition.as_str())
            .collect::<Vec<_>>();
        assert!(conditions.contains(&"(width >= 37rem)"));
        assert!(conditions.contains(&"(width >= 80rem)"));
    }

    #[test]
    fn resolves_container_at_variants_from_css_registry() {
        let source = SourceFile::new(FileId(1), "theme.css", "@theme { --container-md: 40rem; }");
        let theme = parse_tailwind_theme(&source).unwrap();
        let candidate = parse_candidate("@min-md:flex").unwrap();
        assert_eq!(
            resolve_variant(&candidate.variants[0], &theme),
            Some(VariantApplication::Container("(width >= 40rem)".to_owned()))
        );
        for (candidate, condition) in [
            ("@md/sidebar:flex", "sidebar (width >= 40rem)"),
            ("@[48rem]/main:flex", "main (width >= 48rem)"),
            ("@max-[48rem]/main:flex", "main (width < 48rem)"),
        ] {
            let candidate = parse_candidate(candidate).unwrap();
            assert_eq!(
                resolve_variant(&candidate.variants[0], &theme),
                Some(VariantApplication::Container(condition.to_owned()))
            );
        }
    }

    #[test]
    fn resolves_formal_variant_modifier_placeholder() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant rel-* { :where(.peer/{modifier}) ~ & { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();
        assert_eq!(
            resolve_variant(
                &parse_candidate("rel-checked/field:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                r":where(.peer\/field) ~ &".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_wildcard_conditional_variants_from_css() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --breakpoint-narrow: 40rem; }\n@variant narrow-* { @media (width < --value(--breakpoint-*)) { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(&parse_candidate("min-md:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width >= 48rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("narrow-narrow:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::Media("(width < 40rem)".to_owned()))
        );
    }

    const REGISTRY_FILES: &[(&str, &str)] = &[
        ("builtins.css", include_str!("../../../css/builtins.css")),
        (
            "registry-metadata.css",
            include_str!("../../../css/builtins/registry-metadata.css"),
        ),
        (
            "relationship-metadata.css",
            include_str!("../../../css/builtins/relationship-metadata.css"),
        ),
        (
            "layout-utilities.css",
            include_str!("../../../css/builtins/layout-utilities.css"),
        ),
        (
            "spacing-sizing-utilities.css",
            include_str!("../../../css/builtins/spacing-sizing-utilities.css"),
        ),
        (
            "flex-grid-utilities.css",
            include_str!("../../../css/builtins/flex-grid-utilities.css"),
        ),
        (
            "typography-utilities.css",
            include_str!("../../../css/builtins/typography-utilities.css"),
        ),
        (
            "backgrounds-utilities.css",
            include_str!("../../../css/builtins/backgrounds-utilities.css"),
        ),
        (
            "borders-utilities.css",
            include_str!("../../../css/builtins/borders-utilities.css"),
        ),
        (
            "effects-utilities.css",
            include_str!("../../../css/builtins/effects-utilities.css"),
        ),
        (
            "masks-utilities.css",
            include_str!("../../../css/builtins/masks-utilities.css"),
        ),
        (
            "transforms-utilities.css",
            include_str!("../../../css/builtins/transforms-utilities.css"),
        ),
        (
            "motion-utilities.css",
            include_str!("../../../css/builtins/motion-utilities.css"),
        ),
        (
            "interactivity-utilities.css",
            include_str!("../../../css/builtins/interactivity-utilities.css"),
        ),
        (
            "svg-utilities.css",
            include_str!("../../../css/builtins/svg-utilities.css"),
        ),
        (
            "relationship-variants.css",
            include_str!("../../../css/builtins/relationship-variants.css"),
        ),
        (
            "structural-variants.css",
            include_str!("../../../css/builtins/structural-variants.css"),
        ),
        (
            "state-variants.css",
            include_str!("../../../css/builtins/state-variants.css"),
        ),
        (
            "dynamic-selector-variants.css",
            include_str!("../../../css/builtins/dynamic-selector-variants.css"),
        ),
        (
            "conditional-variants.css",
            include_str!("../../../css/builtins/conditional-variants.css"),
        ),
        (
            "properties.css",
            include_str!("../../../css/builtins/properties.css"),
        ),
    ];

    #[test]
    fn builtin_negative_value_definitions_author_their_sign_explicitly() {
        for (file, css) in REGISTRY_FILES {
            assert!(
                !css.contains("--inset()"),
                "{file} must express keyword domains in CSS instead of a Rust inset helper"
            );
            for utility in stillwind_css::parse_builtin_utility_definitions(css) {
                if utility.name.starts_with('-')
                    && (utility.body.contains("--value(") || utility.body.contains("--value-map("))
                {
                    assert!(
                        utility.body.contains("* -1"),
                        "negative utility {} in {file} must author its sign in CSS: {}",
                        utility.name,
                        utility.body
                    );
                }
            }
        }
    }

    fn registry_utilities() -> Vec<(String, CustomUtility)> {
        let mut definitions = Vec::new();
        for (file, css) in REGISTRY_FILES {
            let mut occurrences = HashMap::<String, usize>::new();
            for mut utility in stillwind_css::parse_builtin_utility_definitions(css) {
                utility.body = canonical_builtin_body(&utility.body);
                let occurrence = occurrences.entry(utility.name.clone()).or_default();
                *occurrence += 1;
                definitions.push((format!("{file}:{}#{occurrence}", utility.name), utility));
            }
        }
        definitions
    }

    fn registry_variants() -> Vec<(String, CustomVariant)> {
        let mut definitions = Vec::new();
        for (file, css) in REGISTRY_FILES {
            let mut occurrences = HashMap::<String, usize>::new();
            for variant in stillwind_css::parse_builtin_variant_definitions(css) {
                let occurrence = occurrences.entry(variant.name.clone()).or_default();
                *occurrence += 1;
                definitions.push((format!("{file}:{}#{occurrence}", variant.name), variant));
            }
        }
        definitions
    }

    fn fenced_section<'a>(section: &'a str, heading: &str, language: &str) -> Option<&'a str> {
        let after_heading = section.split_once(heading)?.1;
        let fence = format!("```{language}");
        let after_fence = after_heading.split_once(&fence)?.1;
        after_fence.split_once("```").map(|(body, _)| body.trim())
    }

    fn parity_cases(markdown: &str) -> Vec<(&str, Vec<&str>)> {
        markdown
            .split("\n## `")
            .skip(1)
            .filter_map(|section| {
                let input = fenced_section(section, "### Input CSS", "css")?;
                let candidates = fenced_section(section, "### Candidates", "text")?
                    .split_whitespace()
                    .collect::<Vec<_>>();
                Some((input, candidates))
            })
            .collect()
    }

    fn matched_registry_utilities<'a>(
        definitions: &'a [(String, CustomUtility)],
        lookup: &str,
    ) -> Vec<(usize, &'a CustomUtility)> {
        let matches = |definition: &CustomUtility| {
            definition.name == lookup
                || definition
                    .name
                    .strip_suffix('*')
                    .is_some_and(|prefix| lookup.starts_with(prefix))
        };
        definitions
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, (_, definition))| definition.name.ends_with('*') && matches(definition))
            .map(|(index, (_, definition))| (index, definition))
            .chain(
                definitions
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, (_, definition))| definition.name == lookup)
                    .map(|(index, (_, definition))| (index, definition)),
            )
            .collect()
    }

    fn resolve_registry_variant(
        definition: &CustomVariant,
        value: &str,
        theme: &ThemeIndex,
    ) -> (bool, Option<VariantApplication>) {
        let candidates = registered_variant_candidates(std::slice::from_ref(definition), value);
        let matched = !candidates.is_empty();
        let resolved = candidates.into_iter().find_map(|(custom, capture)| {
            let body = substitute_variant_capture(&custom.body, capture)?;
            let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
            resolve_variant_body(&body).or_else(|| {
                substitute_variant_capture(&custom.selector, capture)
                    .map(VariantApplication::SelectorTemplate)
            })
        });
        (matched, resolved)
    }

    fn resolve_registry_variant_branches(
        definitions: &[(String, CustomVariant)],
        excluded: Option<usize>,
        value: &str,
        theme: &ThemeIndex,
    ) -> Option<VariantApplication> {
        let resolve = |custom: &CustomVariant| {
            registered_variant_candidates(std::slice::from_ref(custom), value)
                .into_iter()
                .find_map(|(custom, capture)| {
                    let body = substitute_variant_capture(&custom.body, capture)?;
                    let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
                    resolve_variant_body(&body).or_else(|| {
                        substitute_variant_capture(&custom.selector, capture)
                            .map(VariantApplication::SelectorTemplate)
                    })
                })
        };
        definitions
            .iter()
            .enumerate()
            .rev()
            .filter(|(index, (_, custom))| Some(*index) != excluded && custom.name == value)
            .find_map(|(_, (_, custom))| resolve(custom))
            .or_else(|| {
                definitions
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(index, (_, custom))| {
                        Some(*index) != excluded && custom.name.ends_with('*')
                    })
                    .find_map(|(_, (_, custom))| resolve(custom))
            })
    }

    #[test]
    #[ignore = "manual exhaustive built-in registry reachability audit"]
    #[allow(clippy::too_many_lines)]
    fn audits_builtin_registry_reachability_from_parity_fixtures() {
        assert_eq!(
            REGISTRY_FILES
                .iter()
                .map(|(_, css)| *css)
                .collect::<String>(),
            BUILTIN_REGISTRY_CSS,
            "audit registry must follow the production registry exactly"
        );
        let utilities = registry_utilities();
        let variants = registry_variants();
        let mut matched_utilities = BTreeSet::new();
        let mut resolved_utilities = BTreeSet::new();
        let mut used_utilities = BTreeSet::new();
        let mut matched_variants = BTreeSet::new();
        let mut resolved_variants = BTreeSet::new();
        let mut used_variants = BTreeSet::new();
        let fixture_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tailwind-v4/cases");
        let mut fixture_paths = fs::read_dir(fixture_root)
            .expect("parity fixture directory")
            .map(|entry| entry.expect("fixture entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
            .collect::<Vec<_>>();
        fixture_paths.sort();

        let mut candidate_count = 0_usize;
        let mut skipped_scope_candidates = 0_usize;
        let mut theme_parse_failures = 0_usize;
        let mut candidate_parse_failures = 0_usize;
        for path in fixture_paths {
            let markdown = fs::read_to_string(&path).expect("parity fixture");
            for (case_index, (input, candidates)) in parity_cases(&markdown).into_iter().enumerate()
            {
                if ["@source", "@config", "@plugin", "@reference"]
                    .iter()
                    .any(|directive| input.contains(directive))
                {
                    skipped_scope_candidates += candidates.len();
                    continue;
                }
                let source =
                    SourceFile::new(FileId(1), format!("{}#{case_index}", path.display()), input);
                let Ok(theme) = parse_tailwind_theme(&source) else {
                    theme_parse_failures += 1;
                    continue;
                };
                for raw in candidates {
                    let Some(candidate) = parse_candidate(raw) else {
                        candidate_parse_failures += 1;
                        continue;
                    };
                    candidate_count += 1;
                    for variant in &candidate.variants {
                        if let VariantKind::Named(value) = &variant.kind {
                            for (index, (_, definition)) in variants.iter().enumerate() {
                                let (matched, resolved) =
                                    resolve_registry_variant(definition, value, &theme);
                                if matched {
                                    matched_variants.insert(index);
                                }
                                if resolved.is_some() {
                                    resolved_variants.insert(index);
                                }
                            }
                            let baseline =
                                resolve_registry_variant_branches(&variants, None, value, &theme);
                            for excluded in 0..variants.len() {
                                let without = resolve_registry_variant_branches(
                                    &variants,
                                    Some(excluded),
                                    value,
                                    &theme,
                                );
                                if without != baseline {
                                    used_variants.insert(excluded);
                                }
                            }
                        }
                    }
                    let Utility::Named { name, modifier } = &candidate.utility else {
                        continue;
                    };
                    let lookup = if candidate.negative {
                        format!("-{name}")
                    } else {
                        name.clone()
                    };
                    let matched = matched_registry_utilities(&utilities, &lookup);
                    matched_utilities.extend(matched.iter().map(|(index, _)| *index));
                    resolved_utilities.extend(matched.iter().filter_map(|(index, definition)| {
                        resolve_css_utility_body(definition, &lookup, modifier.as_ref(), &theme)
                            .map(|_| *index)
                    }));
                    let baseline = resolve_css_utility_branches(
                        matched.iter().map(|(_, definition)| *definition),
                        &lookup,
                        modifier.as_ref(),
                        &theme,
                    );
                    for (excluded, _) in &matched {
                        let without = resolve_css_utility_branches(
                            matched
                                .iter()
                                .filter(|(index, _)| index != excluded)
                                .map(|(_, definition)| *definition),
                            &lookup,
                            modifier.as_ref(),
                            &theme,
                        );
                        if without != baseline {
                            used_utilities.insert(*excluded);
                        }
                    }
                }
            }
        }

        let never_matched_utilities = utilities
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched_utilities.contains(index))
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let unresolved_utilities = utilities
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                matched_utilities.contains(index) && !resolved_utilities.contains(index)
            })
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let non_contributing_utilities = utilities
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                resolved_utilities.contains(index) && !used_utilities.contains(index)
            })
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let never_matched_variants = variants
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched_variants.contains(index))
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let unresolved_variants = variants
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                matched_variants.contains(index) && !resolved_variants.contains(index)
            })
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let shadowed_variants = variants
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                resolved_variants.contains(index) && !used_variants.contains(index)
            })
            .map(|(_, (label, _))| label.as_str())
            .collect::<Vec<_>>();
        let mut duplicate_utilities = HashMap::<(&str, &str), Vec<&str>>::new();
        for (label, utility) in &utilities {
            duplicate_utilities
                .entry((&utility.name, &utility.body))
                .or_default()
                .push(label);
        }
        let mut duplicate_utility_groups = duplicate_utilities
            .into_values()
            .filter(|labels| labels.len() > 1)
            .map(|labels| labels.join(" = "))
            .collect::<Vec<_>>();
        duplicate_utility_groups.sort();
        eprintln!(
            "registry audit: {candidate_count} candidates, {skipped_scope_candidates} out-of-scope candidates, {theme_parse_failures} theme parse failures, {candidate_parse_failures} candidate parse failures, {}/{} utility definitions contribute, {}/{} variant definitions contribute",
            used_utilities.len(),
            utilities.len(),
            used_variants.len(),
            variants.len()
        );
        eprintln!(
            "utility definitions never matched ({}):\n{}",
            never_matched_utilities.len(),
            never_matched_utilities.join("\n")
        );
        eprintln!(
            "utility definitions matched but never resolved ({}):\n{}",
            unresolved_utilities.len(),
            unresolved_utilities.join("\n")
        );
        eprintln!(
            "utility definitions resolved but never contributed ({}):\n{}",
            non_contributing_utilities.len(),
            non_contributing_utilities.join("\n")
        );
        eprintln!(
            "exact duplicate utility groups ({}):\n{}",
            duplicate_utility_groups.len(),
            duplicate_utility_groups.join("\n")
        );
        eprintln!(
            "variant definitions never matched ({}):\n{}",
            never_matched_variants.len(),
            never_matched_variants.join("\n")
        );
        eprintln!(
            "variant definitions matched but never resolved ({}):\n{}",
            unresolved_variants.len(),
            unresolved_variants.join("\n")
        );
        eprintln!(
            "variant definitions resolved but never contributed ({}):\n{}",
            shadowed_variants.len(),
            shadowed_variants.join("\n")
        );
        assert!(
            never_matched_utilities.is_empty(),
            "every utility definition must have fixture coverage"
        );
        assert!(
            unresolved_utilities.is_empty(),
            "every matched utility definition must resolve in a fixture"
        );
        assert!(
            duplicate_utility_groups.is_empty(),
            "exact duplicate utility definitions must be removed"
        );
        assert!(
            never_matched_variants.is_empty()
                && unresolved_variants.is_empty()
                && shadowed_variants.is_empty(),
            "every variant definition must contribute in a fixture"
        );
    }
}
