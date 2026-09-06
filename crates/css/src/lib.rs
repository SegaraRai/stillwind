//! CSS-first theme parsing. Lightning CSS validates the stylesheet before stillwind
//! extracts its small, source-spanned `@theme` surface.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use lightningcss::{
    printer::PrinterOptions,
    properties::{
        Property, PropertyId, background::BackgroundSize, border::BorderSideWidth,
        box_shadow::BoxShadow,
    },
    stylesheet::{ParserOptions, StyleSheet},
    traits::{Parse, ToCss},
    values::{
        angle::{Angle, AnglePercentage},
        color::CssColor,
        easing::EasingFunction,
        image::Image,
        length::{Length, LengthPercentage},
        number::CSSNumber,
        percentage::{NumberOrPercentage, Percentage},
        position::Position,
        ratio::Ratio,
        time::Time,
        url::Url,
    },
};
use stillwind_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

/// Tailwind CSS v4.3.3 Preflight, vendored under its MIT license.
pub const TAILWIND_V4_PREFLIGHT: &str = include_str!("../../../packages/react/css/preflight.css");

const TAILWIND_V4_THEME: &str = include_str!("../../../packages/react/css/theme.css");

/// One curated CSS property definition extracted from the CSSWG Web Living
/// Standards by W3C Webref. The generated table is the source of truth for
/// supported property names and their formal value syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WebrefPropertySyntax {
    pub name: &'static str,
    pub syntax: &'static str,
    pub href: &'static str,
}

include!("../assets/webref-properties.rs");

fn webref_property(name: &str) -> Option<&'static WebrefPropertySyntax> {
    WEBREF_PROPERTIES
        .iter()
        .find(|property| property.name.eq_ignore_ascii_case(name))
}

/// Returns the CSSWG Web Living Standard's formal value syntax for a CSS
/// property, when Webref publishes one.
///
/// This intentionally exposes only the grammar text. Consumers use it to
/// audit their own value domains; parsing it is not part of Stillwind's public
/// utility syntax.
#[must_use]
pub fn csswg_formal_syntax(property: &str) -> Option<&'static str> {
    webref_property(property).map(|definition| definition.syntax)
}

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
    NumberPercentage,
    Length,
    LengthPercentage,
    Angle,
    AnglePercentage,
    Time,
    Ratio,
    Easing,
    LineWidth,
    ZIndex,
    Color,
    Image,
    Url,
    Position,
    BackgroundSize,
    Shadow,
    Content,
    LinearGradientArguments,
    RadialGradientArguments,
    ConicGradientArguments,
    SkewArguments,
    SkewXArguments,
    SkewYArguments,
    Animation,
    FontFamily,
    FontSize,
    FontWeight,
    VerticalAlign,
    GridTrackSize,
    GridLine,
    Columns,
    Contain,
    Cursor,
    FontFeatureSettings,
    FontStretch,
    GridTrackList,
    Flex,
    ListStyleType,
    ListStyleImage,
    TabSize,
    TransitionProperty,
    WillChange,
    Zoom,
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
            "number-percentage" => Some(Self::NumberPercentage),
            "length" => Some(Self::Length),
            "length-percentage" => Some(Self::LengthPercentage),
            "angle" => Some(Self::Angle),
            "angle-percentage" => Some(Self::AnglePercentage),
            "time" => Some(Self::Time),
            "ratio" => Some(Self::Ratio),
            "easing" => Some(Self::Easing),
            "line-width" => Some(Self::LineWidth),
            "z-index" => Some(Self::ZIndex),
            "color" => Some(Self::Color),
            "image" => Some(Self::Image),
            "url" => Some(Self::Url),
            "position" => Some(Self::Position),
            "bg-size" => Some(Self::BackgroundSize),
            "shadow" => Some(Self::Shadow),
            "content" => Some(Self::Content),
            "linear-gradient-arguments" => Some(Self::LinearGradientArguments),
            "radial-gradient-arguments" => Some(Self::RadialGradientArguments),
            "conic-gradient-arguments" => Some(Self::ConicGradientArguments),
            "skew-arguments" => Some(Self::SkewArguments),
            "skew-x-arguments" => Some(Self::SkewXArguments),
            "skew-y-arguments" => Some(Self::SkewYArguments),
            "animation" => Some(Self::Animation),
            "font-family" => Some(Self::FontFamily),
            "font-size" => Some(Self::FontSize),
            "font-weight" => Some(Self::FontWeight),
            "vertical-align" => Some(Self::VerticalAlign),
            "grid-track-size" => Some(Self::GridTrackSize),
            "grid-line" => Some(Self::GridLine),
            "columns" => Some(Self::Columns),
            "contain" => Some(Self::Contain),
            "cursor" => Some(Self::Cursor),
            "font-feature-settings" => Some(Self::FontFeatureSettings),
            "font-stretch" => Some(Self::FontStretch),
            "grid-track-list" => Some(Self::GridTrackList),
            "flex" => Some(Self::Flex),
            "list-style-type" => Some(Self::ListStyleType),
            "list-style-image" => Some(Self::ListStyleImage),
            "tab-size" => Some(Self::TabSize),
            "transition-property" => Some(Self::TransitionProperty),
            "will-change" => Some(Self::WillChange),
            "zoom" => Some(Self::Zoom),
            _ => None,
        }
    }

    /// Returns the CSSWG formal-syntax productions used to audit this simple
    /// value domain. `None` denotes a compound domain whose grammar is
    /// validated by Lightning CSS (or a focused parser) instead.
    ///
    /// The terms deliberately name CSS value productions, not CSS properties:
    /// a registry audit can therefore discover newly used properties without
    /// maintaining a property allow-list.
    #[must_use]
    pub const fn csswg_formal_terms(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Any => Some(&[]),
            // `<integer>` is a valid restricted production wherever the
            // formal syntax accepts `<number>` (for example `tab-size`).
            Self::Integer => Some(&["integer", "number"]),
            Self::Number => Some(&["number"]),
            Self::Percentage => Some(&[
                "percentage",
                "number-percentage",
                "length-percentage",
                "angle-percentage",
            ]),
            Self::NumberPercentage => Some(&["number", "percentage", "number-percentage"]),
            Self::Length => Some(&["length", "length-percentage"]),
            Self::LengthPercentage => Some(&["length-percentage"]),
            Self::Angle => Some(&["angle", "angle-percentage"]),
            Self::AnglePercentage => Some(&["angle-percentage"]),
            Self::Time => Some(&["time"]),
            Self::Ratio => Some(&["ratio"]),
            Self::Easing => Some(&["easing-function"]),
            Self::LineWidth => Some(&["line-width"]),
            Self::ZIndex => Some(&["integer"]),
            Self::Color => Some(&["color"]),
            Self::Image => Some(&["image"]),
            Self::Url => Some(&["url"]),
            Self::Position => Some(&["position"]),
            Self::BackgroundSize => Some(&["bg-size"]),
            Self::Shadow
            | Self::Content
            | Self::LinearGradientArguments
            | Self::RadialGradientArguments
            | Self::ConicGradientArguments
            | Self::SkewArguments
            | Self::SkewXArguments
            | Self::SkewYArguments
            | Self::Animation
            | Self::FontFamily
            | Self::FontSize
            | Self::FontWeight
            | Self::VerticalAlign
            | Self::GridTrackSize
            | Self::GridLine
            | Self::Columns
            | Self::Contain
            | Self::Cursor
            | Self::FontFeatureSettings
            | Self::FontStretch
            | Self::GridTrackList
            | Self::Flex
            | Self::ListStyleType
            | Self::ListStyleImage
            | Self::TabSize
            | Self::TransitionProperty
            | Self::WillChange
            | Self::Zoom => None,
        }
    }

    /// Validates a decoded arbitrary value against its CSS value definition.
    ///
    /// Lightning CSS owns the grammar for browser CSS types. The small
    /// integer check is layered on top because CSS exposes `<number>`, while
    /// Stillwind's utility grammar needs the narrower integral subset.
    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        if is_css_wide_keyword(value) {
            return true;
        }
        // Custom properties are substituted before a property's value grammar
        // is computed. Without generated project typings we cannot know a
        // referenced variable's domain, so `var(--token)` (including inside a
        // `calc()`) is correctly a runtime-typed value rather than a static
        // type error. Plain literals continue through the strict parsers.
        if contains_custom_property_reference(value) {
            return true;
        }
        match self {
            Self::Any => !value.trim().is_empty(),
            Self::Integer => {
                CSSNumber::parse_string(value).is_ok_and(|number| number.fract() == 0.0)
            }
            Self::Number => CSSNumber::parse_string(value).is_ok(),
            Self::Percentage => Percentage::parse_string(value).is_ok(),
            Self::NumberPercentage => NumberOrPercentage::parse_string(value).is_ok(),
            Self::Length => Length::parse_string(value).is_ok(),
            Self::LengthPercentage => LengthPercentage::parse_string(value).is_ok(),
            Self::Angle => Angle::parse_string(value).is_ok(),
            Self::AnglePercentage => AnglePercentage::parse_string(value).is_ok(),
            Self::Time => Time::parse_string(value).is_ok(),
            Self::Ratio => Ratio::parse_string(value).is_ok(),
            Self::Easing => EasingFunction::parse_string(value).is_ok(),
            Self::LineWidth => BorderSideWidth::parse_string(value).is_ok(),
            Self::ZIndex => value.trim() == "auto" || Self::Integer.accepts(value),
            Self::Color => CssColor::parse_string(value).is_ok(),
            Self::Image => Image::parse_string(value).is_ok(),
            Self::Url => Url::parse_string(value).is_ok(),
            Self::Position => Position::parse_string(value).is_ok(),
            Self::BackgroundSize => BackgroundSize::parse_string(value).is_ok(),
            Self::Shadow => BoxShadow::parse_string(value).is_ok(),
            // Lightning CSS has no typed `content` property parser yet. Keep
            // this narrow exception here rather than treating the whole
            // property as unchecked: it accepts the primitive content
            // productions used by Tailwind and rejects bare identifiers.
            Self::Content => content_value(value),
            Self::LinearGradientArguments => gradient_arguments("linear-gradient", value),
            Self::RadialGradientArguments => gradient_arguments("radial-gradient", value),
            Self::ConicGradientArguments => gradient_arguments("conic-gradient", value),
            Self::SkewArguments => transform_function_arguments("skew", value),
            Self::SkewXArguments => transform_function_arguments("skewX", value),
            Self::SkewYArguments => transform_function_arguments("skewY", value),
            Self::Animation => property_value("animation", value),
            Self::FontFamily => property_value("font-family", value),
            Self::FontSize => property_value("font-size", value),
            Self::FontWeight => property_value("font-weight", value),
            Self::VerticalAlign => property_value("vertical-align", value),
            Self::GridTrackSize => property_value("grid-auto-columns", value),
            Self::GridLine => property_value("grid-column", value),
            Self::Columns => property_value("columns", value),
            Self::Contain => property_value("contain", value),
            Self::Cursor => property_value("cursor", value),
            Self::FontFeatureSettings => property_value("font-feature-settings", value),
            Self::FontStretch => property_value("font-stretch", value),
            Self::GridTrackList => property_value("grid-template-columns", value),
            Self::Flex => property_value("flex", value),
            Self::ListStyleType => property_value("list-style-type", value),
            Self::ListStyleImage => property_value("list-style-image", value),
            Self::TabSize => property_value("tab-size", value),
            Self::TransitionProperty => property_value("transition-property", value),
            Self::WillChange => property_value("will-change", value),
            Self::Zoom => property_value("zoom", value),
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
                    | (Self::AnglePercentage, Self::Angle | Self::Percentage)
                    | (Self::NumberPercentage, Self::Number | Self::Percentage)
                    | (Self::LineWidth, Self::Length)
                    | (Self::ZIndex, Self::Integer)
            )
    }
}

fn contains_custom_property_reference(value: &str) -> bool {
    let mut cursor = value;
    while let Some(start) = cursor.find("var(") {
        let after_function = &cursor[start + "var(".len()..];
        let name = after_function.trim_start();
        if name.starts_with("--") {
            return true;
        }
        cursor = after_function;
    }
    false
}

fn gradient_arguments(function: &str, value: &str) -> bool {
    Image::parse_string(&format!("{function}({value})")).is_ok()
}

fn content_value(value: &str) -> bool {
    let value = value.trim();
    matches!(
        value,
        "normal" | "none" | "open-quote" | "close-quote" | "no-open-quote" | "no-close-quote"
    ) || is_css_string(value)
        || ["attr(", "counter(", "counters(", "leader(", "element("]
            .iter()
            .any(|prefix| {
                value
                    .strip_prefix(prefix)
                    .is_some_and(|value| value.ends_with(')'))
            })
        || Image::parse_string(value).is_ok()
}

fn transform_function_arguments(function: &str, value: &str) -> bool {
    let value = format!("{function}({value})");
    matches!(
        Property::parse_string(
            PropertyId::from("transform"),
            &value,
            ParserOptions::default(),
        ),
        Ok(property) if !matches!(property, Property::Unparsed(_) | Property::Custom(_))
    )
}

fn property_value(property: &str, value: &str) -> bool {
    CssProperty::from_name(property).is_some_and(|property| property.accepts(value))
}

/// Rewrites one box-shadow colour with a custom-property override and CSS
/// Color 5 relative-colour fallback. This is the semantic operation Tailwind
/// uses for `shadow-*/opacity` families, while the parser owns the shadow
/// grammar.
#[must_use]
pub fn shadow_with_color_fallback(
    value: &str,
    color_property: &str,
    alpha: &str,
) -> Option<String> {
    let shadow = BoxShadow::parse_string(value).ok()?;
    if let Some((base, color)) = trailing_top_level_css_component(value)
        && CssColor::parse_string(color).is_ok()
    {
        return Some(format!(
            "{base} var({color_property}, oklab(from {color} l a b / {alpha}))"
        ));
    }

    // The fallback remains grammar-driven for shadows whose colour is not
    // written as the final component. Built-in theme shadows take the source
    // preserving path above, so their authored precision is retained.
    let color = shadow.color.to_css_string(PrinterOptions::default()).ok()?;
    let serialized = shadow.to_css_string(PrinterOptions::default()).ok()?;
    let without_color = serialized
        .strip_suffix(&format!(" {color}"))
        .unwrap_or(&serialized);
    Some(format!(
        "{without_color} var({color_property}, oklab(from {color} l a b / {alpha}))"
    ))
}

/// Serializes a theme color as an sRGB fallback with the requested alpha.
/// Tailwind emits the legacy fallback before its Color 5 `color-mix` upgrade;
/// LightningCSS provides the color-space conversion we need here.
#[must_use]
pub fn color_with_alpha_fallback(value: &str, alpha: &str) -> Option<String> {
    let color = CssColor::parse_string(value).ok()?.to_rgb().ok()?;
    let serialized = color.to_css_string(PrinterOptions::default()).ok()?;
    if let (Some(hex), Some(percent)) = (
        serialized.strip_prefix('#'),
        alpha
            .trim()
            .strip_suffix('%')
            .and_then(|value| value.parse::<f64>().ok()),
    ) && hex.len() == 6
        && hex.chars().all(|character| character.is_ascii_hexdigit())
    {
        let byte = (percent.clamp(0.0, 100.0) * 255.0 / 100.0).round() as u8;
        return Some(format!("#{hex}{byte:02x}"));
    }
    Some(format!("{serialized} / {alpha}"))
}

fn trailing_top_level_css_component(value: &str) -> Option<(&str, &str)> {
    let mut depth = 0_u32;
    let mut boundary = None;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            character if character.is_whitespace() && depth == 0 => boundary = Some(index),
            _ => {}
        }
    }
    let boundary = boundary?;
    let (base, component) = value.split_at(boundary);
    let component = component.trim();
    (!base.trim().is_empty() && !component.is_empty()).then_some((base.trim_end(), component))
}

fn is_css_string(value: &str) -> bool {
    let Some(quote) = value
        .chars()
        .next()
        .filter(|quote| matches!(quote, '\'' | '"'))
    else {
        return false;
    };
    value.ends_with(quote)
        && value.len() >= quote.len_utf8() * 2
        && !value[..value.len() - quote.len_utf8()].ends_with('\\')
}

/// A CSSWG-defined property whose value grammar is delegated to Lightning CSS
/// where available, with narrow temporary exceptions where it is not.
///
/// The supported set is generated from W3C Webref's curated CSSWG data,
/// rather than maintained as a partial list. It is an internal validator for
/// named CSS value domains; property names are never part of the public
/// `--value([type])` grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssProperty {
    name: String,
    grammar: CssPropertyGrammar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CssPropertyGrammar {
    LightningCss,
    Columns,
    Contain,
    FontFeatureSettings,
    ObjectPosition,
    WillChange,
    Zoom,
    AnchorName,
    PositionAnchor,
    AnchorScope,
    PositionArea,
    PositionTryFallbacks,
}

const PROPERTY_GRAMMAR_EXCEPTIONS: &[(&str, CssPropertyGrammar)] = &[
    // These established properties are not represented by the current
    // Lightning CSS property table. Keep their grammar exceptions explicit
    // and covered by acceptance/rejection tests.
    ("contain", CssPropertyGrammar::Contain),
    ("columns", CssPropertyGrammar::Columns),
    (
        "font-feature-settings",
        CssPropertyGrammar::FontFeatureSettings,
    ),
    ("object-position", CssPropertyGrammar::ObjectPosition),
    ("will-change", CssPropertyGrammar::WillChange),
    ("zoom", CssPropertyGrammar::Zoom),
    // CSS Anchor Positioning is intentionally kept here until Lightning CSS
    // exposes property parsers for it. Each exception has a narrow validator
    // below and a boundary test; do not add an unchecked catch-all.
    ("anchor-name", CssPropertyGrammar::AnchorName),
    ("position-anchor", CssPropertyGrammar::PositionAnchor),
    ("anchor-scope", CssPropertyGrammar::AnchorScope),
    ("position-area", CssPropertyGrammar::PositionArea),
    (
        "position-try-fallbacks",
        CssPropertyGrammar::PositionTryFallbacks,
    ),
];

impl CssProperty {
    /// Finds a CSSWG-defined property for an internal value-domain validator.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let webref = webref_property(name)?;
        let property_id = PropertyId::from(name);
        if !matches!(property_id, PropertyId::Custom(_)) {
            return Some(Self {
                name: webref.name.to_owned(),
                grammar: CssPropertyGrammar::LightningCss,
            });
        }
        PROPERTY_GRAMMAR_EXCEPTIONS
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(name, grammar)| Self {
                name: (*name).to_owned(),
                grammar: *grammar,
            })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Validates the value through Lightning CSS's parser for this exact CSS
    /// property. This intentionally covers property-specific keyword and
    /// shorthand rules that a generic value type cannot express.
    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        if is_css_wide_keyword(value) {
            return true;
        }
        match self.grammar {
            CssPropertyGrammar::LightningCss => matches!(
                Property::parse_string(
                    PropertyId::from(self.name()),
                    value.trim(),
                    ParserOptions::default(),
                ),
                Ok(property) if !matches!(property, Property::Unparsed(_) | Property::Custom(_))
            ),
            CssPropertyGrammar::Columns => columns_value(value),
            CssPropertyGrammar::Contain => contain_value(value),
            CssPropertyGrammar::FontFeatureSettings => font_feature_settings(value),
            CssPropertyGrammar::ObjectPosition => Position::parse_string(value).is_ok(),
            CssPropertyGrammar::WillChange => will_change(value),
            CssPropertyGrammar::Zoom => {
                matches!(value.trim(), "normal" | "reset")
                    || NumberOrPercentage::parse_string(value).is_ok()
            }
            CssPropertyGrammar::AnchorName => dashed_ident_list(value, "none"),
            CssPropertyGrammar::PositionAnchor => {
                value.trim() == "auto" || dashed_ident(value.trim())
            }
            CssPropertyGrammar::AnchorScope => {
                matches!(value.trim(), "none" | "all") || dashed_ident_list(value, "")
            }
            CssPropertyGrammar::PositionArea => position_area(value),
            CssPropertyGrammar::PositionTryFallbacks => position_try_fallbacks(value),
        }
    }
}

fn columns_value(value: &str) -> bool {
    let values = value.split_whitespace().collect::<Vec<_>>();
    if values.is_empty() || values.len() > 2 {
        return false;
    }
    let mut width = false;
    let mut count = false;
    for value in values {
        if !width && (value == "auto" || Length::parse_string(value).is_ok()) {
            width = true;
        } else if !count && (value == "auto" || value.parse::<u32>().is_ok_and(|value| value > 0)) {
            count = true;
        } else {
            return false;
        }
    }
    width || count
}

fn contain_value(value: &str) -> bool {
    const SINGLE: &[&str] = &["none", "strict", "content"];
    const COMPONENTS: &[&str] = &["size", "inline-size", "layout", "style", "paint"];
    let values = value.split_whitespace().collect::<Vec<_>>();
    SINGLE.contains(&value.trim())
        || (!values.is_empty()
            && values.len() <= COMPONENTS.len()
            && values.iter().all(|value| COMPONENTS.contains(value))
            && values.windows(2).all(|pair| pair[0] != pair[1]))
}

fn font_feature_settings(value: &str) -> bool {
    let value = value.trim();
    value == "normal"
        || value.split(',').all(|feature| {
            let mut parts = feature.split_whitespace();
            let Some(tag) = parts.next() else {
                return false;
            };
            let valid_tag = tag
                .strip_prefix('"')
                .and_then(|tag| tag.strip_suffix('"'))
                .is_some_and(|tag| tag.len() == 4 && tag.is_ascii());
            let valid_value = match parts.next() {
                None | Some("on" | "off") => true,
                Some(number) => number.parse::<i32>().is_ok(),
            };
            valid_tag && valid_value && parts.next().is_none()
        })
}

fn will_change(value: &str) -> bool {
    let value = value.trim();
    value == "auto"
        || value.split(',').all(|item| {
            let item = item.trim();
            matches!(item, "scroll-position" | "contents") || custom_ident(item)
        })
}

fn custom_ident(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| !character.is_ascii_digit())
        && !matches!(
            value,
            "initial" | "inherit" | "unset" | "revert" | "revert-layer"
        )
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || !character.is_ascii()
        })
}

fn dashed_ident_list(value: &str, keyword: &str) -> bool {
    let value = value.trim();
    value == keyword || (!value.is_empty() && value.split_whitespace().all(dashed_ident))
}

fn dashed_ident(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("--") else {
        return false;
    };
    !rest.is_empty()
        && rest.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || !character.is_ascii()
        })
}

fn position_area(value: &str) -> bool {
    const HORIZONTAL: &[&str] = &[
        "left",
        "center",
        "right",
        "span-left",
        "span-right",
        "x-start",
        "x-end",
        "span-x-start",
        "span-x-end",
        "self-x-start",
        "self-x-end",
        "span-self-x-start",
        "span-self-x-end",
        "span-all",
        "inline-start",
        "inline-end",
        "span-inline-start",
        "span-inline-end",
        "self-inline-start",
        "self-inline-end",
        "span-self-inline-start",
        "span-self-inline-end",
    ];
    const VERTICAL: &[&str] = &[
        "top",
        "center",
        "bottom",
        "span-top",
        "span-bottom",
        "y-start",
        "y-end",
        "span-y-start",
        "span-y-end",
        "self-y-start",
        "self-y-end",
        "span-self-y-start",
        "span-self-y-end",
        "span-all",
        "block-start",
        "block-end",
        "span-block-start",
        "span-block-end",
        "self-block-start",
        "self-block-end",
        "span-self-block-start",
        "span-self-block-end",
    ];

    let values = value.split_whitespace().collect::<Vec<_>>();
    matches!(values.as_slice(), [single] if HORIZONTAL.contains(single) || VERTICAL.contains(single))
        || matches!(values.as_slice(), [first, second] if {
            (HORIZONTAL.contains(first) && VERTICAL.contains(second))
                || (VERTICAL.contains(first) && HORIZONTAL.contains(second))
        })
}

fn position_try_fallbacks(value: &str) -> bool {
    const TACTICS: &[&str] = &["flip-block", "flip-inline", "flip-start", "flip-end"];
    value.split(',').all(|item| {
        let item = item.trim();
        !item.is_empty() && (dashed_ident(item) || TACTICS.contains(&item))
    })
}

/// A `--value([…])` domain. These names are CSS value definitions, not CSS
/// property names. Complex built-in grammars receive a named value domain
/// only when they cannot be expressed as a composition of standard types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDomain {
    Type(ValueType),
}

impl ValueDomain {
    #[must_use]
    pub fn from_hint(hint: &str) -> Option<Self> {
        ValueType::from_hint(hint).map(Self::Type)
    }

    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        match self {
            Self::Type(value_type) => value_type.accepts(value),
        }
    }

    #[must_use]
    pub fn accepts_hint(&self, hint: &str) -> bool {
        match self {
            Self::Type(expected) => {
                ValueType::from_hint(hint).is_some_and(|actual| expected.accepts_hint(actual))
            }
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeTokenMode {
    Default,
    Inline,
    Static,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    pub name: Spanned<String>,
    pub value: Spanned<String>,
    pub kind: ThemeTokenKind,
    pub mode: ThemeTokenMode,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeIndex {
    pub tokens: Vec<ThemeToken>,
    pub custom_css: String,
    pub utilities: Vec<CustomUtility>,
    pub variants: Vec<CustomVariant>,
    pub properties: Vec<CustomPropertyDefinition>,
    pub authored_properties: BTreeSet<String>,
    /// Framework layers requested by the authored Tailwind-compatible imports.
    ///
    /// A stylesheet without a framework import retains the historical full
    /// Stillwind baseline. Once an official Tailwind import is present, its
    /// selected entry points are authoritative.
    pub imports: FrameworkImports,
    pub config: StillwindConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindConfig {
    pub prefix: Option<String>,
    pub tailwind_prefix: Option<String>,
    pub lint_rules: BTreeMap<String, String>,
}

impl StillwindConfig {
    #[must_use]
    pub fn lint_enabled(&self, rule: &str) -> bool {
        self.lint_rules
            .get(rule)
            .is_some_and(|level| matches!(level.as_str(), "warn" | "deny"))
    }
}

/// The Tailwind entry points selected by an authored `@import` directive.
/// Layer names are retained rather than inferred during rendering so a custom
/// `layer(...)` placement remains observable in the generated stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkImports {
    pub theme_layer: Option<String>,
    pub base_layer: Option<String>,
    pub utilities_layer: Option<String>,
    pub builtins: bool,
    /// Whether the canonical single full-framework import was authored.
    /// Tailwind emits the global layer-order prelude only for that entrypoint.
    pub layer_order: bool,
    /// Tailwind's class/theme namespace from `prefix(name)` import modifiers.
    pub prefix: Option<String>,
}

impl Default for FrameworkImports {
    fn default() -> Self {
        Self {
            theme_layer: Some("theme".to_owned()),
            base_layer: Some("base".to_owned()),
            utilities_layer: Some("utilities".to_owned()),
            builtins: true,
            layer_order: true,
            prefix: None,
        }
    }
}

impl FrameworkImports {
    fn explicit() -> Self {
        Self {
            theme_layer: None,
            base_layer: None,
            utilities_layer: None,
            builtins: false,
            layer_order: false,
            prefix: None,
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

/// One compiler-owned custom-property registration declared with
/// `@stillwind-property`. Ordinary authored `@property` rules are never added
/// to this registry and remain untouched in the output stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPropertyDefinition {
    pub name: String,
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

/// Parses compiler-owned custom-property registrations without interpreting
/// ordinary CSS `@property` rules.
#[must_use]
pub fn parse_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_property_definitions(css)
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
            if matches!(token.mode, ThemeTokenMode::Static)
                || (!matches!(token.mode, ThemeTokenMode::Inline)
                    && custom_property_references(css).contains(&token.name.value))
            {
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
        // Built-in shadow utilities materialize their token into the
        // --tw-shadow state variable. Once that declaration is present, the
        // intermediate --shadow-* theme alias is not emitted by Tailwind.
        if css.contains("--tw-shadow:") {
            required.retain(|name| !name.starts_with("--shadow-"));
        }
        tokens
            .into_iter()
            .filter(|token| {
                required.contains(&token.name.value)
                    && !matches!(token.mode, ThemeTokenMode::Inline)
            })
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

fn custom_property_references(css: &str) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
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

/// Returns custom properties used by declarations and values, excluding names
/// that occur only inside ordinary authored `@property` registration blocks.
#[must_use]
pub fn used_custom_properties(css: &str) -> BTreeSet<String> {
    custom_property_references(&strip_css_comments(&strip_blocks(css, "@property")))
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
    theme.properties = extract_property_definitions(source.text());
    theme.authored_properties = extract_authored_property_names(source.text());
    theme.config = extract_stillwind_config(source.text());
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
    // Framework imports are compiler directives rather than browser CSS. Parse
    // the directive-stripped view so Tailwind's optional `prefix(...)` import
    // modifier (and other entrypoint modifiers) does not become a CSS syntax
    // error before we extract the semantic registries below.
    StyleSheet::parse(
        &strip_framework_directives(source.text()),
        ParserOptions::default(),
    )
    .map_err(|error| ThemeError::Parse(error.to_string()))?;
    let imports = framework_imports(source.text());
    let mut tokens = if imports.theme_layer.is_some() {
        extract_theme_tokens(FileId(0), TAILWIND_V4_THEME)?.tokens
    } else {
        Vec::new()
    };
    tokens.extend(extract_theme_tokens(source.id(), source.text())?.tokens);
    if let Some(prefix) = imports.prefix.as_deref() {
        let prefix = prefix.trim_end_matches('-');
        for token in &mut tokens {
            if let Some(name) = token.name.value.strip_prefix("--") {
                token.name.value = format!("--{prefix}-{name}");
            }
        }
    }
    Ok(ThemeIndex {
        tokens,
        custom_css: strip_framework_directives(source.text()),
        utilities: extract_custom_utilities(source.text()),
        variants: extract_custom_variants(source.text()),
        properties: extract_property_definitions(source.text()),
        authored_properties: extract_authored_property_names(source.text()),
        imports,
        config: extract_stillwind_config(source.text()),
    })
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("stillwind source files must be smaller than 4 GiB")
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

        let modifier = css[theme_start + "@theme".len()..open].trim();
        let mode = match modifier {
            "inline" => ThemeTokenMode::Inline,
            "static" => ThemeTokenMode::Static,
            _ => ThemeTokenMode::Default,
        };
        tokens.extend(extract_theme_declarations(
            file,
            &css[open + 1..close],
            open + 1,
            mode,
        ));

        cursor = close + 1;
    }

    Ok(ThemeIndex {
        tokens,
        custom_css: String::new(),
        utilities: Vec::new(),
        variants: Vec::new(),
        properties: Vec::new(),
        authored_properties: BTreeSet::new(),
        imports: FrameworkImports::default(),
        config: StillwindConfig::default(),
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
        let prefix = import_prefix(tail);
        match path {
            "tailwindcss" | "stillwind" => {
                found = true;
                output.layer_order = true;
                output.theme_layer = Some(layer.clone().unwrap_or_else(|| "theme".to_owned()));
                output.base_layer = Some(layer.clone().unwrap_or_else(|| "base".to_owned()));
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
                output.builtins = true;
                output.prefix = prefix.clone();
            }
            "tailwindcss/theme" => {
                found = true;
                output.theme_layer = Some(layer.unwrap_or_else(|| "theme".to_owned()));
                output.builtins = true;
                output.prefix = prefix.clone();
            }
            "tailwindcss/preflight" => {
                found = true;
                output.base_layer = Some(layer.unwrap_or_else(|| "base".to_owned()));
                output.builtins = true;
            }
            "tailwindcss/utilities" => {
                found = true;
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
                output.builtins = true;
                output.prefix = prefix.clone();
            }
            "stillwind/theme.css" => {
                found = true;
                output.theme_layer = Some(layer.unwrap_or_else(|| "theme".to_owned()));
            }
            "stillwind/preflight.css" => {
                found = true;
                output.base_layer = Some(layer.unwrap_or_else(|| "base".to_owned()));
            }
            "stillwind/utilities.css" => {
                found = true;
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
            }
            "stillwind/builtins.css" => {
                found = true;
                output.builtins = true;
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

fn import_prefix(tail: &str) -> Option<String> {
    let start = tail.find("prefix(")? + "prefix(".len();
    let end = tail[start..].find(')')? + start;
    let prefix = tail[start..end].trim();
    (!prefix.is_empty()).then(|| prefix.to_owned())
}

fn extract_theme_declarations(
    file: FileId,
    body: &str,
    body_start: usize,
    mode: ThemeTokenMode,
) -> Vec<ThemeToken> {
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
            mode,
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
    let css = strip_framework_imports(css);
    let css = strip_blocks(&css, "@theme");
    let css = strip_blocks(&css, "@utility");
    let css = strip_blocks(&css, "@variant");
    let css = strip_blocks(&css, "@stillwind-property");
    let css = strip_blocks(&css, "@stillwind");
    css.lines()
        .filter(|line| !line.trim_start().starts_with("@custom-variant"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_stillwind_config(css: &str) -> StillwindConfig {
    let mut config = StillwindConfig::default();
    for (_, body) in extract_named_blocks(&strip_css_comments(css), "@stillwind") {
        for declaration in body.split(';') {
            let declaration = declaration.trim();
            let Some((key, value)) = declaration.split_once(':') else {
                continue;
            };
            let key = key
                .rsplit_once('{')
                .map_or(key.trim(), |(_, nested)| nested.trim());
            let value = value.trim().trim_matches(['"', '\'']);
            if key == "prefix" {
                config.prefix = Some(value.to_owned());
            } else if key == "tailwind-prefix" {
                config.tailwind_prefix = Some(value.to_owned());
            } else if matches!(value, "warn" | "deny" | "off") {
                config.lint_rules.insert(key.to_owned(), value.to_owned());
            }
        }
    }
    config
}

fn strip_framework_imports(css: &str) -> String {
    let mut output = String::with_capacity(css.len());
    let mut cursor = 0;

    while let Some(relative_start) = css[cursor..].find("@import") {
        let start = cursor + relative_start;
        output.push_str(&css[cursor..start]);
        let Some(relative_end) = css[start..].find(';') else {
            output.push_str(&css[start..]);
            return output;
        };
        let end = start + relative_end + 1;
        let rule = css[start + "@import".len()..end - 1].trim();
        let is_framework = parse_import_path(rule).is_some_and(|(path, _)| {
            matches!(
                path,
                "tailwindcss"
                    | "tailwindcss/theme"
                    | "tailwindcss/preflight"
                    | "tailwindcss/utilities"
                    | "stillwind"
                    | "stillwind/theme.css"
                    | "stillwind/preflight.css"
                    | "stillwind/utilities.css"
                    | "stillwind/builtins.css"
            )
        });
        if !is_framework {
            output.push_str(&css[start..end]);
        }
        cursor = end;
    }

    output.push_str(&css[cursor..]);
    output
}

fn extract_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_named_blocks(&strip_css_comments(css), "@stillwind-property")
        .into_iter()
        .filter_map(|(header, body)| {
            let name = header.split_whitespace().next()?;
            name.starts_with("--").then(|| CustomPropertyDefinition {
                name: name.to_owned(),
                body,
            })
        })
        .collect()
}

fn extract_authored_property_names(css: &str) -> BTreeSet<String> {
    extract_named_blocks(&strip_css_comments(css), "@property")
        .into_iter()
        .filter_map(|(header, _)| {
            let name = header.split_whitespace().next()?;
            name.starts_with("--").then(|| name.to_owned())
        })
        .collect()
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
            (ValueType::NumberPercentage, ".5", "1px"),
            (ValueType::Length, "calc(1rem + 2px)", "50%"),
            (ValueType::LengthPercentage, "calc(1rem + 50%)", "1s"),
            (ValueType::LineWidth, "thin", "50%"),
            (ValueType::ZIndex, "auto", "1.5"),
            (ValueType::Angle, "0.5turn", "1s"),
            (ValueType::AnglePercentage, "calc(90deg + 50%)", "1s"),
            (ValueType::Time, "150ms", "1px"),
            (ValueType::Ratio, "16 / 9", "1px"),
            (ValueType::Easing, "cubic-bezier(.2, .8, .2, 1)", "1px"),
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
            (ValueType::Content, "'stillwind'", "stillwind"),
            (
                ValueType::LinearGradientArguments,
                "45deg, red, blue",
                "red / blue",
            ),
            (
                ValueType::RadialGradientArguments,
                "at 20% 30%, red, blue",
                "red / blue",
            ),
            (
                ValueType::ConicGradientArguments,
                "from 45deg, red, blue",
                "red / blue",
            ),
            (ValueType::SkewArguments, "15deg", "red"),
            (ValueType::SkewXArguments, "15deg", "red"),
            (ValueType::SkewYArguments, "15deg", "red"),
            (ValueType::Animation, "spin 1s linear infinite", "1px"),
            (ValueType::FontFamily, "ui-sans-serif, sans-serif", "12px"),
            (ValueType::FontSize, "1rem", "red"),
            (ValueType::FontWeight, "650", "red"),
            (ValueType::VerticalAlign, "middle", "red"),
            (ValueType::GridTrackSize, "minmax(0, 1fr)", "red"),
            (ValueType::GridLine, "span 2", "1px"),
            (ValueType::Columns, "12rem 3", "red"),
            (ValueType::Contain, "layout paint", "red"),
            (ValueType::Cursor, "pointer", "red"),
            (ValueType::FontFeatureSettings, "\"liga\" 1", "liga"),
            (ValueType::FontStretch, "125%", "red"),
            (ValueType::GridTrackList, "repeat(3, minmax(0, 1fr))", "red"),
            (ValueType::Flex, "1 1 0%", "red"),
            (ValueType::ListStyleType, "disc", "1px"),
            (ValueType::ListStyleImage, "url(icon.svg)", "red"),
            (ValueType::TabSize, "4", "red"),
            (ValueType::TransitionProperty, "transform", "1px"),
            (ValueType::WillChange, "transform", "1px"),
            (ValueType::Zoom, "125%", "red"),
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
        assert!(ValueType::Length.accepts("calc(var(--gutter) * 2)"));
        assert!(!ValueType::Length.accepts("var(gutter)"));
        assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Percentage));
        assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Length));
        assert!(!ValueType::Length.accepts_hint(ValueType::Percentage));
        assert!(ValueType::ZIndex.accepts_hint(ValueType::Integer));
        assert!(!ValueType::ZIndex.accepts_hint(ValueType::Number));
        assert!(ValueType::LineWidth.accepts_hint(ValueType::Length));
        assert!(ValueType::NumberPercentage.accepts_hint(ValueType::Percentage));
    }

    #[test]
    fn validates_csswg_property_grammars_with_lightningcss_or_exceptions() {
        for (name, valid, invalid) in [
            ("padding", "calc(1rem + 50%)", "red"),
            ("margin", "auto", "red"),
            ("border-width", "thin", "50%"),
            ("transition-duration", "150ms", "red"),
            ("aspect-ratio", "16 / 9", "red"),
            ("opacity", ".5", "red"),
            // This property was deliberately absent from the old hand-written
            // list. Its acceptance proves the domain comes from Lightning
            // CSS's generated property registry, not a fixture-derived list.
            ("text-decoration-line", "underline overline", "12px"),
            ("view-transition-name", "hero", "12px"),
            ("contain", "layout paint", "middle"),
            ("font-feature-settings", "\"liga\" 1", "liga"),
            ("object-position", "left 25%", "middle middle"),
            ("will-change", "transform, opacity", "1px"),
            ("zoom", "125%", "red"),
        ] {
            let property = CssProperty::from_name(name).unwrap();
            assert!(property.accepts(valid), "{name}: {valid}");
            assert!(!property.accepts(invalid), "{name}: {invalid}");
        }

        assert!(webref_property("padding").is_some());
        assert!(webref_property("grid-template-columns").is_some());
        assert!(webref_property("view-transition-name").is_some());
        assert!(webref_property("not-a-css-property").is_none());
        assert!(ValueDomain::from_hint("property:padding").is_none());
    }

    #[test]
    fn generated_webref_snapshot_covers_every_internal_complex_value_domain() {
        for property in [
            "animation",
            "columns",
            "contain",
            "cursor",
            "flex",
            "font-feature-settings",
            "font-stretch",
            "grid-auto-columns",
            "grid-column",
            "grid-template-columns",
            "list-style-image",
            "list-style-type",
            "tab-size",
            "transition-property",
            "vertical-align",
            "will-change",
            "zoom",
        ] {
            let definition = webref_property(property)
                .unwrap_or_else(|| panic!("Webref must define `{property}`"));
            assert!(!definition.syntax.is_empty(), "{property}");
            assert!(definition.href.starts_with("https://"), "{property}");
        }
    }

    #[test]
    fn validates_anchor_positioning_with_explicit_property_exceptions() {
        let domain = |name| CssProperty::from_name(name).unwrap();

        assert!(domain("anchor-name").accepts("--card --tooltip"));
        assert!(!domain("anchor-name").accepts("card"));
        assert!(domain("position-anchor").accepts("--card"));
        assert!(domain("position-anchor").accepts("auto"));
        assert!(!domain("position-anchor").accepts("card"));
        assert!(domain("anchor-scope").accepts("all"));
        assert!(domain("anchor-scope").accepts("--dialog"));
        assert!(!domain("anchor-scope").accepts("dialog"));
        assert!(domain("position-area").accepts("top span-inline-end"));
        assert!(!domain("position-area").accepts("middle middle"));
        assert!(domain("position-try-fallbacks").accepts("--fallback, flip-block"));
        assert!(!domain("position-try-fallbacks").accepts("fallback"));
    }

    #[test]
    fn preserves_authored_shadow_colour_when_applying_an_alpha_modifier() {
        assert_eq!(
            shadow_with_color_fallback(
                "inset 0 2px 4px rgb(0 0 0 / 0.05)",
                "--tw-inset-shadow-color",
                "50%",
            ),
            Some(
                "inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%))"
                    .to_owned()
            )
        );
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
            "stillwind.css",
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
    fn matches_tailwind_theme_token_emission_modes() {
        let source = SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@theme { --color-used: red; --color-unused: blue; }\n@theme static { --spacing-static: 1rem; }\n@theme inline { --color-alias: var(--brand); }",
        );
        let theme = parse_theme(&source).unwrap();
        let required = theme.tokens_required_by("color:var(--color-used);", false);
        assert!(
            required
                .iter()
                .any(|token| token.name.value == "--color-used")
        );
        assert!(
            required
                .iter()
                .any(|token| token.name.value == "--spacing-static")
        );
        assert!(
            !required
                .iter()
                .any(|token| token.name.value == "--color-unused")
        );
        assert!(
            !required
                .iter()
                .any(|token| token.name.value == "--color-alias")
        );
        assert_eq!(theme.value("color", "alias"), Some("var(--brand)"));
    }

    #[test]
    fn extracts_css_first_stillwind_config_and_removes_it_from_custom_css() {
        let source = SourceFile::new(
            FileId(7),
            "stillwind.css",
            r#"@stillwind { prefix: "--app-"; lint { prefer-typed-relationships: warn; } }
.card { color: red; }"#,
        );

        let theme = parse_theme(&source).unwrap();

        assert_eq!(theme.config.prefix.as_deref(), Some("--app-"));
        assert!(theme.config.lint_enabled("prefer-typed-relationships"));
        assert!(!theme.custom_css().contains("@stillwind"));
        assert!(theme.custom_css().contains(".card"));
    }

    #[test]
    fn applies_tailwind_defaults_and_namespace_resets() {
        let source = SourceFile::new(
            FileId(7),
            "stillwind.css",
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
    fn individual_imports_require_builtins_and_gate_default_theme_tokens() {
        let individual = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/utilities.css\" layer(utilities);",
        ))
        .unwrap();
        assert!(!individual.imports.builtins);
        assert!(individual.scale(ThemeNamespace::Spacing).is_none());

        let with_builtins = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ))
        .unwrap();
        assert!(with_builtins.imports.builtins);
        assert_eq!(
            with_builtins
                .scale(ThemeNamespace::Spacing)
                .map(|token| token.value.value.as_str()),
            Some("0.25rem")
        );
        assert!(!with_builtins.custom_css().contains("@import"));

        let aggregate = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind\";",
        ))
        .unwrap();
        assert!(aggregate.imports.builtins);
        assert!(aggregate.imports.theme_layer.is_some());
        assert!(aggregate.imports.base_layer.is_some());
        assert!(aggregate.imports.utilities_layer.is_some());

        let tailwind_compat = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"tailwindcss/utilities\" layer(utilities);",
        ))
        .unwrap();
        assert!(tailwind_compat.imports.builtins);
        assert!(tailwind_compat.imports.utilities_layer.is_some());
    }

    #[test]
    fn consumes_framework_imports_but_preserves_other_css_imports() {
        let theme = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"./fonts.css\"; @import \"stillwind/builtins.css\"; .card { color: red; }",
        ))
        .unwrap();

        assert!(theme.custom_css().contains("@import \"./fonts.css\";"));
        assert!(!theme.custom_css().contains("stillwind/builtins.css"));
        assert!(theme.custom_css().contains(".card"));
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
            "stillwind.css",
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
            "stillwind.css",
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

    #[test]
    fn separates_compiler_property_definitions_from_authored_property_rules() {
        let source = SourceFile::new(
            FileId(7),
            "stillwind.css",
            r#"
                @stillwind-property --example-value {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                @property --authored-value {
                  syntax: "<color>";
                  inherits: false;
                  initial-value: red;
                }
                .example { color: var(--authored-value); }
            "#,
        );

        let theme = parse_theme(&source).unwrap();

        assert_eq!(theme.properties.len(), 1);
        assert_eq!(theme.properties[0].name, "--example-value");
        assert!(theme.authored_properties.contains("--authored-value"));
        assert!(!theme.custom_css().contains("@stillwind-property"));
        assert!(theme.custom_css().contains("@property --authored-value"));
    }
}
