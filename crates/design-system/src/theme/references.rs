//! Lowering of build-time `--theme()` references inside token values.
//!
//! Tailwind resolves these while generating CSS rather than in the browser, so the
//! result depends on which tokens are active. Three outcomes exist and must stay
//! distinguishable: a defined token becomes a `var()` reference, an undefined token
//! with a fallback uses that fallback, and an undefined token whose fallback is
//! `initial` (or that has no fallback) drops the containing declaration entirely,
//! which [`None`] reports to the caller.

use stillwind_css::functions::{
    find_css_function, matching_css_delimiter, split_css_function_arguments,
};

/// Rewrites every `--theme()` reference in `value`, asking `is_defined` about the
/// referenced custom property.
///
/// Scanning respects CSS syntax, so a reference inside a string is left alone. A
/// reference that does not name a `--` property, or one with an unterminated argument
/// list, fails the whole value instead of being passed through.
pub(crate) fn resolve_theme_references(
    value: &str,
    mut is_defined: impl FnMut(&str) -> bool,
) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(start) = find_css_function(value, "--theme", cursor) {
        output.push_str(&value[cursor..start]);
        let open = start + "--theme".len();
        let close = matching_css_delimiter(value, open, '(', ')')?;
        let arguments = &value[open + 1..close];
        let (reference, fallback) = split_css_function_arguments(arguments);
        let reference = reference.trim();
        if !reference.starts_with("--") {
            return None;
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
            let fallback = fallback?;
            if fallback == "initial" {
                return None;
            }
            output.push_str(fallback);
        }
        cursor = close + 1;
    }
    output.push_str(&value[cursor..]);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defined_missing_and_initial_fallbacks_are_distinct() {
        for (value, defined, expected) in [
            ("--theme(--x)", true, Some("var(--x)")),
            ("--theme(--x, initial)", true, Some("var(--x)")),
            ("--theme(--x, red)", true, Some("var(--x, red)")),
            ("--theme(--x, rgb(1, 2, 3))", false, Some("rgb(1, 2, 3)")),
            ("--theme(--x, initial)", false, None),
            ("--theme(--x)", false, None),
            ("--theme(x)", true, None),
            ("--theme(--x", true, None),
        ] {
            assert_eq!(
                resolve_theme_references(value, |_| defined).as_deref(),
                expected,
                "{value}"
            );
        }
        assert_eq!(
            resolve_theme_references("'--theme(--x)'", |_| false).as_deref(),
            Some("'--theme(--x)'")
        );
    }
}
