//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{collections::HashMap, fmt::Write, sync::OnceLock};

use shorth_css::{CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace, ValueType};
use shorth_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
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
}

impl UtilityResolution {
    fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
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

/// Tailwind spellings intentionally owned by Stillwind's relationship-handle
/// API rather than its Tailwind-compatibility registry.
///
/// Keep this list limited to DOM relationship primitives. `--container-*`
/// theme values and ordinary sizing utilities remain Tailwind-compatible.
const STILLWIND_OWNED_UTILITIES: &[&str] = &[
    "container",
    "@container",
    "@container-normal",
    "group",
    "peer",
];

struct BuiltinUtilityRegistry {
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
        let builtins = shorth_css::parse_utility_definitions(BUILTIN_REGISTRY_CSS)
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>();
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
    if is_stillwind_owned_utility(&candidate) {
        return None;
    }
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
    let additional = resolution
        .additional
        .into_iter()
        .map(|mut target| {
            if candidate.important {
                target.declaration = important(&target.declaration);
                for nested in &mut target.nested {
                    nested.declaration = important(&nested.declaration);
                }
            }
            target
        })
        .collect();
    Some(ResolvedUtility {
        declaration,
        selector: resolution.selector,
        nested,
        additional,
        candidate,
    })
}

#[must_use]
pub fn resolve_variant(variant: &Variant, theme: &ThemeIndex) -> Option<VariantApplication> {
    if is_stillwind_owned_variant(variant) {
        return None;
    }
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

/// Whether a Tailwind candidate intentionally belongs to Stillwind's
/// relationship-handle API instead of the Tailwind compatibility surface.
#[must_use]
pub fn is_stillwind_owned_candidate(raw: &str) -> bool {
    let Some(candidate) = parse_candidate(raw) else {
        return false;
    };
    is_stillwind_owned_utility(&candidate)
        || candidate.variants.iter().any(is_stillwind_owned_variant)
}

fn is_stillwind_owned_utility(candidate: &Candidate) -> bool {
    matches!(
        &candidate.utility,
        Utility::Named { name, .. } if STILLWIND_OWNED_UTILITIES.contains(&name.as_str())
    )
}

fn is_stillwind_owned_variant(variant: &Variant) -> bool {
    matches!(
        &variant.kind,
        VariantKind::Named(name)
            if name.starts_with('@')
                || name == "group"
                || name.starts_with("group-")
                || name == "peer"
                || name.starts_with("peer-")
    )
}

fn resolve_registered_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    let (custom, capture) = find_registered_variant(&theme.variants, value)
        .or_else(|| find_registered_variant(builtin_variants(), value))?;
    let body = substitute_variant_capture(&custom.body, capture)?;
    let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
    resolve_variant_body(&body).or_else(|| {
        substitute_variant_capture(&custom.selector, capture)
            .map(VariantApplication::SelectorTemplate)
    })
}

fn find_registered_variant<'a, 'b>(
    variants: &'a [CustomVariant],
    value: &'b str,
) -> Option<(&'a CustomVariant, Option<&'b str>)> {
    if let Some(variant) = variants.iter().rev().find(|variant| variant.name == value) {
        return Some((variant, None));
    }
    variants.iter().rev().find_map(|variant| {
        let prefix = variant.name.strip_suffix('*')?;
        if prefix.is_empty() {
            return None;
        }
        let capture = value.strip_prefix(prefix)?;
        (!capture.is_empty() && !capture.starts_with('[')).then_some((variant, Some(capture)))
    })
}

fn substitute_variant_capture(value: &str, capture: Option<&str>) -> Option<String> {
    if !value.contains("--value()")
        && !value.contains(r"\*")
        && !value.contains(":root")
        && !value.contains("0n")
    {
        return Some(value.to_owned());
    }
    let capture = capture?.replace('_', " ");
    Some(
        value
            .replace("--value()", &capture)
            .replace(r"\*", &capture)
            .replace(":root", &format!(":{capture}"))
            .replace("0n", &capture),
    )
}

fn resolve_variant_css_placeholders(
    body: &str,
    variant_name: &str,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = variant_name
        .strip_suffix('*')
        .and_then(|prefix| value.strip_prefix(prefix));
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
        resolve_value_domains(domain, capture, false, theme)
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
            resolve_css_utility_body(utility, name, modifier, theme)
                .map(|resolution| (utility_branch_specificity(utility, name), resolution))
        })
        .collect::<Vec<_>>();
    branches.sort_by_key(|(specificity, _)| std::cmp::Reverse(*specificity));
    let (_, mut resolution) = branches.first()?.clone();
    for (_, branch) in branches.into_iter().skip(1) {
        // Overloads with a shared selector form one declaration list. CSS
        // fallbacks authored within one overload intentionally retain their
        // order, while competing overloads cannot overwrite a more-specific
        // property branch.
        if branch.selector == resolution.selector {
            resolution.declaration =
                merge_branch_declarations(&resolution.declaration, &branch.declaration);
            resolution.nested.extend(branch.nested);
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
    let (selector, body) = split_utility_selector(&utility.body)?;
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
    let declaration =
        resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
    let mut modifier_resolved = body.contains("--modifier(")
        || body.contains("--modifier-map(")
        || body.contains("--fraction()");
    let nested = nested
        .into_iter()
        .filter_map(|(at_rule, condition, body)| {
            let uses_modifier = body.contains("--modifier(")
                || body.contains("--modifier-map(")
                || body.contains("--fraction()");
            let declaration =
                resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
            let rule = (!declaration.is_empty()).then_some(NestedUtilityRule {
                at_rule,
                condition,
                declaration,
            });
            modifier_resolved |= uses_modifier && rule.is_some();
            rule
        })
        .collect::<Vec<_>>();
    if modifier.is_some() && consumes_modifier && !modifier_resolved {
        return None;
    }
    (!declaration.is_empty() || !nested.is_empty()).then_some(UtilityResolution {
        declaration,
        selector,
        nested,
        additional: Vec::new(),
    })
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
) -> Option<String> {
    let declarations = split_css_declarations(body);
    let uses_placeholder = declarations.iter().any(|declaration| {
        declaration.contains("--value(")
            || declaration.contains("--value-map(")
            || declaration.contains("--modifier(")
            || declaration.contains("--modifier-map(")
            || declaration.contains("--modifier-none()")
            || declaration.contains("--alpha(")
    });
    let uses_value = declarations
        .iter()
        .any(|declaration| declaration.contains("--value("));
    let requires_modifier = declarations
        .iter()
        .any(|declaration| declaration.contains("--modifier("));
    let mut resolved_placeholder = false;
    let mut resolved_value = false;
    let mut resolved_modifier = false;
    let mut output = String::new();

    for declaration in declarations {
        let contains_placeholder = declaration.contains("--value(")
            || declaration.contains("--value-map(")
            || declaration.contains("--modifier(")
            || declaration.contains("--modifier-map(")
            || declaration.contains("--modifier-none()")
            || declaration.contains("--alpha(");
        let contains_modifier = declaration.contains("--modifier(");
        let contains_value = declaration.contains("--value(");
        if let Some(declaration) =
            resolve_css_placeholders(declaration, utility_name, name, modifier, theme)
            && !declaration.trim().is_empty()
        {
            resolved_placeholder |= contains_placeholder;
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            if output.is_empty() {
                output.push_str(declaration.trim_start());
            } else {
                output.push_str(&declaration);
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
    while let Some(start) = body.find("--value-map(") {
        let open = start + "--value-map".len();
        let end = closing_parenthesis(&body, open)?;
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
        let replacement =
            resolve_value_domains(domains, capture, utility_name.starts_with('-'), theme)?;
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
            resolve_value_domains(domains, Some(&capture), false, theme)?
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
    (!body.contains("--value(")
        && !body.contains("--value-map(")
        && !body.contains("--modifier(")
        && !body.contains("--modifier-map(")
        && !body.contains("--modifier-none()")
        && !body.contains("--alpha("))
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
        if domain == "percentage" {
            return capture
                .and_then(|value| value.strip_suffix('%'))
                .filter(|value| value.parse::<f64>().is_ok())
                .map(|value| format!("{value}%"));
        }
        if domain == "--spacing()" {
            return capture.and_then(|capture| {
                theme_value(ThemeNamespace::Spacing, capture, negative, theme)
            });
        }
        if domain == "--fraction()" {
            return capture.and_then(|capture| fraction_value(capture, negative));
        }
        if domain == "--inset()" {
            return capture.and_then(|capture| inset_value(capture, negative));
        }
        if domain == "--width()" {
            return capture.and_then(|capture| axis_size_value(capture, "vw", negative));
        }
        if domain == "--height()" {
            return capture.and_then(|capture| axis_size_value(capture, "vh", negative));
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
        let expected = ValueType::from_hint(domain.strip_prefix('[')?.strip_suffix(']')?)?;
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (hint, value) = arbitrary.split_once(':').unwrap_or(("", arbitrary));
        let value = decode_arbitrary(value);
        let hint_is_compatible = hint.is_empty()
            || expected == ValueType::Any
            || ValueType::from_hint(hint).is_some_and(|actual| actual == expected);
        (hint_is_compatible && expected.accepts(&value)).then_some(value)
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

fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        // These are CSS colour keywords, not `@theme` tokens. They are
        // nevertheless part of every Tailwind colour utility's public value
        // grammar, including after an application resets `--color-*`.
        (ThemeNamespace::Color, "current") => "currentcolor".to_owned(),
        (ThemeNamespace::Color, "inherit" | "transparent") => value.to_owned(),
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = css_variable(value) => format!("var({value})"),
        _ if theme.token_in(namespace, value).is_some() => {
            format!("var(--{}-{value})", namespace.css_name())
        }
        _ if namespace == ThemeNamespace::Spacing
            && theme.scale(ThemeNamespace::Spacing).is_some() =>
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
        }
        _ => return None,
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn typed_theme_arbitrary(namespace: ThemeNamespace, value: &str) -> Option<String> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = decode_arbitrary(value);
    let types = theme_arbitrary_types(namespace);
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint).is_some_and(|hint| types.contains(&hint));
    (hint_is_compatible && (types.is_empty() || types.iter().any(|kind| kind.accepts(&value))))
        .then_some(value)
}

fn theme_arbitrary_types(namespace: ThemeNamespace) -> &'static [ValueType] {
    match namespace {
        ThemeNamespace::Color => &[ValueType::Color],
        ThemeNamespace::Spacing | ThemeNamespace::Radius | ThemeNamespace::Blur => {
            &[ValueType::Length]
        }
        ThemeNamespace::Leading => &[ValueType::Number, ValueType::Length],
        ThemeNamespace::FontStretch => &[ValueType::Percentage],
        ThemeNamespace::ZIndex => &[ValueType::Integer],
        // These namespaces deliberately remain open until each Tailwind value
        // definition has a corresponding browser value parser. They still
        // require a non-empty arbitrary value through `ValueType::Any`.
        _ => &[],
    }
}

fn fraction_value(value: &str, negative: bool) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| {
        let value = format!("calc({numerator} / {denominator} * 100%)");
        if negative {
            format!("calc({value} * -1)")
        } else {
            value
        }
    })
}

fn inset_value(value: &str, negative: bool) -> Option<String> {
    let value = match value {
        "auto" if !negative => "auto".to_owned(),
        "full" => "100%".to_owned(),
        _ => return None,
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn axis_size_value(value: &str, viewport_axis: &str, negative: bool) -> Option<String> {
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
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
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
        let background = resolve_utility("bg-brand/50", &theme).unwrap();
        assert_eq!(
            background.declaration,
            "background-color:color-mix(in srgb,var(--color-brand) 50%,transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.declaration
                    == "background-color:color-mix(in oklab,var(--color-brand) 50%,transparent);"
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
                .contains("--tw-mask-conic-position:calc(1deg*45);"),
            "{resolved:?}"
        );
        assert!(
            resolve_utility("-mask-conic-45", &theme())
                .expect("negative mask conic angle")
                .declaration
                .contains("--tw-mask-conic-position:calc(1deg*-1*45);")
        );
    }

    #[test]
    fn resolves_gradient_interpolation_modifiers_in_the_css_registry() {
        let default = resolve_utility("bg-conic-45", &theme()).expect("default conic");
        assert!(
            default
                .declaration
                .contains("--tw-gradient-position:from calc(1deg*45) in oklab;"),
            "{default:?}"
        );
        let interpolated =
            resolve_utility("bg-conic-45/increasing", &theme()).expect("interpolated conic");
        assert!(
            interpolated
                .declaration
                .contains("--tw-gradient-position:from calc(1deg*45) in oklch increasing hue;")
        );
    }

    #[test]
    fn resolves_css_defined_color_and_gradient_utilities() {
        let theme = theme();
        assert_eq!(
            resolve_utility("fill-brand/50", &theme)
                .unwrap()
                .declaration,
            "fill:color-mix(in srgb,var(--color-brand) 50%,transparent);"
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
    fn resolves_ambiguous_backgrounds_through_typed_utility_branches() {
        let theme = theme();
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
            "background-color:color-mix(in srgb,var(--color-red-500) 50%,transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.declaration
                    == "background-color:color-mix(in oklab,var(--color-red-500) 50%,transparent);"
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
            rule.at_rule == NestedUtilityAtRule::Media
                && rule.condition == "(forced-colors:active)"
                && rule.declaration == "outline:2px solid transparent;outline-offset:2px;"
        }));
    }

    #[test]
    fn excludes_tailwind_relationship_primitives_owned_by_stillwind() {
        let theme = theme();

        for candidate in [
            "container",
            "@container",
            "@container-normal",
            "group",
            "peer",
            "group-hover:flex",
            "peer-checked:flex",
            "@sm:flex",
        ] {
            assert!(is_stillwind_owned_candidate(candidate), "{candidate}");
        }
        assert!(!is_stillwind_owned_candidate("max-w-lg"));
        assert_eq!(resolve_utility("container", &theme), None);
        assert_eq!(
            resolve_variant(
                &parse_candidate("group-hover:flex").unwrap().variants[0],
                &theme
            ),
            None
        );
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
    fn resolves_compact_spacing_families_with_negative_and_fractional_values() {
        let theme = theme();

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
            "left:calc(calc(1 / 2 * 100%) * -1);"
        );
        assert_eq!(
            resolve_utility("-bottom-full", &theme).unwrap().declaration,
            "bottom:calc(100% * -1);"
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
        assert_eq!(resolve("@sm:flex"), None);
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
            "color:color-mix(in srgb,var(--color-brand) 50%,transparent);"
        );
        assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab,red,red)"
                && rule.declaration
                    == "color:color-mix(in oklab,var(--color-brand) 50%,transparent);"
        }));
        assert!(resolve_utility("text-brand/[length:12px]", &theme()).is_none());
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
            ("-z-73", "z-index:calc(73*-1);"),
            ("border-13", "border-width:calc(13*1px);"),
            ("col-span-full", "grid-column:1/-1;"),
            ("row-auto", "grid-row:auto;"),
            (
                "rounded-tl-full",
                "border-top-left-radius:calc(infinity*1px);",
            ),
            ("touch-pan-left", "--tw-pan-x:pan-left;"),
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
    fn resolves_discrete_tailwind_keywords_without_fixture_mappings() {
        let theme = theme();
        for (candidate, declaration, selector) in [
            (
                "divide-y",
                "border-bottom-width:1px;",
                Some("&>:not(:last-child)"),
            ),
            (
                "divide-dashed",
                "border-style:dashed;",
                Some("&>:not(:last-child)"),
            ),
            ("m-auto", "margin:auto;", None),
            ("mix-blend-color-dodge", "mix-blend-mode:color-dodge;", None),
            ("object-bottom-left", "object-position:left bottom;", None),
            ("outline", "outline-style:solid;outline-width:1px;", None),
            ("ring", "--tw-ring-shadow:0 0 0 1px", None),
            (
                "rounded-se-full",
                "border-start-end-radius:calc(infinity*1px);",
                None,
            ),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
            assert_eq!(resolved.selector.as_deref(), selector, "{candidate}");
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
        for candidate in ["p-[1rem]", "p-[50%]", "border-[3px]", "z-[12]"] {
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
            "z-[1.5]",
            "z-[integer:1.5]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_none(),
                "expected `{candidate}` to be rejected"
            );
        }
    }

    #[test]
    fn resolves_placeholder_colours_as_typed_pseudo_element_utilities() {
        let resolved = resolve_utility("placeholder-current/50", &theme()).unwrap();
        assert_eq!(resolved.selector.as_deref(), Some("&::placeholder"));
        assert!(
            resolved
                .declaration
                .contains("color:color-mix(in oklab,currentcolor 50%,transparent);"),
            "{resolved:#?}"
        );
        assert!(resolve_utility("placeholder-[length:1rem]", &theme()).is_none());
    }

    #[test]
    fn resolves_gradient_direction_and_interpolation_from_css_maps() {
        let resolved = resolve_utility("bg-linear-to-r/oklch", &theme()).unwrap();
        assert!(
            resolved
                .declaration
                .contains("--tw-gradient-position:to right in oklch;")
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
            "filter:blur(0);"
        );
        assert_eq!(
            resolve_utility("perspective-dramatic", &theme)
                .unwrap()
                .declaration,
            "perspective:100px;"
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
            "--tw-translate-y:calc(12px * -1);translate:var(--tw-translate-x) var(--tw-translate-y);"
        );
        assert_eq!(
            resolve_utility("scale-[0.8]", &theme).unwrap().declaration,
            "--tw-scale-x:0.8;--tw-scale-y:0.8;--tw-scale-z:0.8;scale:var(--tw-scale-x) var(--tw-scale-y);"
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
            None
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("narrow-narrow:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::Media("(width < 40rem)".to_owned()))
        );
    }
}
