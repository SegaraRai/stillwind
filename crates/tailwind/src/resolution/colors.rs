use super::{
    COMPAT_COLOR_LEGACY_SPACE, COMPAT_COLOR_MODERN_SPACE, CompatColorMode, ThemeIndex,
    ThemeNamespace, shadow_with_color_fallback, shadow_with_color_override, split_domains,
    theme_value_literal,
};

/// Produces the legacy Color 4 fallback for an authored color and alpha.
/// The caller also emits the modern upgrade guarded by the capability query
/// intrinsic to `--compat-color()`.
pub(super) fn resolve_compat_color(arguments: &str, mode: CompatColorMode) -> Option<String> {
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
        // declaration unconditional instead of wrapping the modern form in
        // an unnecessary capability query.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompatColorKind {
    CurrentColor,
    ModernOnlyKeyword,
    General,
}

impl CompatColorKind {
    pub(super) fn from_css(color: &str) -> Self {
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

/// CSS-first extension for theme shadow opacity modifiers. `--value()` emits
/// the normal `var(--shadow-*)` reference; this intentionally reads the
/// authored token to construct a CSS Color 5 relative colour function.
pub(super) fn resolve_shadow_color(
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
