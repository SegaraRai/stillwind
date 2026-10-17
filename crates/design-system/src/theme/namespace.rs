//! The Tailwind theme namespaces that participate in utility and variant resolution,
//! their CSS-authored value types, and the token names that a key selects in any
//! namespace.
//!
//! [`ThemeNamespace`] is a closed list of the namespaces that have built-in meaning:
//! value types, a scale, or classification of `@theme` tokens. A custom property
//! outside it never creates a value for a built-in utility. An authored
//! `--value(--size-*)` pattern can still read any namespace through
//! [`namespace_key_names`].
//!
//! Value types are declared in CSS, not here, so a registry update can change which
//! arbitrary values a namespace accepts without a Rust change.

use std::collections::BTreeMap;

use lean_string::{LeanString, ToLeanString};

use stillwind_css::ValueType;
use stillwind_source::ReplaceText;

/// CSS value domain for named spacing tokens and numeric multiples of the spacing scale.
pub const SPACING_VALUE_DOMAIN: &str = "--spacing()";

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
/// resolution. A custom property outside these namespaces creates no value for a
/// built-in utility.
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

/// Returns the custom-property names that the candidate key `key` can select in the
/// namespace `namespace`, in lookup order, as Tailwind resolves
/// `--value(--namespace-*)`.
///
/// `namespace` is the namespace name without the leading `--` and the trailing `-`,
/// such as `size` or `size-lg`. The first name joins `namespace` and `key` unchanged.
/// When `key` contains `.`, the names with `_` in place of each `.` follow, so `1.5`
/// also selects `--size-1_5`. Each name is followed by its spelling with CSS escapes,
/// such as `--size-1\.5`, because `@theme` keeps token names as authored.
pub(super) fn namespace_key_names(namespace: &str, key: &str) -> Vec<LeanString> {
    let mut keys = vec![key.to_lean_string()];
    if key.contains('.') {
        keys.push(key.replace_char('.', "_"));
    }
    let mut names = Vec::new();
    for key in keys {
        names.push(format_args!("--{namespace}-{key}").to_lean_string());
        let escaped = escape_key(&key);
        if escaped != key {
            names.push(format_args!("--{namespace}-{escaped}").to_lean_string());
        }
    }
    names
}

/// Escapes each ASCII character of `key` that a CSS identifier cannot contain
/// unescaped.
fn escape_key(key: &str) -> LeanString {
    let mut escaped = LeanString::new();
    for character in key.chars() {
        if character.is_ascii()
            && !character.is_ascii_alphanumeric()
            && !matches!(character, '-' | '_')
        {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// Returns whether Tailwind excludes the token `name` from the namespace `namespace`
/// because it belongs to a narrower namespace.
///
/// For example, `--font-weight-bold` is excluded from `font`, and `--inset-shadow-sm`
/// is excluded from `inset`. `namespace` is written as in [`namespace_key_names`].
pub(super) fn excluded_from_namespace(namespace: &str, name: &str) -> bool {
    let narrower: &[&str] = match namespace {
        "font" => &["weight", "size"],
        "inset" => &["shadow", "ring"],
        "text" => &[
            "color",
            "decoration-color",
            "decoration-thickness",
            "indent",
            "shadow",
            "underline-offset",
        ],
        "grid-column" | "grid-row" => &["start", "end"],
        _ => &[],
    };
    let Some(key) = name
        .strip_prefix("--")
        .and_then(|name| name.strip_prefix(namespace))
        .and_then(|name| name.strip_prefix('-'))
    else {
        return false;
    };
    narrower.iter().any(|narrower| {
        key.strip_prefix(narrower)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_value_domain_uses_the_spacing_namespace() {
        assert_eq!(
            SPACING_VALUE_DOMAIN,
            format!("--{}()", ThemeNamespace::Spacing.css_name())
        );
    }

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

    #[test]
    fn namespace_keys_try_underscores_for_dots_and_escaped_spellings() {
        assert_eq!(namespace_key_names("size", "4"), ["--size-4"]);
        assert_eq!(
            namespace_key_names("size-lg", "1.5"),
            ["--size-lg-1.5", r"--size-lg-1\.5", "--size-lg-1_5",]
        );
        assert_eq!(
            namespace_key_names("size", "1/2"),
            ["--size-1/2", r"--size-1\/2"]
        );
    }

    #[test]
    fn narrower_namespaces_are_excluded_like_tailwind() {
        assert!(excluded_from_namespace("font", "--font-weight-bold"));
        assert!(excluded_from_namespace("inset", "--inset-shadow-sm"));
        assert!(excluded_from_namespace("grid-row", "--grid-row-start-2"));
        assert!(!excluded_from_namespace("font", "--font-weighty"));
        assert!(!excluded_from_namespace("font", "--font-sans"));
        assert!(!excluded_from_namespace("size", "--size-weight-1"));
    }
}
