//! Color compatibility branches and theme shadow color modifiers.
//!
//! The compatibility path can emit a color declaration twice: an sRGB branch and a
//! modern branch guarded by a capability query. The legacy branch mixes in sRGB, the
//! modern branch uses `oklab`, and both are produced here so the two passes stay
//! visibly paired.
//!
//! Legacy `currentcolor` skips mixing, while its modern branch can still mix. `inherit`
//! and `transparent` select oklab in both passes when no extra modern alpha is
//! supplied. Identical branch text can then be emitted without a capability wrapper.

use stillwind_css::{shadow_with_color_fallback, shadow_with_color_override};

use super::super::builtins::metadata::{COMPAT_COLOR_LEGACY_SPACE, COMPAT_COLOR_MODERN_SPACE};
use super::domains::split_domains;
use super::theme::theme_value_literal;
use crate::{ThemeIndex, ThemeNamespace};

/// Which of the two compatibility branches is being produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompatColorMode {
    /// Fallback branch.
    Legacy,
    /// Upgraded branch, guarded by the capability query.
    Modern,
}

/// How an authored color interacts with the compatibility branches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompatColorKind {
    /// `currentcolor`, which the legacy branch cannot mix.
    CurrentColor,
    /// `inherit` or `transparent`, selecting the modern space in both passes by default.
    ModernOnlyKeyword,
    /// Any other authored color.
    General,
}

impl CompatColorKind {
    /// Classifies an authored color spelling, case-insensitively.
    pub(crate) fn from_css(color: &str) -> Self {
        if color.eq_ignore_ascii_case("currentcolor") {
            Self::CurrentColor
        } else if color.eq_ignore_ascii_case("inherit") || color.eq_ignore_ascii_case("transparent")
        {
            Self::ModernOnlyKeyword
        } else {
            Self::General
        }
    }
}

/// Produces the legacy Color 4 fallback for an authored color and alpha. The caller
/// also emits the modern upgrade guarded by the capability query intrinsic to
/// `--compat-color()`.
///
/// The argument list is a domain list: a color and an alpha, optionally with a second
/// alpha for the modern branch. `100%` alpha and legacy `currentcolor` are emitted
/// without a `color-mix()` at the initial mixing step. The modern branch may still
/// differ, particularly for `currentcolor` or an additional modern alpha.
pub(crate) fn resolve_compat_color(arguments: &str, mode: CompatColorMode) -> Option<String> {
    let arguments = split_domains(arguments);
    let (color, alpha, modern_alpha) = match arguments.as_slice() {
        [color, alpha] => (*color, *alpha, None),
        [color, alpha, modern_alpha] => (*color, *alpha, Some(modern_alpha.trim())),
        _ => return None,
    };
    let color = color.trim();
    let alpha = alpha.trim();
    if color.is_empty() || alpha.is_empty() {
        return None;
    }
    let color_kind = CompatColorKind::from_css(color);
    if color.eq_ignore_ascii_case("inherit") && alpha == "100%" {
        return Some(color.to_owned());
    }
    let color_space = match (mode, color_kind, modern_alpha.is_none()) {
        // Tailwind does not synthesize an sRGB compatibility branch for CSS
        // wide/transparent keywords. Keeping both passes identical makes the
        // declaration unconditional instead of wrapping the modern form in an
        // unnecessary capability query.
        (_, CompatColorKind::ModernOnlyKeyword, true) | (CompatColorMode::Modern, _, _) => {
            COMPAT_COLOR_MODERN_SPACE
        }
        (CompatColorMode::Legacy, _, _) => COMPAT_COLOR_LEGACY_SPACE,
    };
    let mixed = if alpha == "100%"
        || (mode == CompatColorMode::Legacy && color_kind == CompatColorKind::CurrentColor)
    {
        color.to_owned()
    } else {
        format!("color-mix(in {color_space}, {color} {alpha}, transparent)")
    };
    if mode == CompatColorMode::Modern
        && let Some(modern_alpha) = modern_alpha
    {
        return Some(format!(
            "color-mix(in {COMPAT_COLOR_MODERN_SPACE}, {mixed} {modern_alpha}, transparent)"
        ));
    }
    Some(mixed)
}

/// CSS-first extension for theme shadow opacity modifiers. `--value()` emits the normal
/// `var(--shadow-*)` reference; this intentionally reads the authored token to
/// construct a CSS Color 5 relative color function.
///
/// The first argument names the theme namespace whose token value is read, the second
/// names the color property that will hold the result, and the optional third is an
/// alpha override. The property name must be a custom property, because the generated
/// function replaces that property inside the shadow.
pub(crate) fn resolve_shadow_color(
    arguments: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    let arguments = split_domains(arguments);
    let (domain, color_property, alpha) = match arguments.as_slice() {
        [domain, color_property] => (*domain, *color_property, None),
        [domain, color_property, alpha] => (*domain, *color_property, Some(alpha.trim())),
        _ => return None,
    };
    let namespace = domain
        .trim()
        .strip_prefix("--")
        .and_then(|domain| domain.strip_suffix("-*"))
        .and_then(ThemeNamespace::parse)?;
    let value = theme_value_literal(namespace, capture?, theme)?;
    let color_property = color_property.trim();
    color_property.strip_prefix("--").filter(|name| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
    })?;
    match alpha {
        Some(alpha) => shadow_with_color_fallback(&value, color_property, alpha),
        None => shadow_with_color_override(&value, color_property, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_color_modes_handle_keywords_and_explicit_alpha() {
        assert_eq!(
            CompatColorKind::from_css("CURRENTCOLOR"),
            CompatColorKind::CurrentColor
        );
        assert_eq!(
            CompatColorKind::from_css("transparent"),
            CompatColorKind::ModernOnlyKeyword
        );
        assert_eq!(CompatColorKind::from_css("red"), CompatColorKind::General);
        assert_eq!(
            resolve_compat_color("red, 50%", CompatColorMode::Legacy),
            Some("color-mix(in srgb, red 50%, transparent)".into())
        );
        assert_eq!(
            resolve_compat_color("red, 50%", CompatColorMode::Modern),
            Some("color-mix(in oklab, red 50%, transparent)".into())
        );
        assert_eq!(
            resolve_compat_color("inherit, 100%", CompatColorMode::Legacy),
            Some("inherit".into())
        );
        assert_eq!(resolve_compat_color("red", CompatColorMode::Legacy), None);
        assert_eq!(resolve_compat_color(", 50%", CompatColorMode::Legacy), None);
    }
}
