//! Value-domain resolution: which spellings a `--value(...)` placeholder accepts.
//!
//! A domain list is tried in authored order and the first matching domain wins, so a
//! definition controls precedence by ordering its alternatives. Domains come in four
//! shapes: literal keywords, CSS grammar hints such as `[length]`, the special forms
//! `integer`, `number`, `percentage`, `--spacing()`, `--fraction()`, and the axis-size
//! built-ins, and theme namespace patterns such as `--color-*`.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{ValueDomain, value_is_well_formed};

use super::super::{ArbitraryValueCheck, EvaluationContext};
use super::arbitrary::{decode_arbitrary, parse_css_variable};
use super::expansion::expand_placeholder;
use super::theme::{ViewportAxis, axis_size_value, fraction_value, theme_value};
use crate::conditionals::accepts_runtime_value;
use crate::{ThemeIndex, ThemeNamespace, ThemeTokenKind};

/// Resolves the first matching domain in a comma-separated domain list.
///
/// A theme namespace domain looks the capture up in that namespace and may append an
/// authored suffix; a CSS grammar domain accepts either a parenthesized variable (whose
/// optional hint must fit) or an arbitrary value (whose value and hint must fit). When
/// `theme` checks [`ArbitraryValueCheck::WellFormed`] values, every well-formed
/// arbitrary value without a hint fits a CSS grammar domain. Whole `--if` values must
/// have two branches accepted by the same domain; nested expressions use
/// deferred-variable validation after their conditional syntax is checked. [`None`]
/// means no domain matched, including malformed conditional captures.
pub(in crate::evaluation) fn resolve_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if domain == "integer" {
            return capture
                .filter(|value| value.parse::<i64>().is_ok())
                .map(LeanStr::from);
        }
        if domain == "number" {
            return capture
                .filter(|value| value.parse::<f64>().is_ok())
                .map(LeanStr::from);
        }
        if domain == "percentage" {
            return capture
                .and_then(|value| value.strip_suffix('%'))
                .filter(|value| value.parse::<f64>().is_ok())
                .map(|value| format_args!("{value}%").to_lean_str());
        }
        if domain == "--spacing()" {
            return capture
                .and_then(|capture| theme_value(ThemeNamespace::Spacing, capture, theme));
        }
        if domain == "--fraction()" {
            return capture.and_then(fraction_value);
        }
        if domain == "--stillwind-builtin-width()" {
            return capture.and_then(|capture| axis_size_value(capture, ViewportAxis::Width));
        }
        if domain == "--stillwind-builtin-height()" {
            return capture.and_then(|capture| axis_size_value(capture, ViewportAxis::Height));
        }
        if let Some((namespace, suffix)) = theme_domain_pattern(domain) {
            return capture.and_then(|capture| {
                theme_value(
                    namespace,
                    &format_args!("{capture}{suffix}").to_lean_str(),
                    theme,
                )
            });
        }
        if !domain.starts_with('[') && !domain.starts_with("--") {
            return capture.filter(|value| *value == domain).map(|value| {
                if value == "current" {
                    "currentcolor".to_lean_str()
                } else {
                    value.to_lean_str()
                }
            });
        }
        let expected = ValueDomain::from_hint(domain.strip_prefix('[')?.strip_suffix(']')?)?;
        if let Some(variable) = parse_css_variable(capture?) {
            let hint_is_compatible = variable.hint.is_none_or(|hint| expected.accepts_hint(hint));
            return hint_is_compatible.then(|| {
                theme.record_variable_token(variable.name);
                format_args!("var({})", variable.name).to_lean_str()
            });
        }
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (hint, value) = arbitrary_hint(arbitrary);
        let value = theme.inline_theme_functions(&decode_arbitrary(value))?;
        let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
        let accepted = accepts_runtime_value(expected, &value)
            || (hint.is_empty()
                && theme.arbitrary_check() == ArbitraryValueCheck::WellFormed
                && value_is_well_formed(&value));
        (hint_is_compatible && accepted).then(|| {
            theme.record_value_tokens(&value);
            theme.record_arbitrary_value(&value, Some(expected) == ValueDomain::from_hint("color"));
            value.into()
        })
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

/// Splits a domain list on top-level commas.
///
/// Commas inside parentheses or brackets belong to a function argument list and are not
/// separators. Surrounding whitespace is preserved, because a domain may be a prefix
/// pattern such as `--spacing-*` followed by an authored suffix.
pub(in crate::evaluation) fn split_domains(value: &str) -> Vec<&str> {
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
pub(in crate::evaluation) fn theme_domain_pattern(domain: &str) -> Option<(ThemeNamespace, &str)> {
    let domain = domain.strip_prefix("--")?;
    let (namespace, suffix) = domain.split_once("-*")?;
    Some((ThemeNamespace::parse(namespace)?, suffix))
}

/// Returns the captures written without brackets that the `--value(...)` placeholders
/// of `body` accept by name.
///
/// A theme namespace domain contributes the names of its active tokens, without the
/// domain's suffix, and a keyword domain contributes the keyword. Other domains accept
/// computed or arbitrary values and contribute nothing. Names can repeat.
pub(in crate::evaluation) fn named_domain_values(
    body: &str,
    theme: &ThemeIndex,
) -> Vec<LeanString> {
    let mut names = Vec::new();
    let mut body = body.to_lean_string();
    let _ = expand_placeholder(&mut body, "--value", |domains| {
        for domain in split_domains(domains) {
            let domain = domain.trim();
            if let Some((namespace, suffix)) = theme_domain_pattern(domain) {
                names.extend(theme.tokens_in(namespace).into_iter().filter_map(|token| {
                    let ThemeTokenKind::Token { name, .. } = &token.kind else {
                        return None;
                    };
                    name.strip_suffix(suffix).map(ToLeanString::to_lean_string)
                }));
            } else if !domain.is_empty()
                && !domain.starts_with(['[', '-'])
                && domain
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
            {
                names.push(domain.to_lean_string());
            }
        }
        Some(LeanStr::new())
    });
    names
}

/// Returns the theme namespace patterns, such as `--size-*`, of the `--value(...)`
/// placeholders of the utility body `body` when no domain of `body` can resolve a
/// value.
///
/// A pattern resolves a value when it names a theme namespace that has an active token
/// with the pattern's suffix, or when it is `--spacing-*` and the theme defines the
/// `--spacing` scale. A pattern of a name outside the theme namespaces never resolves,
/// whatever tokens exist. The result is empty when `body` has no `--value(...)`, when a
/// pattern resolves a value, or when a domain is not a namespace pattern, such as
/// `integer`, `[length]`, or a keyword.
#[must_use]
pub fn unresolvable_value_namespaces(body: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    let mut patterns = Vec::new();
    let mut resolves = false;
    let mut body = body.to_lean_string();
    let _ = expand_placeholder(&mut body, "--value", |domains| {
        for domain in split_domains(domains) {
            let domain = domain.trim();
            if !domain.starts_with("--") || !domain.contains("-*") {
                resolves = true;
                continue;
            }
            resolves |= theme_domain_pattern(domain).is_some_and(|(namespace, suffix)| {
                (namespace == ThemeNamespace::Spacing
                    && suffix.is_empty()
                    && theme.scale(namespace).is_some())
                    || theme.tokens_in(namespace).into_iter().any(|token| {
                        matches!(&token.kind, ThemeTokenKind::Token { name, .. } if name.ends_with(suffix))
                    })
            });
            let domain = domain.to_lean_str();
            if !patterns.contains(&domain) {
                patterns.push(domain);
            }
        }
        Some(LeanStr::new())
    });
    if resolves { Vec::new() } else { patterns }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeIndex;

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
        let theme = EvaluationContext::untracked(&theme);
        assert_eq!(
            resolve_value_domains("[length]", Some("[2px]"), &theme),
            Some("2px".into())
        );
        assert_eq!(
            resolve_value_domains("[length]", Some("[red]"), &theme),
            None
        );
        assert_eq!(
            resolve_value_domains("[number]", Some("[2]"), &theme),
            Some("2".into())
        );
        assert_eq!(resolve_value_domains("[number]", None, &theme), None);
    }

    #[test]
    fn value_namespaces_without_values_are_unresolvable() {
        let source = stillwind_source::SourceFile::new(
            stillwind_source::FileId(1),
            None,
            None,
            "@theme { --size-4: 1rem; --color-x: red; }",
        )
        .unwrap();
        let theme = crate::test_support::parse_fixture_theme(&source).unwrap();
        for (body, expected) in [
            ("width: --value(--size-*);", &["--size-*"][..]),
            (
                "color: --value(--color-*-strong, --size-*);",
                &["--color-*-strong", "--size-*"],
            ),
            ("color: --value(--color-*);", &[]),
            ("width: --value(--spacing-*);", &[]),
            ("width: --value(--size-*, integer);", &[]),
            ("width: --value(--size-*, [length]);", &[]),
            ("width: 1px;", &[]),
        ] {
            assert_eq!(
                unresolvable_value_namespaces(body, &theme),
                expected,
                "{body}"
            );
        }
    }
}
