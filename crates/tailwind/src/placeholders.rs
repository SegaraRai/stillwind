use stillwind_css::ThemeIndex;
use stillwind_grammar::Value;

use super::arbitrary::alpha;
use super::colors::{resolve_compat_color, resolve_shadow_color};
use super::nested::closing_parenthesis;
use super::types::CompatColorMode;
use super::value_domains::{resolve_value_domains, split_domains};
use super::value_macros::{
    resolve_literal_value_placeholders, resolve_theme_suffix_placeholders,
    resolve_value_map_placeholders,
};

#[allow(clippy::too_many_lines)]
pub(super) fn resolve_css_placeholders(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
    compat_color_mode: CompatColorMode,
) -> Option<String> {
    let capture = utility_name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    resolve_literal_value_placeholders(&mut body, capture, theme, compat_color_mode)?;
    resolve_theme_suffix_placeholders(&mut body, capture, theme)?;
    resolve_value_map_placeholders(&mut body, capture)?;
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let domains = &body[open + 1..end];
        // The candidate grammar deliberately represents the slash in `w-1/2`
        // as a modifier. Value domains that explicitly opt into fractions can
        // reassemble that spelling without changing slash semantics for color
        // opacity, fonts, or arbitrary modifiers.
        let fraction_capture = if domains.contains("--fraction()") {
            capture.zip(modifier).and_then(|(capture, modifier)| {
                let modifier = match modifier {
                    Value::Named(value) => value,
                    Value::Arbitrary(_) | Value::CssVariable(_) => return None,
                };
                (capture.parse::<u16>().is_ok() && modifier.parse::<u16>().is_ok())
                    .then(|| format!("{capture}/{modifier}"))
            })
        } else {
            None
        };
        let capture = fraction_capture.as_deref().or(capture);
        let replacement = resolve_value_domains(domains, capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier(") {
        let open = start + "--modifier".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let domains = &body[open + 1..end];
        let replacement = if domains.trim().is_empty() {
            match modifier {
                Value::Named(value) | Value::Arbitrary(value) => value.clone(),
                Value::CssVariable(value) => format!("var({value})"),
            }
        } else {
            let capture = match modifier {
                Value::Named(value) => value.clone(),
                // `Value::Arbitrary` is decoded by the grammar. Re-wrap it
                // so the same typed-domain resolver used by `--value(...)`
                // can check an optional arbitrary-value hint.
                Value::Arbitrary(value) => format!("[{value}]"),
                Value::CssVariable(value) => format!("({value})"),
            };
            resolve_value_domains(domains, Some(&capture), theme)?
        };
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-map(") {
        let open = start + "--modifier-map".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let modifier = match modifier {
            Value::Named(value) | Value::Arbitrary(value) => value,
            Value::CssVariable(_) => return None,
        };
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == modifier).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-none()") {
        if modifier.is_some() {
            return None;
        }
        let declaration_start = body[..start].rfind(';').map_or(0, |end| end + 1);
        let declaration_end = body[start..].find(';').map_or_else(
            || start + "--modifier-none()".len(),
            |offset| start + offset + 1,
        );
        body.replace_range(declaration_start..declaration_end, "");
    }
    while let Some(start) = body.find("--alpha(") {
        let open = start + "--alpha".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = alpha(&body[open + 1..end])?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--compat-color(") {
        let open = start + "--compat-color".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_compat_color(&body[open + 1..end], compat_color_mode)?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--stillwind-builtin-shadow-color(") {
        let open = start + "--stillwind-builtin-shadow-color".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_shadow_color(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")
        && !body.contains("--value-literal(")
        && !body.contains("--stillwind-theme-suffix(")
        && !body.contains("--value-map(")
        && !body.contains("--modifier(")
        && !body.contains("--modifier-map(")
        && !body.contains("--modifier-none()")
        && !body.contains("--alpha(")
        && !body.contains("--compat-color(")
        && !body.contains("--stillwind-builtin-shadow-color("))
    .then_some(body)
}
