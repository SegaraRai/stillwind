//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{collections::HashMap, fmt::Write, sync::OnceLock};

use shorth_css::{CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace};
use shorth_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
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
}

impl UtilityResolution {
    fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorMedia { selector: String, condition: String },
    Media(String),
    Container(String),
    Supports(String),
    StartingStyle,
}

const BUILTIN_REGISTRY_CSS: &str = include_str!("../assets/builtins.css");
const DEFAULT_UTILITY_REGISTRY_CSS: &str = include_str!("../assets/default-utilities.css");

struct BuiltinUtilityRegistry {
    default_exact: HashMap<String, Vec<CustomUtility>>,
    builtin_exact: HashMap<String, Vec<CustomUtility>>,
    wildcards: Vec<CustomUtility>,
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
        let defaults = shorth_css::parse_utility_definitions(DEFAULT_UTILITY_REGISTRY_CSS)
            .into_iter()
            .map(&normalize)
            .collect::<Vec<_>>();
        let builtins = shorth_css::parse_utility_definitions(BUILTIN_REGISTRY_CSS)
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let mut default_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut builtin_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut wildcards = Vec::new();
        for utility in defaults {
            default_exact
                .entry(utility.name.clone())
                .or_default()
                .push(utility);
        }
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
            default_exact,
            builtin_exact,
            wildcards,
        }
    })
}

fn builtin_variants() -> &'static [CustomVariant] {
    static VARIANTS: OnceLock<Vec<CustomVariant>> = OnceLock::new();
    VARIANTS
        .get_or_init(|| shorth_css::parse_variant_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

/// Removes non-semantic whitespace from vendored declarations.
///
/// This applies only to Stillwind-maintained CSS; user `@utility` bodies keep
/// their authored serialization. Quoted values are preserved byte-for-byte.
fn canonical_builtin_body(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut quote = None;
    let mut escaped = false;
    let mut whitespace = false;

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
            character if character.is_whitespace() => whitespace = true,
            character => {
                if whitespace && let Some(previous) = output.chars().last() {
                    let adjacent_words =
                        is_word_character(previous) && is_word_character(character);
                    let function_followed_by_word = previous == ')' && is_word_character(character);
                    if adjacent_words || function_followed_by_word {
                        output.push(' ');
                    }
                }
                whitespace = false;
                output.push(character);
            }
        }
    }

    output
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '%')
}

/// Resolves a candidate using only the Tailwind-compatible registry.
#[must_use]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    let resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            UtilityResolution::plain(format!("{property}:{value};"))
        }
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    let declaration = if candidate.important {
        important(&resolution.declaration)
    } else {
        resolution.declaration
    };
    let nested = resolution
        .nested
        .into_iter()
        .map(|mut nested| {
            if candidate.important {
                nested.declaration = important(&nested.declaration);
            }
            nested
        })
        .collect();
    Some(ResolvedUtility {
        declaration,
        selector: resolution.selector,
        nested,
        candidate,
    })
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
    if let Some(condition) = value
        .strip_prefix("supports-[")
        .and_then(|value| value.strip_suffix(']'))
    {
        return Some(VariantApplication::Supports(condition.replace('_', " ")));
    }
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
    for (prefix, attribute) in [("aria-", "aria"), ("data-", "data")] {
        if let Some(name) = value.strip_prefix(prefix) {
            let selector = if attribute == "aria" {
                format!("&[{attribute}-{name}=\"true\"]")
            } else {
                format!("&[{attribute}-{name}]")
            };
            return Some(VariantApplication::SelectorTemplate(selector));
        }
    }
    if let Some(condition) = dynamic_media_condition(value, theme) {
        return Some(VariantApplication::Media(condition));
    }
    if let Some(condition) = dynamic_container_condition(value, theme) {
        return Some(VariantApplication::Container(condition));
    }
    for (prefix, pseudo) in [
        ("nth-[", "nth-child"),
        ("nth-last-[", "nth-last-child"),
        ("nth-of-type-[", "nth-of-type"),
        ("nth-last-of-type-[", "nth-last-of-type"),
    ] {
        if let Some(value) = bracketed(value, prefix) {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&:{pseudo}({})",
                value.replace('_', " ")
            )));
        }
    }
    for (prefix, pseudo) in [
        ("nth-last-of-type-", "nth-last-of-type"),
        ("nth-of-type-", "nth-of-type"),
        ("nth-last-", "nth-last-child"),
        ("nth-", "nth-child"),
    ] {
        if let Some(value) = value.strip_prefix(prefix) {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&:{pseudo}({})",
                value.replace('_', " ")
            )));
        }
    }
    if let Some(value) = value.strip_prefix("has-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            "&:has(:{value})"
        )));
    }
    if let Some(value) = value.strip_prefix("not-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            "&:not(:{value})"
        )));
    }
    if let Some(value) = value.strip_prefix("in-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            ":where(:{value}) &"
        )));
    }
    theme
        .token_in(ThemeNamespace::Breakpoint, value)
        .map(|token| VariantApplication::Media(format!("(width >= {})", token.value.value)))
}

fn resolve_registered_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    let custom = theme
        .variants
        .iter()
        .rev()
        .find(|variant| variant.name == value)
        .or_else(|| {
            builtin_variants()
                .iter()
                .rev()
                .find(|variant| variant.name == value)
        })?;
    resolve_variant_body(&custom.body).or_else(|| {
        Some(VariantApplication::SelectorTemplate(
            custom.selector.clone(),
        ))
    })
}

fn dynamic_media_condition(value: &str, theme: &ThemeIndex) -> Option<String> {
    for (prefix, operator) in [("min-[", ">="), ("max-[", "<")] {
        if let Some(value) = bracketed(value, prefix) {
            return Some(format!("(width {operator} {})", value.replace('_', " ")));
        }
    }
    for (prefix, operator) in [("min-", ">="), ("max-", "<")] {
        if let Some(name) = value.strip_prefix(prefix)
            && let Some(token) = theme.token_in(ThemeNamespace::Breakpoint, name)
        {
            return Some(format!("(width {operator} {})", token.value.value));
        }
    }
    None
}

fn dynamic_container_condition(value: &str, theme: &ThemeIndex) -> Option<String> {
    for (prefix, operator) in [("@min-[", ">="), ("@max-[", "<")] {
        if let Some(value) = bracketed(value, prefix) {
            return Some(format!("(width {operator} {})", value.replace('_', " ")));
        }
    }
    if let Some(value) = bracketed(value, "@[") {
        return Some(format!("(width >= {})", value.replace('_', " ")));
    }
    let (prefix, operator) = if let Some(name) = value.strip_prefix("@max-") {
        (name, "<")
    } else if let Some(name) = value.strip_prefix("@min-") {
        (name, ">=")
    } else {
        (value.strip_prefix('@')?, ">=")
    };
    theme
        .token_in(ThemeNamespace::Container, prefix)
        .map(|token| format!("(width {operator} {})", token.value.value))
}

fn bracketed<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value.strip_prefix(prefix)?.strip_suffix(']')
}

fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
    }
    if body.contains("@starting-style") {
        return Some(VariantApplication::StartingStyle);
    }
    if let Some(start) = body.find("@media ") {
        let condition_start = start + "@media ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let condition = body[condition_start..condition_end].trim().to_owned();
        let nested = &body[condition_end + 1..];
        if let Some(selector) = selector_from_variant_body(nested) {
            return Some(VariantApplication::SelectorMedia {
                selector,
                condition,
            });
        }
        return Some(VariantApplication::Media(condition));
    }
    selector_from_variant_body(body).map(VariantApplication::SelectorTemplate)
}

fn selector_from_variant_body(body: &str) -> Option<String> {
    let start = body.find('&')?;
    let selector_end = body[start..].find('{')? + start;
    let selector = body[start..selector_end].trim();
    (!selector.is_empty()).then(|| selector.to_owned())
}

fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    // Application CSS is the authoritative registry layer. It must be able to
    // replace every built-in name, including the few compatibility fallbacks
    // still implemented below while their value grammar is being generalized.
    if let Some(declaration) = resolve_user_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    if let Some(declaration) = resolve_special(name, modifier, negative, theme) {
        return Some(UtilityResolution::plain(declaration));
    }
    if let Some(declaration) = resolve_builtin_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    if let Some(columns) = name.strip_prefix("grid-cols-")
        && let Ok(columns) = columns.parse::<u8>()
        && (1..=12).contains(&columns)
        && !negative
        && modifier.is_none()
    {
        return Some(UtilityResolution::plain(format!(
            "grid-template-columns:repeat({columns},minmax(0,1fr));"
        )));
    }
    resolve_arbitrary(name, negative).map(UtilityResolution::plain)
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
    matching_utilities(&theme.utilities, lookup)
        .find_map(|utility| resolve_css_utility_body(utility, lookup, modifier, theme))
}

fn resolve_builtin_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    matching_utilities(&builtin_utilities().wildcards, lookup)
        .chain(
            builtin_utilities()
                .builtin_exact
                .get(lookup)
                .into_iter()
                .flat_map(|utilities| utilities.iter().rev()),
        )
        .chain(
            builtin_utilities()
                .default_exact
                .get(lookup)
                .into_iter()
                .flat_map(|utilities| utilities.iter().rev()),
        )
        .find_map(|utility| resolve_css_utility_body(utility, lookup, modifier, theme))
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
    let (selector, body) = split_utility_selector(&utility.body)?;
    let (body, nested) = split_nested_conditions(body)?;
    let declaration =
        resolve_css_placeholders(&body, utility.name.as_str(), name, modifier, theme)?;
    let nested = nested
        .into_iter()
        .map(|(at_rule, condition, body)| {
            resolve_css_placeholders(&body, utility.name.as_str(), name, modifier, theme).map(
                |declaration| NestedUtilityRule {
                    at_rule,
                    condition,
                    declaration,
                },
            )
        })
        .collect::<Option<Vec<_>>>()?;
    (!declaration.is_empty() || !nested.is_empty()).then_some(UtilityResolution {
        declaration,
        selector,
        nested,
    })
}

fn resolve_css_placeholders(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = utility_name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_value_domains(
            &body[open + 1..end],
            capture,
            utility_name.starts_with('-'),
            theme,
        )?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier(") {
        let open = start + "--modifier".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let replacement = match modifier {
            Value::Named(value) | Value::Arbitrary(value) => value.clone(),
            Value::CssVariable(value) => format!("var({value})"),
        };
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--alpha(") {
        let open = start + "--alpha".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = alpha(&body[open + 1..end])?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(") && !body.contains("--modifier(") && !body.contains("--alpha("))
        .then_some(body)
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

fn split_utility_selector(body: &str) -> Option<(Option<String>, &str)> {
    let Some(selector_start) = body.find('&') else {
        return Some((None, body));
    };
    let open = body[selector_start..].find('{')? + selector_start;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[selector_start..open].trim();
    (!selector.is_empty()).then(|| (Some(selector.to_owned()), body[open + 1..close].trim()))
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
    negative: bool,
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
        if domain == "--spacing()" {
            return capture.and_then(|capture| {
                theme_value(ThemeNamespace::Spacing, capture, negative, theme)
            });
        }
        if let Some(namespace) = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)
        {
            return capture.and_then(|capture| theme_value(namespace, capture, negative, theme));
        }
        if !domain.starts_with('[') && !domain.starts_with("--") {
            return capture.filter(|value| *value == domain).map(str::to_owned);
        }
        let hint = domain.strip_prefix('[')?.strip_suffix(']')?;
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (actual_hint, value) = arbitrary.split_once(':').unwrap_or(("*", arbitrary));
        (hint == "*" || hint == actual_hint).then(|| decode_arbitrary(value))
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

fn resolve_special(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    if let Some(value) = name.strip_prefix("animate-")
        && theme.token_in(ThemeNamespace::Animate, value).is_some()
        && !negative
        && modifier.is_none()
    {
        return Some(format!("animation:var(--animate-{value});"));
    }
    if let Some(value) = name.strip_prefix("size-") {
        let value = theme_value(ThemeNamespace::Spacing, value, negative, theme)?;
        return Some(format!("width:{value};height:{value};"));
    }
    for (prefix, property) in [
        ("translate-x-", "translateX"),
        ("translate-y-", "translateY"),
    ] {
        if let Some(value) = name.strip_prefix(prefix) {
            let value = theme_value(ThemeNamespace::Spacing, value, negative, theme)?;
            return Some(format!("transform:{property}({value});"));
        }
    }
    if let Some(percent) = name
        .strip_prefix("scale-")
        .and_then(|value| value.parse::<u16>().ok())
        && !negative
        && modifier.is_none()
    {
        return Some(format!("transform:scale({});", f64::from(percent) / 100.0));
    }
    None
}

fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = if let Some(value) = arbitrary(value) {
        value
    } else if let Some(value) = css_variable(value) {
        format!("var({value})")
    } else if theme.token_in(namespace, value).is_some() {
        format!("var(--{}-{value})", namespace.css_name())
    } else if namespace == ThemeNamespace::Spacing && theme.scale(ThemeNamespace::Spacing).is_some()
    {
        if value == "px" {
            "1px".to_owned()
        } else if value.parse::<f64>().is_ok() {
            let multiplier = if negative {
                format!("-{value}")
            } else {
                value.to_owned()
            };
            return Some(format!("calc(var(--spacing) * {multiplier})"));
        } else {
            return None;
        }
    } else {
        return None;
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn resolve_arbitrary(name: &str, negative: bool) -> Option<String> {
    let mappings = [
        ("bg-", "background-color"),
        ("text-", "color"),
        ("border-", "border-color"),
        ("outline-", "outline-color"),
        ("p-", "padding"),
        ("px-", "padding-inline"),
        ("py-", "padding-block"),
        ("m-", "margin"),
        ("mx-", "margin-inline"),
        ("my-", "margin-block"),
        ("w-", "width"),
        ("h-", "height"),
        ("min-w-", "min-width"),
        ("min-h-", "min-height"),
        ("max-w-", "max-width"),
        ("max-h-", "max-height"),
        ("gap-", "gap"),
        ("rounded-", "border-radius"),
    ];
    for (prefix, property) in mappings {
        if let Some(value) = name.strip_prefix(prefix).and_then(arbitrary) {
            let value = if negative {
                format!("calc({value} * -1)")
            } else {
                value
            };
            return Some(format!("{property}:{value};"));
        }
    }
    None
}

fn arbitrary(value: &str) -> Option<String> {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map(decode_arbitrary)
}

fn css_variable(value: &str) -> Option<&str> {
    value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
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
    use shorth_css::parse_tailwind_theme;
    use shorth_source::{FileId, SourceFile};

    use super::*;

    fn theme() -> ThemeIndex {
        parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-brand: red; }",
        ))
        .unwrap()
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

        assert_eq!(resolved.declaration, "border-inline-end-width:2px;");
        assert_eq!(resolved.selector.as_deref(), Some("&>:not(:last-child)"));
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
        assert_eq!(
            resolve_utility("bg-brand/50", &theme).unwrap().declaration,
            "background-color:color-mix(in oklab,var(--color-brand) 50%,transparent);"
        );
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
    fn resolves_css_defined_color_and_gradient_utilities() {
        let theme = theme();
        assert_eq!(
            resolve_utility("fill-brand/50", &theme)
                .unwrap()
                .declaration,
            "fill:color-mix(in oklab,var(--color-brand) 50%,transparent);"
        );
        assert_eq!(
            resolve_utility("from-brand", &theme).unwrap().declaration,
            "--tw-gradient-from:var(--color-brand);--tw-gradient-stops:var(--tw-gradient-via-stops,var(--tw-gradient-position),var(--tw-gradient-from) var(--tw-gradient-from-position),var(--tw-gradient-to) var(--tw-gradient-to-position));"
        );
        assert_eq!(
            resolve_utility("bg-linear-to-r", &theme)
                .unwrap()
                .declaration,
            "--tw-gradient-position:to right;background-image:linear-gradient(var(--tw-gradient-stops));"
        );
    }

    #[test]
    fn preserves_supports_and_media_conditions_from_css_utilities() {
        let theme = theme();
        let container = resolve_utility("container", &theme).unwrap();
        let outline = resolve_utility("outline-hidden", &theme).unwrap();

        assert_eq!(container.declaration, "width:100%;");
        assert!(container.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Media
                && rule.condition == "(width>=40rem)"
                && rule.declaration == "max-width:40rem;"
        }));
        assert!(outline.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Media
                && rule.condition == "(forced-colors:active)"
                && rule.declaration == "outline:2px solid transparent;outline-offset:2px;"
        }));
    }

    #[test]
    fn resolves_numeric_css_domains_and_default_filter_tokens() {
        let theme = theme();
        let custom_theme = ThemeIndex {
            utilities: shorth_css::parse_utility_definitions(
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
    fn resolves_dynamic_media_container_structural_and_attribute_variants() {
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
    }

    #[test]
    fn resolves_user_utilities_with_the_same_value_domains_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@theme { --color-brand: red; }
@utility tone-* { color: --value(--color-*); }
@utility rail-* { inline-size: --value(--spacing()); }
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
    fn resolves_user_block_variants_with_the_same_slot_syntax_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant pressed { &:active { @slot; } }\n@variant narrow { @media (width < 40rem) { @slot; } }",
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
    }
}
