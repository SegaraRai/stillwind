//! CSS color conversion, declaration colors, and rewriting for shadow utilities.
//!
//! Lightning CSS supplies color and shadow parsing. [`color_rgba`] resolves concrete
//! colors for editor displays, and [`css_color_values`] collects them from declarations.
//! Shadow rewriting preserves custom-property overrides and relative-color fallbacks.
//!
//! Authored text is preserved where it matters: a shadow whose color is written last
//! keeps its exact spelling inside the fallback, so precision the author wrote is not
//! lost to a reprint.

use lean_string::{LeanString, ToLeanString};
use lightningcss::printer::PrinterOptions;
use lightningcss::properties::box_shadow::BoxShadow;
use lightningcss::traits::{Parse, ToCss};
use lightningcss::values::color::CssColor;

use stillwind_source::JoinText;

use crate::{CssRule, Declaration};

/// Rewrites box-shadow colors with a custom-property override and CSS Color 5
/// relative-color fallback. This is the semantic operation Tailwind uses for
/// `shadow-*/opacity` families, while the parser owns the shadow grammar.
#[must_use]
pub fn shadow_with_color_fallback(
    value: &str,
    color_property: &str,
    alpha: &str,
) -> Option<LeanString> {
    shadow_with_color_override(value, color_property, Some(alpha))
}

/// Rewrites every shadow in a comma-separated shadow list so a utility color can
/// override the authored color. When `alpha` is present, the authored color is
/// additionally converted to Tailwind's relative-color fallback.
///
/// Returns `None` when any shadow cannot be parsed or serialized. An omitted color
/// uses the parser's default color; the whole list is rewritten or no result is
/// returned.
#[must_use]
pub fn shadow_with_color_override(
    value: &str,
    color_property: &str,
    alpha: Option<&str>,
) -> Option<LeanString> {
    split_top_level_commas(value)
        .into_iter()
        .map(|shadow| rewrite_shadow_color(shadow, color_property, alpha))
        .collect::<Option<Vec<_>>>()
        .map(|shadows| shadows.join_text(", "))
}

/// Rewrites one shadow, returning `None` when it cannot be parsed or its color cannot
/// be serialized.
///
/// The trailing-component path keeps the authored color text; the fallback path
/// reprints the parsed shadow, which is why it is only used when the color is not the
/// final component.
fn rewrite_shadow_color(
    value: &str,
    color_property: &str,
    alpha: Option<&str>,
) -> Option<LeanString> {
    let shadow = BoxShadow::parse_string(value).ok()?;
    if let Some((base, color)) = trailing_top_level_css_component(value)
        && CssColor::parse_string(color).is_ok()
    {
        let fallback = alpha.map_or_else(
            || color.to_lean_string(),
            |alpha| format_args!("oklab(from {color} l a b / {alpha})").to_lean_string(),
        );
        return Some(format_args!("{base} var({color_property}, {fallback})").to_lean_string());
    }

    // The fallback remains grammar-driven for shadows whose color is not written as the
    // final component. Built-in theme shadows take the source preserving path above, so
    // their authored precision is retained.
    let color: LeanString = shadow
        .color
        .to_css_string(PrinterOptions::default())
        .ok()?
        .into();
    let lengths = [
        &shadow.x_offset,
        &shadow.y_offset,
        &shadow.blur,
        &shadow.spread,
    ]
    .into_iter()
    .map(|length| length.to_css_string(PrinterOptions::default()).ok())
    .collect::<Option<Vec<_>>>()?
    .join_text(" ");
    let without_color = if shadow.inset {
        format_args!("inset {lengths}").to_lean_string()
    } else {
        lengths
    };
    let fallback = alpha.map_or_else(
        || color.clone(),
        |alpha| format_args!("oklab(from {color} l a b / {alpha})").to_lean_string(),
    );
    Some(format_args!("{without_color} var({color_property}, {fallback})").to_lean_string())
}

/// Splits on commas that are not inside parentheses, brackets, quotes or escapes, and
/// trims each component.
fn split_top_level_commas(value: &str) -> Vec<&str> {
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

/// Serializes a theme color as an sRGB fallback with the requested alpha. Tailwind
/// emits the legacy fallback before its Color 5 `color-mix` upgrade; `LightningCSS`
/// provides the color-space conversion we need here.
///
/// A six-digit hex color with a percentage alpha becomes an eight-digit hex, because
/// that is the legacy form; every other color keeps its serialized spelling with the
/// alpha appended. `None` means the value is not a color.
#[must_use]
pub fn color_with_alpha_fallback(value: &str, alpha: &str) -> Option<LeanString> {
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
        // Clamping and rounding guarantee an integer-valued result in the inclusive u8
        // range before this intentional float conversion.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = (percent.clamp(0.0, 100.0) * 255.0 / 100.0).round() as u8;
        return Some(format_args!("#{hex}{byte:02x}").to_lean_string());
    }
    Some(format_args!("{serialized} / {alpha}").to_lean_string())
}

/// Converts a fully determined CSS color to sRGB components in the inclusive range 0..=1.
/// Returns `None` for contextual colors, invalid values, or unsupported conversions.
#[must_use]
pub fn color_rgba(value: &str) -> Option<[f64; 4]> {
    let CssColor::RGBA(color) = CssColor::parse_string(value).ok()?.to_rgb().ok()? else {
        return None;
    };
    Some([color.red, color.green, color.blue, color.alpha].map(|value| f64::from(value) / 255.0))
}

/// Returns concrete sRGB colors from flat declarations and nested style rules.
/// The caller resolves theme variables first. Contextual colors and opaque values are skipped.
#[must_use]
pub fn css_color_values(css: &str) -> Vec<[f64; 4]> {
    fn collect(css: &str, depth: usize, output: &mut Vec<[f64; 4]>) {
        if depth > 64 {
            return;
        }
        if let Some(declarations) = Declaration::parse(css) {
            for declaration in declarations {
                if declaration.property == "color"
                    || declaration.property.ends_with("-color")
                    || declaration.property.starts_with("--")
                    || matches!(
                        declaration.property.as_str(),
                        "fill" | "stroke" | "background"
                    )
                {
                    let value = declaration
                        .css
                        .split_once(':')
                        .map(|(_, value)| value.trim().trim_end_matches("!important").trim());
                    if let Some(color) = value.and_then(color_rgba) {
                        output.push(color);
                    }
                }
            }
        } else if let Ok(rules) = CssRule::parse(css) {
            for rule in rules {
                if !rule.prelude.trim_start().starts_with("@property")
                    && let Some(body) = rule.body
                {
                    collect(&body, depth + 1, output);
                }
            }
        }
    }

    let mut values = Vec::new();
    collect(css, 0, &mut values);
    values
}

/// Splits `value` into everything before the last top-level whitespace and the
/// non-empty component after it.
///
/// Used to find a shadow's trailing color without assuming the color is a single token;
/// `None` when there is no such boundary.
fn trailing_top_level_css_component(value: &str) -> Option<(&str, &str)> {
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

/// Checks matching quote delimiters and rejects a backslash immediately before the
/// last quote. This lexical approximation accepts empty strings and does not validate
/// interior quotes or count backslashes to determine escape parity.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_colors_include_custom_properties_and_skip_contextual_values() {
        assert_eq!(
            css_color_values("& { --gradient-from: red; --size: 1rem; color: currentColor; }"),
            vec![[1.0, 0.0, 0.0, 1.0]]
        );
    }

    #[test]
    fn rewrites_each_authored_color_in_a_shadow_list() {
        assert_eq!(
            shadow_with_color_override(
                "0 0 1px rgb(255 0 0 / 0.4), 0 0 3px rgb(0 0 255 / 0.2)",
                "--tw-shadow-color",
                None,
            ),
            Some(
                "0 0 1px var(--tw-shadow-color, rgb(255 0 0 / 0.4)), 0 0 3px var(--tw-shadow-color, rgb(0 0 255 / 0.2))"
                    .to_lean_string()
            )
        );
    }

    #[test]
    fn preserves_authored_shadow_color_when_applying_an_alpha_modifier() {
        assert_eq!(
            shadow_with_color_fallback(
                "inset 0 2px 4px rgb(0 0 0 / 0.05)",
                "--tw-inset-shadow-color",
                "50%",
            ),
            Some(
                "inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%))"
                    .to_lean_string()
            )
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn comma_and_component_scanners_preserve_nested_and_quoted_text() {
        assert_eq!(
            split_top_level_commas("rgb(1, 2, 3), 'a,b', blue"),
            vec!["rgb(1, 2, 3)", "'a,b'", "blue"]
        );
        assert!(is_css_string("'a b'"));
        assert!(!is_css_string("'a"));
        assert!(!is_css_string("a"));
        assert_eq!(
            trailing_top_level_css_component("0 1px rgb(1, 2, 3)"),
            Some(("0 1px", "rgb(1, 2, 3)"))
        );

        assert_eq!(color_with_alpha_fallback("invalid!", "50%"), None);
    }
}
