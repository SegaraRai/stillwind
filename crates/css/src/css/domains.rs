use super::ValueType;

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

pub(super) fn is_css_wide_keyword(value: &str) -> bool {
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
