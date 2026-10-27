//! Theme-token lookup for candidate values.
//!
//! Two lookups exist and must not be confused. [`theme_value`] produces the value the
//! generated declaration should reference, which is a `var()` reference for an ordinary
//! token and the authored text for an inline token. [`theme_value_literal`] reads the
//! authored token text so shadow color overrides can replace colors inside the value.
//!
//! Typed captures - `[hint:value]`, parenthesized variables, and arbitrary lengths -
//! are validated against the namespace's CSS-declared value types, so the accepted
//! grammar follows the registry rather than this file. Numeric captures use the
//! namespace's scale only when the registry declares one.

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{ValueType, value_is_well_formed};
use stillwind_syntax::decode_arbitrary_value;

use super::super::context::{ArbitraryValueCheck, EvaluationContext};
use super::arbitrary::parse_css_variable;
use crate::{ThemeIndex, ThemeNamespace, ThemeValue, ThemeValueMode};

/// The namespace that a theme namespace pattern such as `--color-*` reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::evaluation) enum PatternNamespace<'a> {
    /// A namespace that the registry declares. [`theme_value`] resolves its captures,
    /// so its declared value types, typed arbitrary values, and its scale apply.
    Declared(&'a ThemeNamespace),
    /// Any other namespace name, such as `size` or `color-red`, without the leading
    /// `--` and the trailing `-`. Only named tokens resolve, as
    /// [`ThemeIndex::namespace_token`] describes, and no value types are declared.
    Other(&'a str),
}

impl<'a> PatternNamespace<'a> {
    /// Reads the namespace name `name`, such as `color` or `size-lg`, against the
    /// namespaces that `theme` declares.
    pub(in crate::evaluation) fn parse(name: &'a str, theme: &'a ThemeIndex) -> Self {
        theme
            .namespaces()
            .get(name)
            .map_or(Self::Other(name), Self::Declared)
    }

    /// Returns the namespace name without the leading `--`, such as `radius`.
    pub(in crate::evaluation) fn name(self) -> &'a str {
        match self {
            Self::Declared(namespace) => &namespace.name,
            Self::Other(name) => name,
        }
    }

    /// Resolves the capture `key` with the pattern suffix `suffix`, such as `--alt` of
    /// `--size-*--alt`, to the text a declaration should contain. Returns [`None`] when
    /// the namespace has no matching value.
    pub(in crate::evaluation) fn resolve(
        self,
        key: &str,
        suffix: &str,
        theme: &EvaluationContext<'_>,
    ) -> Option<LeanStr> {
        match self {
            Self::Declared(namespace) => theme_value(
                namespace,
                &format_args!("{key}{suffix}").to_lean_str(),
                theme,
            ),
            Self::Other(_) => self
                .token(key, suffix, theme)
                .map(|token| token_reference(token, theme)),
        }
    }

    /// Returns the active token that the capture `key` with the pattern suffix `suffix`
    /// selects.
    ///
    /// A [`Self::Declared`] namespace reads the token named by `key` followed by
    /// `suffix`. A [`Self::Other`] namespace with a suffix first requires the token
    /// that `key` selects, such as `--size-x`, and then reads the token whose name adds
    /// `suffix`, such as `--size-x--alt`, as Tailwind does.
    pub(in crate::evaluation) fn token<'t>(
        self,
        key: &str,
        suffix: &str,
        theme: &'t ThemeIndex,
    ) -> Option<&'t ThemeValue> {
        match self {
            Self::Declared(namespace) => theme.token_in(
                &namespace.name,
                &format_args!("{key}{suffix}").to_lean_str(),
            ),
            Self::Other(namespace) => {
                let token = theme.namespace_token(namespace, key)?;
                if suffix.is_empty() {
                    return Some(token);
                }
                theme.token_property(&format_args!("{}{suffix}", token.name.value).to_lean_str())
            }
        }
    }

    /// Returns the captures that select an active token with the pattern suffix
    /// `suffix`, as [`Self::token`] describes. An active scale adds its fixed keys when
    /// `suffix` is empty. Captures can repeat, and the scale token is not included.
    pub(in crate::evaluation) fn keys(self, suffix: &str, theme: &'a ThemeIndex) -> Vec<&'a str> {
        match self {
            Self::Declared(namespace) => {
                let mut keys: Vec<_> = theme
                    .tokens_in(&namespace.name)
                    .into_iter()
                    .filter_map(|(key, _)| key.strip_suffix(suffix))
                    .collect();
                if suffix.is_empty()
                    && theme.scale(&namespace.name).is_some()
                    && let Some(scale) = &namespace.scale
                {
                    keys.extend(scale.keys.keys().map(LeanStr::as_str));
                }
                keys
            }
            Self::Other(namespace) => {
                let keys = theme
                    .namespace_tokens(namespace)
                    .into_iter()
                    .map(|(key, _)| key)
                    .collect::<Vec<_>>();
                keys.iter()
                    .filter_map(|key| key.strip_suffix(suffix))
                    .filter(|key| suffix.is_empty() || keys.contains(key))
                    .collect()
            }
        }
    }

    /// The value types declared for the namespace, which are empty for [`Self::Other`].
    pub(in crate::evaluation) fn value_types(self) -> &'a [ValueType] {
        match self {
            Self::Declared(namespace) => &namespace.types,
            Self::Other(_) => &[],
        }
    }
}

/// Returns the text that refers to the bare token of the namespace `namespace`, such as
/// `--radius` for `radius`, as [`token_reference`] describes. Returns `None` when the
/// token is not active.
pub(in crate::evaluation) fn bare_token_value(
    namespace: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let token = theme.token_property(&format_args!("--{namespace}").to_lean_str())?;
    Some(token_reference(token, theme))
}

/// Returns the authored value of the bare token of the namespace `namespace`, such as
/// `--shadow` for `shadow`, and records the token as used. Returns `None` when the token
/// is not active.
pub(in crate::evaluation) fn bare_token_literal(
    namespace: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let token = theme.token_property(&format_args!("--{namespace}").to_lean_str())?;
    theme.record_token(&token.name.value);
    Some(token.value.value.clone())
}

/// Returns the text that refers to `token` in a declaration: the authored value of an
/// inline token, and a `var()` reference otherwise. Records the token as used.
fn token_reference(token: &ThemeValue, theme: &EvaluationContext<'_>) -> LeanStr {
    theme.record_token(&token.name.value);
    if matches!(token.mode, ThemeValueMode::Inline) {
        token.value.value.clone()
    } else {
        format_args!("var({})", token.name.value).to_lean_str()
    }
}

/// Resolves a value in `namespace` to the text a declaration should contain.
///
/// Order matters: a typed arbitrary or variable capture wins over a named token, a
/// named token wins over the namespace's scale, and a value that matches nothing
/// returns [`None`]. The named-token path rejects non-ASCII captures; this is a
/// restriction of this lookup, not of the general candidate parser. Typed arbitrary
/// and variable captures are attempted before that restriction.
pub(in crate::evaluation) fn theme_value(
    namespace: &ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format_args!("var({value})").to_lean_str()
        }
        // This named-token lookup is ASCII-only; the earlier arbitrary-value and
        // variable paths do not share that restriction.
        _ if !value.is_ascii() => return None,
        _ if let Some(token) = theme.token_in(&namespace.name, value) => {
            token_reference(token, theme)
        }
        _ if let Some(scale) = namespace.scale.as_ref()
            && let Some(token) = theme.scale(&namespace.name) =>
        {
            if let Some(value) = scale.keys.get(value) {
                theme.record_value_tokens(value);
                value.clone()
            } else if value.parse::<f64>().is_ok() {
                if value.parse::<f64>().is_ok_and(|number| number == 0.0) {
                    return Some("0".to_lean_str());
                }
                let scale = token_reference(token, theme);
                return Some(format_args!("calc({scale} * {value})").to_lean_str());
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
/// Shadow color overrides read the authored value to find the colors they replace.
/// Falls back to [`theme_value`] when no token matches.
pub(in crate::evaluation) fn theme_value_literal(
    namespace: &ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let value = match (namespace, value) {
        _ if let Some(value) = typed_theme_arbitrary(namespace, value, theme) => value,
        _ if let Some(value) = typed_css_variable(namespace, value, theme) => {
            format_args!("var({value})").to_lean_str()
        }
        _ if let Some(token) = theme.token_in(&namespace.name, value) => {
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
/// `2s` is a valid value of some other type. When `theme` checks
/// [`ArbitraryValueCheck::WellFormed`] values, every well-formed value without a hint is
/// accepted.
fn typed_theme_arbitrary(
    namespace: &ThemeNamespace,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanStr> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = theme.inline_theme_functions(&decode_arbitrary_value(value))?;
    let value = theme.resolve_theme_references(&value).ok()??;
    let value = theme.resolve_css_functions(&value)?;
    let types = &namespace.types;
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint)));
    let accepted = types.is_empty()
        || types.iter().any(|kind| kind.accepts(&value))
        || (hint.is_empty()
            && theme.arbitrary_check() == ArbitraryValueCheck::WellFormed
            && value_is_well_formed(&value));
    (hint_is_compatible && accepted).then(|| {
        theme.record_value_tokens(&value);
        theme.record_arbitrary_value(&value, types.contains(&ValueType::Color));
        value.into()
    })
}

/// Resolves a `n/d` fraction capture into a percentage.
///
/// Zero is a valid numerator but not a denominator, and both components are limited to
/// unsigned 16-bit integers so a fraction cannot overflow.
pub(in crate::evaluation) fn fraction_value(value: &str) -> Option<LeanStr> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    ((denominator != 0)
        .then(|| format_args!("calc({numerator} / {denominator} * 100%)").to_lean_string()))
    .map(Into::into)
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
) -> Option<LeanStr> {
    let value = match value {
        "full" => "100%".to_lean_string(),
        "screen" => format_args!("100{}", viewport_axis.unit()).to_lean_string(),
        "svw" => "100svw".to_lean_string(),
        "lvw" => "100lvw".to_lean_string(),
        "dvw" => "100dvw".to_lean_string(),
        "svh" => "100svh".to_lean_string(),
        "lvh" => "100lvh".to_lean_string(),
        "dvh" => "100dvh".to_lean_string(),
        "min" => "min-content".to_lean_string(),
        "max" => "max-content".to_lean_string(),
        "fit" => "fit-content".to_lean_string(),
        "lh" => "1lh".to_lean_string(),
        _ => return None,
    };
    Some((value).into())
}

fn typed_css_variable<'a>(
    namespace: &ThemeNamespace,
    value: &'a str,
    theme: &EvaluationContext<'_>,
) -> Option<&'a str> {
    let variable = parse_css_variable(value)?;
    let types = &namespace.types;
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
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use stillwind_source::{CssSourceName, FileId, SourceFile};
    use stillwind_test_support::PACKAGE_CSS;

    use super::*;
    use crate::theme::ThemeBuilder;
    use crate::{StillwindDefinitions, parse_theme};

    #[test]
    fn scales_use_inline_values_and_track_fixed_key_dependencies() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@stillwind-theme-namespace --step-* [length] scale(hair: var(--hairline, 1px));\
             @theme inline { --step: 2px; }",
        )
        .unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("theme.css")).unwrap();
        let namespace = theme.namespaces().get("step").unwrap();
        let context = EvaluationContext::tracked(&theme, ArbitraryValueCheck::Grammar);
        assert_eq!(
            theme_value(namespace, "2", &context).as_deref(),
            Some("calc(2px * 2)")
        );
        assert_eq!(
            context.take_selections().tokens,
            BTreeSet::from(["--step".into()])
        );
        assert_eq!(
            theme_value(namespace, "hair", &context).as_deref(),
            Some("var(--hairline, 1px)")
        );
        assert_eq!(
            context.take_selections().tokens,
            BTreeSet::from(["--hairline".into()])
        );
    }

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
            registry: Arc::new(StillwindDefinitions::from_css(PACKAGE_CSS)),
            ..Default::default()
        }
        .finish();
        let spacing = configured.namespaces().get("spacing").unwrap().clone();
        let color = configured.namespaces().get("color").unwrap().clone();
        let configured = EvaluationContext::untracked(&configured);
        assert_eq!(
            typed_theme_arbitrary(&spacing, "[length:2px]", &configured),
            Some("2px".into())
        );
        assert_eq!(
            typed_theme_arbitrary(&spacing, "[time:2s]", &configured),
            None
        );
        assert_eq!(
            typed_css_variable(&color, "(color:--x)", &configured),
            Some("--x")
        );
        assert_eq!(
            typed_css_variable(&color, "(length:--x)", &configured),
            None
        );
        let theme = ThemeIndex::default();
        let theme = EvaluationContext::untracked(&theme);
        assert_eq!(theme_value(&color, "missing", &theme), None);
        assert_eq!(
            theme_value_literal(&color, "[red]", &theme),
            Some("red".into())
        );
    }

    proptest::proptest! {
        #[test]
        fn valid_fraction_components_are_never_reversed(a in 0u16..1000, b in 1u16..1000) {
            proptest::prop_assert_eq!(fraction_value(&format_args!("{a}/{b}").to_lean_string()), Some(format_args!("calc({a} / {b} * 100%)").to_lean_str()));
        }
    }
}
