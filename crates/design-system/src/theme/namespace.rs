//! The Tailwind theme namespaces that participate in utility and variant resolution,
//! and their CSS-authored value types.
//!
//! Namespace membership is a closed list. A custom property outside it stays available
//! as authored CSS but never creates a candidate value, which is what stops a project's
//! own variable from accidentally becoming a utility.
//!
//! Value types are declared in CSS, not here, so a registry update can change which
//! arbitrary values a namespace accepts without a Rust change.

use std::collections::BTreeMap;

use stillwind_css::ValueType;

/// Parses the CSS-authored type set for each Tailwind theme namespace.
///
/// Each statement names one namespace pattern and the value types it accepts; unknown
/// namespaces, types, and statements without any recognized type are skipped, and a
/// later statement replaces an earlier one for the same namespace.
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

/// Tailwind v4 theme namespaces that participate in built-in utility and variant
/// resolution. Unknown custom properties remain available as authored CSS but never
/// accidentally create a utility.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ThemeNamespace {
    /// `--animate-*`: animation shorthand values.
    Animate,
    /// `--aspect-ratio-*`: `aspect-ratio` values.
    AspectRatio,
    /// `--blur-*`: blur radii for filter utilities.
    Blur,
    /// `--breakpoint-*`: media-query minimum widths for responsive variants.
    Breakpoint,
    /// `--color-*`: color values.
    Color,
    /// `--container-*`: container-query widths for `@container` variants.
    Container,
    /// `--drop-shadow-*`: `drop-shadow()` filter values.
    DropShadow,
    /// `--ease-*`: transition timing functions.
    Ease,
    /// `--font-*`: font family lists.
    Font,
    /// `--font-stretch-*`: `font-stretch` percentages.
    FontStretch,
    /// `--font-weight-*`: numeric font weights.
    FontWeight,
    /// `--inset-shadow-*`: inset box-shadow values.
    InsetShadow,
    /// `--leading-*`: line heights.
    Leading,
    /// `--perspective-*`: 3D perspective distances.
    Perspective,
    /// `--radius-*`: corner radii.
    Radius,
    /// `--shadow-*`: box-shadow values.
    Shadow,
    /// `--spacing-*`: the spacing scale used by padding, margin, gap, and size utilities.
    Spacing,
    /// `--text-*`: font sizes.
    Text,
    /// `--text-shadow-*`: text-shadow values.
    TextShadow,
    /// `--tracking-*`: letter spacing.
    Tracking,
    /// `--z-index-*`: stacking order values.
    ZIndex,
    /// `--duration-*`: transition durations.
    Duration,
}

impl ThemeNamespace {
    /// The namespace's name as it appears in CSS custom properties, for example `color`
    /// for `--color-*`.
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

    /// Returns the namespace with the longest name that the theme token `name` belongs
    /// to: `text-shadow` for `--text-shadow-sm` and `text` for `--text-sm`.
    ///
    /// Returns `None` for a name outside every namespace. It also returns `None` for a
    /// name with a second `--`, such as `--text-sm--line-height`, because that token
    /// holds a different property of `--text-sm`, and for a namespace reset such as
    /// `--color-*`.
    #[must_use]
    pub fn of_token(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("--")?;
        if rest.contains("--") || rest.contains('*') {
            return None;
        }
        rest.match_indices('-')
            .map(|(index, _)| &rest[..index])
            .chain(std::iter::once(rest))
            .filter_map(Self::parse)
            .next_back()
    }

    /// Parses a namespace name; matching is case-sensitive.
    ///
    /// The order of the candidates is checked by the test suite against
    /// [`Self::css_name`] for every variant, so a new namespace must be added to both
    /// lists.
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
    fn tokens_belong_to_the_longest_namespace_without_token_properties() {
        assert_eq!(
            ThemeNamespace::of_token("--text-sm"),
            Some(ThemeNamespace::Text)
        );
        assert_eq!(
            ThemeNamespace::of_token("--text-shadow-sm"),
            Some(ThemeNamespace::TextShadow)
        );
        assert_eq!(
            ThemeNamespace::of_token("--spacing"),
            Some(ThemeNamespace::Spacing)
        );
        for name in ["--text-sm--line-height", "--my-len", "--color-*", "--*"] {
            assert_eq!(ThemeNamespace::of_token(name), None, "{name}");
        }
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
