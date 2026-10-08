//! Theme-token lookup for candidate values.
//!
//! Two lookups exist and must not be confused. [`theme_value`] produces the value the
//! generated declaration should reference, which is a `var()` reference for an ordinary
//! token and the authored text for an inline token. [`theme_value_literal`] always
//! produces the authored text, because the compatibility fallback branch of a color
//! declaration must carry a usable value even where a `var()` reference would be the
//! only thing emitted.
//!
//! Typed captures - `[hint:value]`, parenthesized variables, and arbitrary lengths -
//! are validated against the namespace's CSS-declared value types, so the accepted
//! grammar follows the registry rather than this file.

use crate::evaluation::EvaluationContext;

use stillwind_css::ValueType;

use super::super::builtins::builtin_theme_namespace_types;
use super::arbitrary::{decode_arbitrary, parse_css_variable};
use crate::{ThemeIndex, ThemeNamespace, ThemeTokenMode};

/// Resolves a value in `namespace` to the text a declaration should contain.
///
/// Order matters: a typed arbitrary or variable capture wins over a named token, a
/// named token wins over the numeric spacing scale, and a value that matches nothing
/// returns [`None`]. The named-token path rejects non-ASCII captures; this is a
/// restriction of this lookup, not of the general candidate parser. Typed arbitrary
/// and variable captures are attempted before that restriction.
pub(in crate::evaluation) fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format!("var({value})")
        }
        // This named-token lookup is ASCII-only; the earlier arbitrary-value and
        // variable paths do not share that restriction.
        _ if !value.is_ascii() => return None,
        _ if let Some(token) = theme.token_in(namespace, value) => {
            theme.record_token(&token.name.value);
            if matches!(token.mode, ThemeTokenMode::Inline) {
                token.value.value.clone()
            } else {
                format!("var({})", token.name.value)
            }
        }
        _ if namespace == ThemeNamespace::Spacing
            && theme.scale(ThemeNamespace::Spacing).is_some() =>
        {
            if value == "px" {
                "1px".to_owned()
            } else if value.parse::<f64>().is_ok() {
                if value.parse::<f64>().is_ok_and(|number| number == 0.0) {
                    return Some("0".to_owned());
                }
                let scale_name = theme.scale(ThemeNamespace::Spacing)?.name.value.as_str();
                theme.record_token(scale_name);
                return Some(format!("calc(var({scale_name}) * {value})"));
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(value)
}

/// Resolves a value in `namespace` to its authored token text.
///
/// Use this for fallback branches that must not depend on a variable being defined
/// elsewhere; it falls back to [`theme_value`] when no token matches.
pub(in crate::evaluation) fn theme_value_literal(
    namespace: ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format!("var({value})")
        }
        _ if let Some(token) = theme.token_in(namespace, value) => {
            theme.record_token(&token.name.value);
            token.value.value.clone()
        }
        _ => return theme_value(namespace, value, theme),
    };
    Some(value)
}

/// Resolves a `[hint:value]` capture against the namespace's value types.
///
/// A namespace without declared types accepts any spelling; a hint is checked before
/// the value itself, so `[time:2s]` is rejected for a length-only namespace even though
/// `2s` is a valid value of some other type.
fn typed_theme_arbitrary(
    namespace: ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<String> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = decode_arbitrary(value);
    let types = theme_arbitrary_types(namespace, theme);
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint)));
    (hint_is_compatible && (types.is_empty() || types.iter().any(|kind| kind.accepts(&value))))
        .then(|| {
            theme.record_value_tokens(&value);
            theme.record_arbitrary_value(&value, types.contains(&ValueType::Color));
            value
        })
}

/// The value types declared for a namespace by the selected built-ins.
pub(in crate::evaluation) fn theme_arbitrary_types(
    namespace: ThemeNamespace,
    theme: &ThemeIndex,
) -> &[ValueType] {
    builtin_theme_namespace_types(theme)
        .get(&namespace)
        .map_or(&[], Vec::as_slice)
}

/// Resolves a `n/d` fraction capture into a percentage.
///
/// Zero is a valid numerator but not a denominator, and both components are limited to
/// unsigned 16-bit integers so a fraction cannot overflow.
pub(in crate::evaluation) fn fraction_value(value: &str) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| format!("calc({numerator} / {denominator} * 100%)"))
}

/// The viewport axis used by the `screen` size keyword.
#[derive(Clone, Copy, Debug)]
pub(in crate::evaluation) enum ViewportAxis {
    Width,
    Height,
}

impl ViewportAxis {
    fn unit(self) -> &'static str {
        match self {
            Self::Width => "vw",
            Self::Height => "vh",
        }
    }
}

/// Resolves a built-in axis keyword such as `full`, `screen`, or `svh`.
///
/// `viewport_axis` only decides which viewport unit the bare `screen` keyword expands
/// to for the axis the utility belongs to.
pub(in crate::evaluation) fn axis_size_value(
    value: &str,
    viewport_axis: ViewportAxis,
) -> Option<String> {
    let value = match value {
        "auto" => "auto".to_owned(),
        "full" => "100%".to_owned(),
        "screen" => format!("100{}", viewport_axis.unit()),
        "svw" => "100svw".to_owned(),
        "lvw" => "100lvw".to_owned(),
        "dvw" => "100dvw".to_owned(),
        "svh" => "100svh".to_owned(),
        "lvh" => "100lvh".to_owned(),
        "dvh" => "100dvh".to_owned(),
        "min" => "min-content".to_owned(),
        "max" => "max-content".to_owned(),
        "fit" => "fit-content".to_owned(),
        "lh" => "1lh".to_owned(),
        _ => return None,
    };
    Some(value)
}

fn typed_css_variable<'a>(
    namespace: ThemeNamespace,
    value: &'a str,
    theme: &EvaluationContext<'_>,
) -> Option<&'a str> {
    let variable = parse_css_variable(value)?;
    let types = theme_arbitrary_types(namespace, theme);
    (variable.hint.is_none()
        || types.is_empty()
        || variable
            .hint
            .and_then(ValueType::from_hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint))))
    .then(|| {
        theme.record_variable_token(variable.name);
        variable.name
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_test_support::BUILTIN_REGISTRY_CSS;

    use super::*;
    use crate::BuiltinDefinitions;
    use crate::theme::ThemeBuilder;

    #[test]
    fn fractions_reject_zero_negative_and_overflowing_denominators() {
        assert_eq!(fraction_value("1/2"), Some("calc(1 / 2 * 100%)".into()));
        assert_eq!(fraction_value("0/3"), Some("calc(0 / 3 * 100%)".into()));
        for value in ["1/0", "-1/2", "1/65536", "1/2/3", "half"] {
            assert_eq!(fraction_value(value), None);
        }
        assert_eq!(
            axis_size_value("screen", ViewportAxis::Height),
            Some("100vh".into())
        );
        assert_eq!(
            axis_size_value("full", ViewportAxis::Width),
            Some("100%".into())
        );
        assert_eq!(axis_size_value("nope", ViewportAxis::Height), None);
    }

    #[test]
    fn arbitrary_values_and_variables_must_match_namespace_types() {
        let configured = ThemeBuilder {
            registry: Arc::new(BuiltinDefinitions::from_css(BUILTIN_REGISTRY_CSS)),
            ..Default::default()
        }
        .finish();
        let configured = EvaluationContext::untracked(&configured);
        assert_eq!(
            typed_theme_arbitrary(ThemeNamespace::Spacing, "[length:2px]", &configured),
            Some("2px".into())
        );
        assert_eq!(
            typed_theme_arbitrary(ThemeNamespace::Spacing, "[time:2s]", &configured),
            None
        );
        assert_eq!(
            typed_css_variable(ThemeNamespace::Color, "(color:--x)", &configured),
            Some("--x")
        );
        assert_eq!(
            typed_css_variable(ThemeNamespace::Color, "(length:--x)", &configured),
            None
        );
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        assert_eq!(theme_value(ThemeNamespace::Color, "missing", &theme), None);
        assert_eq!(
            theme_value_literal(ThemeNamespace::Color, "[red]", &theme),
            Some("red".into())
        );
    }

    proptest::proptest! {
        #[test]
        fn valid_fraction_components_are_never_reversed(a in 0u16..1000, b in 1u16..1000) {
            proptest::prop_assert_eq!(fraction_value(&format!("{a}/{b}")), Some(format!("calc({a} / {b} * 100%)")));
        }
    }
}
