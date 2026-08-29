//! CSS-first theme parsing. Lightning CSS validates the stylesheet before shorth
//! extracts its small, source-spanned `@theme` surface.

use std::{collections::BTreeMap, fmt::Write};

use lightningcss::{
    properties::{background::BackgroundSize, border::BorderSideWidth, box_shadow::BoxShadow},
    stylesheet::{ParserOptions, StyleSheet},
    traits::Parse,
    values::{
        color::CssColor,
        image::Image,
        length::{Length, LengthPercentage},
        number::CSSNumber,
        percentage::Percentage,
        position::Position,
        url::Url,
    },
};
use shorth_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

/// Tailwind CSS v4.3.3 Preflight, vendored under its MIT license.
pub const TAILWIND_V4_PREFLIGHT: &str = include_str!("../assets/tailwind-v4-preflight.css");

const TAILWIND_V4_THEME: &str = include_str!("../assets/tailwind-v4-theme.css");
const TAILWIND_V4_PROPERTIES: &str = include_str!("../assets/tailwind-v4-properties.css");

/// A CSS value type accepted by a typed `--value([type])` domain.
///
/// The variants intentionally model CSS value definitions, rather than the
/// utility that happens to consume them. This keeps `p-*`, `border-*`, and
/// `z-*` distinct even when their source spelling contains the same digits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Any,
    Integer,
    Number,
    Percentage,
    Length,
    LengthPercentage,
    LineWidth,
    ZIndex,
    Color,
    Image,
    Url,
    Position,
    BackgroundSize,
    Shadow,
}

impl ValueType {
    /// Parses the public arbitrary-value hint used inside `[hint:value]`.
    #[must_use]
    pub fn from_hint(value: &str) -> Option<Self> {
        match value {
            "*" => Some(Self::Any),
            "integer" => Some(Self::Integer),
            "number" => Some(Self::Number),
            "percentage" => Some(Self::Percentage),
            "length" => Some(Self::Length),
            "length-percentage" => Some(Self::LengthPercentage),
            "line-width" => Some(Self::LineWidth),
            "z-index" => Some(Self::ZIndex),
            "color" => Some(Self::Color),
            "image" => Some(Self::Image),
            "url" => Some(Self::Url),
            "position" => Some(Self::Position),
            "bg-size" => Some(Self::BackgroundSize),
            "shadow" => Some(Self::Shadow),
            _ => None,
        }
    }

    /// Validates a decoded arbitrary value against its CSS value definition.
    ///
    /// Lightning CSS owns the grammar for browser CSS types. The small
    /// integer check is layered on top because CSS exposes `<number>`, while
    /// Stillwind's utility grammar needs the narrower integral subset.
    #[must_use]
    pub fn accepts(self, value: &str) -> bool {
        if is_css_wide_keyword(value) {
            return true;
        }
        match self {
            Self::Any => !value.trim().is_empty(),
            Self::Integer => {
                CSSNumber::parse_string(value).is_ok_and(|number| number.fract() == 0.0)
            }
            Self::Number => CSSNumber::parse_string(value).is_ok(),
            Self::Percentage => Percentage::parse_string(value).is_ok(),
            Self::Length => Length::parse_string(value).is_ok(),
            Self::LengthPercentage => LengthPercentage::parse_string(value).is_ok(),
            Self::LineWidth => BorderSideWidth::parse_string(value).is_ok(),
            Self::ZIndex => value.trim() == "auto" || Self::Integer.accepts(value),
            Self::Color => CssColor::parse_string(value).is_ok(),
            Self::Image => Image::parse_string(value).is_ok(),
            Self::Url => Url::parse_string(value).is_ok(),
            Self::Position => Position::parse_string(value).is_ok(),
            Self::BackgroundSize => BackgroundSize::parse_string(value).is_ok(),
            Self::Shadow => BoxShadow::parse_string(value).is_ok(),
        }
    }

    /// Returns whether an arbitrary-value hint is a valid narrower spelling
    /// for this CSS value type. For example, `<length>` and `<percentage>`
    /// may annotate a `<length-percentage>` value, while `<integer>` may
    /// annotate a `z-index` value.
    #[must_use]
    pub fn accepts_hint(self, actual: Self) -> bool {
        self == Self::Any
            || self == actual
            || matches!(
                (self, actual),
                (Self::LengthPercentage, Self::Length | Self::Percentage)
                    | (Self::LineWidth, Self::Length)
                    | (Self::ZIndex, Self::Integer)
            )
    }
}

fn is_css_wide_keyword(value: &str) -> bool {
    matches!(
        value.trim(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer"
    )
}

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
    FontStretch,
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
            Self::FontStretch => "font-stretch",
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
            Self::FontStretch,
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

/// Returns only the vendored keyframes named by the selected animation theme
/// tokens. Keeping this beside the theme source makes tree-shaking a semantic
/// property of the registry rather than a fixture-specific post-process.
#[must_use]
pub fn tailwind_v4_keyframes_for(names: &[String]) -> String {
    let mut output = String::new();
    for name in names {
        let needle = format!("@keyframes {name}");
        let Some(start) = TAILWIND_V4_THEME.find(&needle) else {
            continue;
        };
        let Some(open_relative) = TAILWIND_V4_THEME[start..].find('{') else {
            continue;
        };
        let open = start + open_relative;
        let Some(close) = matching_brace(TAILWIND_V4_THEME, open) else {
            continue;
        };
        output.push_str(&TAILWIND_V4_THEME[start..=close]);
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
    /// Framework layers requested by the authored Tailwind-compatible imports.
    ///
    /// A stylesheet without a framework import retains the historical full
    /// Stillwind baseline. Once an official Tailwind import is present, its
    /// selected entry points are authoritative.
    pub imports: FrameworkImports,
}

/// The Tailwind entry points selected by an authored `@import` directive.
/// Layer names are retained rather than inferred during rendering so a custom
/// `layer(...)` placement remains observable in the generated stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkImports {
    pub theme_layer: Option<String>,
    pub base_layer: Option<String>,
    pub utilities_layer: Option<String>,
    pub properties: bool,
}

impl Default for FrameworkImports {
    fn default() -> Self {
        Self {
            theme_layer: Some("theme".to_owned()),
            base_layer: Some("base".to_owned()),
            utilities_layer: Some("utilities".to_owned()),
            properties: true,
        }
    }
}

impl FrameworkImports {
    fn explicit() -> Self {
        Self {
            theme_layer: None,
            base_layer: None,
            utilities_layer: None,
            properties: false,
        }
    }

    #[must_use]
    pub fn has_any(&self) -> bool {
        self.theme_layer.is_some() || self.base_layer.is_some() || self.utilities_layer.is_some()
    }
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

    /// Selects the effective tokens required by emitted declarations. Project
    /// tokens are retained as authored; vendored defaults are tree-shaken and
    /// their `var()` dependencies are followed transitively.
    #[must_use]
    pub fn tokens_required_by(&self, css: &str, include_preflight: bool) -> Vec<&ThemeToken> {
        let tokens = self.active_tokens();
        let mut required = custom_property_references(css);
        if include_preflight {
            required.extend(custom_property_references(&tailwind_v4_preflight()));
        }
        for token in &tokens {
            if token.name.span.file != FileId(0) {
                required.insert(token.name.value.clone());
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for token in &tokens {
                if required.contains(&token.name.value) {
                    for dependency in custom_property_references(&token.value.value) {
                        changed |= required.insert(dependency);
                    }
                }
            }
        }
        tokens
            .into_iter()
            .filter(|token| required.contains(&token.name.value))
            .collect()
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

fn custom_property_references(css: &str) -> std::collections::BTreeSet<String> {
    let mut references = std::collections::BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("--") {
        let start = cursor + relative;
        let end = css[start..]
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .last()
            .map_or(start + 2, |(offset, character)| {
                start + offset + character.len_utf8()
            });
        if end > start + 2 {
            references.insert(css[start..end].to_owned());
        }
        cursor = end.max(start + 2);
    }
    references
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
        imports: framework_imports(source.text()),
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
        imports: FrameworkImports::default(),
    })
}

/// Extracts only Stillwind's compatible Tailwind entry points.  This is kept
/// independent from fixtures: it understands ordinary CSS import syntax and
/// ignores all non-framework imports for the browser to resolve.
fn framework_imports(css: &str) -> FrameworkImports {
    let imports = css
        .split('@')
        .filter_map(|segment| segment.strip_prefix("import "))
        .filter_map(|segment| segment.split_once(';').map(|(rule, _)| rule));
    let mut found = false;
    let mut output = FrameworkImports::explicit();

    for rule in imports {
        let rule = rule.trim();
        let Some((path, tail)) = parse_import_path(rule) else {
            continue;
        };
        let layer = import_layer(tail);
        match path {
            "tailwindcss" => {
                found = true;
                output.theme_layer = Some(layer.clone().unwrap_or_else(|| "theme".to_owned()));
                output.base_layer = Some(layer.clone().unwrap_or_else(|| "base".to_owned()));
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
                output.properties = false;
            }
            "tailwindcss/theme" => {
                found = true;
                output.theme_layer = Some(layer.unwrap_or_else(|| "theme".to_owned()));
            }
            "tailwindcss/preflight" => {
                found = true;
                output.base_layer = Some(layer.unwrap_or_else(|| "base".to_owned()));
            }
            "tailwindcss/utilities" => {
                found = true;
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
            }
            _ => {}
        }
    }

    if found {
        output
    } else {
        FrameworkImports::default()
    }
}

fn parse_import_path(rule: &str) -> Option<(&str, &str)> {
    let quote = rule.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let end = rule[1..].find(quote)? + 1;
    Some((&rule[1..end], &rule[end + 1..]))
}

fn import_layer(tail: &str) -> Option<String> {
    let start = tail.find("layer(")? + "layer(".len();
    let end = tail[start..].find(')')? + start;
    let layer = tail[start..end].trim();
    (!layer.is_empty()).then(|| layer.to_owned())
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
        ThemeNamespace::FontStretch,
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
            if domain == Some("mask-stop") {
                return expand_mask_stop_family(&body);
            }
            if domain == Some("mask-radial-stop") {
                return expand_mask_radial_stop_family(&body);
            }
            if domain == Some("color-alpha-default") {
                return expand_color_alpha_default_family(&body);
            }
            if domain == Some("color-alpha") {
                return expand_color_alpha_family(&body);
            }
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

/// Expands colour-with-alpha utility roots without a palette enumeration.
/// The same source DSL emits Tailwind's sRGB fallback and its Oklab upgrade;
/// individual utilities therefore cannot accidentally drift in colour-space
/// behaviour.
fn expand_color_alpha_family(body: &str) -> Vec<CustomUtility> {
    body.split(';')
        .filter_map(|entry| {
            let (names, properties) = entry.split_once(':')?;
            let properties = properties
                .split(',')
                .map(str::trim)
                .filter(|property| !property.is_empty())
                .collect::<Vec<_>>();
            (!properties.is_empty()).then_some((names, properties))
        })
        .flat_map(|(names, properties)| {
            names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(move |name| {
                    let mut body = String::new();
                    for property in &properties {
                        write!(
                            body,
                            "{property}:color-mix(in srgb,--value(--color-*,[color]) --alpha(--modifier()),transparent);"
                        )
                        .expect("writing to a String cannot fail");
                    }
                    body.push_str("@supports (color:color-mix(in lab,red,red)){");
                    for property in &properties {
                        write!(
                            body,
                            "{property}:color-mix(in oklab,--value(--color-*,[color]) --alpha(--modifier()),transparent);"
                        )
                        .expect("writing to a String cannot fail");
                    }
                    body.push('}');
                    CustomUtility {
                        name: format!("{name}-*"),
                        body,
                    }
                })
        })
        .collect()
}

/// Expands bare colour utilities such as `border/50` without inventing a
/// default-palette table. The value is always `currentcolor`; a slash modifier
/// is required by the generated body and is validated by the shared
/// `--modifier(...)` resolver.
fn expand_color_alpha_default_family(body: &str) -> Vec<CustomUtility> {
    body.split(';')
        .filter_map(|entry| {
            let (names, properties) = entry.split_once(':')?;
            let properties = properties
                .split(',')
                .map(str::trim)
                .filter(|property| !property.is_empty())
                .collect::<Vec<_>>();
            (!properties.is_empty()).then_some((names, properties))
        })
        .flat_map(|(names, properties)| {
            names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(move |name| {
                    let mut body = String::new();
                    for property in &properties {
                        write!(
                            body,
                            "{property}:color-mix(in oklab,currentcolor --alpha(--modifier()),transparent);"
                        )
                        .expect("writing to a String cannot fail");
                    }
                    CustomUtility {
                        name: name.to_owned(),
                        body,
                    }
                })
        })
        .collect()
}

/// Expands composable edge-mask stop utilities. The CSS source owns the
/// utility roots and the affected edges; this helper merely stamps the shared
/// custom-property protocol into each definition. It deliberately enumerates
/// neither colors nor spacing values.
fn expand_mask_stop_family(body: &str) -> Vec<CustomUtility> {
    body.split(';')
        .filter_map(|entry| {
            let (name, edges) = entry.split_once(':')?;
            let name = name.trim();
            let stop = name.strip_prefix("mask-")?.rsplit_once('-')?.1.trim();
            matches!(stop, "from" | "to").then_some((
                name,
                stop,
                edges
                    .split(',')
                    .map(str::trim)
                    .filter(|edge| matches!(*edge, "top" | "right" | "bottom" | "left"))
                    .collect::<Vec<_>>(),
            ))
        })
        .flat_map(|(name, stop, edges)| {
            [
                mask_stop_body(&edges, stop, "position"),
                mask_stop_body(&edges, stop, "color"),
                mask_stop_body(&edges, stop, "alpha-color"),
            ]
            .into_iter()
            .map(move |body| CustomUtility {
                name: format!("{name}-*"),
                body,
            })
        })
        .collect()
}

fn mask_stop_body(edges: &[&str], stop: &str, kind: &str) -> String {
    let mut body = String::from(
        "mask-image:var(--tw-mask-linear),var(--tw-mask-radial),var(--tw-mask-conic);mask-composite:intersect;--tw-mask-linear:var(--tw-mask-left),var(--tw-mask-right),var(--tw-mask-bottom),var(--tw-mask-top);",
    );
    for edge in edges {
        write!(
            body,
            "--tw-mask-{edge}:linear-gradient(to {edge},var(--tw-mask-{edge}-from-color,black) var(--tw-mask-{edge}-from-position,0%),var(--tw-mask-{edge}-to-color,transparent) var(--tw-mask-{edge}-to-position,100%));"
        )
        .expect("writing to a String cannot fail");
    }
    let value = match kind {
        "position" => "--value(--spacing-*, --spacing(), percentage, [length])",
        "color" => "--value(--color-*, current, inherit, transparent, [color])",
        "alpha-color" => {
            "color-mix(in oklab,--value(--color-*, current, inherit, transparent, [color]) --alpha(--modifier()),transparent)"
        }
        _ => unreachable!("mask stop body kind is fixed by the expander"),
    };
    let suffix = if kind == "position" {
        "position"
    } else {
        "color"
    };
    for edge in edges {
        write!(body, "--tw-mask-{edge}-{stop}-{suffix}:{value};")
            .expect("writing to a String cannot fail");
    }
    body
}

fn expand_mask_radial_stop_family(body: &str) -> Vec<CustomUtility> {
    body.split(';')
        .filter_map(|entry| entry.split_once(':').map(|(name, _)| name.trim()))
        .filter_map(|name| {
            let stop = name.strip_prefix("mask-radial-")?;
            matches!(stop, "from" | "to").then_some((name, stop))
        })
        .flat_map(|(name, stop)| {
            ["position", "color", "alpha-color"]
                .into_iter()
                .map(move |kind| CustomUtility {
                    name: format!("{name}-*"),
                    body: mask_radial_stop_body(stop, kind),
                })
        })
        .collect()
}

fn mask_radial_stop_body(stop: &str, kind: &str) -> String {
    let value = match kind {
        "position" => "--value(--spacing-*, --spacing(), percentage, [length])",
        "color" => "--value(--color-*, current, inherit, transparent, [color])",
        "alpha-color" => {
            "color-mix(in oklab,--value(--color-*, current, inherit, transparent, [color]) --alpha(--modifier()),transparent)"
        }
        _ => unreachable!("radial mask stop body kind is fixed by the expander"),
    };
    let suffix = if kind == "position" {
        "position"
    } else {
        "color"
    };
    format!(
        "mask-image:var(--tw-mask-linear),var(--tw-mask-radial),var(--tw-mask-conic);mask-composite:intersect;--tw-mask-radial-stops:var(--tw-mask-radial-shape,ellipse) var(--tw-mask-radial-size,farthest-corner) at var(--tw-mask-radial-position,center),var(--tw-mask-radial-from-color,black) var(--tw-mask-radial-from-position,0%),var(--tw-mask-radial-to-color,transparent) var(--tw-mask-radial-to-position,100%);--tw-mask-radial:radial-gradient(var(--tw-mask-radial-stops));--tw-mask-radial-{stop}-{suffix}:{value};"
    )
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
        "color-alpha" => Some(
            "color-mix(in oklab, --value(--color-*, current, inherit, transparent, [color]) --alpha(--modifier()), transparent)",
        ),
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
    fn validates_arbitrary_values_with_lightningcss_value_parsers() {
        for (value_type, valid, invalid) in [
            (ValueType::Integer, "calc(4 / 2)", "1.5"),
            (ValueType::Number, "calc(1 / 2)", "1px"),
            (ValueType::Percentage, "12.5%", "12px"),
            (ValueType::Length, "calc(1rem + 2px)", "50%"),
            (ValueType::LengthPercentage, "calc(1rem + 50%)", "1s"),
            (ValueType::LineWidth, "thin", "50%"),
            (ValueType::ZIndex, "auto", "1.5"),
            (ValueType::Color, "oklch(62% .2 280)", "12px"),
            (ValueType::Image, "linear-gradient(red, blue)", "red"),
            (
                ValueType::Url,
                "url(/hero.png)",
                "linear-gradient(red, blue)",
            ),
            (ValueType::Position, "right 1rem bottom 2px", "red"),
            (ValueType::BackgroundSize, "cover", "red"),
            (ValueType::Shadow, "0 1px 2px #000", "1rem"),
        ] {
            assert!(value_type.accepts(valid), "{value_type:?}: {valid}");
            assert!(!value_type.accepts(invalid), "{value_type:?}: {invalid}");
        }
        assert_eq!(ValueType::from_hint("length"), Some(ValueType::Length));
        assert_eq!(
            ValueType::from_hint("length-percentage"),
            Some(ValueType::LengthPercentage)
        );
        assert_eq!(ValueType::from_hint("unknown"), None);
        assert!(ValueType::Integer.accepts("initial"));
        assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Percentage));
        assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Length));
        assert!(!ValueType::Length.accepts_hint(ValueType::Percentage));
        assert!(ValueType::ZIndex.accepts_hint(ValueType::Integer));
        assert!(!ValueType::ZIndex.accepts_hint(ValueType::Number));
        assert!(ValueType::LineWidth.accepts_hint(ValueType::Length));
    }

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
    fn expands_bare_colour_modifier_families() {
        let utilities = parse_utility_definitions(
            "@utility-family color-alpha-default { border, outline: border-color, outline-color; }",
        );

        assert_eq!(utilities.len(), 2);
        assert_eq!(utilities[0].name, "border");
        assert!(utilities[0].body.contains("border-color:color-mix"));
        assert_eq!(utilities[1].name, "outline");
        assert!(utilities[1].body.contains("outline-color:color-mix"));
    }

    #[test]
    fn expands_mask_stop_roots_from_edge_metadata() {
        let utilities =
            parse_utility_definitions("@utility-family mask-stop { mask-x-from: left, right; }");

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
