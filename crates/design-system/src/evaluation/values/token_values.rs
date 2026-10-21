//! Checks of authored theme token values against the value types of their namespace.
//!
//! A namespace feeds its tokens to properties and conditions of one type: `--color-*`
//! to colors and `--breakpoint-*` to media query widths, for example. Resolution uses a
//! token without checking its value, so `bg-brand` emits `var(--color-brand)` even when
//! the token holds a length. [`theme_value_mismatch`] finds such values for lint.

use cssparser::{Delimiter, ParseError, Parser};
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{ValueType, find_css_functions};

use crate::ThemeIndex;

/// A theme token value that every value type of its namespace rejects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeValueMismatch {
    /// Name of the token's namespace, such as `color`.
    pub namespace: LeanStr,
    /// Type hints that the namespace accepts, joined with ` or `, such as `color` or
    /// `number or length`.
    pub expected: LeanStr,
}

/// Returns the mismatch when `value`, the authored value of the theme token `name`,
/// matches no value type of the token's namespace.
///
/// The `@stillwind-theme-namespace` statements of `theme` declare the value types of
/// each namespace. A value of a shadow namespace may be a comma-separated list of
/// shadows.
///
/// Returns `None` when `value` matches, and also when the check does not apply: `name`
/// belongs to no namespace as [`crate::ThemeNamespaces::of_token`] describes, the
/// namespace has no declared types, or `value` contains `var()`, `theme()`, or
/// `--theme()`, whose result is unknown before substitution. A CSS-wide keyword such as
/// `initial` always matches.
#[must_use]
pub fn theme_value_mismatch(
    name: &str,
    value: &str,
    theme: &ThemeIndex,
) -> Option<ThemeValueMismatch> {
    let namespace = theme.namespaces().of_token(name)?;
    let types = &namespace.types;
    let value = value.trim();
    if types.is_empty()
        || value.is_empty()
        || !find_css_functions(value, &["theme", "--theme"], 0).is_empty()
    {
        return None;
    }
    let accepts = |value_type: &ValueType| {
        if *value_type == ValueType::Shadow {
            top_level_items(value)
                .iter()
                .all(|item| value_type.accepts(item))
        } else {
            value_type.accepts(value)
        }
    };
    (!types.iter().any(accepts)).then(|| ThemeValueMismatch {
        namespace: namespace.name.clone(),
        expected: types
            .iter()
            .map(|value_type| value_type.hint())
            .collect::<Vec<_>>()
            .join(" or ")
            .to_lean_str(),
    })
}

/// Splits `value` at commas outside functions, blocks, and strings, and returns each
/// trimmed item.
fn top_level_items(value: &str) -> Vec<&str> {
    let mut parser = Parser::new(value);
    let mut items = Vec::new();
    loop {
        let start = parser.position().byte_index();
        let _: Result<(), ParseError<()>> = parser.parse_until_before(Delimiter::Comma, |item| {
            while item.next_including_whitespace_and_comments().is_ok() {}
            Ok(())
        });
        items.push(value[start..parser.position().byte_index()].trim());
        if parser.next().is_err() {
            return items;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_source::{FileId, SourceFile};
    use stillwind_test_support::BUILTIN_REGISTRY_CSS;

    use super::*;
    use crate::StillwindDefinitions;
    use crate::test_support::parse_fixture_theme;
    use crate::theme::ThemeBuilder;

    fn theme() -> ThemeIndex {
        ThemeBuilder {
            registry: Arc::new(StillwindDefinitions::from_css(BUILTIN_REGISTRY_CSS)),
            ..Default::default()
        }
        .finish()
    }

    #[test]
    fn values_must_match_a_type_of_their_namespace() {
        let theme = theme();
        for (name, value, expected) in [
            ("--color-brand", "12px", Some("color")),
            ("--breakpoint-tab", "blue", Some("length")),
            ("--spacing", "red", Some("length")),
            ("--container-card", "auto", Some("length")),
            ("--text-huge", "red", Some("font-size")),
            ("--leading-loose", "red", Some("number or length")),
            ("--duration-slow", "10px", Some("time")),
            ("--shadow-card", "0 1px 2px red, 12px", Some("shadow")),
            ("--color-brand", "oklch(60% 0.2 25)", None),
            ("--text-huge", "4rem", None),
            ("--leading-loose", "2", None),
            ("--shadow-card", "0 1px 2px red, inset 0 0 1px blue", None),
            (
                "--font-display",
                "\"Inter\", ui-sans-serif, sans-serif",
                None,
            ),
            ("--ease-snappy", "cubic-bezier(0.2, 0, 0, 1)", None),
            ("--animate-wiggle", "wiggle 1s ease-in-out infinite", None),
            ("--color-brand", "initial", None),
            ("--color-brand", "var(--brand)", None),
            ("--color-brand", "--theme(--color-red-500)", None),
            ("--color-brand", "theme(--color-red-500)", None),
            ("--color-*", "12px", None),
            ("--text-sm--line-height", "red", None),
            ("--my-length", "red", None),
        ] {
            assert_eq!(
                theme_value_mismatch(name, value, &theme)
                    .as_ref()
                    .map(|mismatch| mismatch.expected.as_str()),
                expected,
                "{name}: {value}"
            );
        }
    }

    #[test]
    fn default_theme_values_match_their_namespaces() {
        let source = SourceFile::new(FileId(1), None, None, "@import 'tailwindcss';").unwrap();
        let theme = parse_fixture_theme(&source).unwrap();
        assert!(theme.active_tokens().len() > 100);
        for token in theme.active_tokens() {
            assert_eq!(
                theme_value_mismatch(&token.name.value, &token.value.value, &theme),
                None,
                "{}: {}",
                token.name.value,
                token.value.value
            );
        }
    }
}
