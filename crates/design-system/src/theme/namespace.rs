use std::collections::BTreeMap;

use stillwind_css::ValueType;

/// Parses the CSS-authored type set for each Tailwind theme namespace.
#[must_use]
pub fn parse_theme_namespace_types(css: &str) -> BTreeMap<ThemeNamespace, Vec<ValueType>> {
    css.split('@')
        .filter_map(|segment| segment.strip_prefix("stillwind-theme-namespace"))
        .filter_map(|segment| {
            segment
                .split_once(';')
                .map(|(definition, _)| definition.trim())
        })
        .filter_map(|definition| {
            let mut parts = definition.split_whitespace();
            let namespace = parts
                .next()?
                .strip_prefix("--")?
                .strip_suffix("-*")
                .and_then(ThemeNamespace::parse)?;
            let types = parts
                .filter_map(|value| value.strip_prefix('[')?.strip_suffix(']'))
                .filter_map(ValueType::from_hint)
                .collect::<Vec<_>>();
            (!types.is_empty()).then_some((namespace, types))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_namespace_value_types() {
        let css = "@stillwind-animation-option duration-* { animation-duration: --value(--duration-*); }\
                       @stillwind-animation-option-property transition-duration: animation-duration;\
                       @stillwind-theme-namespace --duration-* [time];\
                       @stillwind-theme-namespace --leading-* [number] [length];";
        let namespaces = parse_theme_namespace_types(css);
        assert_eq!(namespaces[&ThemeNamespace::Duration], vec![ValueType::Time]);
        assert_eq!(
            namespaces[&ThemeNamespace::Leading],
            vec![ValueType::Number, ValueType::Length]
        );
    }

    #[test]
    fn namespace_names_round_trip_and_metadata_ignores_unknown_types() {
        for name in [
            "animate",
            "aspect-ratio",
            "blur",
            "breakpoint",
            "color",
            "container",
            "drop-shadow",
            "ease",
            "font",
            "font-stretch",
            "font-weight",
            "inset-shadow",
            "leading",
            "perspective",
            "radius",
            "shadow",
            "spacing",
            "text",
            "text-shadow",
            "tracking",
            "z-index",
            "duration",
        ] {
            assert_eq!(ThemeNamespace::parse(name).unwrap().css_name(), name);
        }
        assert_eq!(ThemeNamespace::parse("Color"), None);
        let types = parse_theme_namespace_types(
            "@stillwind-theme-namespace --duration-* [time] [unknown]; @stillwind-theme-namespace --bogus-* [length];",
        );
        assert_eq!(
            types,
            BTreeMap::from([(ThemeNamespace::Duration, vec![ValueType::Time])])
        );
    }
}
