//! CSS-first theme parsing. Lightning CSS validates the stylesheet before shorth
//! extracts its small, source-spanned `@theme` surface.

use std::{collections::BTreeMap, fmt::Write};

use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use shorth_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

/// Tailwind CSS v4.3.3 Preflight, vendored under its MIT license.
pub const TAILWIND_V4_PREFLIGHT: &str = include_str!("../assets/tailwind-v4-preflight.css");

const TAILWIND_V4_THEME: &str = include_str!("../assets/tailwind-v4-theme.css");
const TAILWIND_V4_PROPERTIES: &str = include_str!("../assets/tailwind-v4-properties.css");

/// Tailwind v4 theme namespaces that participate in built-in utility and
/// variant resolution. Unknown custom properties remain available as authored
/// CSS but never accidentally create a utility.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ThemeNamespace {
    Animate,
    AspectRatio,
    Blur,
    Breakpoint,
    Color,
    Container,
    DropShadow,
    Ease,
    Font,
    FontWeight,
    InsetShadow,
    Leading,
    Perspective,
    Radius,
    Shadow,
    Spacing,
    Text,
    TextShadow,
    Tracking,
    ZIndex,
    Duration,
}

impl ThemeNamespace {
    #[must_use]
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::Animate => "animate",
            Self::AspectRatio => "aspect-ratio",
            Self::Blur => "blur",
            Self::Breakpoint => "breakpoint",
            Self::Color => "color",
            Self::Container => "container",
            Self::DropShadow => "drop-shadow",
            Self::Ease => "ease",
            Self::Font => "font",
            Self::FontWeight => "font-weight",
            Self::InsetShadow => "inset-shadow",
            Self::Leading => "leading",
            Self::Perspective => "perspective",
            Self::Radius => "radius",
            Self::Shadow => "shadow",
            Self::Spacing => "spacing",
            Self::Text => "text",
            Self::TextShadow => "text-shadow",
            Self::Tracking => "tracking",
            Self::ZIndex => "z-index",
            Self::Duration => "duration",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::AspectRatio,
            Self::FontWeight,
            Self::InsetShadow,
            Self::DropShadow,
            Self::TextShadow,
            Self::Breakpoint,
            Self::Perspective,
            Self::Container,
            Self::Tracking,
            Self::Duration,
            Self::Animate,
            Self::Color,
            Self::Spacing,
            Self::Radius,
            Self::Shadow,
            Self::Leading,
            Self::Font,
            Self::Text,
            Self::Blur,
            Self::Ease,
            Self::ZIndex,
        ]
        .into_iter()
        .find(|namespace| namespace.css_name() == value)
    }
}

/// The semantic identity of one custom property in a `@theme` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeTokenKind {
    /// `--spacing` is a scalar used by numeric spacing utilities.
    Scale(ThemeNamespace),
    /// A named token such as `--color-accent` or `--breakpoint-md`.
    Token {
        namespace: ThemeNamespace,
        name: String,
    },
    /// A property that Stillwind emits unchanged but does not use for built-ins.
    Custom,
}

/// Browser-ready Preflight with Tailwind's build-time `--theme()` calls
/// lowered to standard `var()` calls.
#[must_use]
pub fn tailwind_v4_preflight() -> String {
    TAILWIND_V4_PREFLIGHT.replace("--theme(", "var(")
}

/// The standard animation keyframes paired with the vendored default theme.
#[must_use]
pub fn tailwind_v4_keyframes() -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = TAILWIND_V4_THEME[cursor..].find("@keyframes") {
        let start = cursor + relative;
        let Some(open_relative) = TAILWIND_V4_THEME[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        let mut depth = 1_u32;
        let mut close = open;
        for (offset, character) in TAILWIND_V4_THEME[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        output.push_str(&TAILWIND_V4_THEME[start..=close]);
        cursor = close + 1;
    }
    output
}

/// Typed custom-property registrations used by Tailwind-compatible utilities.
#[must_use]
pub const fn tailwind_v4_properties() -> &'static str {
    TAILWIND_V4_PROPERTIES
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    pub name: Spanned<String>,
    pub value: Spanned<String>,
    pub kind: ThemeTokenKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeIndex {
    pub tokens: Vec<ThemeToken>,
    pub custom_css: String,
    pub utilities: Vec<CustomUtility>,
    pub variants: Vec<CustomVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    pub name: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomVariant {
    pub name: String,
    pub selector: String,
    /// Full `@variant` body. Empty for the `@custom-variant` selector shorthand.
    pub body: String,
}

/// Parses `@utility` definitions without requiring a complete theme stylesheet.
/// This is used for vendored built-in registries as well as tooling.
#[must_use]
pub fn parse_utility_definitions(css: &str) -> Vec<CustomUtility> {
    extract_custom_utilities(css)
}

/// Parses `@custom-variant` and block `@variant` definitions without
/// requiring a complete theme stylesheet. This mirrors the utility registry
/// entry point so tools can build a registry once and share it at runtime.
#[must_use]
pub fn parse_variant_definitions(css: &str) -> Vec<CustomVariant> {
    extract_custom_variants(css)
}

impl ThemeIndex {
    #[must_use]
    pub fn token_in(&self, namespace: ThemeNamespace, name: &str) -> Option<&ThemeToken> {
        self.active_tokens().into_iter().find(|token| {
            token.kind
                == (ThemeTokenKind::Token {
                    namespace,
                    name: name.to_owned(),
                })
        })
    }

    #[must_use]
    pub fn tokens_in(&self, namespace: ThemeNamespace) -> Vec<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .filter(|token| {
                matches!(
                    token.kind,
                    ThemeTokenKind::Token {
                        namespace: token_namespace,
                        ..
                    } if token_namespace == namespace
                )
            })
            .collect()
    }

    #[must_use]
    pub fn scale(&self, namespace: ThemeNamespace) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.kind == ThemeTokenKind::Scale(namespace))
    }

    #[must_use]
    pub fn token(&self, namespace: &str, name: &str) -> Option<&ThemeToken> {
        if let Some(namespace) = ThemeNamespace::parse(namespace) {
            return self.token_in(namespace, name);
        }
        let expected = format!("--{namespace}-{name}");
        self.active_tokens()
            .into_iter()
            .find(|token| token.name.value == expected)
    }

    #[must_use]
    pub fn value(&self, namespace: &str, name: &str) -> Option<&str> {
        self.token(namespace, name)
            .map(|token| token.value.value.as_str())
    }

    #[must_use]
    pub fn custom_property(&self, name: &str) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.name.value == name)
    }

    /// Returns the effective CSS custom properties after v4 namespace resets.
    #[must_use]
    pub fn active_tokens(&self) -> Vec<&ThemeToken> {
        let mut values = BTreeMap::new();
        let mut clear_all = false;
        for token in &self.tokens {
            if token.name.value == "--*" && token.value.value == "initial" {
                values.clear();
                clear_all = true;
                continue;
            }
            if let Some(namespace) = token
                .name
                .value
                .strip_prefix("--")
                .and_then(|value| value.strip_suffix("-*"))
                && token.value.value == "initial"
            {
                values.retain(|name: &String, _| !name.starts_with(&format!("--{namespace}-")));
                continue;
            }
            if clear_all || token.name.value.starts_with("--") {
                values.insert(token.name.value.clone(), token);
            }
        }
        values.into_values().collect()
    }

    /// CSS outside of `@theme` blocks, retained verbatim for custom properties,
    /// theme selectors, layers, and ordinary authored CSS.
    #[must_use]
    pub fn custom_css(&self) -> &str {
        &self.custom_css
    }

    #[must_use]
    pub fn utility(&self, name: &str) -> Option<&CustomUtility> {
        self.utilities
            .iter()
            .rev()
            .find(|utility| utility.name == name)
    }

    #[must_use]
    pub fn variant(&self, name: &str) -> Option<&CustomVariant> {
        self.variants
            .iter()
            .rev()
            .find(|variant| variant.name == name)
    }
}

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("CSS parse error: {0}")]
    Parse(String),
    #[error("unterminated @theme block")]
    UnterminatedTheme,
}

/// Parses and indexes source-spanned `@theme` token declarations.
///
/// # Errors
///
/// Returns an error when Lightning CSS rejects the stylesheet or a discovered
/// `@theme` block has no closing brace.
pub fn parse_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    StyleSheet::parse(source.text(), ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.to_string()))?;

    let mut theme = extract_theme_tokens(source.id(), source.text())?;
    theme.custom_css = strip_framework_directives(source.text());
    theme.utilities = extract_custom_utilities(source.text());
    theme.variants = extract_custom_variants(source.text());
    Ok(theme)
}

/// Parses the Tailwind v4-compatible theme used by Stillwind's compiler.
///
/// The vendored Tailwind defaults form the base theme, then the project CSS
/// extends, overrides, or clears namespaces with the normal `@theme` syntax.
///
/// # Errors
///
/// Returns an error when the project stylesheet is invalid or contains an
/// unterminated `@theme` block.
pub fn parse_tailwind_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    StyleSheet::parse(source.text(), ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.to_string()))?;
    let mut tokens = extract_theme_tokens(FileId(0), TAILWIND_V4_THEME)?.tokens;
    tokens.extend(extract_theme_tokens(source.id(), source.text())?.tokens);
    Ok(ThemeIndex {
        tokens,
        custom_css: strip_framework_directives(source.text()),
        utilities: extract_custom_utilities(source.text()),
        variants: extract_custom_variants(source.text()),
    })
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("shorth source files must be smaller than 4 GiB")
}

fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeIndex, ThemeError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while let Some(theme_relative) = css[cursor..].find("@theme") {
        let theme_start = cursor + theme_relative;
        let Some(open_relative) = css[theme_start..].find('{') else {
            return Err(ThemeError::UnterminatedTheme);
        };
        let open = theme_start + open_relative;
        let Some(close) = matching_brace(css, open) else {
            return Err(ThemeError::UnterminatedTheme);
        };

        tokens.extend(extract_theme_declarations(
            file,
            &css[open + 1..close],
            open + 1,
        ));

        cursor = close + 1;
    }

    Ok(ThemeIndex {
        tokens,
        custom_css: String::new(),
        utilities: Vec::new(),
        variants: Vec::new(),
    })
}

fn extract_theme_declarations(file: FileId, body: &str, body_start: usize) -> Vec<ThemeToken> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        cursor += body[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(body[cursor..].len());
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += end + 4;
            continue;
        }
        if body[cursor..].starts_with('@') {
            let Some((offset, delimiter)) = body[cursor..]
                .char_indices()
                .find(|(_, character)| matches!(character, '{' | ';'))
            else {
                break;
            };
            if delimiter == ';' {
                cursor += offset + 1;
            } else if let Some(close) = matching_brace(body, cursor + offset) {
                cursor = close + 1;
            } else {
                break;
            }
            continue;
        }

        let declaration_start = cursor;
        let Some(end) = body[cursor..].find(';') else {
            break;
        };
        let declaration = &body[cursor..cursor + end];
        cursor += end + 1;
        let Some((raw_name, raw_value)) = declaration.split_once(':') else {
            continue;
        };
        let name = raw_name.trim();
        let value = raw_value.trim();
        if !name.starts_with("--") || value.is_empty() {
            continue;
        }
        let name_start =
            body_start + declaration_start + declaration.find(name).unwrap_or_default();
        let value_start =
            body_start + declaration_start + declaration.find(value).unwrap_or_default();
        tokens.push(ThemeToken {
            name: Spanned::new(
                name.to_owned(),
                Span::new(file, offset(name_start), offset(name_start + name.len())),
            ),
            value: Spanned::new(
                value.to_owned(),
                Span::new(file, offset(value_start), offset(value_start + value.len())),
            ),
            kind: classify_theme_token(name),
        });
    }
    tokens
}

fn matching_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in value[open..].char_indices() {
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

fn classify_theme_token(name: &str) -> ThemeTokenKind {
    let Some(name) = name.strip_prefix("--") else {
        return ThemeTokenKind::Custom;
    };
    for namespace in [
        ThemeNamespace::AspectRatio,
        ThemeNamespace::FontWeight,
        ThemeNamespace::InsetShadow,
        ThemeNamespace::DropShadow,
        ThemeNamespace::TextShadow,
        ThemeNamespace::Breakpoint,
        ThemeNamespace::Perspective,
        ThemeNamespace::Container,
        ThemeNamespace::Tracking,
        ThemeNamespace::Duration,
        ThemeNamespace::Animate,
        ThemeNamespace::Color,
        ThemeNamespace::Spacing,
        ThemeNamespace::Radius,
        ThemeNamespace::Shadow,
        ThemeNamespace::Leading,
        ThemeNamespace::Font,
        ThemeNamespace::Text,
        ThemeNamespace::Blur,
        ThemeNamespace::Ease,
        ThemeNamespace::ZIndex,
    ] {
        let namespace_name = namespace.css_name();
        if name == namespace_name {
            return ThemeTokenKind::Scale(namespace);
        }
        if let Some(token_name) = name.strip_prefix(&format!("{namespace_name}-")) {
            return ThemeTokenKind::Token {
                namespace,
                name: token_name.to_owned(),
            };
        }
    }
    ThemeTokenKind::Custom
}

fn strip_framework_directives(css: &str) -> String {
    let css = strip_blocks(css, "@theme");
    let css = strip_blocks(&css, "@utility");
    let css = strip_blocks(&css, "@variant");
    css.lines()
        .filter(|line| !line.trim_start().starts_with("@custom-variant"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_blocks(css: &str, directive: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        output.push_str(&css[cursor..start]);
        let Some(open_relative) = css[start..].find('{') else {
            output.push_str(&css[start..]);
            break;
        };
        let open = start + open_relative;
        let mut depth = 1_u32;
        let mut close = open + 1;
        for (offset, character) in css[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        cursor = close.saturating_add(1);
    }
    if cursor < css.len() {
        output.push_str(&css[cursor..]);
    }
    output
}

fn extract_custom_utilities(css: &str) -> Vec<CustomUtility> {
    let css = strip_css_comments(css);
    let mut utilities = extract_named_blocks(&css, "@utility")
        .into_iter()
        .filter_map(|(header, body)| {
            if header.starts_with("-family") {
                return None;
            }
            let name = header.split_whitespace().next()?.to_owned();
            Some(CustomUtility { name, body })
        })
        .collect::<Vec<_>>();
    utilities.extend(extract_utility_families(&css));
    utilities
}

/// Expands Stillwind's compact built-in utility-family syntax.
///
/// ```css
/// @utility-family spacing negative {
///   inset: inset;
///   inset-x: inset-inline;
/// }
/// ```
///
/// The example declares `inset-*`, `inset-x-*`, and their negative forms,
/// all backed by the same typed Tailwind spacing value domain. It keeps a
/// family definition reviewable without enumerating its default values.
fn extract_utility_families(css: &str) -> Vec<CustomUtility> {
    extract_named_blocks(css, "@utility-family")
        .into_iter()
        .flat_map(|(header, body)| {
            let mut header = header.split_whitespace();
            let domain = header.next();
            let negative = header.any(|flag| flag == "negative");
            let Some(value) = domain.and_then(utility_family_value) else {
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
                        let value = value.to_owned();
                        move |name| {
                            let mut body = String::new();
                            for property in &properties {
                                write!(body, "{property}:{value};")
                                    .expect("writing to a String cannot fail");
                            }
                            let mut utilities = vec![CustomUtility {
                                name: format!("{name}-*"),
                                body: body.clone(),
                            }];
                            if negative {
                                utilities.push(CustomUtility {
                                    name: format!("-{name}-*"),
                                    body,
                                });
                            }
                            utilities
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn utility_family_value(domain: &str) -> Option<&'static str> {
    match domain {
        "spacing" => Some("--value(--spacing-*, --spacing(), [length])"),
        "spacing-fraction" => Some("--value(--spacing-*, --spacing(), --fraction(), [length])"),
        "inset" => Some("--value(--spacing-*, --spacing(), --fraction(), --inset(), [length])"),
        "length" => {
            Some("--value(--spacing-*, --spacing(), --container-*, --fraction(), [length])")
        }
        "width" => Some(
            "--value(--spacing-*, --spacing(), --container-*, --fraction(), --width(), [length])",
        ),
        "height" => Some(
            "--value(--spacing-*, --spacing(), --container-*, --fraction(), --height(), [length])",
        ),
        "integer" => Some("--value(integer, [integer])"),
        "number" => Some("--value(number, [number])"),
        "color" => Some("--value(--color-*, current, inherit, transparent, [color])"),
        _ => None,
    }
}

fn strip_css_comments(css: &str) -> String {
    let mut output = String::new();
    let mut remaining = css;
    while let Some(start) = remaining.find("/*") {
        output.push_str(&remaining[..start]);
        let Some(end) = remaining[start + 2..].find("*/") else {
            break;
        };
        remaining = &remaining[start + 2 + end + 2..];
    }
    output.push_str(remaining);
    output
}

fn extract_named_blocks(css: &str, directive: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        let Some(open_relative) = css[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        let header = css[start + directive.len()..open].trim().to_owned();
        let mut depth = 1_u32;
        let mut close = open;
        for (offset, character) in css[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        blocks.push((header, css[open + 1..close].trim().to_owned()));
        cursor = close + 1;
    }
    blocks
}

fn extract_custom_variants(css: &str) -> Vec<CustomVariant> {
    let css = strip_css_comments(css);
    let mut variants = css
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let value = line.strip_prefix("@custom-variant ")?.trim_end_matches(';');
            let (name, selector) = value.split_once(' ')?;
            Some(CustomVariant {
                name: name.to_owned(),
                selector: selector
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .to_owned(),
                body: String::new(),
            })
        })
        .collect::<Vec<_>>();
    variants.extend(
        extract_named_blocks(&css, "@variant")
            .into_iter()
            .filter_map(|(header, body)| {
                let name = header.split_whitespace().next()?.to_owned();
                Some(CustomVariant {
                    name,
                    selector: String::new(),
                    body,
                })
            }),
    );
    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_compact_utility_families_without_enumerating_values() {
        let utilities = parse_utility_definitions(
            "@utility-family spacing negative { inset: inset; inset-x: inset-inline; }",
        );

        assert_eq!(
            utilities,
            vec![
                CustomUtility {
                    name: "inset-*".to_owned(),
                    body: "inset:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                },
                CustomUtility {
                    name: "-inset-*".to_owned(),
                    body: "inset:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                },
                CustomUtility {
                    name: "inset-x-*".to_owned(),
                    body: "inset-inline:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                },
                CustomUtility {
                    name: "-inset-x-*".to_owned(),
                    body: "inset-inline:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn extracts_spanned_theme_tokens_after_lightningcss_validates_css() {
        let source = SourceFile::new(
            FileId(7),
            "shorth.css",
            "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; }",
        );

        let theme = parse_theme(&source).unwrap();

        assert_eq!(theme.tokens.len(), 2);
        assert_eq!(theme.tokens[0].name.value, "--color-accent");
        assert_eq!(source.slice(theme.tokens[1].value.span).unwrap(), "1rem");
        assert_eq!(theme.value("color", "accent"), Some("oklch(.6 .2 250)"));
        assert!(theme.token("spacing", "missing").is_none());
    }

    #[test]
    fn applies_tailwind_defaults_and_namespace_resets() {
        let source = SourceFile::new(
            FileId(7),
            "shorth.css",
            "@theme { --color-*: initial; --color-brand: oklch(.6 .2 250); }",
        );

        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(theme.value("color", "brand"), Some("oklch(.6 .2 250)"));
        assert!(theme.token("color", "red-500").is_none());
        assert!(
            theme
                .active_tokens()
                .iter()
                .any(|token| token.name.value == "--spacing")
        );
    }

    #[test]
    fn retains_default_tokens_after_nested_theme_keyframes() {
        let theme = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@theme { --color-brand: rebeccapurple; }",
        ))
        .unwrap();

        assert!(theme.token_in(ThemeNamespace::Blur, "sm").is_some());
        assert!(
            theme
                .token_in(ThemeNamespace::Perspective, "normal")
                .is_some()
        );
        assert!(theme.token_in(ThemeNamespace::Color, "brand").is_some());
    }

    #[test]
    fn classifies_tailwind_theme_namespaces_without_losing_source_spans() {
        let source = SourceFile::new(
            FileId(7),
            "shorth.css",
            "@theme { --spacing: .25rem; --color-accent: oklch(.6 .2 250); --vendor-value: 1; }",
        );

        let theme = parse_theme(&source).unwrap();
        let color = theme.token_in(ThemeNamespace::Color, "accent").unwrap();

        assert_eq!(
            color.kind,
            ThemeTokenKind::Token {
                namespace: ThemeNamespace::Color,
                name: "accent".to_owned(),
            }
        );
        assert_eq!(source.slice(color.name.span).unwrap(), "--color-accent");
        assert_eq!(
            theme.scale(ThemeNamespace::Spacing).unwrap().value.value,
            ".25rem"
        );
        assert_eq!(theme.tokens_in(ThemeNamespace::Color).len(), 1);
        assert_eq!(theme.tokens[2].kind, ThemeTokenKind::Custom);
    }

    #[test]
    fn parses_block_variants_and_omits_them_from_authored_output() {
        let source = SourceFile::new(
            FileId(7),
            "shorth.css",
            "@variant pressed { &:active { @slot; } }\n.card { color: red; }",
        );

        let theme = parse_theme(&source).unwrap();

        assert_eq!(
            theme.variant("pressed").unwrap().body,
            "&:active { @slot; }"
        );
        assert!(theme.custom_css().contains(".card"));
        assert!(!theme.custom_css().contains("@variant"));
    }
}
