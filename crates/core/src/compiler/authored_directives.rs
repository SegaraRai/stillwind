use super::{
    find_at_rule_block, order_builtin_utilities, parse_candidate, prepend_missing_declarations,
    resolve_utility,
};

pub(super) fn expand_authored_directives(css: &str, theme: &stillwind_css::ThemeIndex) -> String {
    let mut output = css.to_owned();
    while let Some(start) = output.find("@apply ") {
        let Some(end_rel) = output[start..].find(';') else {
            break;
        };
        let end = start + end_rel;
        let candidates = output[start + 7..end].split_whitespace();
        let utilities = candidates
            .filter_map(|candidate| resolve_utility(candidate, theme))
            .collect::<Vec<_>>();
        let declarations = order_builtin_utilities(utilities.iter().collect())
            .into_iter()
            .map(|utility| utility.declaration.as_str())
            .collect::<String>();
        output.replace_range(start..=end, &declarations);
    }
    while let Some((start, prelude, open, close)) = find_at_rule_block(&output, "variant") {
        let Some(replacement) = expand_authored_variant(&prelude, &output[open + 1..close], theme)
        else {
            break;
        };
        output.replace_range(start..=close, &replacement);
    }
    output
}

pub(super) fn expand_authored_variant(
    prelude: &str,
    body: &str,
    theme: &stillwind_css::ThemeIndex,
) -> Option<String> {
    let mut output = String::new();
    for variant_sequence in split_css_top_level(prelude, ',') {
        let candidate = parse_candidate(&format!("{}:flex", variant_sequence.trim()))?;
        if candidate.variants.is_empty() {
            return None;
        }
        let mut paths = vec![Vec::new()];
        for variant in &candidate.variants {
            let application = stillwind_tailwind::resolve_variant(variant, theme)?;
            let alternatives = match application {
                stillwind_tailwind::VariantApplication::Compound(applications) => applications,
                application => vec![application],
            };
            paths = paths
                .into_iter()
                .flat_map(|path| {
                    alternatives.iter().cloned().map(move |application| {
                        let mut path = path.clone();
                        path.push(application);
                        path
                    })
                })
                .collect();
        }
        for applications in &paths {
            output.push_str(&render_authored_variant_path(body, applications));
        }
    }
    Some(output)
}

pub(super) fn split_css_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' | '[' | '{' => stack.push(character),
            ')' | ']' | '}' => {
                stack.pop();
            }
            _ if character == separator && stack.is_empty() => {
                parts.push(&value[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

pub(super) fn render_authored_variant_path(
    body: &str,
    applications: &[stillwind_tailwind::VariantApplication],
) -> String {
    applications
        .iter()
        .fold(body.to_owned(), |body, application| match application {
            stillwind_tailwind::VariantApplication::Compound(_) => unreachable!(),
            stillwind_tailwind::VariantApplication::SelectorSuffix(suffix) => {
                format!("&{suffix}{{{body}}}")
            }
            stillwind_tailwind::VariantApplication::SelectorTemplate(selector) => {
                format!("{selector}{{{body}}}")
            }
            stillwind_tailwind::VariantApplication::SelectorRule {
                selector,
                declarations,
            } => {
                let mut body = body;
                prepend_missing_declarations(&mut body, declarations);
                format!("{selector}{{{body}}}")
            }
            stillwind_tailwind::VariantApplication::SelectorMedia {
                selector,
                condition,
            } => format!("{selector}{{@media {condition}{{{body}}}}}"),
            stillwind_tailwind::VariantApplication::SelectorMediaRule {
                selector,
                declarations,
                condition,
            } => {
                let mut body = body;
                prepend_missing_declarations(&mut body, declarations);
                format!("{selector}{{@media {condition}{{{body}}}}}")
            }
            stillwind_tailwind::VariantApplication::Media(condition) => {
                format!("@media {condition}{{{body}}}")
            }
            stillwind_tailwind::VariantApplication::Container(condition) => {
                format!("@container {condition}{{{body}}}")
            }
            stillwind_tailwind::VariantApplication::Supports(condition) => {
                format!("@supports ({condition}){{{body}}}")
            }
            stillwind_tailwind::VariantApplication::StartingStyle => {
                format!("@starting-style{{{body}}}")
            }
        })
}
