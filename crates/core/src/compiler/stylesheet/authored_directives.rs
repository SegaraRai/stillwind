use stillwind_css::ThemeIndex;
use stillwind_grammar::parse_candidate;
use stillwind_tailwind::{VariantApplication, resolve_variant};

use crate::compiler::utilities::declaration_order::{
    order_builtin_utilities, prepend_missing_declarations,
};
use crate::compiler::utilities::resolve::resolve_utility;
use crate::compiler::variants::{VariantStep, variant_paths};

use super::blocks::find_at_rule_block;

pub(in crate::compiler) fn expand_authored_directives(css: &str, theme: &ThemeIndex) -> String {
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

pub(in crate::compiler) fn expand_authored_variant(
    prelude: &str,
    body: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let mut output = String::new();
    for variant_sequence in split_css_top_level(prelude, ',') {
        let candidate = parse_candidate(&format!("{}:flex", variant_sequence.trim()))?;
        if candidate.variants.is_empty() {
            return None;
        }
        let mut paths = vec![Vec::new()];
        for variant in &candidate.variants {
            let application = resolve_variant(variant, theme)?;
            let alternatives = match application {
                VariantApplication::Compound(applications) => applications,
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

pub(in crate::compiler) fn split_css_top_level(value: &str, separator: char) -> Vec<&str> {
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

pub(in crate::compiler) fn render_authored_variant_path(
    body: &str,
    applications: &[VariantApplication],
) -> String {
    variant_paths(applications)
        .iter()
        .map(|path| {
            path.iter()
                .fold(body.to_owned(), |body, application| match *application {
                    VariantStep::SelectorSuffix(suffix) => {
                        format!("&{suffix}{{{body}}}")
                    }
                    VariantStep::SelectorTemplate(selector) => {
                        format!("{selector}{{{body}}}")
                    }
                    VariantStep::SelectorRule {
                        selector,
                        declarations,
                    } => {
                        let mut body = body;
                        prepend_missing_declarations(&mut body, declarations);
                        format!("{selector}{{{body}}}")
                    }
                    VariantStep::SelectorMedia {
                        selector,
                        condition,
                    } => format!("{selector}{{@media {condition}{{{body}}}}}"),
                    VariantStep::SelectorMediaRule {
                        selector,
                        declarations,
                        condition,
                    } => {
                        let mut body = body;
                        prepend_missing_declarations(&mut body, declarations);
                        format!("{selector}{{@media {condition}{{{body}}}}}")
                    }
                    VariantStep::Media(condition) => {
                        format!("@media {condition}{{{body}}}")
                    }
                    VariantStep::Container(condition) => {
                        format!("@container {condition}{{{body}}}")
                    }
                    VariantStep::Supports(condition) => {
                        format!("@supports ({condition}){{{body}}}")
                    }
                    VariantStep::StartingStyle => {
                        format!("@starting-style{{{body}}}")
                    }
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Compiler;
    use stillwind_source::FileId;

    #[test]
    fn expands_authored_variants_through_the_shared_registry() {
        let source = crate::css_fixture::source(
            FileId(1),
            "stillwind.css",
            r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/utilities.css" layer(utilities); @import "stillwind/builtins.css";
                .button {
                  @variant hover { opacity: 0.8; }
                  @variant focus { outline-width: 2px; }
                  @variant print { color: black; }
                }
            "#,
        )
        .unwrap();
        let theme = Compiler::read_theme(&source).unwrap();
        let expanded = expand_authored_directives(theme.custom_css(), &theme);
        let compact = expanded
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();

        assert!(
            compact.contains("&:hover{@media(hover:hover){opacity:0.8;}}"),
            "{compact}"
        );
        assert!(compact.contains("&:focus{outline-width:2px;}"), "{compact}");
        assert!(compact.contains("@mediaprint{color:black;}"), "{compact}");
    }

    #[test]
    fn expands_authored_apply_in_builtin_registry_order() {
        let source = crate::css_fixture::source(
            FileId(1),
            "stillwind.css",
            r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/utilities.css" layer(utilities); @import "stillwind/builtins.css";
                .button { @apply px-4 bg-red-500 inline-flex; }
            "#,
        )
        .unwrap();
        let theme = Compiler::read_theme(&source).unwrap();
        let expanded = expand_authored_directives(theme.custom_css(), &theme);
        let compact = expanded
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();

        assert!(compact.contains(
            "display:inline-flex;padding-inline:calc(var(--spacing)*4);background-color:var(--color-red-500);"
        ), "actual output: {compact}");
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn top_level_list_splitting_keeps_quoted_and_nested_separators() {
        assert_eq!(
            split_css_top_level("a,[b,c],f(x,y),'z,w'", ','),
            vec!["a", "[b,c]", "f(x,y)", "'z,w'"]
        );
        assert_eq!(split_css_top_level("", ','), vec![""]);
        assert_eq!(split_css_top_level("a,", ','), vec!["a", ""]);
    }
}
