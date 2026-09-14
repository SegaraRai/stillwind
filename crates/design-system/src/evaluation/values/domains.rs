//! Value-domain resolution: which spellings a `--value(...)` placeholder accepts.
//!
//! A domain list is tried in authored order and the first matching domain wins, so a
//! definition controls precedence by ordering its alternatives. Domains come in four
//! shapes: literal keywords, CSS grammar hints such as `[length]`, the special forms
//! `integer`, `number`, `percentage`, `--spacing()`, `--fraction()`, and the axis-size
//! built-ins, and theme namespace patterns such as `--color-*`.

use cssparser::{Parser, Token};

use stillwind_css::ValueDomain;

use super::arbitrary::{decode_arbitrary, parse_css_variable};
use super::theme::{axis_size_value, fraction_value, theme_value, theme_value_literal};
use crate::conditionals::accepts_runtime_value;
use crate::{ThemeIndex, ThemeNamespace};

/// Resolves the first matching domain in a comma-separated domain list.
///
/// A theme namespace domain looks the capture up in that namespace and may append an
/// authored suffix; a CSS grammar domain accepts either a parenthesized variable (whose
/// optional hint must fit) or an arbitrary value (whose value and hint must fit).
/// Whole `--if` values must have two branches accepted by the same domain; nested
/// expressions use deferred-variable validation after their conditional syntax is checked.
/// [`None`] means no domain matched, including malformed conditional captures.
pub(crate) fn resolve_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if domain == "integer" {
            return capture
                .filter(|value| value.parse::<i64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "number" {
            return capture
                .filter(|value| value.parse::<f64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "percentage" {
            return capture
                .and_then(|value| value.strip_suffix('%'))
                .filter(|value| value.parse::<f64>().is_ok())
                .map(|value| format!("{value}%"));
        }
        if domain == "--spacing()" {
            return capture
                .and_then(|capture| theme_value(ThemeNamespace::Spacing, capture, theme));
        }
        if domain == "--fraction()" {
            return capture.and_then(fraction_value);
        }
        if domain == "--stillwind-builtin-width()" {
            return capture.and_then(|capture| axis_size_value(capture, "vw"));
        }
        if domain == "--stillwind-builtin-height()" {
            return capture.and_then(|capture| axis_size_value(capture, "vh"));
        }
        if let Some((namespace, suffix)) = theme_domain_pattern(domain) {
            return capture
                .and_then(|capture| theme_value(namespace, &format!("{capture}{suffix}"), theme));
        }
        if !domain.starts_with('[') && !domain.starts_with("--") {
            return capture.filter(|value| *value == domain).map(|value| {
                if value == "current" {
                    "currentcolor".to_owned()
                } else {
                    value.to_owned()
                }
            });
        }
        let expected = ValueDomain::from_hint(domain.strip_prefix('[')?.strip_suffix(']')?)?;
        if let Some(variable) = parse_css_variable(capture?) {
            let hint_is_compatible = variable.hint.is_none_or(|hint| expected.accepts_hint(hint));
            return hint_is_compatible.then(|| format!("var({})", variable.name));
        }
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (hint, value) = arbitrary_hint(arbitrary);
        let value = decode_arbitrary(value);
        let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
        (hint_is_compatible && accepts_runtime_value(expected, &value)).then_some(value)
    })
}

/// Separates a type hint at the first top-level colon, leaving conditional tests,
/// quoted strings, and function arguments intact.
/// Returns `(hint, value)` without trimming, or `("", input)` if no colon is found.
fn arbitrary_hint(value: &str) -> (&str, &str) {
    let mut parser = Parser::new(value);
    while let Ok(token) = parser.next() {
        if matches!(token, Token::Colon) {
            let end = parser.position().byte_index();
            return (&value[..end - 1], &value[end..]);
        }
    }
    ("", value)
}

/// Resolves a typed domain to its authored theme token value rather than a
/// `var(--token)` reference. Tailwind uses this form for broad-gamut-safe sRGB
/// fallbacks, while retaining the variable reference in the upgraded branch.
///
/// Non-theme domains fallback to [`resolve_value_domains`] so a list mixing literal and
/// theme domains keeps its authored order.
pub(crate) fn resolve_literal_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if let Some((namespace, suffix)) = theme_domain_pattern(domain) {
            return capture.and_then(|capture| {
                theme_value_literal(namespace, &format!("{capture}{suffix}"), theme)
            });
        }
        resolve_value_domains(domain, capture, theme)
    })
}

/// Splits a domain list on top-level commas.
///
/// Commas inside parentheses or brackets belong to a function argument list and are not
/// separators. Surrounding whitespace is preserved, because a domain may be a prefix
/// pattern such as `--spacing-*` followed by an authored suffix.
pub(crate) fn split_domains(value: &str) -> Vec<&str> {
    let mut domains = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (index, character) in value.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                domains.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    domains.push(&value[start..]);
    domains
}

/// Splits a theme namespace pattern into its namespace and trailing suffix.
///
/// Only a pattern of the form `--namespace-*` is recognized; an unknown namespace or a
/// missing `-*` marker is not a theme domain, which keeps a custom property out of
/// utility resolution.
pub(crate) fn theme_domain_pattern(domain: &str) -> Option<(ThemeNamespace, &str)> {
    let domain = domain.strip_prefix("--")?;
    let (namespace, suffix) = domain.split_once("-*")?;
    Some((ThemeNamespace::parse(namespace)?, suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_lists_keep_function_commas_and_typed_capture_boundaries() {
        assert_eq!(
            split_domains("[length], --spacing-*, f(a,b)"),
            vec!["[length]", " --spacing-*", " f(a,b)"]
        );
        assert_eq!(
            theme_domain_pattern("--color-*"),
            Some((ThemeNamespace::Color, ""))
        );
        assert_eq!(theme_domain_pattern("--missing-*"), None);
        let theme = ThemeIndex::default();
        assert_eq!(
            resolve_value_domains("[length]", Some("[2px]"), &theme),
            Some("2px".into())
        );
        assert_eq!(
            resolve_value_domains("[length]", Some("[red]"), &theme),
            None
        );
        assert_eq!(
            resolve_literal_value_domains("[number]", Some("[2]"), &theme),
            Some("2".into())
        );
        assert_eq!(resolve_value_domains("[number]", None, &theme), None);
    }
}
