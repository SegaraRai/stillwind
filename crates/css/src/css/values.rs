use super::{
    Angle, AnglePercentage, BackgroundSize, BorderSideWidth, BoxShadow, CSSNumber, CssColor,
    EasingFunction, Image, Length, LengthPercentage, NumberOrPercentage, Parse, Percentage,
    Position, Ratio, Time, Url, contains_custom_property_reference, content_value,
    gradient_arguments, is_css_wide_keyword, property_value, transform_function_arguments,
};

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
