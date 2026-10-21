//! CSS color conversion, declaration colors, and rewriting for shadow utilities.
//!
//! Lightning CSS supplies color and shadow parsing. [`color_rgba`] and [`color_hex`]
//! resolve concrete colors for editor displays. Shadow rewriting preserves
//! custom-property overrides and relative-color fallbacks.
//!
//! Authored text is preserved where it matters: a shadow whose color is written last
//! keeps its exact spelling inside the fallback, so precision the author wrote is not
//! lost to a reprint.

use cssparser::{Parser, Token};
use lean_string::{LeanString, ToLeanString};
use lightningcss::printer::PrinterOptions;
use lightningcss::properties::box_shadow::BoxShadow;
use lightningcss::traits::{Parse, ToCss};
use lightningcss::values::color::{CssColor, SRGB};

use stillwind_source::JoinText;

use crate::lists::split_top_level;
use crate::parser::{closer, consume_css_block, validate_css_syntax};

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
    split_top_level(value, |token| matches!(token, Token::Comma))
        .into_iter()
        .map(|shadow| rewrite_shadow_color(shadow.trim(), color_property, alpha))
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
    // final component. The package's theme shadows take the source preserving path
    // above, so their authored precision is retained.
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

/// Converts a fully determined CSS color to sRGB components in the inclusive range 0..=1.
/// Returns `None` for contextual colors, invalid values, or unsupported conversions.
#[must_use]
pub fn color_rgba(value: &str) -> Option<[f64; 4]> {
    let CssColor::RGBA(color) = CssColor::parse_string(value).ok()?.to_rgb().ok()? else {
        return None;
    };
    Some([color.red, color.green, color.blue, color.alpha].map(|value| f64::from(value) / 255.0))
}

/// A fully determined CSS color serialized as hexadecimal sRGB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexColor {
    /// Lowercase `#rrggbb`, or `#rrggbbaa` when the color is not fully opaque.
    pub hex: LeanString,
    /// False when the source color lies outside the sRGB gamut. The hex value then
    /// contains the CSS Color 4 gamut-mapped color, which [`color_rgba`] also returns.
    pub in_gamut: bool,
}

/// Converts a fully determined CSS color to hexadecimal sRGB.
/// Returns `None` for contextual colors, invalid values, or unsupported conversions.
#[must_use]
pub fn color_hex(value: &str) -> Option<HexColor> {
    // Components within half of one 8-bit step serialize to the same hex digits as the
    // exact color, so they count as inside the gamut.
    const TOLERANCE: f32 = 0.5 / 255.0;
    let color = CssColor::parse_string(value).ok()?;
    let srgb = SRGB::try_from(&color).ok()?;
    let in_gamut = [srgb.r, srgb.g, srgb.b]
        .iter()
        .all(|component| (-TOLERANCE..=1.0 + TOLERANCE).contains(component));
    let CssColor::RGBA(rgba) = color.to_rgb().ok()? else {
        return None;
    };
    let mut hex =
        format_args!("#{:02x}{:02x}{:02x}", rgba.red, rgba.green, rgba.blue).to_lean_string();
    if rgba.alpha != u8::MAX {
        hex.push_str(&format_args!("{:02x}", rgba.alpha).to_lean_string());
    }
    Some(HexColor { hex, in_gamut })
}

/// Splits `value` into everything before the last top-level whitespace and the
/// non-empty component after it.
///
/// Used to find a shadow's trailing color without assuming the color is a single token;
/// `None` when there is no such boundary.
fn trailing_top_level_css_component(value: &str) -> Option<(&str, &str)> {
    let mut parser = Parser::new(value);
    let mut boundary = None;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        let block = closer(token).is_some();
        if matches!(token, Token::WhiteSpace(_)) {
            boundary = Some(start);
        }
        if block {
            let _ = consume_css_block(&mut parser);
        }
    }
    let boundary = boundary?;
    let (base, component) = value.split_at(boundary);
    let component = component.trim();
    (!base.trim().is_empty() && !component.is_empty()).then_some((base.trim_end(), component))
}

/// Accepts one complete CSS string token with no trailing tokens.
pub(super) fn is_css_string(value: &str) -> bool {
    let mut parser = Parser::new(value);
    matches!(
        parser.next_including_whitespace_and_comments(),
        Ok(Token::QuotedString(_))
    ) && parser.position().byte_index() == value.len()
        && validate_css_syntax(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_report_whether_gamut_mapping_was_needed() {
        assert_eq!(
            color_hex("oklch(63.7% 0.237 25.331)"),
            Some(HexColor {
                hex: "#fb2c36".into(),
                in_gamut: true,
            })
        );
        assert_eq!(
            color_hex("rgb(255 0 0 / 50%)"),
            Some(HexColor {
                hex: "#ff000080".into(),
                in_gamut: true,
            })
        );
        // The mapped hex matches the gamut-mapped components that color swatches use.
        assert_eq!(
            color_hex("oklch(72.3% 0.219 149.579)"),
            Some(HexColor {
                hex: "#00c758".into(),
                in_gamut: false,
            })
        );
        assert_eq!(
            color_rgba("oklch(72.3% 0.219 149.579)"),
            Some([0.0, 199.0 / 255.0, 88.0 / 255.0, 1.0])
        );
        assert_eq!(color_hex("currentColor"), None);
        assert_eq!(color_hex("1rem"), None);
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
        assert!(is_css_string("'a b'"));
        assert!(!is_css_string("'a"));
        assert!(!is_css_string("a"));
        assert_eq!(
            trailing_top_level_css_component("0 1px rgb(1, 2, 3)"),
            Some(("0 1px", "rgb(1, 2, 3)"))
        );
    }
}
