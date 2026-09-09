use stillwind_css::{BuiltinVariantModifierType, CustomVariant, ThemeIndex};

use crate::syntax::closing_parenthesis;
use crate::values::domains::{resolve_value_domains, split_domains, theme_domain_pattern};

use super::body::resolve_variant_body;
use super::resolve::{VariantApplication, VariantDeclaration};

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

pub(crate) fn substitute_quoted_variant_value(body: &str, value: &str) -> Option<String> {
    substitute_quoted_macro(body, ":--value(\"", "{value}", value)
}

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

pub(crate) fn apply_nested_selector(body: &str, selector: &str) -> Option<VariantApplication> {
    let fragment = selector.strip_prefix('&')?;
    resolve_variant_body(&body.replace(":--value()", fragment))
}

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
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_variant_value_domains(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")).then_some(body)
}

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
