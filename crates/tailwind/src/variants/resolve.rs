use stillwind_css::{BuiltinVariantValueType, CustomVariant, ThemeIndex};
use stillwind_grammar::{Utility, Value, Variant, VariantKind, parse_candidate};

use crate::builtins::metadata::{builtin_marker_utilities, builtin_relationship_patterns};
use crate::builtins::registry::builtin_variants;
use crate::values::arbitrary::decode_arbitrary;

use super::body::resolve_variant_body;
use super::order::variant_sort_namespace;
use super::substitution::{
    apply_nested_variant, registered_variant_candidates, resolve_variant_css_placeholders,
    substitute_quoted_variant_value, substitute_typed_modifier, substitute_variant_capture,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    Compound(Vec<VariantApplication>),
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
    },
    SelectorMedia {
        selector: String,
        condition: String,
    },
    SelectorMediaRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
        condition: String,
    },
    Media(String),
    Container(String),
    Supports(String),
    StartingStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    pub property: String,
    pub value: String,
}

#[must_use]
pub fn resolve_variant(variant: &Variant, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let VariantKind::Arbitrary(value) = &variant.kind {
        if let Some(condition) = value
            .strip_prefix("@supports(")
            .and_then(|value| value.strip_suffix(')'))
        {
            return Some(VariantApplication::Supports(condition.to_owned()));
        }
        return Some(VariantApplication::SelectorTemplate(value.clone()));
    }
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    if let Some(application) = resolve_registered_variant(value, theme) {
        return Some(application);
    }
    resolve_bare_registered_variant(value, theme)
}

/// Whether a Tailwind candidate uses a relationship primitive for which
/// projects may prefer Stillwind's typed handle API.
#[must_use]
pub fn is_tailwind_relationship_candidate(raw: &str, theme: &ThemeIndex) -> bool {
    let Some(candidate) = parse_candidate(raw) else {
        return false;
    };
    let matches_pattern = |value: &str| {
        theme
            .relationship_patterns
            .iter()
            .chain(builtin_relationship_patterns().iter())
            .any(|pattern| {
                pattern
                    .strip_suffix('*')
                    .map_or(value == pattern, |prefix| value.starts_with(prefix))
            })
    };
    matches!(
        &candidate.utility,
        Utility::Named { name, .. } if matches_pattern(name)
        || name.starts_with('@'))
        || raw.starts_with('@')
        || candidate.variants.iter().any(|variant| {
            matches!(&variant.kind, VariantKind::Named(name) if matches_pattern(name))
                || variant.raw.starts_with('@')
        })
}

/// Returns the literal marker class carried by Tailwind's declaration-only
/// `group` and `peer` utilities. These classes intentionally bypass hashing so
/// relational selectors can find them across independent `style()` calls.
#[must_use]
pub fn compatibility_marker_class(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let candidate = parse_candidate(raw)?;
    if candidate.negative || candidate.important || !candidate.variants.is_empty() {
        return None;
    }
    let Utility::Named { name, modifier } = candidate.utility else {
        return None;
    };
    if !theme.marker_utilities.contains(name.as_str())
        && !builtin_marker_utilities().contains(name.as_str())
    {
        return None;
    }
    match modifier {
        None => Some(name),
        Some(Value::Named(modifier)) => Some(format!("{name}/{modifier}")),
        Some(Value::Arbitrary(_) | Value::CssVariable(_)) => None,
    }
}

pub(crate) fn resolve_registered_variant(
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    if let Some(application) = resolve_registered_variant_from(&theme.variants, value, theme) {
        return Some(application);
    }
    theme
        .imports
        .builtins
        .then(|| resolve_registered_variant_from(builtin_variants(), value, theme))?
}

pub(crate) fn resolve_bare_registered_variant(
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    theme.imports.builtins.then(|| {
        builtin_variants().iter().find_map(|variant| {
            let sort = variant.sort.as_ref().filter(|sort| sort.bare)?;
            let namespace = sort.key.as_deref().and_then(variant_sort_namespace)?;
            theme.token_in(namespace, value)?;
            let prefix = variant.name.strip_suffix('*')?;
            let expanded = format!("{prefix}{value}");
            resolve_registered_variant_candidate(variant, Some(value), &expanded, theme)
        })
    })?
}

pub(crate) fn resolve_registered_variant_from(
    variants: &[CustomVariant],
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    registered_variant_candidates(variants, value)
        .into_iter()
        .find_map(|(custom, capture)| {
            resolve_registered_variant_candidate(custom, capture, value, theme)
        })
}

pub(crate) fn resolve_registered_variant_candidate(
    custom: &CustomVariant,
    capture: Option<&str>,
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    if let Some(value_type) = custom.value_type {
        return resolve_typed_variant(custom, value_type, capture?, theme);
    }
    let body = substitute_variant_capture(&custom.body, capture)?;
    let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
    resolve_variant_body(&body).or_else(|| {
        substitute_variant_capture(&custom.selector, capture)
            .map(VariantApplication::SelectorTemplate)
    })
}

pub(crate) fn resolve_typed_variant(
    custom: &CustomVariant,
    value_type: BuiltinVariantValueType,
    capture: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    let (capture, modifier) = split_variant_modifier(capture);
    let body = substitute_typed_modifier(&custom.body, custom.modifier_type, modifier)?;
    match value_type {
        BuiltinVariantValueType::Variant => {
            let inner = parse_nested_variant(capture);
            let application = resolve_variant(&inner, theme)?;
            apply_nested_variant(&body, &application)
        }
        BuiltinVariantValueType::Aria => {
            let value = attribute_variant_selector("aria", capture, true)?;
            resolve_variant_body(&body.replace(":--value()", &value))
        }
        BuiltinVariantValueType::Data => {
            let value = attribute_variant_selector("data", capture, false)?;
            resolve_variant_body(&body.replace(":--value()", &value))
        }
        BuiltinVariantValueType::Nth | BuiltinVariantValueType::SupportsCondition => {
            let value = decode_variant_capture(capture);
            if !value_type.accepts_css(&value) {
                return None;
            }
            let body = if value_type == BuiltinVariantValueType::Nth {
                substitute_quoted_variant_value(&body, &value)?
            } else {
                body.replace("--value()", &value)
            };
            resolve_variant_body(&body)
        }
    }
}

pub(crate) fn split_variant_modifier(value: &str) -> (&str, Option<&str>) {
    let mut depth = 0_u32;
    for (index, character) in value.char_indices() {
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '/' if depth == 0 => return (&value[..index], Some(&value[index + 1..])),
            _ => {}
        }
    }
    (value, None)
}

pub(crate) fn decode_variant_capture(value: &str) -> String {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(|| value.replace('_', " "), decode_arbitrary)
}

pub(crate) fn parse_nested_variant(value: &str) -> Variant {
    let raw = value.to_owned();
    let kind = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(
            || VariantKind::Named(value.to_owned()),
            |value| VariantKind::Arbitrary(decode_arbitrary(value)),
        );
    Variant { kind, raw }
}

pub(crate) fn attribute_variant_selector(
    prefix: &str,
    capture: &str,
    default_true: bool,
) -> Option<String> {
    let arbitrary = capture
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'));
    let value = arbitrary.map_or_else(|| capture.to_owned(), decode_arbitrary);
    if value.is_empty() {
        return None;
    }
    if let Some((name, expected)) = value.split_once('=') {
        if name.is_empty() || expected.is_empty() {
            return None;
        }
        return Some(format!(
            "[{prefix}-{name}=\"{}\"]",
            expected.replace('"', r#"\""#)
        ));
    }
    Some(if default_true {
        format!("[{prefix}-{value}=\"true\"]")
    } else {
        format!("[{prefix}-{value}]")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_attribute_captures_preserve_nested_syntax() {
        assert_eq!(
            split_variant_modifier("[x/y]/group"),
            ("[x/y]", Some("group"))
        );
        assert_eq!(split_variant_modifier("[x/y]"), ("[x/y]", None));
        assert_eq!(decode_variant_capture("[a_b]"), "a b");
        assert_eq!(
            attribute_variant_selector("aria", "checked", true),
            Some("[aria-checked=\"true\"]".into())
        );
        assert_eq!(
            attribute_variant_selector("data", "open", false),
            Some("[data-open]".into())
        );
        assert_eq!(
            attribute_variant_selector("data", "[state=on]", false),
            Some("[data-state=\"on\"]".into())
        );
        assert_eq!(attribute_variant_selector("data", "state=", false), None);
        assert!(matches!(
            parse_nested_variant("[&:hover]").kind,
            VariantKind::Arbitrary(_)
        ));
    }
}
