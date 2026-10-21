//! Value-domain resolution: which spellings a `--value(...)` placeholder accepts.
//!
//! A domain list is tried in authored order and the first matching domain wins, so a
//! definition controls precedence by ordering its alternatives. Domains come in four
//! shapes: literal keywords; CSS grammar hints such as `[length]`; special forms, which
//! are `integer`, `number`, `percentage`, `--fraction()`, the axis-size built-ins, and
//! the domain of each declared scale, such as `--spacing()`; and theme namespace
//! patterns such as `--color-*`.

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{ValueDomain, split_once_top_level, split_top_level, value_is_well_formed};
use stillwind_syntax::decode_arbitrary_value;

use super::super::context::{ArbitraryValueCheck, EvaluationContext};
use super::arbitrary::parse_css_variable;
use super::expansion::expand_placeholder;
use super::theme::{PatternNamespace, ViewportAxis, axis_size_value, fraction_value, theme_value};
use crate::ThemeIndex;
use crate::conditionals::accepts_runtime_value;

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
    split_top_level(domains, |token| matches!(token, Token::Comma))
        .into_iter()
        .find_map(|domain| {
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
            if let Some(namespace) = theme.namespaces().scale_domain(domain) {
                return capture.and_then(|capture| theme_value(namespace, capture, theme));
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
            if let Some((namespace, suffix)) = theme_domain_pattern(domain, theme) {
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
                let hint_is_compatible =
                    variable.hint.is_none_or(|hint| expected.accepts_hint(hint));
                return hint_is_compatible.then(|| {
                    theme.record_variable_token(variable.name);
                    format_args!("var({})", variable.name).to_lean_str()
                });
            }
            let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
            let (head, tail) =
                split_once_top_level(arbitrary, |token| matches!(token, Token::Colon));
            let (hint, value) = tail.map_or(("", head), |value| (head, value));
            let value = theme.inline_theme_functions(&decode_arbitrary_value(value))?;
            let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
            let accepted = accepts_runtime_value(expected, &value)
                || (hint.is_empty()
                    && theme.arbitrary_check() == ArbitraryValueCheck::WellFormed
                    && value_is_well_formed(&value));
            (hint_is_compatible && accepted).then(|| {
                theme.record_value_tokens(&value);
                theme.record_arbitrary_value(
                    &value,
                    Some(expected) == ValueDomain::from_hint("color"),
                );
                value.into()
            })
        })
}

/// Splits a theme namespace pattern such as `--color-*` or `--text-*--line-height` into
/// its namespace and trailing suffix.
///
/// Only a pattern of the form `--namespace-*` with a non-empty namespace name is
/// recognized. The namespace can have any name, such as `size` or `size-lg`; `theme`
/// decides whether it is a declared namespace.
pub(in crate::evaluation) fn theme_domain_pattern<'a>(
    domain: &'a str,
    theme: &'a ThemeIndex,
) -> Option<(PatternNamespace<'a>, &'a str)> {
    let domain = domain.strip_prefix("--")?;
    let (namespace, suffix) = domain.split_once("-*")?;
    (!namespace.is_empty() && !namespace.contains(char::is_whitespace))
        .then(|| (PatternNamespace::parse(namespace, theme), suffix))
}

/// Returns the captures written without brackets that the `--value(...)` placeholders
/// of `body` accept by name.
///
/// A theme namespace domain contributes the names of its active tokens, without the
/// domain's suffix. A declared scale also contributes its fixed keys while its scale
/// token is active. A keyword domain contributes the keyword. Other domains accept
/// computed or arbitrary values and contribute nothing. Names can repeat.
pub fn named_domain_values(body: &str, theme: &ThemeIndex) -> Vec<LeanString> {
    let mut names = Vec::new();
    let mut body = body.to_lean_string();
    let _ = expand_placeholder(&mut body, "--value", |domains| {
        for domain in split_top_level(domains, |token| matches!(token, Token::Comma)) {
            let domain = domain.trim();
            let pattern = theme
                .namespaces()
                .scale_domain(domain)
                .map(|namespace| (PatternNamespace::Declared(namespace), ""))
                .or_else(|| theme_domain_pattern(domain, theme));
            if let Some((namespace, suffix)) = pattern {
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
/// active token with the pattern's suffix, or when its namespace has a scale, the
/// pattern has no suffix, and the theme defines the scale token, such as `--spacing`.
/// The result is empty when `body` has no `--value(...)`, when a pattern resolves a
/// value, or when a domain is not a namespace pattern, such as `integer`, `[length]`,
/// or a keyword.
#[must_use]
pub fn unresolvable_value_namespaces(body: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    let mut patterns = Vec::new();
    let mut resolves = false;
    let mut body = body.to_lean_string();
    let _ = expand_placeholder(&mut body, "--value", |domains| {
        for domain in split_top_level(domains, |token| matches!(token, Token::Comma)) {
            let domain = domain.trim();
            if !domain.starts_with("--") || !domain.contains("-*") {
                resolves = true;
                continue;
            }
            resolves |= theme_domain_pattern(domain, theme).is_some_and(|(namespace, suffix)| {
                (matches!(namespace, PatternNamespace::Declared(namespace)
                    if namespace.scale.is_some() && theme.scale(&namespace.name).is_some())
                    && suffix.is_empty())
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
    use std::sync::Arc;

    use stillwind_source::{FileId, SourceFile};
    use stillwind_test_support::PACKAGE_CSS;

    use super::*;
    use crate::test_support::parse_fixture_theme;
    use crate::theme::ThemeBuilder;
    use crate::{StillwindDefinitions, ThemeIndex};

    #[test]
    fn domain_lists_keep_function_commas_and_typed_capture_boundaries() {
        let configured = ThemeBuilder {
            registry: Arc::new(StillwindDefinitions::from_css(PACKAGE_CSS)),
            ..Default::default()
        }
        .finish();
        assert_eq!(
            theme_domain_pattern("--color-*", &configured),
            Some((
                PatternNamespace::Declared(configured.namespaces().get("color").unwrap()),
                ""
            ))
        );
        let theme = ThemeIndex::default();
        assert_eq!(
            theme_domain_pattern("--color-*", &theme),
            Some((PatternNamespace::Other("color"), ""))
        );
        assert_eq!(
            theme_domain_pattern("--size-lg-*--alt", &theme),
            Some((PatternNamespace::Other("size-lg"), "--alt"))
        );
        for domain in ["---*", "--size", "-size-*"] {
            assert_eq!(theme_domain_pattern(domain, &theme), None, "{domain}");
        }
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
