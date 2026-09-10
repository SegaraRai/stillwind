use stillwind_css::ValueType;

use super::super::builtins::metadata::builtin_theme_namespace_types;
use super::arbitrary::{decode_arbitrary, parse_css_variable};
use crate::{ThemeIndex, ThemeNamespace, ThemeTokenMode};

pub(crate) fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format!("var({value})")
        }
        // Tailwind's candidate grammar accepts ASCII identifier tokens only;
        // Unicode theme names remain valid CSS but are not utility candidates.
        _ if !value.is_ascii() => return None,
        _ if let Some(token) = theme.token_in(namespace, value) => {
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
                return Some(format!("calc(var({scale_name}) * {value})"));
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(value)
}

pub(crate) fn theme_value_literal(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format!("var({value})")
        }
        _ if let Some(token) = theme.token_in(namespace, value) => token.value.value.clone(),
        _ => return theme_value(namespace, value, theme),
    };
    Some(value)
}

pub(crate) fn typed_theme_arbitrary(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
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
        .then_some(value)
}

pub(crate) fn theme_arbitrary_types(namespace: ThemeNamespace, theme: &ThemeIndex) -> &[ValueType] {
    builtin_theme_namespace_types(theme)
        .get(&namespace)
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn fraction_value(value: &str) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| format!("calc({numerator} / {denominator} * 100%)"))
}

pub(crate) fn axis_size_value(value: &str, viewport_axis: &str) -> Option<String> {
    let value = match value {
        "auto" => "auto".to_owned(),
        "full" => "100%".to_owned(),
        "screen" => format!("100{viewport_axis}"),
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

pub(crate) fn typed_css_variable<'a>(
    namespace: ThemeNamespace,
    value: &'a str,
    theme: &ThemeIndex,
) -> Option<&'a str> {
    let variable = parse_css_variable(value)?;
    let types = theme_arbitrary_types(namespace, theme);
    (variable.hint.is_none()
        || types.is_empty()
        || variable
            .hint
            .and_then(ValueType::from_hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint))))
    .then_some(variable.name)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::BuiltinDefinitions;

    #[test]
    fn fractions_reject_zero_negative_and_overflowing_denominators() {
        assert_eq!(fraction_value("1/2"), Some("calc(1 / 2 * 100%)".into()));
        assert_eq!(fraction_value("0/3"), Some("calc(0 / 3 * 100%)".into()));
        for value in ["1/0", "-1/2", "1/65536", "1/2/3", "half"] {
            assert_eq!(fraction_value(value), None);
        }
        assert_eq!(axis_size_value("screen", "vh"), Some("100vh".into()));
        assert_eq!(axis_size_value("full", "vw"), Some("100%".into()));
        assert_eq!(axis_size_value("nope", "vh"), None);
    }

    #[test]
    fn arbitrary_values_and_variables_must_match_namespace_types() {
        let configured = ThemeIndex {
            registry: Arc::new(BuiltinDefinitions::from_css(
                crate::test_defaults::BUILTIN_REGISTRY_CSS,
            )),
            ..ThemeIndex::default()
        };
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
