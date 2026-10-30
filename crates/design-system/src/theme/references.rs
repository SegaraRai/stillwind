//! Replaces `--theme()` and `theme()` references while CSS is generated.
//!
//! Tailwind replaces these references at build time, not in the browser, so the result
//! depends on which tokens are active. In a token value, `--theme()` has three possible
//! results, and callers must handle each one:
//!
//! - A defined token becomes a `var()` reference.
//! - An undefined token with a fallback becomes the fallback.
//! - An undefined token without a fallback, or with the fallback `initial`, removes the
//!   declaration that contains it. The functions report this with `None`.
//!
//! A modifier such as `/50` applies 50% opacity to the reference or the fallback. In an
//! arbitrary value, `theme()` is replaced with the value of the token as written.
//!
//! Elsewhere, `theme()` with a custom property behaves as `--theme()`. The v3 path
//! form, such as `theme(colors.red.500)`, stays unchanged, and stylesheet lint reports
//! it.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{find_css_functions, split_once_top_level};

/// Lowers scale functions such as `--spacing(<value>)` and Tailwind's public
/// `--alpha(<color> / <alpha>)` function outside CSS strings, as Tailwind 4.3 lowers
/// them.
///
/// Each entry of `scales` names a scale function, such as `--spacing`, and the text
/// that refers to its scale token, such as `var(--spacing)`, or `None` when the scale
/// token is not active. `--spacing(n)` becomes `calc(<scale> * n)`, except that `0`
/// becomes `0px` and `1` becomes the scale itself. Empty `--spacing()` is the value
/// domain of `--value()` and `--modifier()`, so it is kept. `--alpha()` without a
/// top-level `/` is Stillwind's alpha placeholder of utility bodies, so it is also
/// kept.
///
/// Returns `None` for a malformed call, for a scale function without an active scale,
/// and for expansion beyond the size or nesting limits.
pub(crate) fn resolve_css_functions(
    value: &str,
    scales: &[(LeanString, Option<LeanString>)],
) -> Option<LeanString> {
    let names = std::iter::once("--alpha")
        .chain(scales.iter().map(|(name, _)| name.as_str()))
        .collect::<Vec<_>>();
    let mut output = LeanString::from(value);
    for _ in 0..32 {
        let mut changed = false;
        let calls = find_css_functions(&output, &names, 0);
        let leaves: Vec<_> = calls
            .iter()
            .enumerate()
            .filter(|(index, call)| {
                calls
                    .get(index + 1)
                    .is_none_or(|next| next.start >= call.end.unwrap_or(output.len()))
            })
            .map(|(_, call)| call.clone())
            .collect();
        for call in leaves.into_iter().rev() {
            let end = call.end?;
            let arguments = output[call.arguments.clone()].trim();
            let replacement = if let Some((_, scale)) = call
                .name_index
                .checked_sub(1)
                .and_then(|index| scales.get(index))
            {
                if arguments.is_empty() {
                    continue;
                }
                let scale = scale.as_deref()?;
                // Tailwind replaces 0 with `0px` to keep a <length> for `calc()`, and 1
                // with the scale itself. Other values keep the authored argument text.
                match arguments.parse::<f64>() {
                    Ok(0.0) => "0px".to_owned(),
                    Ok(1.0) => scale.to_owned(),
                    _ => format!("calc({scale} * {arguments})"),
                }
            } else {
                let (color, opacity) =
                    split_once_top_level(arguments, |token| matches!(token, Token::Delim('/')));
                let Some(opacity) = opacity else {
                    continue;
                };
                let color = color.trim();
                let opacity = opacity.trim();
                if color.is_empty() || opacity.is_empty() {
                    return None;
                }
                // Tailwind's `withAlpha()` multiplies every plain number by 100, so
                // `--alpha(red / 50)` means 5000%, which `color-mix()` rejects, so the
                // declaration is dropped by the browser. Stillwind's own alpha modifiers
                // treat a number above 1 as a percentage instead; this function keeps
                // Tailwind's result because CSS packages are written against it.
                let percentage = opacity.parse::<f64>().map_or_else(
                    |_| opacity.to_owned(),
                    |value| format!("{}%", value * 100.0),
                );
                if percentage == "100%" {
                    color.to_owned()
                } else {
                    format!("color-mix(in oklab, {color} {percentage}, transparent)")
                }
            };
            output.replace_range(call.start..end, &replacement);
            changed = true;
        }
        if !changed {
            return Some(output);
        }
        if output.len() > 1_048_576 {
            return None;
        }
    }
    None
}

/// Respells each `theme()` call that names a custom property as `--theme()`, which
/// utility bodies keep until the stylesheet renders.
///
/// Returns `None` when `value` has no such call. A call that reads a v3 path stays
/// unchanged.
pub(crate) fn dashed_theme_calls(value: &str) -> Option<LeanString> {
    let mut output = LeanString::new();
    let mut cursor = 0;
    for found in find_css_functions(value, &["theme"], 0) {
        if value[found.arguments].trim().starts_with("--") {
            output.push_str(&value[cursor..found.start]);
            output.push_str("--");
            cursor = found.start;
        }
    }
    (cursor != 0 || !output.is_empty()).then(|| {
        output.push_str(&value[cursor..]);
        output
    })
}

/// A malformed `--theme()` reference and its byte offset within a value.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ThemeReferenceError {
    /// The function starting at the byte offset has no closing parenthesis.
    #[error("unterminated --theme() at byte {offset}")]
    Unterminated {
        /// Zero-based byte offset within the value.
        offset: usize,
    },
    /// The function does not name a custom property.
    #[error("invalid --theme() property `{reference}` at byte {offset}")]
    InvalidProperty {
        /// Zero-based byte offset within the value.
        offset: usize,
        /// Name supplied as the first function argument.
        reference: LeanStr,
    },
}

impl ThemeReferenceError {
    /// Byte offset of the failing function within its input value.
    pub const fn offset(&self) -> usize {
        match self {
            Self::Unterminated { offset } | Self::InvalidProperty { offset, .. } => *offset,
        }
    }
}

/// Rewrites every `--theme()` reference and every `theme()` reference to a custom
/// property in `value`, asking `is_defined` about the referenced custom property.
///
/// Scanning respects CSS syntax, so a reference inside a string is left alone. A
/// `Ok(None)` omits the containing declaration when an undefined token has no usable
/// fallback.
///
/// # Errors
///
/// Returns the offset and syntax cause for an invalid reference or argument list.
pub(crate) fn resolve_theme_references(
    value: &str,
    mut is_defined: impl FnMut(&str) -> bool,
) -> Result<Option<LeanString>, ThemeReferenceError> {
    resolve_references(value, &mut is_defined, 0, 0)
}

/// Replaces the references in `value` in the same way as `resolve_theme_references`.
/// `depth` is the nesting level of fallback arguments. `base_offset` is the offset of
/// `value` in the original input, and error offsets include it. Returns `Ok(None)` when
/// references are nested deeper than 64 levels.
///
/// # Errors
///
/// Returns the offset and the cause of an invalid reference or argument list.
fn resolve_references(
    value: &str,
    is_defined: &mut impl FnMut(&str) -> bool,
    depth: usize,
    base_offset: usize,
) -> Result<Option<LeanString>, ThemeReferenceError> {
    if depth > 64 {
        return Ok(None);
    }

    let mut output = LeanString::with_capacity(value.len());
    let mut cursor = 0;
    let mut omitted = false;
    for found in find_css_functions(value, &["--theme", "theme"], 0) {
        let start = found.start;
        if start < cursor
            || (found.name_index == 1 && !value[found.arguments.clone()].trim().starts_with("--"))
        {
            continue;
        }
        output.push_str(&value[cursor..start]);
        let end = found.end.ok_or(ThemeReferenceError::Unterminated {
            offset: base_offset + start,
        })?;
        let arguments_end = found.arguments.end;
        let arguments = &value[found.arguments];
        let (reference, fallback) =
            split_once_top_level(arguments, |token| matches!(token, Token::Comma));
        let (reference, opacity) =
            split_once_top_level(reference, |token| matches!(token, Token::Delim('/')));
        let Some(reference) = property_reference(reference) else {
            return Err(ThemeReferenceError::InvalidProperty {
                offset: base_offset + start,
                reference: reference.trim().to_lean_str(),
            });
        };
        let fallback = fallback
            .map(str::trim_start)
            .filter(|fallback| !fallback.trim_end().is_empty() && fallback.trim_end() != "initial")
            .map(|fallback| {
                resolve_references(
                    fallback.trim_end(),
                    is_defined,
                    depth + 1,
                    base_offset + arguments_end - fallback.len(),
                )
            })
            .transpose()?
            .flatten();
        let mut replacement = String::new();
        if is_defined(reference) {
            replacement.push_str("var(");
            replacement.push_str(reference);
            if let Some(fallback) = fallback {
                replacement.push_str(", ");
                replacement.push_str(&fallback);
            }
            replacement.push(')');
        } else {
            let Some(fallback) = fallback else {
                omitted = true;
                cursor = end;
                continue;
            };
            if fallback == "initial" {
                omitted = true;
                cursor = end;
                continue;
            }
            replacement.push_str(&fallback);
        }
        if let Some(opacity) = opacity {
            let opacity = opacity.trim();
            if opacity.is_empty() {
                return Err(ThemeReferenceError::InvalidProperty {
                    offset: base_offset + start,
                    reference: reference.to_lean_str(),
                });
            }
            let opacity = opacity
                .parse::<f64>()
                .map_or_else(|_| opacity.to_string(), |number| format!("{number}%"));
            if opacity != "100%" {
                replacement = format!("color-mix(in oklab, {replacement} {opacity}, transparent)");
            }
        }
        output.push_str(&replacement);
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    Ok((!omitted).then_some(output))
}

/// Returns the custom property name in `value`, such as `--color-red` for
/// ` --color-red `. Whitespace and comments around the name are ignored, and escapes in
/// the name are kept. Returns `None` when `value` is not exactly one custom property
/// name.
fn property_reference(value: &str) -> Option<&str> {
    let mut parser = Parser::new(value);
    loop {
        let start = parser.position().byte_index();
        match parser
            .next_including_whitespace_and_comments()
            .cloned()
            .ok()?
        {
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::Ident(name) if name.starts_with("--") && name.len() > 2 => {
                let end = parser.position().byte_index();
                return parser.next().is_err().then_some(&value[start..end]);
            }
            _ => return None,
        }
    }
}

/// Replaces every `theme(--name)` and `theme(--name, fallback)` call in an arbitrary
/// value with the authored value that `token_value` returns for `--name`, or with the
/// fallback when `token_value` returns [`None`].
///
/// A reference inside a string is left alone. Returns [`None`] when a call is
/// unterminated, does not name a custom property, or names an unavailable token
/// without a fallback, because the browser cannot evaluate `theme()`.
pub(crate) fn inline_theme_functions<'a>(
    value: &str,
    mut token_value: impl FnMut(&str) -> Option<&'a str>,
) -> Option<LeanString> {
    let mut output = LeanString::with_capacity(value.len());
    let mut cursor = 0;
    for found in find_css_functions(value, &["theme"], 0) {
        if found.start < cursor {
            continue;
        }
        output.push_str(&value[cursor..found.start]);
        let (reference, fallback) = split_once_top_level(&value[found.arguments], |token| {
            matches!(token, Token::Comma)
        });
        let reference = reference.trim();
        if !reference.starts_with("--") {
            return None;
        }
        let replacement = token_value(reference).or_else(|| {
            fallback
                .map(str::trim)
                .filter(|fallback| !fallback.is_empty())
        })?;
        output.push_str(replacement);
        cursor = found.end?;
    }
    output.push_str(&value[cursor..]);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_references_lower_opacity_and_nested_fallbacks() {
        for (raw, expected) in [
            (
                "--theme(--red/50)",
                "color-mix(in oklab, var(--red) 50%, transparent)",
            ),
            ("--theme(--red/100%)", "var(--red)"),
            (
                "--theme(--missing,--theme(--red/25))",
                "color-mix(in oklab, var(--red) 25%, transparent)",
            ),
            (
                "--theme(--red/var(--opacity))",
                "color-mix(in oklab, var(--red) var(--opacity), transparent)",
            ),
        ] {
            assert_eq!(
                resolve_theme_references(raw, |name| name == "--red")
                    .unwrap()
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn nested_reference_errors_identify_the_function_in_the_original_value() {
        let value = "red --theme(--missing,  --theme(invalid)  )";
        let error = resolve_theme_references(value, |_| false).unwrap_err();
        assert_eq!(error.offset(), value.find("--theme(invalid)").unwrap());
        assert!(matches!(error, ThemeReferenceError::InvalidProperty { .. }));
    }

    #[test]
    fn public_css_functions_respect_nesting_strings_and_opacity() {
        let scale = |value: Option<&str>| [("--spacing".into(), value.map(Into::into))];
        let spacing = scale(Some("var(--spacing)"));
        for (source, expected) in [
            ("--spacing(4)", "calc(var(--spacing) * 4)"),
            ("--spacing(0)", "0px"),
            ("--spacing(1)", "var(--spacing)"),
            (
                "--spacing(--spacing(2))",
                "calc(var(--spacing) * calc(var(--spacing) * 2))",
            ),
            (
                "--alpha(rgb(1 2 3 / .5) / .25)",
                "color-mix(in oklab, rgb(1 2 3 / .5) 25%, transparent)",
            ),
            (
                "--alpha(red / 50)",
                "color-mix(in oklab, red 5000%, transparent)",
            ),
            ("--alpha(red / 1)", "red"),
            ("'--spacing(4)'", "'--spacing(4)'"),
            ("--alpha(50)", "--alpha(50)"),
            (
                "--value(--spacing-*, --spacing())",
                "--value(--spacing-*, --spacing())",
            ),
        ] {
            assert_eq!(
                resolve_css_functions(source, &spacing).as_deref(),
                Some(expected),
                "{source}"
            );
        }
        assert_eq!(
            resolve_css_functions("--spacing(4)", &scale(Some("0.25rem"))).as_deref(),
            Some("calc(0.25rem * 4)")
        );
        assert_eq!(
            resolve_css_functions("--gap(4)", &spacing).as_deref(),
            Some("--gap(4)")
        );
        assert!(resolve_css_functions("--spacing(4)", &scale(None)).is_none());
        assert!(resolve_css_functions("--spacing(", &spacing).is_none());
        assert!(resolve_css_functions("--alpha(red / )", &spacing).is_none());
    }

    #[test]
    fn theme_calls_with_custom_properties_become_dashed_calls() {
        assert_eq!(
            dashed_theme_calls("theme(--a) --theme(--b) theme(colors.red) theme(--c, red)")
                .as_deref(),
            Some("--theme(--a) --theme(--b) theme(colors.red) --theme(--c, red)")
        );
        for value in ["red", "--theme(--a)", "theme(colors.red)", "'theme(--a)'"] {
            assert_eq!(dashed_theme_calls(value), None, "{value}");
        }
    }

    #[test]
    fn theme_functions_inline_token_values_or_fallbacks() {
        let token = |name: &str| (name == "--color-red").then_some("oklch(60% 0.2 25)");
        for (value, expected) in [
            ("theme(--color-red)", Some("oklch(60% 0.2 25)")),
            (
                "0 0 2px theme(--color-red)",
                Some("0 0 2px oklch(60% 0.2 25)"),
            ),
            ("theme(--color-blue, blue)", Some("blue")),
            ("--theme(--color-red)", Some("--theme(--color-red)")),
            ("'theme(--color-red)'", Some("'theme(--color-red)'")),
            ("theme(--color-blue)", None),
            ("theme(colors.red.500)", None),
            ("theme(--color-red", None),
        ] {
            assert_eq!(
                inline_theme_functions(value, token).as_deref(),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn defined_missing_and_initial_fallbacks_are_distinct() {
        for (value, defined, expected) in [
            ("--theme(--x)", true, Some("var(--x)")),
            ("--theme(--x/* , */)", true, Some("var(--x)")),
            (r"--theme(--x\,part)", true, Some(r"var(--x\,part)")),
            ("--theme(--x, initial)", true, Some("var(--x)")),
            ("--theme(--x, red)", true, Some("var(--x, red)")),
            ("--theme(--x, rgb(1, 2, 3))", false, Some("rgb(1, 2, 3)")),
            ("--theme(--x, initial)", false, None),
            ("--theme(--x)", false, None),
        ] {
            assert_eq!(
                resolve_theme_references(value, |_| defined)
                    .unwrap()
                    .as_deref(),
                expected,
                "{value}"
            );
        }
        assert_eq!(
            resolve_theme_references("'--theme(--x)'", |_| false)
                .unwrap()
                .as_deref(),
            Some("'--theme(--x)'")
        );
        assert!(matches!(
            resolve_theme_references("--theme(x)", |_| true),
            Err(ThemeReferenceError::InvalidProperty { offset: 0, .. })
        ));
        assert_eq!(
            resolve_theme_references("--theme(--x", |_| true),
            Err(ThemeReferenceError::Unterminated { offset: 0 })
        );
        assert!(matches!(
            resolve_theme_references("--theme(--missing) --theme(bad)", |_| false),
            Err(ThemeReferenceError::InvalidProperty { offset: 19, .. })
        ));
    }
}
