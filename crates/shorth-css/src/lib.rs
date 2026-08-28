//! CSS-first theme parsing. Lightning CSS validates the stylesheet before shorth
//! extracts its small, source-spanned `@theme` surface.

use std::collections::BTreeMap;

use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use shorth_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

/// Tailwind CSS v4.3.3 Preflight, vendored under its MIT license.
pub const TAILWIND_V4_PREFLIGHT: &str = include_str!("../assets/tailwind-v4-preflight.css");

const TAILWIND_V4_THEME: &str = include_str!("../assets/tailwind-v4-theme.css");

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    pub name: Spanned<String>,
    pub value: Spanned<String>,
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
}

impl ThemeIndex {
    #[must_use]
    pub fn token(&self, namespace: &str, name: &str) -> Option<&ThemeToken> {
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
        let Some(close_relative) = css[open + 1..].find('}') else {
            return Err(ThemeError::UnterminatedTheme);
        };
        let close = open + 1 + close_relative;

        let body = &css[open + 1..close];
        let mut body_cursor = 0;
        for declaration in body.split_inclusive(';') {
            let declaration_start = open + 1 + body_cursor;
            body_cursor += declaration.len();
            let Some((raw_name, raw_value)) = declaration.trim_end_matches(';').split_once(':')
            else {
                continue;
            };
            let name = raw_name.trim();
            let value = raw_value.trim();
            if !name.starts_with("--") || value.is_empty() {
                continue;
            }

            let name_start = declaration_start + declaration.find(name).unwrap_or_default();
            let value_start = declaration_start + declaration.find(value).unwrap_or_default();
            tokens.push(ThemeToken {
                name: Spanned::new(
                    name.to_owned(),
                    Span::new(file, offset(name_start), offset(name_start + name.len())),
                ),
                value: Spanned::new(
                    value.to_owned(),
                    Span::new(file, offset(value_start), offset(value_start + value.len())),
                ),
            });
        }

        cursor = close + 1;
    }

    Ok(ThemeIndex {
        tokens,
        custom_css: String::new(),
        utilities: Vec::new(),
        variants: Vec::new(),
    })
}

fn strip_framework_directives(css: &str) -> String {
    let css = strip_blocks(css, "@theme");
    let css = strip_blocks(&css, "@utility");
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
    extract_named_blocks(css, "@utility")
        .into_iter()
        .filter_map(|(header, body)| {
            let name = header.split_whitespace().next()?.to_owned();
            Some(CustomUtility { name, body })
        })
        .collect()
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
    css.lines()
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
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
