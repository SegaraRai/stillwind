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
