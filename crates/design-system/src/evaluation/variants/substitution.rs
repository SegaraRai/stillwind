//! Capture substitution and composition of nested variants.
//!
//! Substitution is a fixed sequence of textual replacements rather than a template
//! language: `--value()`, an escaped capture marker, the modifier forms, and the two
//! placeholders used by `nth`-style selectors are replaced in that order. Inserted
//! capture text can therefore be changed by later replacements. This path replaces
//! underscores and modifier slashes; it is not a general CSS selector escaper.
//!
//! Composition goes the other way: an inner variant's selector is folded into the outer
//! definition's body, and conditions or declarations can only attach to applications
//! that have a selector, because a bare condition has no place to carry them.

use super::super::values::domains::{resolve_value_domains, split_domains, theme_domain_pattern};
use super::super::values::expansion::expand_placeholder;
use super::body::resolve_variant_body;
use super::resolve::{VariantApplication, VariantDeclaration};
use crate::{BuiltinVariantModifierType, CustomVariant, ThemeIndex};

/// Substitutes a typed modifier into a definition's quoted macro.
///
/// A definition that declares a modifier type requires a modifier and fails without
/// one; a definition without a type leaves the body untouched, so an unused modifier
/// never changes the emitted selector.
pub(crate) fn substitute_typed_modifier(
    body: &str,
    modifier_type: Option<BuiltinVariantModifierType>,
    modifier: Option<&str>,
) -> Option<String> {
    let Some(modifier_type) = modifier_type else {
        return Some(body.to_owned());
    };
    let modifier = modifier_type.serialize(modifier?)?;
    substitute_quoted_macro(body, ":--modifier(\"", "{modifier}", &modifier)
}

/// Substitutes a capture into the quoted `:--value("...")` macro form.
pub(crate) fn substitute_quoted_variant_value(body: &str, value: &str) -> Option<String> {
    substitute_quoted_macro(body, ":--value(\"", "{value}", value)
}

/// Replaces one quoted macro template with its expanded text.
///
/// The template's closing delimiter must be present; a truncated macro fails rather
/// than leaving half a selector behind.
pub(crate) fn substitute_quoted_macro(
    body: &str,
    prefix: &str,
    placeholder: &str,
    value: &str,
) -> Option<String> {
    let start = body.find(prefix)?;
    let template_start = start + prefix.len();
    let template_end = body[template_start..].find("\")")? + template_start;
    let template = &body[template_start..template_end];
    let replacement = template.replace(placeholder, value);
    let mut result = body.to_owned();
    result.replace_range(start..template_end + 2, &replacement);
    Some(result)
}

/// Applies an inner variant's application to an outer definition's body.
///
/// Only applications that produce a selector can be folded in: a media, container, or
/// support condition has no position in the outer body, so combining them fails instead
/// of dropping the condition.
pub(crate) fn apply_nested_variant(
    body: &str,
    inner: &VariantApplication,
) -> Option<VariantApplication> {
    match inner {
        VariantApplication::Compound(applications) => applications
            .iter()
            .map(|application| apply_nested_variant(body, application))
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound),
        VariantApplication::SelectorSuffix(suffix) => {
            apply_nested_selector(body, &format!("&{suffix}"))
        }
        VariantApplication::SelectorTemplate(selector) => apply_nested_selector(body, selector),
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
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}

/// Folds an inner selector into the definition body at its `:--value()` slot.
///
/// The selector must be relative to the utility - it starts with `&` - and the fragment
/// after `&` is what replaces the slot.
pub(crate) fn apply_nested_selector(body: &str, selector: &str) -> Option<VariantApplication> {
    let fragment = selector.strip_prefix('&')?;
    resolve_variant_body(&body.replace(":--value()", fragment))
}

/// Attaches a media condition to a selector application.
///
/// A condition can only wrap an application that already has a selector; the compound
/// and rule-less variants are rejected.
pub(crate) fn attach_media(
    application: VariantApplication,
    condition: &str,
) -> Option<VariantApplication> {
    match application {
        VariantApplication::SelectorTemplate(selector) => Some(VariantApplication::SelectorMedia {
            selector,
            condition: condition.to_owned(),
        }),
        VariantApplication::SelectorRule {
            selector,
            declarations,
        } => Some(VariantApplication::SelectorMediaRule {
            selector,
            declarations,
            condition: condition.to_owned(),
        }),
        _ => None,
    }
}

/// Attaches a variant's own declarations to a selector application.
pub(crate) fn attach_declarations(
    application: VariantApplication,
    declarations: Vec<VariantDeclaration>,
) -> Option<VariantApplication> {
    match application {
        VariantApplication::SelectorTemplate(selector) => Some(VariantApplication::SelectorRule {
            selector,
            declarations,
        }),
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
pub(crate) fn registered_variant_candidates<'a, 'b>(
    variants: &'a [CustomVariant],
    value: &'b str,
) -> Vec<(&'a CustomVariant, Option<&'b str>)> {
    let mut candidates = Vec::new();
    for variant in variants.iter().rev() {
        if !variant.wildcard && variant.name == value {
            candidates.push((variant, None));
        }
    }
    for variant in variants.iter().rev() {
        let Some(prefix) = variant
            .wildcard
            .then(|| variant.name.strip_suffix('*'))
            .flatten()
        else {
            continue;
        };
        if prefix.is_empty() {
            continue;
        }
        let Some(capture) = value.strip_prefix(prefix) else {
            continue;
        };
        if !capture.is_empty() {
            candidates.push((variant, Some(capture)));
        }
    }
    candidates
}

/// Fills a definition body's capture placeholders.
///
/// A body without any placeholder is returned as an unchanged owned copy. An arbitrary
/// selector capture loses its `&` and underscore escapes, and a modifier is escaped as
/// `\/` so it stays part of the selector rather than starting a new one.
pub(crate) fn substitute_variant_capture(value: &str, capture: Option<&str>) -> Option<String> {
    if !value.contains("--value()")
        && !value.contains(r"\*")
        && !value.contains("{modifier}")
        && !value.contains(r"\{modifier\}")
        && !value.contains(":root")
        && !value.contains("0n")
    {
        return Some(value.to_owned());
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
        inner.strip_prefix('&').unwrap_or(inner).replace('_', " ")
    } else {
        capture.replace('_', " ")
    };
    let modifier = modifier
        .map(|modifier| modifier.replace('_', " ").replace('/', r"\/"))
        .unwrap_or_default();
    let modifier_suffix = if modifier.is_empty() {
        String::new()
    } else {
        format!(r"\/{modifier}")
    };
    let mut expanded = value
        .replace("--value()", &capture)
        .replace(r"\*", &capture)
        .replace(r"\/\{modifier\}", &modifier_suffix)
        .replace(r"\{modifier\}", &modifier)
        .replace("/{modifier}", &modifier_suffix)
        .replace("{modifier}", &modifier)
        .replace(":root", &format!(":{capture}"))
        .replace("0n", &capture);
    if capture.starts_with('[') {
        expanded = expanded.replace("):[", ")[");
    }
    Some(expanded)
}

/// Resolves `--value(...)` placeholders in a variant body.
///
/// A theme namespace domain yields the authored token value, not a `var()` reference,
/// because the value becomes part of a selector or condition where a variable cannot
/// appear. A placeholder that cannot be resolved fails the body.
pub(crate) fn resolve_variant_css_placeholders(
    body: &str,
    variant_name: &str,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = variant_name
        .strip_suffix('*')
        .and_then(|prefix| value.strip_prefix(prefix))
        .map(|capture| capture.split_once('/').map_or(capture, |(value, _)| value));
    let mut body = body.to_owned();
    expand_placeholder(&mut body, "--value", |arguments| {
        resolve_variant_value_domains(arguments, capture, theme)
    })?;
    Some(body)
}

/// Resolves a variant's value-domain list against a capture.
///
/// Theme namespace domains read the token value directly; every other domain uses the
/// utility resolver, so a variant and a utility accept the same spellings.
pub(crate) fn resolve_variant_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if let Some((namespace, suffix)) = theme_domain_pattern(domain) {
            return capture.and_then(|name| {
                theme
                    .token_in(namespace, &format!("{name}{suffix}"))
                    .map(|token| token.value.value.clone())
            });
        }
        resolve_value_domains(domain, capture, theme)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_variant_theme_values_are_rejected() {
        let source = stillwind_source::SourceFile::new(
            stillwind_source::FileId(1),
            "theme.css",
            "@theme { --breakpoint-loop: --value(--breakpoint-*); }",
        )
        .unwrap();
        let theme = crate::parse_theme(&source).unwrap();
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
            substitute_typed_modifier("&", Some(BuiltinVariantModifierType::ClassIdentifier), None),
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
}
