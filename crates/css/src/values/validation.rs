//! Grammar check for one arbitrary value against one [`ValueType`].
//!
//! Lightning CSS owns the browser CSS grammars, so this module delegates each variant
//! to the matching parser and only adds the checks Stillwind needs on top. The hint
//! spellings documented on [`ValueType`] are part of the authored grammar: a variant
//! may not be renamed without changing what users write in a hint.
//!
//! Validation is total and side-effect free. It is deliberately permissive in two
//! places, both documented on [`ValueType::accepts`]: a CSS-wide keyword and a `var()`
//! reference are accepted without consulting the grammar.

use lightningcss::properties::background::BackgroundSize;
use lightningcss::properties::border::BorderSideWidth;
use lightningcss::properties::box_shadow::BoxShadow;
use lightningcss::traits::Parse;
use lightningcss::values::angle::{Angle, AnglePercentage};
use lightningcss::values::color::CssColor;
use lightningcss::values::easing::EasingFunction;
use lightningcss::values::image::Image;
use lightningcss::values::length::{Length, LengthPercentage};
use lightningcss::values::number::CSSNumber;
use lightningcss::values::percentage::{NumberOrPercentage, Percentage};
use lightningcss::values::position::Position;
use lightningcss::values::ratio::Ratio;
use lightningcss::values::time::Time;
use lightningcss::values::url::Url;

use super::domain::is_css_wide_keyword;
use super::syntax::{
    contains_custom_property_reference, content_value, gradient_arguments, gradient_position,
    property_value, transform_function_arguments,
};

/// A CSS value type accepted by a typed `--value([type])` domain.
///
/// The variants intentionally model CSS value definitions, rather than the utility that
/// happens to consume them. This keeps `p-*`, `border-*`, and `z-*` distinct even when
/// their source spelling contains the same digits.
///
/// Each variant documents the hint spelling that resolves to it. Hints are part of the
/// authored grammar, so a variant may not be renamed without changing what users write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    /// `*`: any non-empty value, with no grammar check.
    Any,
    /// `integer`: an integral `<number>`, including a `calc()` that folds to one.
    Integer,
    /// `number`: `<number>`.
    Number,
    /// `percentage`: `<percentage>`.
    Percentage,
    /// `number-percentage`: `<number> | <percentage>`.
    NumberPercentage,
    /// `length`: `<length>`, which also accepts `<length-percentage>` values.
    Length,
    /// `length-percentage`: `<length-percentage>`.
    LengthPercentage,
    /// `angle`: `<angle>`, which also accepts `<angle-percentage>` values.
    Angle,
    /// `angle-percentage`: `<angle-percentage>`.
    AnglePercentage,
    /// `time`: `<time>`.
    Time,
    /// `ratio`: `<ratio>`, such as `16 / 9`.
    Ratio,
    /// `easing`: `<easing-function>`.
    Easing,
    /// `line-width`: a border-width keyword or `<length>`; percentages are not lengths here.
    LineWidth,
    /// `z-index`: `auto` or an integer.
    ZIndex,
    /// `color`: `<color>`.
    Color,
    /// `image`: `<image>`, including gradients.
    Image,
    /// `url`: a `url()` value.
    Url,
    /// `position`: `<position>`.
    Position,
    /// `bg-size`: `<bg-size>`.
    BackgroundSize,
    /// `shadow`: one box shadow; list splitting is a separate operation.
    Shadow,
    /// `content`: the primitive `content` productions; bare identifiers are rejected.
    Content,
    /// `linear-gradient-arguments`: the argument list of `linear-gradient()`.
    LinearGradientArguments,
    /// `radial-gradient-arguments`: the argument list of `radial-gradient()`.
    RadialGradientArguments,
    /// `conic-gradient-arguments`: the argument list of `conic-gradient()`.
    ConicGradientArguments,
    /// `linear-gradient-position`: the arguments of `linear-gradient()` that come
    /// before additional color stops, such as `to right` or `45deg, red`.
    LinearGradientPosition,
    /// `radial-gradient-position`: the arguments of `radial-gradient()` that come
    /// before additional color stops, such as `at center`.
    RadialGradientPosition,
    /// `conic-gradient-position`: the arguments of `conic-gradient()` that come
    /// before additional color stops, such as `from 45deg`.
    ConicGradientPosition,
    /// `skew-arguments`: the argument list of `skew()`.
    SkewArguments,
    /// `skew-x-arguments`: the argument list of `skewX()`.
    SkewXArguments,
    /// `skew-y-arguments`: the argument list of `skewY()`.
    SkewYArguments,
    /// `animation`: an animation shorthand value, including comma-separated animations.
    Animation,
    /// `font-family` or Tailwind's `family-name`: a font family list.
    FontFamily,
    /// `font-size`: `<font-size>`.
    FontSize,
    /// `font-weight`: `<font-weight>`.
    FontWeight,
    /// `vertical-align`: `<vertical-align>`.
    VerticalAlign,
    /// `grid-track-size`: one track size, validated as `grid-auto-columns`.
    GridTrackSize,
    /// `grid-line`: one grid line, validated as `grid-column`.
    GridLine,
    /// `columns`: the `columns` shorthand.
    Columns,
    /// `contain`: the `contain` property.
    Contain,
    /// `cursor`: the `cursor` property.
    Cursor,
    /// `font-feature-settings`: an OpenType feature list such as `"liga" 1`.
    FontFeatureSettings,
    /// `font-stretch`: `<font-stretch>`.
    FontStretch,
    /// `grid-track-list`: a track list, validated as `grid-template-columns`.
    GridTrackList,
    /// `flex`: the `flex` shorthand.
    Flex,
    /// `list-style-type`: the `list-style-type` property.
    ListStyleType,
    /// `list-style-image`: the `list-style-image` property.
    ListStyleImage,
    /// `tab-size`: the `tab-size` property.
    TabSize,
    /// `transition-property`: the `transition-property` property.
    TransitionProperty,
    /// `will-change`: the `will-change` property.
    WillChange,
    /// `zoom`: the `zoom` property.
    Zoom,
}

impl ValueType {
    /// Parses the public arbitrary-value hint used inside `[hint:value]`.
    ///
    /// `None` means the hint is not part of the grammar at all; a registry that sees an
    /// unknown hint must report it rather than fall back to [`ValueType::Any`].
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
            "linear-gradient-position" => Some(Self::LinearGradientPosition),
            "radial-gradient-arguments" => Some(Self::RadialGradientArguments),
            "radial-gradient-position" => Some(Self::RadialGradientPosition),
            "conic-gradient-arguments" => Some(Self::ConicGradientArguments),
            "conic-gradient-position" => Some(Self::ConicGradientPosition),
            "skew-arguments" => Some(Self::SkewArguments),
            "skew-x-arguments" => Some(Self::SkewXArguments),
            "skew-y-arguments" => Some(Self::SkewYArguments),
            "animation" => Some(Self::Animation),
            "font-family" | "family-name" => Some(Self::FontFamily),
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

    /// Returns the CSSWG formal-syntax productions used to audit this simple value
    /// domain. `None` denotes a compound domain whose grammar is validated by Lightning
    /// CSS (or a focused parser) instead.
    ///
    /// The terms deliberately name CSS value productions, not CSS properties: a
    /// registry audit can therefore discover newly used properties without maintaining
    /// a property allow-list.
    ///
    /// The lists are deliberately loose where CSS is (for example `Integer` also lists
    /// `number`); this is audit metadata, never a validity check.
    #[must_use]
    pub const fn csswg_formal_terms(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Any => Some(&[]),
            // `<integer>` is a valid restricted production wherever the formal syntax
            // accepts `<number>` (for example `tab-size`).
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
            | Self::LinearGradientPosition
            | Self::RadialGradientArguments
            | Self::RadialGradientPosition
            | Self::ConicGradientArguments
            | Self::ConicGradientPosition
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
    /// Lightning CSS owns the grammar for browser CSS types. The small integer check is
    /// layered on top because CSS exposes `<number>`, while Stillwind's utility grammar
    /// needs the narrower integral subset.
    ///
    /// Two rules apply before the grammar: a CSS-wide keyword is always valid, and a
    /// `var()` reference is accepted because a custom property's type is only known at
    /// runtime.
    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        if is_css_wide_keyword(value) {
            return true;
        }
        // Custom properties are substituted before a property's value grammar is
        // computed. Without generated project typings we cannot know a referenced
        // variable's domain, so `var(--token)` (including inside a `calc()`) is
        // correctly a runtime-typed value rather than a static type error. Plain
        // literals continue through the strict parsers.
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
            // Lightning CSS has no typed `content` property parser yet. Keep this
            // narrow exception here rather than treating the whole property as
            // unchecked: it accepts the primitive content productions used by Tailwind
            // and rejects bare identifiers.
            Self::Content => content_value(value),
            Self::LinearGradientArguments => gradient_arguments("linear-gradient", value),
            Self::LinearGradientPosition => gradient_position("linear-gradient", value),
            Self::RadialGradientArguments => gradient_arguments("radial-gradient", value),
            Self::RadialGradientPosition => gradient_position("radial-gradient", value),
            Self::ConicGradientArguments => gradient_arguments("conic-gradient", value),
            Self::ConicGradientPosition => gradient_position("conic-gradient", value),
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

    /// Returns whether an arbitrary-value hint is a valid narrower spelling for this
    /// CSS value type. For example, `<length>` and `<percentage>` may annotate a
    /// `<length-percentage>` value, while `<integer>` may annotate a `z-index` value.
    ///
    /// The relation is one-way: a broader domain does not accept a narrower hint's
    /// siblings, and `Any` accepts every hint.
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
            (ValueType::LinearGradientPosition, "to right", "red / blue"),
            (
                ValueType::RadialGradientArguments,
                "at 20% 30%, red, blue",
                "red / blue",
            ),
            (ValueType::RadialGradientPosition, "at center", "at red"),
            (
                ValueType::ConicGradientArguments,
                "from 45deg, red, blue",
                "red / blue",
            ),
            (ValueType::ConicGradientPosition, "from 45deg", "from red"),
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
        assert_eq!(
            ValueType::from_hint("family-name"),
            Some(ValueType::FontFamily)
        );
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
}
