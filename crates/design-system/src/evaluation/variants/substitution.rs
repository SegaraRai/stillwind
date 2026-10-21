//! Capture substitution and composition of nested variants.
//!
//! Explicit capture and modifier placeholders are expanded once. Inserted values
//! are never interpreted as more template syntax; ordinary selectors remain literal.
//!
//! Composition goes the other way: an inner variant's selector is folded into the outer
//! definition's body, and conditions or declarations can only attach to applications
//! that have a selector, because a bare condition has no place to carry them.

use std::sync::Arc;

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{find_css_functions, selector_is_element_condition, split_top_level};
use stillwind_source::ReplaceText;

use super::super::context::EvaluationContext;
use super::super::values::{expand_placeholder, resolve_value_domains, theme_domain_pattern};
use super::application::{VariantApplication, VariantDeclaration, VariantObstacle};
use super::body::resolve_variant_body;
use super::usage::VariantValueSource;
use crate::{CustomVariant, StillwindVariantModifierType};

/// Substitutes a typed modifier into a definition's quoted macro.
///
/// A definition that declares a modifier type requires a modifier and fails without
/// one; a definition without a type leaves the body untouched, so an unused modifier
/// never changes the emitted selector.
pub(in crate::evaluation) fn substitute_typed_modifier(
    body: &str,
    modifier_type: Option<StillwindVariantModifierType>,
    modifier: Option<&str>,
) -> Option<LeanString> {
    let Some(modifier_type) = modifier_type else {
        return Some(body.to_lean_string());
    };
    let modifier = modifier_type.serialize(modifier?)?;
    substitute_quoted_macro(body, ":--modifier(\"", "{modifier}", &modifier)
}

/// Substitutes a capture into the quoted `:--value("...")` macro form.
pub(in crate::evaluation) fn substitute_quoted_variant_value(
    body: &str,
    value: &str,
) -> Option<LeanString> {
    substitute_quoted_macro(body, ":--value(\"", "{value}", value)
}

/// Replaces one quoted macro template with its expanded text.
///
/// The template's closing delimiter must be present; a truncated macro fails rather
/// than leaving half a selector behind.
fn substitute_quoted_macro(
    body: &str,
    prefix: &str,
    placeholder: &str,
    value: &str,
) -> Option<LeanString> {
    let start = body.find(prefix)?;
    let template_start = start + prefix.len();
    let template_end = body[template_start..].find("\")")? + template_start;
    let template = &body[template_start..template_end];
    let replacement = template.replace_text(placeholder, value);
    let mut result = body.to_lean_string();
    result.replace_range(start..template_end + 2, &replacement);
    Some(result)
}

/// Applies an inner variant's application to an outer definition's body.
///
/// Only applications that produce a selector can be folded in: a media, container, or
/// support condition has no position in the outer body, so combining them fails instead
/// of dropping the condition.
pub(in crate::evaluation) fn apply_nested_variant(
    body: &str,
    inner: &VariantApplication,
) -> Option<VariantApplication> {
    match inner {
        VariantApplication::Compound(applications) => applications
            .iter()
            .map(|application| apply_nested_variant(body, application))
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound),
        VariantApplication::Sequence(applications) => {
            let mut outer = None;
            let mut conditions = Vec::new();
            for application in applications {
                if application.uses_subject() {
                    if outer.is_some() {
                        return None;
                    }
                    outer = Some(apply_nested_variant(body, application)?);
                } else {
                    conditions.push(application.clone());
                }
            }
            conditions.push(outer?);
            Some(VariantApplication::Sequence(conditions))
        }
        VariantApplication::SelectorSuffix(suffix) => {
            apply_nested_selector(body, &format_args!("&{suffix}").to_lean_string())
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. } => {
            apply_nested_selector(body, selector)
        }
        VariantApplication::SelectorMedia {
            selector,
            condition,
        } => attach_media(apply_nested_selector(body, selector)?, condition),
        VariantApplication::SelectorRule {
            selector,
            declarations,
        } => attach_declarations(apply_nested_selector(body, selector)?, declarations.clone()),
        VariantApplication::SelectorMediaRule {
            selector,
            declarations,
            condition,
        } => attach_media(
            attach_declarations(apply_nested_selector(body, selector)?, declarations.clone())?,
            condition,
        ),
        VariantApplication::Conditional(_)
        | VariantApplication::SelectorCondition(_)
        | VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}

/// Returns the part of `inner` that [`apply_nested_variant`] cannot place in a
/// selector.
///
/// The first such part is reported, in the order the application lists its parts. A
/// sequence that styles more than one element reports the second one.
pub(in crate::evaluation) fn nesting_obstacle(inner: &VariantApplication) -> VariantObstacle {
    match inner {
        VariantApplication::Compound(applications) => applications
            .iter()
            .map(nesting_obstacle)
            .find(|obstacle| *obstacle != VariantObstacle::Unsupported)
            .unwrap_or(VariantObstacle::Unsupported),
        VariantApplication::Sequence(applications) => {
            let mut subjects = applications
                .iter()
                .filter(|application| application.uses_subject());
            match (subjects.next(), subjects.next()) {
                (Some(application), None) => nesting_obstacle(application),
                (Some(_), Some(_)) => VariantObstacle::OtherElement,
                (None, _) => VariantObstacle::AtRule,
            }
        }
        VariantApplication::SelectorSuffix(suffix) => {
            VariantObstacle::of_selector(&format_args!("&{suffix}").to_lean_string())
                .unwrap_or(VariantObstacle::Unsupported)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            VariantObstacle::of_selector(selector).unwrap_or(VariantObstacle::Unsupported)
        }
        VariantApplication::Conditional(_) => VariantObstacle::RuntimeCondition,
        VariantApplication::SelectorCondition(_) => VariantObstacle::Unsupported,
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => VariantObstacle::AtRule,
    }
}

/// Folds an inner selector into the definition body at its `:--value()` slot.
///
/// The selector must be relative to the utility - it starts with `&` - and the fragment
/// after `&` is what replaces the slot. A selector with a pseudo-element, such as
/// `&::before`, returns `None` for every body, as in Tailwind. The package relationship
/// variants place the slot inside `:is()`, `:where()`, or `:has()`, and those functions
/// cannot contain pseudo-elements.
fn apply_nested_selector(body: &str, selector: &str) -> Option<VariantApplication> {
    if !selector_is_element_condition(selector, false) {
        return None;
    }
    let fragment = selector.strip_prefix('&')?;
    resolve_variant_body(&body.replace_text(":--value()", fragment))
}

/// Folds a relative selector, such as `>img`, into the definition body.
///
/// Every `:--value()` slot must be the complete argument of `:has()`, because CSS
/// accepts a relative selector only there. Other bodies return `None`.
pub(in crate::evaluation) fn apply_relative_selector(
    body: &str,
    selector: &str,
) -> Option<VariantApplication> {
    let slots = body.matches(":--value()").count();
    if slots == 0 || body.matches(":has(:--value())").count() != slots {
        return None;
    }
    resolve_variant_body(&body.replace_text(":--value()", selector))
}

/// Attaches a media condition to a selector application.
///
/// A condition can only wrap an application that already has a selector; the compound
/// and rule-less variants are rejected.
fn attach_media(application: VariantApplication, condition: &str) -> Option<VariantApplication> {
    match application {
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. } => {
            Some(VariantApplication::SelectorMedia {
                selector,
                condition: condition.to_lean_str(),
            })
        }
        VariantApplication::SelectorRule {
            selector,
            declarations,
        } => Some(VariantApplication::SelectorMediaRule {
            selector,
            declarations,
            condition: condition.to_lean_str(),
        }),
        _ => None,
    }
}

/// Attaches a variant's own declarations to a selector application.
pub(in crate::evaluation) fn attach_declarations(
    application: VariantApplication,
    declarations: Vec<VariantDeclaration>,
) -> Option<VariantApplication> {
    match application {
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. } => {
            Some(VariantApplication::SelectorRule {
                selector,
                declarations,
            })
        }
        VariantApplication::SelectorMedia {
            selector,
            condition,
        } => Some(VariantApplication::SelectorMediaRule {
            selector,
            declarations,
            condition,
        }),
        _ => None,
    }
}

/// The definitions that can capture `value`, most specific first.
///
/// Exact names are collected before wildcards so a definition that names the variant
/// exactly is never shadowed by a broader family, and wildcards are tried in reverse
/// authored order so the newest registration wins. A wildcard that would capture an
/// empty string is skipped.
pub(in crate::evaluation) fn registered_variant_candidates<'a, 'b>(
    variants: &'a [Arc<CustomVariant>],
    value: &'b str,
) -> Vec<(&'a CustomVariant, Option<&'b str>)> {
    let mut candidates = Vec::new();
    for variant in variants.iter().rev() {
        if variant.wildcard.is_none() && variant.name == value {
            candidates.push((variant.as_ref(), None));
        }
    }
    for variant in variants.iter().rev() {
        if let Some(capture) = variant.capture(value) {
            candidates.push((variant.as_ref(), Some(capture)));
        }
    }
    candidates
}

/// Fills a definition body's capture placeholders.
///
/// A body without any placeholder is returned as an unchanged owned copy. An arbitrary
/// selector capture loses its `&` and underscore escapes, and a modifier is escaped as
/// `\/` so it stays part of the selector rather than starting a new one.
pub(in crate::evaluation) fn substitute_variant_capture(
    value: &str,
    capture: Option<&str>,
) -> Option<LeanString> {
    if !value.contains("--value()")
        && !value.contains(r"\*")
        && !value.contains("{modifier}")
        && !value.contains(r"\{modifier\}")
    {
        return Some(value.to_lean_string());
    }
    let capture = capture?;
    let (capture, modifier) = capture
        .split_once('/')
        .map_or((capture, None), |(capture, modifier)| {
            (capture, Some(modifier))
        });
    let capture = if let Some(inner) = capture
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        // Arbitrary selector variants use `[&...]`; relationship CSS expects
        // the selector fragment without the variant's leading `&` wrapper.
        inner
            .strip_prefix('&')
            .unwrap_or(inner)
            .replace_char('_', " ")
    } else {
        capture.replace_char('_', " ")
    };
    let modifier = modifier
        .map(|modifier| modifier.replace_char('_', " ").replace_char('/', r"\/"))
        .unwrap_or_default();
    let modifier_suffix = if modifier.is_empty() {
        LeanString::new()
    } else {
        format_args!(r"\/{modifier}").to_lean_string()
    };
    let substitutions = [
        ("--value()", capture.as_str()),
        (r"\*", capture.as_str()),
        (r"\/\{modifier\}", modifier_suffix.as_str()),
        (r"\{modifier\}", modifier.as_str()),
        ("/{modifier}", modifier_suffix.as_str()),
        ("{modifier}", modifier.as_str()),
    ];
    let mut expanded = LeanString::new();
    let mut remaining = value;
    while !remaining.is_empty() {
        if let Some((marker, replacement)) = substitutions
            .iter()
            .find(|(marker, _)| remaining.starts_with(marker))
        {
            expanded.push_str(replacement);
            remaining = &remaining[marker.len()..];
        } else {
            let length = remaining.chars().next()?.len_utf8();
            expanded.push_str(&remaining[..length]);
            remaining = &remaining[length..];
        }
    }
    Some(expanded)
}

/// Resolves `--value(...)` placeholders in a variant body.
///
/// A theme namespace domain yields the authored token value, not a `var()` reference,
/// because the value becomes part of a selector or condition where a variable cannot
/// appear. A placeholder that cannot be resolved fails the body.
pub(in crate::evaluation) fn resolve_variant_css_placeholders(
    body: &str,
    variant_name: &str,
    value: &str,
    theme: &EvaluationContext<'_>,
) -> Option<LeanString> {
    let capture = variant_value_capture(variant_name, value);
    let mut body = body.to_lean_string();
    expand_placeholder(&mut body, "--value", |arguments| {
        (resolve_variant_value_domains(arguments, capture, theme)).map(Into::into)
    })?;
    Some(body)
}

/// Returns the value-domain capture without a modifier, as the placeholder resolver uses it.
pub(super) fn variant_value_capture<'a>(variant_name: &str, value: &'a str) -> Option<&'a str> {
    variant_name
        .strip_suffix('*')
        .and_then(|prefix| value.strip_prefix(prefix))
        .map(|capture| capture.split_once('/').map_or(capture, |(value, _)| value))
}

/// Resolves a variant's value-domain list against a capture.
///
/// Theme namespace domains read the token value directly; every other domain uses the
/// utility resolver, so a variant and a utility accept the same spellings.
fn resolve_variant_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<LeanString> {
    (resolve_variant_value_with_source(domains, capture, theme).map(|(value, _)| value))
        .map(Into::into)
}

pub(super) fn resolve_variant_value_with_source(
    domains: &str,
    capture: Option<&str>,
    theme: &EvaluationContext<'_>,
) -> Option<(LeanStr, VariantValueSource)> {
    split_top_level(domains, |token| matches!(token, Token::Comma))
        .into_iter()
        .find_map(|domain| {
            let domain = domain.trim();
            if let Some((namespace, suffix)) = theme_domain_pattern(domain, theme) {
                return capture.and_then(|name| {
                    namespace.token(name, suffix, theme).map(|token| {
                        theme.record_token(&token.name.value);
                        (
                            token.value.value.clone(),
                            VariantValueSource::ThemeToken(token.name.value.clone()),
                        )
                    })
                });
            }
            let value = resolve_value_domains(domain, capture, theme)?;
            // Media queries, container queries, and selectors cannot contain `var()`, so
            // `min-(--x)` would emit a rule that never matches.
            if !find_css_functions(&value, &["var"], 0).is_empty() {
                return None;
            }
            let source = if let Some(namespace) = theme.namespaces().scale_domain(domain)
                && let Some(token) = capture.and_then(|name| theme.token_in(&namespace.name, name))
            {
                VariantValueSource::ThemeToken(token.name.value.clone())
            } else {
                VariantValueSource::Explicit
            };
            Some((value, source))
        })
}

#[cfg(test)]
mod tests {
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::parse_theme;

    #[test]
    fn recursive_variant_theme_values_are_rejected() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --breakpoint-loop: --value(--breakpoint-*); }",
        )
        .unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        let theme = EvaluationContext::untracked(&theme);
        assert!(
            resolve_variant_css_placeholders(
                "@media (width >= --value(--breakpoint-*)) { @slot; }",
                "screen-*",
                "screen-loop",
                &theme
            )
            .is_none()
        );
    }

    #[test]
    fn explicit_variant_values_reject_custom_property_references() {
        let source =
            SourceFile::new(FileId(1), None, None, "@theme { --breakpoint-sm: 40rem; }").unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        let theme = EvaluationContext::untracked(&theme);
        let body = "@media (width >= --value([length])) { @slot; }";
        for value in ["min-(--x)", "min-[var(--x)]", "min-[calc(var(--x)*2)]"] {
            assert_eq!(
                resolve_variant_css_placeholders(body, "min-*", value, &theme),
                None,
                "{value}"
            );
        }
        assert_eq!(
            resolve_variant_css_placeholders(body, "min-*", "min-[40rem]", &theme),
            Some("@media (width >= 40rem) { @slot; }".into())
        );
    }

    #[test]
    fn quoted_captures_and_typed_modifiers_require_complete_macros() {
        assert_eq!(
            substitute_quoted_variant_value(r#"&:--value("[data-x={value}]")"#, "yes"),
            Some("&[data-x=yes]".into())
        );
        assert_eq!(
            substitute_quoted_variant_value("&:--value(\"missing", "x"),
            None
        );
        assert_eq!(substitute_typed_modifier("&", None, None), Some("&".into()));
        assert_eq!(
            substitute_typed_modifier(
                "&",
                Some(StillwindVariantModifierType::ClassIdentifier),
                None
            ),
            None
        );
        assert_eq!(apply_nested_selector("&:--value() { @slot; }", "div"), None);
    }

    #[test]
    fn media_and_declarations_attach_only_to_selector_applications() {
        let selector = VariantApplication::SelectorTemplate("&:hover".into());
        assert_eq!(
            attach_media(selector.clone(), "print"),
            Some(VariantApplication::SelectorMedia {
                selector: "&:hover".into(),
                condition: "print".into()
            })
        );
        assert!(attach_declarations(selector, vec![]).is_some());
        assert!(attach_media(VariantApplication::StartingStyle, "print").is_none());
        assert!(attach_declarations(VariantApplication::StartingStyle, vec![]).is_none());
        assert!(
            apply_nested_variant(
                "&:--value() { @slot; }",
                &VariantApplication::Media("print".into())
            )
            .is_none()
        );
    }

    #[test]
    fn ordinary_selectors_and_inserted_capture_text_are_literal() {
        assert_eq!(
            substitute_variant_capture(":root:nth-child(0n)", None),
            Some(":root:nth-child(0n)".into())
        );
        assert_eq!(
            substitute_variant_capture("&--value()", Some("{modifier}")),
            Some("&{modifier}".into())
        );
    }
}
