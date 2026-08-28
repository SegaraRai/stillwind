//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{fmt::Write, sync::OnceLock};

use shorth_css::{CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace};
use shorth_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    pub declaration: String,
    pub selector: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UtilityResolution {
    declaration: String,
    selector: Option<String>,
}

impl UtilityResolution {
    fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorMedia { selector: String, condition: String },
    Media(String),
    Supports(String),
}

const BUILTIN_REGISTRY_CSS: &str = include_str!("../assets/builtins.css");

fn builtin_utilities() -> &'static [CustomUtility] {
    static UTILITIES: OnceLock<Vec<CustomUtility>> = OnceLock::new();
    UTILITIES
        .get_or_init(|| {
            shorth_css::parse_utility_definitions(BUILTIN_REGISTRY_CSS)
                .into_iter()
                .map(|mut utility| {
                    // The registry asset is intentionally formatted for humans. The generated
                    // output must nevertheless be byte-stable across formatter versions.
                    utility.body = canonical_builtin_body(&utility.body);
                    utility
                })
                .collect()
        })
        .as_slice()
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
    Some(ResolvedUtility {
        declaration: if candidate.important {
            important(&resolution.declaration)
        } else {
            resolution.declaration
        },
        selector: resolution.selector,
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
    if let Some(custom) = theme
        .variants
        .iter()
        .rev()
        .find(|variant| variant.name == *value)
        .or_else(|| {
            builtin_variants()
                .iter()
                .rev()
                .find(|variant| variant.name == *value)
        })
    {
        if let Some(application) = resolve_variant_body(&custom.body) {
            return Some(application);
        }
        return Some(VariantApplication::SelectorTemplate(
            custom.selector.clone(),
        ));
    }
    theme
        .token_in(ThemeNamespace::Breakpoint, value)
        .map(|token| VariantApplication::Media(format!("(width >= {})", token.value.value)))
}

fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
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
    if let Some(declaration) = resolve_css_utility(name, modifier, negative, theme) {
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
    resolve_arbitrary(name, negative)
        .or_else(|| resolve_special(name, modifier, negative, theme))
        .map(UtilityResolution::plain)
}

/// Resolves the same `@utility` body language for user CSS and vendored
/// built-ins. `--value(...)` tries its comma-separated value domains in order.
fn resolve_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    matching_utilities(&theme.utilities, lookup)
        .chain(matching_utilities(builtin_utilities(), lookup))
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
    let capture = utility
        .name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_value_domains(
            &body[open + 1..end],
            capture,
            utility.name.starts_with('-'),
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
        .then_some(UtilityResolution {
            declaration: body,
            selector,
        })
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
