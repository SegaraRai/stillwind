use lightningcss::printer::PrinterOptions;
use lightningcss::properties::box_shadow::BoxShadow;
use lightningcss::traits::{Parse, ToCss};
use lightningcss::values::color::CssColor;

/// Rewrites box-shadow colours with a custom-property override and CSS Color 5
/// relative-colour fallback. This is the semantic operation Tailwind uses for
/// `shadow-*/opacity` families, while the parser owns the shadow grammar.
#[must_use]
pub fn shadow_with_color_fallback(
    value: &str,
    color_property: &str,
    alpha: &str,
) -> Option<String> {
    shadow_with_color_override(value, color_property, Some(alpha))
}

/// Rewrites every shadow in a comma-separated shadow list so a utility color
/// can override the authored color. When `alpha` is present, the authored
/// color is additionally converted to Tailwind's relative-color fallback.
#[must_use]
pub fn shadow_with_color_override(
    value: &str,
    color_property: &str,
    alpha: Option<&str>,
) -> Option<String> {
    split_top_level_commas(value)
        .into_iter()
        .map(|shadow| rewrite_shadow_color(shadow, color_property, alpha))
        .collect::<Option<Vec<_>>>()
        .map(|shadows| shadows.join(", "))
}

pub(super) fn rewrite_shadow_color(
    value: &str,
    color_property: &str,
    alpha: Option<&str>,
) -> Option<String> {
    let shadow = BoxShadow::parse_string(value).ok()?;
    if let Some((base, color)) = trailing_top_level_css_component(value)
        && CssColor::parse_string(color).is_ok()
    {
        let fallback = alpha.map_or_else(
            || color.to_owned(),
            |alpha| format!("oklab(from {color} l a b / {alpha})"),
        );
        return Some(format!("{base} var({color_property}, {fallback})"));
    }

    // The fallback remains grammar-driven for shadows whose colour is not
    // written as the final component. Built-in theme shadows take the source
    // preserving path above, so their authored precision is retained.
    let color = shadow.color.to_css_string(PrinterOptions::default()).ok()?;
    let serialized = shadow.to_css_string(PrinterOptions::default()).ok()?;
    let without_color = serialized
        .strip_suffix(&format!(" {color}"))
        .unwrap_or(&serialized);
    let fallback = alpha.map_or_else(
        || color.clone(),
        |alpha| format!("oklab(from {color} l a b / {alpha})"),
    );
    Some(format!("{without_color} var({color_property}, {fallback})"))
}

pub(super) fn split_top_level_commas(value: &str) -> Vec<&str> {
    let mut components = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
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
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                components.push(value[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    components.push(value[start..].trim());
    components
}

/// Serializes a theme color as an sRGB fallback with the requested alpha.
/// Tailwind emits the legacy fallback before its Color 5 `color-mix` upgrade;
/// `LightningCSS` provides the color-space conversion we need here.
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
        // Clamping and rounding guarantee an integer-valued result in the
        // inclusive u8 range before this intentional float conversion.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = (percent.clamp(0.0, 100.0) * 255.0 / 100.0).round() as u8;
        return Some(format!("#{hex}{byte:02x}"));
    }
    Some(format!("{serialized} / {alpha}"))
}

pub(super) fn trailing_top_level_css_component(value: &str) -> Option<(&str, &str)> {
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

pub(super) fn is_css_string(value: &str) -> bool {
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

/// Canonicalizes serializer-only transparent color spellings emitted by CSS
/// color conversion. This is independent of any utility or framework name.
pub fn normalize_serialized_declaration(value: &str) -> String {
    value
        .replace(
            "oklab(0% none none/0) oklab(0% none none/0) oklab(0% none none/0) oklab(0% none none/0)",
            "#0000",
        )
        .replace(":oklab(0% none none/0);", ":#0000;")
}
