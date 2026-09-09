use super::{
    ThemeIndex, ThemeNamespace, ValueType, builtin_theme_namespace_types, decode_arbitrary,
    parse_css_variable,
};

pub(super) fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = typed_css_variable(namespace, value) => format!("var({value})"),
        // Tailwind's candidate grammar accepts ASCII identifier tokens only;
        // Unicode theme names remain valid CSS but are not utility candidates.
        _ if !value.is_ascii() => return None,
        _ if let Some(token) = theme.token_in(namespace, value) => {
            if matches!(token.mode, stillwind_css::ThemeTokenMode::Inline) {
                token.value.value.clone()
            } else {
                let prefix = theme
                    .imports
                    .prefix
                    .as_deref()
                    .map(|prefix| format!("{}-", prefix.trim_end_matches('-')))
                    .unwrap_or_default();
                format!("var(--{prefix}{}-{value})", namespace.css_name())
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

pub(super) fn theme_value_literal(
    namespace: ThemeNamespace,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = typed_css_variable(namespace, value) => format!("var({value})"),
        _ if let Some(token) = theme.token_in(namespace, value) => token.value.value.clone(),
        _ => return theme_value(namespace, value, theme),
    };
    Some(value)
}

pub(super) fn typed_theme_arbitrary(namespace: ThemeNamespace, value: &str) -> Option<String> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = decode_arbitrary(value);
    let types = theme_arbitrary_types(namespace);
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint)));
    (hint_is_compatible && (types.is_empty() || types.iter().any(|kind| kind.accepts(&value))))
        .then_some(value)
}

pub(super) fn theme_arbitrary_types(namespace: ThemeNamespace) -> &'static [ValueType] {
    builtin_theme_namespace_types()
        .get(&namespace)
        .map_or(&[], Vec::as_slice)
}

pub(super) fn fraction_value(value: &str) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| format!("calc({numerator} / {denominator} * 100%)"))
}

pub(super) fn axis_size_value(value: &str, viewport_axis: &str) -> Option<String> {
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

pub(super) fn typed_css_variable(namespace: ThemeNamespace, value: &str) -> Option<&str> {
    let variable = parse_css_variable(value)?;
    let types = theme_arbitrary_types(namespace);
    (variable.hint.is_none()
        || types.is_empty()
        || variable
            .hint
            .and_then(ValueType::from_hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint))))
    .then_some(variable.name)
}
