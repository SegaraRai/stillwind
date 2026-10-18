//! Lowering of build-time `--theme()` and `theme()` references.
//!
//! Tailwind resolves these while generating CSS rather than in the browser, so the
//! result depends on which tokens are active. In token values, `--theme()` has three
//! outcomes that must stay distinguishable: a defined token becomes a `var()`
//! reference, an undefined token with a fallback uses that fallback, and an undefined
//! token whose fallback is `initial` (or that has no fallback) drops the containing
//! declaration entirely, which [`None`] reports to the caller.
//!
//! In arbitrary values, `theme()` is replaced with the token's authored value.

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{find_css_functions, split_css_function_arguments};

/// Lowers Tailwind's public `--spacing(<value>)` and `--alpha(<color> / <alpha>)`
/// functions outside CSS strings, as Tailwind 4.3 lowers them.
///
/// `spacing` is the text that refers to the spacing scale token, such as
/// `var(--spacing)`, or `None` when the theme has no scale. `--spacing(n)` becomes
/// `calc(<spacing> * n)`, except that `0` becomes `0px` and `1` becomes the scale
/// itself. Empty `--spacing()` is the spacing value domain of `--value()` and
/// `--modifier()`, so it is kept. `--alpha()` without a top-level `/` is Stillwind's
/// alpha placeholder of utility bodies, so it is also kept.
///
/// Returns `None` for a malformed call, for `--spacing(n)` without a scale, and for
/// expansion beyond the size or nesting limits.
pub(crate) fn resolve_css_functions(value: &str, spacing: Option<&str>) -> Option<LeanString> {
    let mut output = LeanString::from(value);
    for _ in 0..32 {
        let mut changed = false;
        let calls = find_css_functions(&output, &["--spacing", "--alpha"], 0);
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
            let replacement = if call.name_index == 0 {
                if arguments.is_empty() {
                    continue;
                }
                let spacing = spacing?;
                // Tailwind replaces 0 with `0px` to keep a <length> for `calc()`, and 1
                // with the scale itself. Other values keep the authored argument text.
                match arguments.parse::<f64>() {
                    Ok(0.0) => "0px".to_owned(),
                    Ok(1.0) => spacing.to_owned(),
                    _ => format!("calc({spacing} * {arguments})"),
                }
            } else {
                let mut depth = 0_u32;
                let mut quote = None;
                let mut escaped = false;
                let slash = arguments.char_indices().find_map(|(index, character)| {
                    if escaped {
                        escaped = false;
                        return None;
                    }
                    if character == '\\' {
                        escaped = true;
                        return None;
                    }
                    if let Some(current) = quote {
                        if current == character {
                            quote = None;
                        }
                        return None;
                    }
                    if matches!(character, '\'' | '"') {
                        quote = Some(character);
                    } else if character == '(' || character == '[' {
                        depth += 1;
                    } else if character == ')' || character == ']' {
                        depth = depth.saturating_sub(1);
                    } else if character == '/' && depth == 0 {
                        return Some(index);
                    }
                    None
                });
                let Some(slash) = slash else {
                    continue;
                };
                let color = arguments[..slash].trim();
                let opacity = arguments[slash + 1..].trim();
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

/// Rewrites every `--theme()` reference in `value`, asking `is_defined` about the
/// referenced custom property.
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
    let mut output = LeanString::with_capacity(value.len());
    let mut cursor = 0;
    let mut omitted = false;
    for found in find_css_functions(value, &["--theme"], 0) {
        let start = found.start;
        if start < cursor {
            continue;
        }
        output.push_str(&value[cursor..start]);
        let end = found
            .end
            .ok_or(ThemeReferenceError::Unterminated { offset: start })?;
        let arguments = &value[found.arguments];
        let (reference, fallback) = split_css_function_arguments(arguments);
        let reference = reference.trim();
        if !reference.starts_with("--") {
            return Err(ThemeReferenceError::InvalidProperty {
                offset: start,
                reference: reference.to_lean_str(),
            });
        }
        let fallback = fallback
            .map(str::trim)
            .filter(|fallback| !fallback.is_empty());
        if is_defined(reference) {
            output.push_str("var(");
            output.push_str(reference);
            if let Some(fallback) = fallback.filter(|fallback| *fallback != "initial") {
                output.push_str(", ");
                output.push_str(fallback);
            }
            output.push(')');
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
            output.push_str(fallback);
        }
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    Ok((!omitted).then_some(output))
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
        let (reference, fallback) = split_css_function_arguments(&value[found.arguments]);
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
    fn public_css_functions_respect_nesting_strings_and_opacity() {
        let spacing = Some("var(--spacing)");
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
                resolve_css_functions(source, spacing).as_deref(),
                Some(expected),
                "{source}"
            );
        }
        assert_eq!(
            resolve_css_functions("--spacing(4)", Some("0.25rem")).as_deref(),
            Some("calc(0.25rem * 4)")
        );
        assert!(resolve_css_functions("--spacing(4)", None).is_none());
        assert!(resolve_css_functions("--spacing(", spacing).is_none());
        assert!(resolve_css_functions("--alpha(red / )", spacing).is_none());
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
