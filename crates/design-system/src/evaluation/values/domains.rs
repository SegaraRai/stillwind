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

use super::super::context::{ArbitraryValueCheck, EvaluationContext};
use super::arbitrary::{decode_arbitrary, parse_css_variable};
use super::expansion::expand_placeholder;
use super::theme::{PatternNamespace, ViewportAxis, axis_size_value, fraction_value, theme_value};
use crate::conditionals::accepts_runtime_value;
use crate::{SPACING_VALUE_DOMAIN, ThemeIndex, ThemeNamespace};

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
        if domain == SPACING_VALUE_DOMAIN {
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
            return capture.and_then(|capture| namespace.resolve(capture, suffix, theme));
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

/// Splits a theme namespace pattern such as `--color-*` or `--text-*--line-height` into
/// its namespace and trailing suffix.
///
/// Only a pattern of the form `--namespace-*` with a non-empty namespace name is
/// recognized. The namespace can have any name, such as `size` or `size-lg`.
pub(in crate::evaluation) fn theme_domain_pattern(
    domain: &str,
) -> Option<(PatternNamespace<'_>, &str)> {
    let domain = domain.strip_prefix("--")?;
    let (namespace, suffix) = domain.split_once("-*")?;
    (!namespace.is_empty() && !namespace.contains(char::is_whitespace))
        .then(|| (PatternNamespace::parse(namespace), suffix))
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
                names.extend(
                    namespace
                        .keys(suffix, theme)
                        .into_iter()
                        .map(ToLeanString::to_lean_string),
                );
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
/// A pattern resolves a value when its namespace, which can have any name, has an
/// active token with the pattern's suffix, or when it is `--spacing-*` and the theme
/// defines the `--spacing` scale. The result is empty when `body` has no
/// `--value(...)`, when a pattern resolves a value, or when a domain is not a namespace
/// pattern, such as `integer`, `[length]`, or a keyword.
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
                (namespace == PatternNamespace::Builtin(ThemeNamespace::Spacing)
                    && suffix.is_empty()
                    && theme.scale(ThemeNamespace::Spacing).is_some())
                    || !namespace.keys(suffix, theme).is_empty()
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
    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::ThemeIndex;
    use crate::test_support::parse_fixture_theme;

    #[test]
    fn domain_lists_keep_function_commas_and_typed_capture_boundaries() {
        assert_eq!(
            split_domains("[length], --spacing-*, f(a,b)"),
            vec!["[length]", " --spacing-*", " f(a,b)"]
        );
        assert_eq!(
            theme_domain_pattern("--color-*"),
            Some((PatternNamespace::Builtin(ThemeNamespace::Color), ""))
        );
        assert_eq!(
            theme_domain_pattern("--size-lg-*--alt"),
            Some((PatternNamespace::Other("size-lg"), "--alt"))
        );
        for domain in ["---*", "--size", "-size-*"] {
            assert_eq!(theme_domain_pattern(domain), None, "{domain}");
        }
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
    fn namespace_patterns_read_tokens_of_any_namespace_by_name_only() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            r"@theme {
                --size-4: 1rem; --size-lg-2: 3rem; --size-1_5: 6px; --size-2\.5: 10px;
                --size-x: 1px; --size-x--alt: 2px; --size-y--alt: 2px; --inset-x: 3px; --inset-shadow-x: 4px;
             }
             @theme inline { --size-in: 5px; }",
        )
        .unwrap();
        let theme = parse_fixture_theme(&source).unwrap();
        let theme = EvaluationContext::untracked(&theme);
        for (domains, capture, expected) in [
            ("--size-*", "4", Some("var(--size-4)")),
            ("--size-*", "lg-2", Some("var(--size-lg-2)")),
            ("--size-lg-*", "2", Some("var(--size-lg-2)")),
            ("--size-*", "1.5", Some("var(--size-1_5)")),
            ("--size-*", "2.5", Some(r"var(--size-2\.5)")),
            ("--size-*--alt", "x", Some("var(--size-x--alt)")),
            ("--size-*--alt", "y", None),
            ("--size-*", "in", Some("5px")),
            ("--inset-*", "x", Some("var(--inset-x)")),
            ("--inset-*", "shadow-x", None),
            ("--size-*", "1_5", Some("var(--size-1_5)")),
            ("--size-*", "3", None),
            ("--size-*", "[2px]", None),
            ("--size-*", "(--v)", None),
            ("--size-*, [length]", "[2px]", Some("2px")),
        ] {
            assert_eq!(
                resolve_value_domains(domains, Some(capture), &theme).as_deref(),
                expected,
                "{domains} {capture}"
            );
        }
    }

    #[test]
    fn value_namespaces_without_values_are_unresolvable() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --size-4: 1rem; --color-x: red; }",
        )
        .unwrap();
        let theme = parse_fixture_theme(&source).unwrap();
        for (body, expected) in [
            ("width: --value(--size-*);", &[][..]),
            ("width: --value(--size-lg-*);", &["--size-lg-*"]),
            (
                "color: --value(--color-*-strong, --box-*);",
                &["--color-*-strong", "--box-*"],
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
