//! Value-domain resolution: which spellings a `--value(...)` placeholder accepts.
//!
//! A domain list is tried in authored order and the first matching domain wins, so a
//! definition controls precedence by ordering its alternatives. Domains come in four
//! shapes: literal keywords; CSS grammar hints such as `[length]`; special forms, which
//! are `integer`, `number`, `percentage`, `--fraction()`, the axis-size built-ins, and
//! the domain of each declared scale, such as `--spacing()`; and theme namespace
//! patterns such as `--color-*`. A numeric domain can limit its values with an
//! inclusive range, such as `percentage [50,200]`.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    ValueDomain, find_css_functions, split_once_top_level, split_top_level, value_is_well_formed,
};
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
            if let Some(numeric) = NumericDomain::parse(domain) {
                return capture.and_then(|capture| numeric.resolve(capture));
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
                let keyword = quoted_keyword(domain).unwrap_or(domain);
                return capture.filter(|value| *value == keyword).map(|value| {
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
            let value = theme.resolve_theme_references(&value).ok()??;
            let value = theme.resolve_css_functions(&value)?;
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

/// A `--value(...)` domain that accepts a bare number: `integer`, `number`, or
/// `percentage`. The domain can end with an inclusive range, such as
/// `percentage [50,200]`, which limits the number without its `%`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericDomain {
    /// The written form that the domain accepts.
    pub kind: NumericKind,
    /// The smallest accepted number. It is negative infinity without a range.
    pub min: f64,
    /// The largest accepted number. It is positive infinity without a range.
    pub max: f64,
}

impl NumericDomain {
    /// Parses one domain, such as `integer` or `percentage [50,200]`. Returns `None`
    /// for other domains and for a range that is not two numbers in `[min,max]` form.
    #[must_use]
    pub fn parse(domain: &str) -> Option<Self> {
        let (name, range) = domain
            .trim()
            .split_once(char::is_whitespace)
            .map_or((domain.trim(), None), |(name, range)| {
                (name, Some(range.trim()))
            });
        let kind = match name {
            "integer" => NumericKind::Integer,
            "number" => NumericKind::Number,
            "percentage" => NumericKind::Percentage,
            _ => return None,
        };
        let (min, max) = match range {
            None => (f64::NEG_INFINITY, f64::INFINITY),
            Some(range) => {
                let (min, max) = range
                    .strip_prefix('[')?
                    .strip_suffix(']')?
                    .split_once(',')?;
                (min.trim().parse().ok()?, max.trim().parse().ok()?)
            }
        };
        Some(Self { kind, min, max })
    }

    /// Returns the CSS value for `capture`, or `None` when the domain does not accept
    /// it. An integer must parse as an integer, and a percentage must end with `%`.
    #[must_use]
    pub fn resolve(&self, capture: &str) -> Option<LeanStr> {
        let number = match self.kind {
            NumericKind::Integer => capture.parse::<i64>().ok().and(capture.parse::<f64>().ok()),
            NumericKind::Number => capture.parse::<f64>().ok(),
            NumericKind::Percentage => capture.strip_suffix('%')?.parse::<f64>().ok(),
        }?;
        (self.min..=self.max)
            .contains(&number)
            .then(|| capture.to_lean_str())
    }
}

/// The written form of a number that a [`NumericDomain`] accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericKind {
    /// An integer without a unit.
    Integer,
    /// A number without a unit.
    Number,
    /// A number followed by `%`.
    Percentage,
}

/// Splits a theme namespace pattern into its namespace and the suffix after `-*`. For
/// example, `--text-*--line-height` gives the namespace `text` and the suffix
/// `--line-height`. `--color` without `-*` also selects the namespace `color`, with an
/// empty suffix.
///
/// Returns `None` when the name is empty, contains whitespace, or is a function call.
/// `theme` decides whether the namespace is declared.
pub(in crate::evaluation) fn theme_domain_pattern<'a>(
    domain: &'a str,
    theme: &'a ThemeIndex,
) -> Option<(PatternNamespace<'a>, &'a str)> {
    let pattern = domain.strip_prefix("--")?;
    let (namespace, suffix) = pattern.split_once("-*").unwrap_or((pattern, ""));
    let name = &domain[..namespace.len() + 2];
    let mut parser = Parser::new(name);
    (!namespace.is_empty()
        && !namespace.contains(char::is_whitespace)
        && matches!(parser.next(), Ok(Token::Ident(_)))
        && parser.position().byte_index() == name.len())
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
            let keyword = quoted_keyword(domain);
            if let Some(keyword) = keyword {
                names.push(keyword.to_lean_string());
                continue;
            }
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

/// Returns the theme namespaces that the `function` placeholders in the utility body
/// `body` accept. `function` is `--value` or `--modifier`. For example,
/// `--value(--color-*, integer)` gives `--color-`.
///
/// Each result keeps its leading `--` and ends with `-`, so `--color-*` and `--color`
/// both give `--color-`. Arguments that are not a namespace, such as `integer` or a
/// function call, give nothing. Results keep their order in `body` and can repeat.
#[must_use]
pub fn placeholder_namespaces(body: &str, function: &str) -> Vec<LeanStr> {
    find_css_functions(body, &[function], 0)
        .into_iter()
        .flat_map(|found| body[found.arguments].split(','))
        .map(str::trim)
        .filter(|part| part.starts_with("--") && !part.contains('('))
        .map(|part| {
            part.strip_suffix('*')
                .map_or_else(|| format!("{part}-").into(), LeanStr::from)
        })
        .collect()
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

/// Returns the keyword between the quotes of `domain`, such as `auto` for `'auto'` or
/// `"auto"`. Returns `None` when `domain` is not quoted.
fn quoted_keyword(domain: &str) -> Option<&str> {
    ['"', '\'']
        .into_iter()
        .find_map(|quote| domain.strip_prefix(quote)?.strip_suffix(quote))
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
        assert_eq!(
            theme_domain_pattern("--size", &theme),
            Some((PatternNamespace::Other("size"), ""))
        );
        assert_eq!(
            named_domain_values("--name:--value('--size');", &theme),
            ["--size"]
        );
        for domain in ["---*", "-size-*", "--spacing()", "--size(value)"] {
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

    #[test]
    fn numeric_domains_limit_values_to_their_range() {
        let stretch = NumericDomain::parse("percentage [50,200]").unwrap();
        assert_eq!(stretch.resolve("50%").as_deref(), Some("50%"));
        assert_eq!(stretch.resolve("200%").as_deref(), Some("200%"));
        assert_eq!(stretch.resolve("300%"), None);
        assert_eq!(stretch.resolve("100"), None);

        let integer = NumericDomain::parse("integer").unwrap();
        assert_eq!(integer.resolve("-3").as_deref(), Some("-3"));
        assert_eq!(integer.resolve("1.5"), None);

        assert_eq!(NumericDomain::parse("percentage [50]"), None);
        assert_eq!(NumericDomain::parse("length"), None);
    }
}
