//! Evaluation of one registered variant definition for a concrete capture.
//!
//! An untyped definition substitutes its capture into the body. A typed definition
//! reads the capture as a nested variant, an attribute value, an `nth` argument, a
//! support condition, or a runtime condition, and serializes its modifier through the
//! declared modifier type.

use cssparser::{Parser, Token};
use lean_string::{LeanString, ToLeanStr, ToLeanString};

use stillwind_source::ReplaceText;
use stillwind_syntax::decode_arbitrary_value;

use super::super::super::builtins::builtin_variants;
use super::super::super::context::EvaluationContext;
use super::super::application::VariantApplication;
use super::super::body::{resolve_scoped_variant_body, resolve_variant_body};
use super::super::negation::{negate, negation_obstacle};
use super::super::order::variant_sort_namespace;
use super::super::substitution::{
    apply_nested_variant, apply_relative_selector, nesting_obstacle, registered_variant_candidates,
    resolve_variant_css_placeholders, substitute_quoted_variant_value, substitute_typed_modifier,
    substitute_variant_capture,
};
use super::arbitrary::{
    parse_nested_variant, relative_selector, resolve_nested_variant, shorthand_variant_application,
};
use super::{ConditionContext, VariantFailure, VariantFailureReason};
use crate::{BuiltinVariantValueType, CustomVariant, RuntimeCondition, ThemeIndex};

/// Visits definitions in resolution order, including the registry's bare token form.
/// Only a successful evaluation stops the search.
pub(in crate::evaluation) fn resolve_named_variant_with<T>(
    value: &str,
    theme: &ThemeIndex,
    mut evaluate: impl FnMut(&CustomVariant, Option<&str>, &str) -> Option<T>,
) -> Option<T> {
    for definitions in [theme.variants(), builtin_variants(theme)] {
        for (custom, capture) in registered_variant_candidates(definitions, value) {
            if let Some(result) = evaluate(custom, capture, value) {
                return Some(result);
            }
        }
    }
    for custom in builtin_variants(theme) {
        let Some(namespace) = custom
            .sort
            .as_ref()
            .filter(|sort| sort.bare)
            .and_then(|sort| sort.key.as_deref())
            .and_then(variant_sort_namespace)
        else {
            continue;
        };
        if theme.token_in(namespace, value).is_none() {
            continue;
        }
        let Some((prefix, suffix)) = custom.wildcard_parts() else {
            continue;
        };
        if let Some(result) = evaluate(
            custom,
            Some(value),
            &format_args!("{prefix}{value}{suffix}").to_lean_string(),
        ) {
            return Some(result);
        }
    }
    None
}

/// Returns whether the variant definition `custom` resolves for its own name.
///
/// A definition without a capture resolves the same way for every use. When it
/// resolves, it hides every earlier definition with the same name, because resolution
/// tries later definitions first and falls back only after a failure. A definition with
/// a capture or a value type returns `false`.
#[must_use]
pub fn exact_variant_definition_resolves(custom: &CustomVariant, theme: &ThemeIndex) -> bool {
    custom.wildcard.is_none()
        && custom.value_type.is_none()
        && resolve_registered_variant_candidate(custom, None, &custom.name, 0, theme).is_ok()
}

/// Resolves one definition for a concrete capture and explains a failure.
///
/// A typed definition delegates to [`resolve_typed_variant`]; otherwise the body is
/// filled in and classified, falling back to the definition's shorthand when the body
/// cannot produce an application. The shorthand resolves as
/// [`shorthand_variant_application`] describes.
///
/// `value` is the concrete variant name, and `capture_offset` is the byte offset of
/// `capture` in the variant text as written. Failure ranges are relative to that text.
pub(in crate::evaluation) fn resolve_registered_variant_candidate(
    custom: &CustomVariant,
    capture: Option<&str>,
    value: &str,
    capture_offset: usize,
    theme: &ThemeIndex,
) -> Result<VariantApplication, VariantFailure> {
    if let Some(value_type) = custom.value_type {
        let Some(capture) = capture else {
            return Err(VariantFailure::new(
                0..value.len(),
                VariantFailureReason::InvalidDefinition {
                    variant: custom.name.clone(),
                },
            ));
        };
        return resolve_typed_variant(custom, value_type, capture, capture_offset, theme);
    }
    let fail = || untyped_value_failure(custom, capture, value, capture_offset);
    let body = substitute_variant_capture(&custom.body, capture).ok_or_else(fail)?;
    let body = resolve_variant_css_placeholders(
        &body,
        &custom.name,
        value,
        &EvaluationContext::untracked(theme),
    )
    .ok_or_else(fail)?;
    resolve_scoped_variant_body(&body, custom.scope)
        .or_else(|| {
            shorthand_variant_application(
                &substitute_variant_capture(&custom.selector, capture)?,
                custom.scope,
            )
        })
        .ok_or_else(fail)
}

/// Explains why an untyped definition rejects its capture.
///
/// A `var()` capture names the construct that cannot contain it. A capture written
/// without brackets or parentheses is a name that no theme token or keyword of the
/// definition has. Other captures are invalid
/// values. A definition without a capture produces no application at all.
fn untyped_value_failure(
    custom: &CustomVariant,
    capture: Option<&str>,
    value: &str,
    capture_offset: usize,
) -> VariantFailure {
    let Some(capture) = capture else {
        return VariantFailure::new(
            0..value.len(),
            VariantFailureReason::InvalidDefinition {
                variant: custom.name.clone(),
            },
        );
    };
    let (captured, _) = split_variant_modifier(capture);
    let range = capture_offset..capture_offset + captured.len();
    let prefix = custom
        .wildcard_parts()
        .map_or(custom.name.as_str(), |(prefix, _)| prefix);
    let reason = if is_variable_capture(captured) {
        VariantFailureReason::VariableInCondition {
            context: ConditionContext::of_body(&custom.body),
        }
    } else if captured.starts_with(['[', '(']) {
        VariantFailureReason::InvalidValue {
            variant: prefix.to_lean_str(),
            value: decode_variant_capture(captured).into(),
        }
    } else {
        VariantFailureReason::UnknownValue {
            variant: prefix.to_lean_str(),
            value: captured.to_lean_str(),
        }
    };
    VariantFailure::new(range, reason)
}

/// Whether a capture is a `var()` reference, written as `(--name)` or inside brackets.
fn is_variable_capture(capture: &str) -> bool {
    capture.starts_with("(--") || decode_variant_capture(capture).contains("var(")
}

/// Resolves a definition that declared a value type, which decides how the capture
/// becomes CSS.
///
/// A variant-stack capture is resolved recursively and applied to the body, or negated
/// when the body is exactly the `builtin-not-variant` intrinsic; aria and data captures
/// become attribute selectors; `nth` and support conditions are validated by CSS itself
/// before substitution. A modifier is serialized through the declared modifier type, so
/// a modifier that is not a valid class identifier fails instead of producing a broken
/// selector. A variant-stack definition without a modifier declaration passes the
/// modifier to its inner variant, as described by [`split_typed_capture`].
/// `[if-condition]` captures bypass modifier splitting and become their canonical
/// condition text, which keeps slashes in comparison values. They require an exact
/// `builtin-if-variant` intrinsic body and no modifier declaration.
///
/// `capture_offset` is the byte offset of `capture` in the variant text as written.
/// A failure of an inner variant points at the inner text.
#[allow(clippy::too_many_lines)] // One match explains each value type's failures.
fn resolve_typed_variant(
    custom: &CustomVariant,
    value_type: BuiltinVariantValueType,
    capture: &str,
    capture_offset: usize,
    theme: &ThemeIndex,
) -> Result<VariantApplication, VariantFailure> {
    let prefix = custom
        .wildcard_parts()
        .map_or(custom.name.as_str(), |(prefix, _)| prefix)
        .to_lean_str();
    let invalid_definition = || {
        VariantFailure::new(
            capture_offset..capture_offset + capture.len(),
            VariantFailureReason::InvalidDefinition {
                variant: custom.name.clone(),
            },
        )
    };
    if value_type == BuiltinVariantValueType::IfCondition {
        if custom.modifier_type.is_some() || !has_intrinsic_body(custom, "builtin-if-variant") {
            return Err(invalid_definition());
        }
        return RuntimeCondition::parse(capture)
            .map(|condition| VariantApplication::Conditional(condition.to_lean_str()))
            .ok_or_else(|| {
                VariantFailure::new(
                    capture_offset..capture_offset + capture.len(),
                    VariantFailureReason::InvalidCondition {
                        condition: capture.to_lean_str(),
                    },
                )
            });
    }
    let (value, modifier) = split_typed_capture(custom, capture);
    let value_range = capture_offset..capture_offset + value.len();
    let Some(body) = substitute_typed_modifier(&custom.body, custom.modifier_type, modifier) else {
        return Err(match modifier {
            None => VariantFailure::new(
                value_range,
                VariantFailureReason::MissingModifier { variant: prefix },
            ),
            Some(modifier) => VariantFailure::new(
                value_range.end..capture_offset + capture.len(),
                VariantFailureReason::InvalidModifier {
                    variant: prefix,
                    modifier: modifier.to_lean_str(),
                },
            ),
        });
    };
    let invalid_value = |shown: &str| {
        VariantFailure::new(
            value_range.clone(),
            VariantFailureReason::InvalidValue {
                variant: prefix.clone(),
                value: shown.to_lean_str(),
            },
        )
    };
    match value_type {
        BuiltinVariantValueType::IfCondition => Err(invalid_definition()),
        BuiltinVariantValueType::Variant => {
            let inner = parse_nested_variant(value);
            if let Some(selector) = relative_selector(&inner) {
                return apply_relative_selector(&body, selector).ok_or_else(|| {
                    VariantFailure::new(
                        value_range.clone(),
                        VariantFailureReason::RelativeSelector {
                            selector: selector.to_lean_str(),
                        },
                    )
                });
            }
            let application = resolve_nested_variant(&inner, theme)
                .map_err(|failure| failure.shifted(capture_offset))?;
            if has_intrinsic_body(custom, "builtin-not-variant") {
                if custom.modifier_type.is_some() {
                    return Err(invalid_definition());
                }
                return negate(&application).ok_or_else(|| {
                    VariantFailure::new(
                        value_range.clone(),
                        VariantFailureReason::CannotNegate {
                            variant: prefix.clone(),
                            inner: value.to_lean_str(),
                            obstacle: negation_obstacle(&application),
                        },
                    )
                });
            }
            apply_nested_variant(&body, &application).ok_or_else(|| {
                VariantFailure::new(
                    value_range.clone(),
                    VariantFailureReason::CannotNest {
                        variant: prefix.clone(),
                        inner: value.to_lean_str(),
                        obstacle: nesting_obstacle(&application),
                    },
                )
            })
        }
        BuiltinVariantValueType::Aria | BuiltinVariantValueType::Data => {
            let (name, default_true) = if value_type == BuiltinVariantValueType::Aria {
                ("aria", true)
            } else {
                ("data", false)
            };
            attribute_variant_selector(name, value, default_true)
                .and_then(|selector| {
                    resolve_variant_body(&body.replace_text(":--value()", &selector))
                })
                .ok_or_else(|| invalid_value(&decode_variant_capture(value)))
        }
        BuiltinVariantValueType::Nth | BuiltinVariantValueType::SupportsCondition => {
            let decoded = decode_variant_capture(value);
            let css = if value_type == BuiltinVariantValueType::SupportsCondition {
                supports_feature_test(decoded.clone())
            } else {
                decoded.clone()
            };
            if !value_type.accepts_css(&css) {
                if is_variable_capture(value) {
                    return Err(VariantFailure::new(
                        value_range,
                        VariantFailureReason::VariableInCondition {
                            context: if value_type == BuiltinVariantValueType::Nth {
                                ConditionContext::Selector
                            } else {
                                ConditionContext::Supports
                            },
                        },
                    ));
                }
                return Err(invalid_value(&decoded));
            }
            let body = if value_type == BuiltinVariantValueType::Nth {
                substitute_quoted_variant_value(&body, &css)
            } else {
                Some(body.replace_text("--value()", &css))
            };
            body.and_then(|body| resolve_variant_body(&body))
                .ok_or_else(|| invalid_value(&decoded))
        }
    }
}

/// Recognizes a body that consists only of `@stillwind-intrinsic <intrinsic>;`.
fn has_intrinsic_body(custom: &CustomVariant, intrinsic: &str) -> bool {
    let mut parser = Parser::new(&custom.body);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.as_ref() == "stillwind-intrinsic")
        && matches!(parser.next(), Ok(Token::Ident(name)) if name.as_ref() == intrinsic)
        && matches!(parser.next(), Ok(Token::Semicolon))
        && parser.is_exhausted()
}

/// Splits a typed definition's capture into its value and the definition's own
/// modifier.
///
/// A `[variant]` definition without a `modifier:` declaration has no modifier of its
/// own, so the whole capture, including `/modifier`, belongs to the inner variant:
/// `not-group-hover/menu` passes `group-hover/menu` to the inner variant. Other
/// definitions split the capture with [`split_variant_modifier`].
pub(in crate::evaluation) fn split_typed_capture<'a>(
    custom: &CustomVariant,
    capture: &'a str,
) -> (&'a str, Option<&'a str>) {
    if custom.value_type == Some(BuiltinVariantValueType::Variant) && custom.modifier_type.is_none()
    {
        (capture, None)
    } else {
        split_variant_modifier(capture)
    }
}

/// Splits a capture at the first top-level `/`, leaving nested brackets and parentheses
/// intact.
///
/// Returns the capture paired with the modifier, which is `None` when no top-level
/// slash exists.
pub(in crate::evaluation) fn split_variant_modifier(value: &str) -> (&str, Option<&str>) {
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

/// Decodes a variant capture into CSS text.
///
/// An arbitrary capture is decoded with escape handling; a named capture only has
/// underscores turned into spaces, because its text is a Tailwind spelling rather than
/// CSS.
fn decode_variant_capture(value: &str) -> LeanString {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(|| value.replace_char('_', " "), decode_arbitrary_value)
}

/// Turns a bare property name in a `@supports` capture into a declaration test.
///
/// `@supports (grid)` is always false, so `grid` becomes `grid: var(--tw)`, which
/// is true when the browser knows the property. A capture that starts with a function,
/// such as `selector(...)`, or that contains `:` is already a condition and is kept.
fn supports_feature_test(value: LeanString) -> LeanString {
    let is_function = value.find('(').is_some_and(|index| {
        value[..index].trim_end().chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    });
    if is_function || value.contains(':') {
        value
    } else {
        format_args!("{value}: var(--tw)").to_lean_string()
    }
}

/// Builds an attribute selector for an aria or data variant.
///
/// `name=value` becomes an equality selector with the value quoted and escaped; a bare
/// capture becomes `="true"` for aria variants and a presence check for data variants.
/// An empty name or value is rejected rather than emitted.
fn attribute_variant_selector(
    prefix: &str,
    capture: &str,
    default_true: bool,
) -> Option<LeanString> {
    let arbitrary = capture
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'));
    let value = arbitrary.map_or_else(|| capture.to_lean_string(), decode_arbitrary_value);
    if value.is_empty() {
        return None;
    }
    if let Some((name, expected)) = value.split_once('=') {
        if name.is_empty() || expected.is_empty() {
            return None;
        }
        return Some(
            format_args!(
                "[{prefix}-{name}=\"{}\"]",
                expected.replace_char('"', r#"\""#)
            )
            .to_lean_string(),
        );
    }
    Some(if default_true {
        format_args!("[{prefix}-{value}=\"true\"]").to_lean_string()
    } else {
        format_args!("[{prefix}-{value}]").to_lean_string()
    })
}

#[cfg(test)]
mod tests {
    use stillwind_syntax::VariantKind;

    use super::*;

    #[test]
    fn supports_captures_test_bare_properties_and_keep_conditions() {
        for (capture, expected) in [
            ("grid", "grid: var(--tw)"),
            ("--foo", "--foo: var(--tw)"),
            ("display:grid", "display:grid"),
            ("selector(a:hover)", "selector(a:hover)"),
            ("font-format(woff2)", "font-format(woff2)"),
            ("(display:grid) and (gap:0)", "(display:grid) and (gap:0)"),
        ] {
            assert_eq!(supports_feature_test(capture.into()), expected, "{capture}");
        }
    }

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
