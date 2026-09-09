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
