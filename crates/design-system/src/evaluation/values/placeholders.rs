//! Ordered expansion of every placeholder family in one resolved declaration.
//!
//! Placeholders are resolved in a fixed order because later ones may depend on earlier
//! text: nested macros first, then `--value(...)`, then the modifier forms, then
//! `--alpha(...)`, and finally the color forms. Every remaining placeholder at the end
//! from the recognized marker list fails the declaration. Unknown function names
//! are not rejected by this check and can remain in generated CSS.
//!
//! The capture a placeholder sees is the candidate text after the matched definition's
//! prefix, with a slash modifier kept separately; a fraction domain is the one case
//! where the two are reassembled, and only when both sides are unsigned integers.

use stillwind_css::declaration_ranges;
use stillwind_css::functions::find_css_functions;
use stillwind_syntax::Value;

use super::arbitrary::alpha;
use super::colors::{CompatColorMode, resolve_compat_color, resolve_shadow_color};
use super::domains::{resolve_value_domains, split_domains};
use super::expansion::expand_placeholders;
use super::macros::{
    resolve_literal_value_placeholder, resolve_theme_suffix_placeholder,
    resolve_value_map_placeholder,
};
use crate::ThemeIndex;

// Order is semantic: replacements may introduce functions from later families.
const PLACEHOLDERS: &[&str] = &[
    "--value-literal",
    "--stillwind-theme-suffix",
    "--value-map",
    "--value",
    "--modifier",
    "--modifier-map",
    "--modifier-none",
    "--alpha",
    "--compat-color",
    "--stillwind-builtin-shadow-color",
];

/// Resolves all placeholders in `body` for one candidate.
///
/// `utility_name` supplies the wildcard prefix that yields the capture; `modifier` is
/// the authored slash modifier, which is optional and may be an arbitrary value or a
/// variable. [`None`] means the declaration does not apply to this candidate, which
/// lets the caller try the next overload.
#[allow(clippy::too_many_lines)]
pub(crate) fn resolve_css_placeholders(
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
    expand_placeholders(
        &mut body,
        &PLACEHOLDERS[..6],
        |function, arguments| match function {
            "--value-literal" => {
                resolve_literal_value_placeholder(arguments, capture, theme, compat_color_mode)
            }
            "--stillwind-theme-suffix" => {
                resolve_theme_suffix_placeholder(arguments, capture, theme)
            }
            "--value-map" => resolve_value_map_placeholder(arguments, capture),
            "--value" => {
                let domains = arguments;
                // The candidate grammar deliberately represents the slash in `w-1/2` as a
                // modifier. Value domains that explicitly opt into fractions can reassemble
                // that spelling without changing slash semantics for color opacity, fonts, or
                // arbitrary modifiers.
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
                Some(replacement)
            }
            "--modifier" => {
                let modifier = modifier?;
                let domains = arguments;
                let replacement = if domains.trim().is_empty() {
                    match modifier {
                        Value::Named(value) | Value::Arbitrary(value) => value.clone(),
                        Value::CssVariable(value) => format!("var({value})"),
                    }
                } else {
                    let capture = match modifier {
                        Value::Named(value) => value.clone(),
                        // `Value::Arbitrary` is decoded by the grammar. Re-wrap it so the same
                        // typed-domain resolver used by `--value(...)` can check an optional
                        // arbitrary-value hint.
                        Value::Arbitrary(value) => format!("[{value}]"),
                        Value::CssVariable(value) => format!("({value})"),
                    };
                    resolve_value_domains(domains, Some(&capture), theme)?
                };
                Some(replacement)
            }
            "--modifier-map" => {
                let modifier = modifier?;
                let modifier = match modifier {
                    Value::Named(value) | Value::Arbitrary(value) => value,
                    Value::CssVariable(_) => return None,
                };
                let replacement = split_domains(arguments)
                    .into_iter()
                    .find_map(|entry| {
                        let (key, value) = entry.trim().split_once(':')?;
                        (key.trim() == modifier).then(|| value.trim())
                    })?
                    .to_owned();
                Some(replacement)
            }
            _ => None,
        },
    )?;
    // This family removes the enclosing declaration, so it forms a phase boundary.
    remove_modifier_none(&mut body, modifier)?;
    expand_placeholders(
        &mut body,
        &PLACEHOLDERS[7..],
        |function, arguments| match function {
            "--alpha" => alpha(arguments),
            "--compat-color" => resolve_compat_color(arguments, compat_color_mode),
            "--stillwind-builtin-shadow-color" => resolve_shadow_color(arguments, capture, theme),
            _ => None,
        },
    )?;
    find_css_functions(&body, PLACEHOLDERS, 0)
        .is_empty()
        .then_some(body)
}

/// Removes each declaration selected by an empty modifier guard once.
fn remove_modifier_none(body: &mut String, modifier: Option<&Value>) -> Option<()> {
    let matches = find_css_functions(body, &["--modifier-none"], 0);
    if matches.is_empty() {
        return Some(());
    }
    if modifier.is_some() {
        return None;
    }
    let declarations = declaration_ranges(body);
    let mut removed = Vec::new();
    let mut cursor = 0;
    for found in matches {
        // A removed declaration also consumes subsequent guards in that declaration.
        if found.start < cursor {
            continue;
        }
        if !body[found.start..].starts_with("--modifier-none()") {
            return None;
        }
        let range = declarations
            .iter()
            .find(|range| range.contains(&found.start))?;
        cursor = range.end + usize::from(body.as_bytes().get(range.end) == Some(&b';'));
        removed.push(range.start..cursor);
    }
    for range in removed.into_iter().rev() {
        body.replace_range(range, "");
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_resolution_preserves_phase_barrier_and_rejects_reintroduced_families() {
        let theme = ThemeIndex::default();
        let resolve = |body| {
            resolve_css_placeholders(body, "x-*", "x-a", None, &theme, CompatColorMode::Legacy)
        };
        assert_eq!(
            resolve("opacity:--value-map(a:--alpha(.5));"),
            Some("opacity:50%;".into())
        );
        assert_eq!(resolve("x:--value-map(a:--value-literal([number]));"), None);
        assert_eq!(
            resolve(
                "x:--modifier-none() --modifier-none(invalid);y:--alpha(.5);z:--modifier-none();"
            ),
            Some("y:50%;".into())
        );
        assert_eq!(resolve("x:--value([number]) --modifier-none();"), None);
        assert_eq!(
            resolve("x:--modifier-none() --alpha(invalid);"),
            Some(String::new())
        );
        assert_eq!(
            resolve("content:'--value([number])';x:custom(1);"),
            Some("content:'--value([number])';x:custom(1);".into())
        );
    }

    #[test]
    fn placeholders_reject_incompatible_values_and_missing_modifiers() {
        let theme = ThemeIndex::default();
        assert_eq!(
            resolve_css_placeholders(
                "width:--value([length]);",
                "w-*",
                "w-[2px]",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            Some("width:2px;".into())
        );
        assert_eq!(
            resolve_css_placeholders(
                "width:--value([length]);",
                "w-*",
                "w-[red]",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            None
        );
        assert_eq!(
            resolve_css_placeholders(
                "opacity:--modifier([number]);",
                "x-*",
                "x-a",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            None
        );
        assert_eq!(
            resolve_css_placeholders(
                "opacity:--alpha(.5);",
                "x",
                "x",
                None,
                &theme,
                CompatColorMode::Legacy
            ),
            Some("opacity:50%;".into())
        );
    }
}
