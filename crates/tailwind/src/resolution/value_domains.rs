use stillwind_css::{ThemeIndex, ThemeNamespace, ValueDomain};

use super::arbitrary::{decode_arbitrary, parse_css_variable};
use super::theme_values::{axis_size_value, fraction_value, theme_value, theme_value_literal};

pub(super) fn resolve_value_domains(
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
        let (hint, value) = arbitrary.split_once(':').unwrap_or(("", arbitrary));
        let value = decode_arbitrary(value);
        let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
        (hint_is_compatible && expected.accepts(&value)).then_some(value)
    })
}

/// Resolves a typed domain to its authored theme token value rather than a
/// `var(--token)` reference. Tailwind uses this form for broad-gamut-safe sRGB
/// fallbacks, while retaining the variable reference in the upgraded branch.
pub(super) fn resolve_literal_value_domains(
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

pub(super) fn split_domains(value: &str) -> Vec<&str> {
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

pub(super) fn theme_domain_pattern(domain: &str) -> Option<(ThemeNamespace, &str)> {
    let domain = domain.strip_prefix("--")?;
    let (namespace, suffix) = domain.split_once("-*")?;
    Some((ThemeNamespace::parse(namespace)?, suffix))
}
